FROM --platform=$BUILDPLATFORM rustlang/rust:nightly-bullseye AS builder

RUN apt update && apt install -y build-essential cmake libclang-dev libclang1 golang wget

RUN case $(uname -m) in \
    x86_64) ARCH="x86_64";; \
    aarch64) ARCH="aarch64";; \
    *) echo "Unsupported architecture" && exit 1;; \
    esac && \
    wget https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-${ARCH}-unknown-linux-musl.tgz \
    && tar -xvf cargo-binstall-${ARCH}-unknown-linux-musl.tgz \
    && cp cargo-binstall /usr/local/cargo/bin

# Install cargo-leptos and add wasm target
RUN cargo binstall cargo-leptos -y \
    && rustup target add wasm32-unknown-unknown

RUN cargo install --force --locked bindgen-cli

WORKDIR /app
COPY . .
RUN git clone https://github.com/jaydh/inject-git \
    && cargo run --manifest-path=./inject-git/Cargo.toml ./src \
    && cargo leptos build --release -vv

FROM --platform=$TARGETPLATFORM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=builder --chown=nonroot:nonroot /app/target/release/jaydanhoward /app/
COPY --from=builder --chown=nonroot:nonroot /app/target/site /app/site
COPY --from=builder --chown=nonroot:nonroot /app/Cargo.toml /app/
ENV RUST_LOG="info" \
    APP_ENVIRONMENT="production" \
    LEPTOS_SITE_ADDR="0.0.0.0:8000" \
    LEPTOS_SITE_ROOT="site"
EXPOSE 8000
USER nonroot
CMD ["./jaydanhoward"]
