# Job container for CI's clippy / security-audit / integration-test jobs:
# everything those jobs used to install on every run, baked in once.
# general.yml's ci-image job tags this by a hash of this file +
# package-lock.json + the month, and only builds when that tag isn't in
# Harbor yet — so it rebuilds when either file changes, and monthly to pick
# up a new stable Rust and Debian security updates.
# amd64 only: every job that uses it runs on arc-runner-set-k8s.

FROM docker.io/library/node:22-bookworm-slim AS node
FROM docker.io/library/caddy:2.8.4 AS caddy

FROM docker.io/library/rust:1-bookworm

# CI sets RUSTUP_TOOLCHAIN=stable, so install a toolchain by that name
# (the base image's is named after its exact version).
RUN rustup toolchain install stable --profile minimal \
        --component clippy --target wasm32-unknown-unknown \
    && rustup default stable \
    && cargo install wasm-pack cargo-audit --locked \
    && rm -rf /usr/local/cargo/registry /usr/local/cargo/git

COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
    && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx

COPY --from=caddy /usr/bin/caddy /usr/bin/caddy

# Chromium + its system libraries at exactly the Playwright version the
# lockfile pins, in a fixed path the tests find via PLAYWRIGHT_BROWSERS_PATH.
ENV PLAYWRIGHT_BROWSERS_PATH=/ms-playwright
COPY package.json package-lock.json /tmp/pw/
RUN cd /tmp/pw && npm ci && npx playwright install --with-deps chromium \
    && cd / && rm -rf /tmp/pw /root/.npm /var/lib/apt/lists/*
