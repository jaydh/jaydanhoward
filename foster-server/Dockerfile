# Multi-stage build replacing the old Bazel/distroless_cc pipeline — see
# .claude/plans/iridescent-skipping-wall.md (milestone 1/7) for the full
# migration plan. Container contract must match the current Leptos image
# exactly (port 8000, GET /health_check, same env vars) so cutover is just
# "ship a new image under the same Harbor tag," no homelab manifest changes.

ARG FOSTER_REV=46109a8

# WASM/JS output is architecture-independent bytecode + glue code, but
# wasm-bindgen embeds a per-build content hash in that glue code's internal
# function names. Building it separately per target architecture (the old
# single-stage version did, inheriting --platform from the top-level build)
# meant the amd64 and arm64 images shipped two different, non-interchangeable
# JS/WASM pairs — invisible with one replica, but with production's 3
# replicas and no session affinity, a browser's requests could land on pods
# serving mismatched pairs mid-session and throw a binding error, breaking
# hydration entirely. Pinning this stage to one fixed platform regardless of
# the final image's target guarantees every replica of every architecture
# ships byte-identical /pkg contents.
FROM --platform=linux/amd64 docker.io/library/rust:1-bookworm AS wasm-builder
ARG FOSTER_REV
RUN rustup target add wasm32-unknown-unknown \
    && cargo install wasm-pack --locked
RUN git clone https://github.com/jaydh/foster /foster-src \
    && cd /foster-src \
    && git checkout ${FOSTER_REV}
RUN cd /foster-src/crates/foster-client \
    && wasm-pack build --target web --out-dir pkg

FROM docker.io/library/rust:1-bookworm AS builder
WORKDIR /build
COPY foster-server/Cargo.toml Cargo.toml
COPY foster-server/src src
COPY foster-server/static static
COPY foster-server/migrations migrations
RUN cargo build --release

FROM gcr.io/distroless/cc-debian13
WORKDIR /app
COPY --from=builder /build/target/release/jaydanhoward /app/jaydanhoward
COPY --from=wasm-builder /foster-src/crates/foster-client/pkg /app/pkg
COPY --from=builder /build/static /app/static

EXPOSE 8000
ENTRYPOINT ["/app/jaydanhoward"]
