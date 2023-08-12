FROM rustlang/rust:nightly-alpine AS chef

WORKDIR /app
COPY . .
RUN apk update && apk add lld clang musl-dev pkgconfig gcompat libressl libressl-dev build-base libstdc++ 
RUN rustup update
RUN rustup default nightly
RUN rustup target add wasm32-unknown-unknown
RUN cargo +nightly install cargo-leptos
#RUN cargo +nightly install cargo-chef
run cargo +nightly leptos build 

ENTRYPOINT ["./target/release/jaydanhoward"]

#FROM chef as planner
#COPY . .
#run cargo +nightly chef prepare --recipe-path recipe.json


#FROM chef as builder
#COPY . .
#COPY --from=planner /app/recipe.json recipe.json
#RUN cargo +nightly chef cook --release --recipe-path recipe.json

#FROM rustlang/rust:nightly-alpine AS runtime 
#WORKDIR /app
#COPY . .

#RUN apk update \
#    && apk add ca-certificates \
#    && rm -rf /var/lib/apt/lists/*

#COPY --from=builder /app/target/release/jaydanhoward jaydanhoward 

#ENTRYPOINT ["./jaydanhoward"]
