FROM --platform=${BUILDPLATFORM} rust:1.71 AS build
WORKDIR /opt/build
COPY . .
RUN rustup target add wasm32-wasi && cargo build --target wasm32-wasi --release

FROM scratch
COPY ./spin.toml ./spin.toml
COPY ./target/wasm32-wasi/release/spin_keyvalue.wasm ./target/wasm32-wasi/release/spin_keyvalue.wasm
COPY ./runtime-config.toml ./runtime-config.toml