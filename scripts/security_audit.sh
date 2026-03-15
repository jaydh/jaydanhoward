#!/usr/bin/env bash
set -euo pipefail

# Find the cargo-audit binary in runfiles (works with both WORKSPACE and bzlmod canonical names)
RUNFILES_ROOT="${RUNFILES_DIR:-${TEST_SRCDIR:-}}"
CARGO_AUDIT=$(find "$RUNFILES_ROOT" -maxdepth 2 -name "cargo-audit" 2>/dev/null | head -1)

if [ -z "$CARGO_AUDIT" ]; then
    echo "ERROR: cargo-audit binary not found"
    exit 1
fi

# Check if advisory database exists, use --no-fetch if it does
AUDIT_FLAGS=""
ADVISORY_DB="${HOME:-}/.cargo/advisory-db"
if [ -n "${HOME:-}" ] && [ -d "$ADVISORY_DB" ]; then
    AUDIT_FLAGS="--no-fetch"
    echo "Using cached advisory database"
fi

# RUSTSEC-2023-0071: Marvin Attack timing side-channel in rsa crate, pulled in
# transitively by sqlx-core for Postgres SCRAM auth. No upstream fix available.
#
# Unmaintained transitive deps — no fix available upstream; ignoring until
# the relevant upstream crates are updated:
#
# RUSTSEC-2024-0436: paste — unmaintained, pulled in by Leptos (tachys/reactive_graph/actix-service)
# RUSTSEC-2024-0370: proc-macro-error — unmaintained, pulled in by rstml → leptos_macro
# RUSTSEC-2025-0134: rustls-pemfile — unmaintained, pulled in by reqwest and kube-client
# RUSTSEC-2024-0384: instant — unmaintained, pulled in by three-d (WASM only); three-d is at latest
# RUSTSEC-2022-0081: json — unmaintained, pulled in by satkit (WASM only); major version upgrade needed
IGNORE_FLAGS="--ignore RUSTSEC-2023-0071 \
  --ignore RUSTSEC-2024-0436 \
  --ignore RUSTSEC-2024-0370 \
  --ignore RUSTSEC-2025-0134 \
  --ignore RUSTSEC-2024-0384 \
  --ignore RUSTSEC-2022-0081"

echo "Running cargo-audit on server dependencies..."
"$CARGO_AUDIT" audit $AUDIT_FLAGS $IGNORE_FLAGS --file Cargo.server.lock

echo "Running cargo-audit on WASM dependencies..."
"$CARGO_AUDIT" audit $AUDIT_FLAGS $IGNORE_FLAGS --file Cargo.wasm.lock

echo "Security audit passed!"
