# Replaces the old Bazel pkg_tar/oci_image build (BUILD's
# security_audit_image target, which packaged a prebuilt @cargo_audit_*
# binary + entrypoint.py). Only needs amd64 — the real CronJob
# (homelab/service/security-audit.yaml) pins nodeSelector
# kubernetes.io/arch: amd64 — so unlike the main site image this is
# single-arch only.

FROM docker.io/library/rust:1-bookworm AS builder
RUN cargo install cargo-audit --locked --features vendored-openssl

FROM docker.io/library/python:3-slim
WORKDIR /app
COPY --from=builder /usr/local/cargo/bin/cargo-audit /app/cargo-audit
COPY security-audit/entrypoint.py /app/entrypoint.py
ENTRYPOINT ["python3", "/app/entrypoint.py"]
