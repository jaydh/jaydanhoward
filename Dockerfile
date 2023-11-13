FROM rustlang/rust:nightly-bullseye as builder

RUN apt-get update && apt-get install git -y
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo install --locked cargo-leptos
RUN rustup target add wasm32-unknown-unknown

RUN mkdir -p /app
WORKDIR /app
RUN git clone https://github.com/jaydh/inject-git
COPY . .
RUN cargo run --manifest-path=./inject-git/Cargo.toml ./src

RUN cargo leptos build --release -vv

FROM debian:bullseye-slim AS runtime 
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates nginx \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/jaydanhoward /app/
COPY --from=builder /app/target/site /app/site
COPY --from=builder /app/Cargo.toml /app/

WORKDIR /app
ENV RUST_LOG="info"
ENV APP_ENVIRONMENT="production"
ENV LEPTOS_SITE_ADDR="0.0.0.0:8000"
ENV LEPTOS_SITE_ROOT="site"

EXPOSE 8080

COPY nginx.conf /etc/nginx/nginx.conf
COPY entrypoint.sh entrypoint.sh
ENTRYPOINT ["./entrypoint.sh"]
