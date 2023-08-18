# Image is huge, haven't split into separate services because not sure how DigitalOcean billing works and I'm cheap (does each node have a static cost?)

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
    && apt-get install -y --no-install-recommends openssl ca-certificates curl chromium

RUN curl -sSL https://deb.nodesource.com/setup_18.x | bash
# Might put nginx in this container
# RUN curl -o /tmp/netdata-kickstart.sh https://my-netdata.io/kickstart.sh && sh /tmp/netdata-kickstart.sh
RUN apt-get install -y nodejs
ARG CACHEBUST=1
RUN npm install -g lighthouse

RUN apt-get autoremove -y \
    && apt-get clean -y \
    && apt-get purge --auto-remove -y gnupg \
    && rm -rf /var/lib/apt/lists/* 

COPY --from=builder /app/target/server/release/jaydanhoward /app/
COPY --from=builder /app/target/site /app/site
COPY --from=builder /app/Cargo.toml /app/

WORKDIR /app
ENV RUST_LOG="info"
ENV APP_ENVIRONMENT="production"
ENV LEPTOS_SITE_ADDR="0.0.0.0:8080"
ENV LEPTOS_SITE_ROOT="site"

EXPOSE 8080
EXPOSE 19999

COPY control_job.sh control_job.sh
RUN chmod +x ./control_job.sh

RUN groupadd -r chrome && useradd -r -g chrome -G audio,video chrome \
  && mkdir -p /home/chrome && chown -R chrome:chrome /home/chrome

CMD ./control_job.sh
