FROM lukemathwalker/cargo-chef:latest-rust-1.71.0 as chef

RUN mkdir -p /app
WORKDIR /app

RUN apt-get update && apt-get install lld clang curl git -y
run rustup install nightly
run rustup default nightly
run rustup target add wasm32-unknown-unknown
RUN git clone https://github.com/jaydh/inject-git

RUN cargo install --locked cargo-leptos

FROM chef as planner
RUN mkdir -p /app
WORKDIR /app

COPY . .
COPY --from=chef /app/inject-git inject-git
RUN cargo run --manifest-path=./inject-git/Cargo.toml ./src
run cargo +nightly chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo +nightly chef cook --release --recipe-path recipe.json

COPY --from=planner /app .
RUN cargo build --release

FROM chef as leptos_builder
COPY --from=builder /app .
RUN cargo leptos build --release -vv

FROM debian:bullseye-slim AS runtime 
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates nginx \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*

COPY --from=leptos_builder /app/target/release/jaydanhoward /app/
COPY --from=leptos_builder /app/target/site /app/site
COPY --from=leptos_builder /app/Cargo.toml /app/

WORKDIR /app
ENV RUST_LOG="info"
ENV APP_ENVIRONMENT="production"
ENV LEPTOS_SITE_ADDR="0.0.0.0:8000"
ENV LEPTOS_SITE_ROOT="site"

EXPOSE 8080

COPY nginx.conf /etc/nginx/nginx.conf
COPY entrypoint.sh entrypoint.sh
ENTRYPOINT ["./entrypoint.sh"]
