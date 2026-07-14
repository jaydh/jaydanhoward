# Multi-stage build replacing the old Bazel/distroless_cc pipeline — see
# .claude/plans/iridescent-skipping-wall.md (milestone 1/7) for the full
# migration plan. Container contract must match the current Leptos image
# exactly (port 8000, GET /health_check, same env vars) so cutover is just
# "ship a new image under the same Harbor tag," no homelab manifest changes.

ARG FOSTER_REV=46109a8

FROM rust:1-bookworm AS builder
ARG FOSTER_REV
RUN rustup target add wasm32-unknown-unknown \
    && cargo install wasm-pack --locked

WORKDIR /build

# foster-client (the WASM runtime) isn't published — it's a path-dependency
# workspace member in the foster repo, so we check out the same rev our
# Cargo.toml git-depends on and build it with wasm-pack, same as
# foster/scripts/build-wasm.sh does locally.
RUN git clone https://github.com/jaydh/foster foster-src \
    && cd foster-src \
    && git checkout ${FOSTER_REV}
RUN cd foster-src/crates/foster-client \
    && wasm-pack build --target web --out-dir pkg

COPY foster-server/Cargo.toml Cargo.toml
COPY foster-server/src src
COPY foster-server/static static
RUN cargo build --release

FROM gcr.io/distroless/cc-debian13
WORKDIR /app
COPY --from=builder /build/target/release/jaydanhoward /app/jaydanhoward
COPY --from=builder /build/foster-src/crates/foster-client/pkg /app/pkg
COPY --from=builder /build/static /app/static

EXPOSE 8000
ENTRYPOINT ["/app/jaydanhoward"]
