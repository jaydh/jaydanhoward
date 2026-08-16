# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Personal website monorepo for jaydanhoward.com, built on **Foster**: server-owned state
machines with a generic WASM client that patches the DOM via `fx-*` attributes and
server-sent events. The Foster framework itself (`foster-core`, `foster-server`,
`foster-client`) lives in a separate repo, https://github.com/jaydh/foster, and is pulled
in as a git dependency pinned to a specific rev in `foster-server/Cargo.toml`.

- **foster-server/**: the actual application — axum server, all routes/pages, static
  assets, SQL migrations, Dockerfile. This is what gets built and deployed.
- **foster/pkg/**: build output directory for the compiled `foster-client` WASM/JS
  bundle (gitignored, produced by `wasm-pack` — see Dockerfile and CI).
- **lighthouse/**: standalone Lighthouse CI helper image (Chromium + Node), unrelated to
  the Foster/Rust build — plain Dockerfile, not part of the Cargo workspace.
- **security-audit/**: standalone `cargo-audit` CronJob image, plain Dockerfile.

There is no Bazel, no Leptos, and no WASM-hydration-of-Rust-components model in this
repo anymore — that stack was fully migrated off and removed.

## Build Commands

### Development
```bash
cd foster-server
cargo run
```

The server expects the `foster-client` WASM bundle at `../foster/pkg` (relative to
`foster-server/`, i.e. repo-root `foster/pkg/`). Build it with:
```bash
# from a checkout of https://github.com/jaydh/foster, pinned to the FOSTER_REV
# in foster-server/Dockerfile:
cd crates/foster-client && wasm-pack build --target web --out-dir pkg
cp -r pkg/. <this-repo>/foster/pkg/
```

### Tests
```bash
# Rust unit/integration tests
cd foster-server && cargo test

# Playwright end-to-end tests (against a running server, see playwright.config.ts)
npm install
npx playwright test
```

### Production Build
```bash
# from repo root — Dockerfile builds the WASM client from the pinned foster
# rev, then the axum server, then assembles a distroless image
docker build -f foster-server/Dockerfile -t jaydanhoward .
```

CI (`.github/workflows/general.yml`) builds and pushes this image (amd64 + arm64) to
Harbor via kaniko on every push to `main`, then creates a multi-arch manifest.

## Architecture

**Entry point**: `foster-server/src/main.rs` wires up the axum router, Postgres pool,
migrations, and all feature modules.

**Feature modules** (`foster-server/src/`): each file is a largely self-contained
feature — routes, state machine, and any background tasks:
- `cluster.rs` / `cluster_audit.rs` — homelab k8s cluster stats panel + daily
  Claude-reviewed audit of firing Prometheus alerts
- `conjunction.rs` — satellite conjunction screening (SGP4/TCA)
- `satellites.rs` — TLE-backed 3D satellite globe (WebGL2 client, TLE cache in Postgres)
- `visitors.rs` — geo-IP visitor logging/map
- `lighthouse.rs` — Lighthouse report ingest/display endpoint
- `security_audit.rs` — `cargo-audit` report ingest/display
- `photography.rs`, `request_trace.rs`, `site_middleware.rs`, `prometheus_client.rs` —
  supporting features/middleware

**Static assets**: `foster-server/static/` — HTML pages, per-feature JS (the `fx-*`
DOM-patching client scripts), fonts, favicon. Served directly by axum
(`tower-http::services::fs`).

**Migrations**: `foster-server/migrations/` — sqlx migrations, run automatically at
startup via `sqlx::migrate!()`.

**Foster framework conventions** (see [[foster_client_gotchas]] in memory for gotchas):
- Server owns state; the WASM client only patches the DOM per SSE-pushed instructions
  and reads `fx-*`/`data-fx-*` attributes — there's no client-side component tree.
- Wrapping `fx-for` items in extra markup breaks `data-fx-item` lookups.
- Pages using SSE can hang Playwright's `networkidle` wait — use explicit
  selectors/events instead.

## Deployment

Deployed on self-hosted k8s (homelab repo at `/Users/jayhoward/repos/homelab`) via
Flux CD. Postgres via CNPG in the `service` namespace
(`jaydanhoward-postgres-rw:5432/jaydanhoward`). Image pulled from Harbor
(`harbor.home.local`), tag tracked via Flux image automation.

## Development Notes

- `foster-server/Cargo.toml` pins the Foster framework rev (`foster-core`,
  `foster-server` deps as `git = "https://github.com/jaydh/foster", rev = "..."`).
  Bumping Foster means bumping this rev (and the matching `FOSTER_REV` build arg in
  `foster-server/Dockerfile`, and the `FOSTER_REV` grep'd out of that Dockerfile in
  CI's WASM build step).
- `foster-server/rust-toolchain.toml` pins stable Rust for this crate. CI additionally
  sets `RUSTUP_TOOLCHAIN=stable` as an env var everywhere, since some steps run outside
  `foster-server/`'s directory and rustup would otherwise walk up to nothing (no stray
  toolchain file at repo root anymore, but this is left in place defensively).
