#!/usr/bin/env bash
set -euo pipefail

# Find the cargo-audit binary (Bazel makes it available in runfiles)
CARGO_AUDIT=""
for binary in ../cargo_audit_linux_x86_64/cargo-audit ../cargo_audit_linux_arm64/cargo-audit ../cargo_audit_macos_x86_64/cargo-audit; do
    if [ -f "$binary" ]; then
        CARGO_AUDIT="$binary"
        break
    fi
done

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

echo "Running cargo-audit on server dependencies..."
"$CARGO_AUDIT" audit $AUDIT_FLAGS --file Cargo.server.lock

echo "Running cargo-audit on WASM dependencies..."
"$CARGO_AUDIT" audit $AUDIT_FLAGS --file Cargo.wasm.lock

echo "Security audit passed!"
