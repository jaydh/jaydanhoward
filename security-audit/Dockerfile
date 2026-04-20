FROM rust:1-slim-bookworm AS cargo-audit-builder
RUN cargo install cargo-audit --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends \
        ca-certificates curl python3 git \
    && rm -rf /var/lib/apt/lists/*

COPY --from=cargo-audit-builder /usr/local/cargo/bin/cargo-audit /usr/local/bin/cargo-audit

RUN groupadd -r audit && useradd -r -g audit -m audit
USER audit
WORKDIR /home/audit

ENV SITE_API_ADDR="https://jaydanhoward.com/api/security-audit"

COPY entrypoint.sh entrypoint.sh
ENTRYPOINT ["./entrypoint.sh"]
