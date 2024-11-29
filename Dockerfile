FROM rustlang/rust:nightly-bullseye AS builder
RUN apt update && apt install -y build-essential cmake libclang-dev libclang1 golang
RUN wget https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-unknown-linux-musl.tgz \
    && tar -xvf cargo-binstall-x86_64-unknown-linux-musl.tgz \
    && cp cargo-binstall /usr/local/cargo/bin

RUN cargo binstall cargo-leptos -y \
    && rustup target add wasm32-unknown-unknown
RUN cargo install --force --locked bindgen-cli

WORKDIR /app
COPY . .

RUN git clone https://github.com/jaydh/inject-git \
    && cargo run --manifest-path=./inject-git/Cargo.toml ./src \
    && cargo leptos build --release -vv

# Runtime stage
FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app

COPY --from=builder /app/target/release/jaydanhoward /app/
COPY --from=builder /app/target/site /app/site
COPY --from=builder /app/Cargo.toml /app/

ENV RUST_LOG="info" \
    APP_ENVIRONMENT="production" \
    LEPTOS_SITE_ADDR="0.0.0.0:8000" \
    LEPTOS_SITE_ROOT="site"

EXPOSE 8000
USER nonroot
CMD ["./jaydanhoward"]
