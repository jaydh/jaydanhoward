# Multi-stage build replacing the old Bazel/distroless_cc pipeline. Container
# contract: port 8000, GET /health_check, same env vars as the homelab
# manifests expect.
#
# The browser-side WASM is NOT built here: foster/pkg (the foster-client
# bundle) and foster-server/static/widgets must already exist in the build
# context. wasm-bindgen embeds a per-build hash in its JS glue's internal
# names, so building it inside each per-arch image build gave the amd64 and
# arm64 images different, non-interchangeable JS/WASM pairs — and with
# replicas on both arches and no session affinity, a browser could load the
# JS from one pod and the WASM from another and break hydration. (A
# `FROM --platform=linux/amd64` stage was supposed to prevent that, but
# kaniko ignores FROM --platform.) CI builds the bundle once, in the
# integration-test job, and hands the same files to both image builds as a
# workflow artifact. Locally, build them first — see CLAUDE.md.

# cargo-chef splits dependency compilation into its own layer, keyed only on
# recipe.json (Cargo.toml + Cargo.lock, not our source), so CI's kaniko layer
# cache (--cache-repo in .github/workflows/general.yml) skips rebuilding the
# whole dependency tree unless dependencies actually change.
FROM docker.io/library/rust:1-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /build

FROM chef AS planner
COPY foster-server/Cargo.toml foster-server/Cargo.lock ./
COPY foster-server/src src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY foster-server/Cargo.toml foster-server/Cargo.lock ./
COPY foster-server/src src
# Only index.html is compiled in (include_str!); the rest of static/ goes
# straight to the final stage, so widget changes don't rebuild the server.
COPY foster-server/static/index.html static/index.html
COPY foster-server/migrations migrations
RUN cargo build --release

FROM gcr.io/distroless/cc-debian13
WORKDIR /app
COPY --from=builder /build/target/release/jaydanhoward /app/jaydanhoward
COPY foster/pkg /app/pkg
COPY foster-server/static /app/static

EXPOSE 8000
ENTRYPOINT ["/app/jaydanhoward"]
