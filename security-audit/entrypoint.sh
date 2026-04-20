#!/bin/bash
set -euo pipefail

REPO="jaydh/jaydanhoward"
RAW="https://raw.githubusercontent.com/${REPO}/main"
IGNORE_FLAGS="--ignore RUSTSEC-2023-0071 --ignore RUSTSEC-2024-0436 --ignore RUSTSEC-2024-0370 --ignore RUSTSEC-2024-0384"

echo "Fetching lock files from ${REPO}..."
curl -sf "${RAW}/Cargo.server.lock" -o Cargo.server.lock
curl -sf "${RAW}/Cargo.wasm.lock"   -o Cargo.wasm.lock

echo "Updating advisory database..."
cargo-audit fetch

echo "Running cargo-audit..."
SERVER_JSON=$(cargo-audit audit $IGNORE_FLAGS --json --file Cargo.server.lock 2>/dev/null || true)
WASM_JSON=$(cargo-audit audit   $IGNORE_FLAGS --json --file Cargo.wasm.lock   2>/dev/null || true)

echo "Checking upstream versions on crates.io..."
UPSTREAM_JSON=$(python3 - <<'EOF'
import json, urllib.request

def latest_stable(crate):
    url = f"https://crates.io/api/v1/crates/{crate}/versions"
    req = urllib.request.Request(url, headers={"User-Agent": "jaydanhoward-sec-audit/1.0"})
    with urllib.request.urlopen(req) as r:
        data = json.load(r)
    versions = [v["num"] for v in data["versions"] if not v["yanked"] and "-" not in v["num"]]
    return versions[0] if versions else None

def dep_versions(crate, version, dep):
    url = f"https://crates.io/api/v1/crates/{crate}/{version}/dependencies"
    req = urllib.request.Request(url, headers={"User-Agent": "jaydanhoward-sec-audit/1.0"})
    with urllib.request.urlopen(req) as r:
        data = json.load(r)
    return [d["req"] for d in data["dependencies"] if d["crate_id"] == dep]

watches = []

# sqlx: waiting for stable 0.9 which drops rsa
sqlx_latest = latest_stable("sqlx")
sqlx_rsa = dep_versions("sqlx-core", sqlx_latest, "rsa") if sqlx_latest else []
watches.append({
    "crate": "sqlx",
    "advisory": "RUSTSEC-2023-0071",
    "watching_for": "stable 0.9 without rsa dep",
    "latest_stable": sqlx_latest,
    "actionable": sqlx_latest is not None and sqlx_latest.startswith("0.9") and not sqlx_rsa,
})

# tachys (Leptos renderer): waiting for paste removal
tachys_latest = latest_stable("tachys")
tachys_paste = dep_versions("tachys", tachys_latest, "paste") if tachys_latest else []
watches.append({
    "crate": "tachys",
    "advisory": "RUSTSEC-2024-0436",
    "watching_for": "paste dep removed",
    "latest_stable": tachys_latest,
    "actionable": tachys_latest is not None and not tachys_paste,
})

# three-d: waiting for winit >=0.30 (which dropped instant)
threed_latest = latest_stable("three-d")
threed_winit = dep_versions("three-d", threed_latest, "winit") if threed_latest else []
winit_req = threed_winit[0] if threed_winit else None
# winit >=0.30 dropped instant; req like "^0.28" means old
actionable_winit = winit_req is not None and not winit_req.startswith("^0.28") and not winit_req.startswith("^0.29")
watches.append({
    "crate": "three-d",
    "advisory": "RUSTSEC-2024-0384",
    "watching_for": "winit >=0.30 (dropped instant)",
    "latest_stable": threed_latest,
    "winit_req": winit_req,
    "actionable": actionable_winit,
})

print(json.dumps(watches))
EOF
)

echo "Building combined report..."
REPORT=$(python3 - <<EOF
import json, datetime

server = json.loads("""${SERVER_JSON}""")
wasm   = json.loads("""${WASM_JSON}""")
upstream = json.loads("""${UPSTREAM_JSON}""")

# Merge vulnerabilities (deduplicate by advisory ID)
seen = set()
vulns = []
for v in server.get("vulnerabilities", {}).get("list", []) + wasm.get("vulnerabilities", {}).get("list", []):
    aid = v["advisory"]["id"]
    if aid not in seen:
        seen.add(aid)
        vulns.append(v)

# Merge warnings
warn_seen = set()
warns = []
for kind, entries in {**server.get("warnings", {}), **wasm.get("warnings", {})}.items():
    for w in entries:
        aid = w["advisory"]["id"]
        if aid not in warn_seen:
            warn_seen.add(aid)
            warns.append(w)

report = {
    "scanned_at": datetime.datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ"),
    "database": server.get("database", {}),
    "lockfile": {
        "server_dependency_count": server.get("lockfile", {}).get("dependency-count", 0),
        "wasm_dependency_count": wasm.get("lockfile", {}).get("dependency-count", 0),
        "dependency-count": (
            server.get("lockfile", {}).get("dependency-count", 0) +
            wasm.get("lockfile", {}).get("dependency-count", 0)
        ),
    },
    "settings": server.get("settings", {}),
    "vulnerabilities": {"found": len(vulns) > 0, "count": len(vulns), "list": vulns},
    "warnings": {"unmaintained": warns},
    "upstream_watches": upstream,
}
print(json.dumps(report))
EOF
)

echo "Posting report to ${SITE_API_ADDR}..."
HTTP_STATUS=$(curl -sf -o /dev/null -w "%{http_code}" \
    -X POST "${SITE_API_ADDR}" \
    -H "Authorization: Basic $(printf 'jay:%s' "${LIGHTHOUSE_UPDATE_TOKEN}" | base64 -w0)" \
    -H "Content-Type: application/json" \
    --data-raw "${REPORT}" || echo "000")

if [ "${HTTP_STATUS}" = "200" ]; then
    echo "Report uploaded successfully."
else
    echo "Upload failed with status ${HTTP_STATUS}."
    exit 1
fi
