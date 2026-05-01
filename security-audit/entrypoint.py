#!/usr/bin/env python3
import base64
import datetime
import json
import os
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request

REPO = "jaydh/jaydanhoward"
RAW = f"https://raw.githubusercontent.com/{REPO}/main"
IGNORE_FLAGS = [
    "--ignore", "RUSTSEC-2023-0071",
    "--ignore", "RUSTSEC-2024-0436",
    "--ignore", "RUSTSEC-2024-0370",
    "--ignore", "RUSTSEC-2024-0384",
]


def fetch_bytes(url):
    req = urllib.request.Request(url, headers={"User-Agent": "jaydanhoward-sec-audit/1.0"})
    with urllib.request.urlopen(req) as r:
        return r.read()


def crates_io_get(url):
    req = urllib.request.Request(url, headers={"User-Agent": "jaydanhoward-sec-audit/1.0"})
    with urllib.request.urlopen(req) as r:
        return json.load(r)


def latest_stable(crate):
    data = crates_io_get(f"https://crates.io/api/v1/crates/{crate}/versions")
    versions = [v["num"] for v in data["versions"] if not v["yanked"] and "-" not in v["num"]]
    return versions[0] if versions else None


def dep_versions(crate, version, dep):
    data = crates_io_get(f"https://crates.io/api/v1/crates/{crate}/{version}/dependencies")
    return [d["req"] for d in data["dependencies"] if d["crate_id"] == dep]


def run_audit(cargo_audit, db_path, workdir, lock_file):
    result = subprocess.run(
        [cargo_audit, "audit", "--db", db_path, "--json", "--file", lock_file] + IGNORE_FLAGS,
        capture_output=True,
        text=True,
        cwd=workdir,
    )
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError:
        return {}


def main():
    site_api = os.environ["SITE_API_ADDR"]
    token = os.environ["LIGHTHOUSE_UPDATE_TOKEN"]

    script_dir = os.path.dirname(os.path.abspath(__file__))
    cargo_audit = os.path.join(script_dir, "cargo-audit")
    workdir = tempfile.mkdtemp()
    db_path = os.path.join(workdir, "advisory-db")

    print(f"Fetching lock files from {REPO}...")
    for lock in ["Cargo.server.lock", "Cargo.wasm.lock"]:
        content = fetch_bytes(f"{RAW}/{lock}")
        with open(os.path.join(workdir, lock), "wb") as f:
            f.write(content)

    print("Running cargo-audit...")
    server = run_audit(cargo_audit, db_path, workdir, "Cargo.server.lock")
    wasm = run_audit(cargo_audit, db_path, workdir, "Cargo.wasm.lock")

    print("Checking upstream versions on crates.io...")

    sqlx_latest = latest_stable("sqlx")
    sqlx_rsa = dep_versions("sqlx-core", sqlx_latest, "rsa") if sqlx_latest else []
    watches = [
        {
            "crate": "sqlx",
            "advisory": "RUSTSEC-2023-0071",
            "watching_for": "stable 0.9 without rsa dep",
            "latest_stable": sqlx_latest,
            "actionable": sqlx_latest is not None and sqlx_latest.startswith("0.9") and not sqlx_rsa,
        },
    ]

    tachys_latest = latest_stable("tachys")
    tachys_paste = dep_versions("tachys", tachys_latest, "paste") if tachys_latest else []
    watches.append({
        "crate": "tachys",
        "advisory": "RUSTSEC-2024-0436",
        "watching_for": "paste dep removed",
        "latest_stable": tachys_latest,
        "actionable": tachys_latest is not None and not tachys_paste,
    })

    threed_latest = latest_stable("three-d")
    threed_winit = dep_versions("three-d", threed_latest, "winit") if threed_latest else []
    winit_req = threed_winit[0] if threed_winit else None
    actionable_winit = (
        winit_req is not None
        and not winit_req.startswith("^0.28")
        and not winit_req.startswith("^0.29")
    )
    watches.append({
        "crate": "three-d",
        "advisory": "RUSTSEC-2024-0384",
        "watching_for": "winit >=0.30 (dropped instant)",
        "latest_stable": threed_latest,
        "winit_req": winit_req,
        "actionable": actionable_winit,
    })

    seen: set[str] = set()
    vulns = []
    for v in server.get("vulnerabilities", {}).get("list", []) + wasm.get("vulnerabilities", {}).get("list", []):
        aid = v["advisory"]["id"]
        if aid not in seen:
            seen.add(aid)
            vulns.append(v)

    warn_seen: set[str] = set()
    warns = []
    for entries in list(server.get("warnings", {}).values()) + list(wasm.get("warnings", {}).values()):
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
                server.get("lockfile", {}).get("dependency-count", 0)
                + wasm.get("lockfile", {}).get("dependency-count", 0)
            ),
        },
        "settings": server.get("settings", {}),
        "vulnerabilities": {"found": len(vulns) > 0, "count": len(vulns), "list": vulns},
        "warnings": {"unmaintained": warns},
        "upstream_watches": watches,
    }

    print(f"Posting report to {site_api}...")
    credentials = base64.b64encode(f"jay:{token}".encode()).decode()
    body = json.dumps(report).encode()
    req = urllib.request.Request(
        site_api,
        data=body,
        method="POST",
        headers={
            "Authorization": f"Basic {credentials}",
            "Content-Type": "application/json",
        },
    )
    try:
        with urllib.request.urlopen(req) as r:
            if r.status == 200:
                print("Report uploaded successfully.")
            else:
                print(f"Upload failed with status {r.status}.")
                sys.exit(1)
    except urllib.error.HTTPError as e:
        print(f"Upload failed with status {e.code}.")
        sys.exit(1)


if __name__ == "__main__":
    main()
