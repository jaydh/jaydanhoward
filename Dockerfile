FROM rustlang/rust:nightly-bullseye AS builder

# If you’re using stable, use this instead
# FROM rust:1.74-bullseye as builder

# Install cargo-binstall, which makes it easier to install other
# cargo extensions like cargo-leptos
RUN wget https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-unknown-linux-musl.tgz
RUN tar -xvf cargo-binstall-x86_64-unknown-linux-musl.tgz
RUN cp cargo-binstall /usr/local/cargo/bin

# Install cargo-leptos
RUN cargo binstall cargo-leptos -y

# Add the WASM target
RUN rustup target add wasm32-unknown-unknown

RUN mkdir -p /app
WORKDIR /app
RUN git clone https://github.com/jaydh/inject-git
COPY . .
RUN cargo run --manifest-path=./inject-git/Cargo.toml ./src

RUN cargo leptos build --release -vv


FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update -y \
  && apt-get install -y --no-install-recommends openssl ca-certificates wget \
  && apt-get autoremove -y \
  && apt-get clean -y \
  && rm -rf /var/lib/apt/lists/*

RUN wget http://nz2.archive.ubuntu.com/ubuntu/pool/main/o/openssl/libssl1.1_1.1.1f-1ubuntu2_amd64.deb
RUN dpkg -i libssl1.1_1.1.1f-1ubuntu2_amd64.deb

COPY --from=builder /app/target/release/jaydanhoward /app/
COPY --from=builder /app/target/site /app/site
COPY --from=builder /app/Cargo.toml /app/

WORKDIR /app
ENV RUST_LOG="info"
ENV APP_ENVIRONMENT="production"
ENV LEPTOS_SITE_ADDR="0.0.0.0:8000"
ENV LEPTOS_SITE_ROOT="site"

EXPOSE 8000

CMD ["./jaydanhoward"]
