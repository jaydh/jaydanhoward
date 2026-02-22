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
IGNORE_FLAGS="--ignore RUSTSEC-2023-0071"

echo "Running cargo-audit on server dependencies..."
"$CARGO_AUDIT" audit $AUDIT_FLAGS $IGNORE_FLAGS --file Cargo.server.lock

echo "Running cargo-audit on WASM dependencies..."
"$CARGO_AUDIT" audit $AUDIT_FLAGS $IGNORE_FLAGS --file Cargo.wasm.lock

echo "Security audit passed!"
