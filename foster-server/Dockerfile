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

# Dependencies compile in their own layer, keyed only on Cargo.toml +
# Cargo.lock: build once against a placeholder main.rs, then swap in the
# real source. CI's kaniko layer cache (--cache-repo in
# .github/workflows/general.yml) then skips the whole dependency tree
# unless dependencies change. (Not cargo-chef: its planner stage hands
# recipe.json over via COPY --from, which kaniko fails on once that stage
# is fully cached — "failed to get fileinfo for /kaniko/1/build/recipe.json".)
FROM docker.io/library/rust:1-bookworm AS builder
WORKDIR /build
COPY foster-server/Cargo.toml foster-server/Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release --locked \
    && rm -rf src target/release/jaydanhoward target/release/deps/jaydanhoward-* \
              target/release/.fingerprint/jaydanhoward-*
COPY foster-server/src src
# Only index.html is compiled in (include_str!); the rest of static/ goes
# straight to the final stage, so widget changes don't rebuild the server.
COPY foster-server/static/index.html static/index.html
COPY foster-server/migrations migrations
# Copy the binary out in the same RUN: cargo makes target/release/jaydanhoward
# a hard link into target/release/deps/, and kaniko drops hard links when it
# restores a cached layer — so whenever this step came from the cache, the
# binary was missing ("cp: cannot stat 'target/release/jaydanhoward'",
# "failed to get fileinfo for /kaniko/0/build/target/release/jaydanhoward").
# A plain copy is a regular file and survives.
RUN cargo build --release --locked && cp target/release/jaydanhoward /build/jaydanhoward-bin
# Keeps this stage from ever being fully cached: kaniko doesn't hand a
# fully cached stage's files to a later COPY --from (see above). CI passes
# the commit SHA plus the run attempt — a re-run of a build that got this
# far would otherwise find the layer cached; this is the only layer it
# invalidates.
ARG GIT_SHA=local
RUN echo "$GIT_SHA" > /build/.rev

FROM gcr.io/distroless/cc-debian13
WORKDIR /app
COPY --from=builder /build/jaydanhoward-bin /app/jaydanhoward
COPY foster/pkg /app/pkg
COPY foster-server/static /app/static

EXPOSE 8000
ENTRYPOINT ["/app/jaydanhoward"]
