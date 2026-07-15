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
LOCKFILE = "foster-server/Cargo.lock"
# RUSTSEC-2024-0436/-0370/-0384 were Leptos-toolchain-specific (tachys,
# rstml, three-d/winit) — gone along with Leptos after the Foster
# migration, so only the sqlx/rsa watch (still a real transitive dep)
# remains relevant here.
IGNORE_FLAGS = [
    "--ignore", "RUSTSEC-2023-0071",
]


def fetch_bytes(url):
    req = urllib.request.Request(url, headers={"User-Agent": "jaydanhoward-sec-audit/1.0"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return r.read()


def crates_io_get(url):
    req = urllib.request.Request(url, headers={"User-Agent": "jaydanhoward-sec-audit/1.0"})
    with urllib.request.urlopen(req, timeout=30) as r:
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

    print(f"Fetching {LOCKFILE} from {REPO}...")
    content = fetch_bytes(f"{RAW}/{LOCKFILE}")
    lock_path = os.path.join(workdir, "Cargo.lock")
    with open(lock_path, "wb") as f:
        f.write(content)

    print("Running cargo-audit...")
    result = run_audit(cargo_audit, db_path, workdir, "Cargo.lock")

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

    vulns = result.get("vulnerabilities", {}).get("list", [])
    warns = [w for entries in result.get("warnings", {}).values() for w in entries]

    report = {
        "scanned_at": datetime.datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ"),
        "database": result.get("database", {}),
        "lockfile": result.get("lockfile", {}),
        "settings": result.get("settings", {}),
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
            "User-Agent": "jaydanhoward-sec-audit/1.0",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
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
