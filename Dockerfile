FROM lukemathwalker/cargo-chef:latest-rust-1.71.0 as chef

RUN apt-get update && apt-get install lld clang -y
run rustup install nightly
run rustup default nightly
run rustup target add wasm32-unknown-unknown

RUN wget https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-unknown-linux-musl.tgz
RUN tar -xvf cargo-binstall-x86_64-unknown-linux-musl.tgz
RUN cp cargo-binstall /usr/local/cargo/bin
RUN cargo binstall cargo-leptos -y

RUN mkdir -p /app
WORKDIR /app

FROM chef as planner
COPY . .
run cargo +nightly chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo +nightly chef cook --release --recipe-path recipe.json

COPY . .
RUN cargo leptos build --release -vv

FROM debian:bullseye-slim AS runtime 

RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates nodejs npm chromium-browser \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*

RUN npm install -g lighthouse

COPY --from=builder /app/target/server/release/jaydanhoward /app/
COPY --from=builder /app/target/site /app/site
COPY --from=builder /app/Cargo.toml /app/

WORKDIR /app
ENV RUST_LOG="info"
ENV APP_ENVIRONMENT="production"
ENV LEPTOS_SITE_ADDR="0.0.0.0:8080"
ENV LEPTOS_SITE_ROOT="site"
EXPOSE 8080

CMD ["/app/leptos_start"]
