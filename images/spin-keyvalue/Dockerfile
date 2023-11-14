FROM --platform=${BUILDPLATFORM} rust:1.71 AS build
WORKDIR /opt/build
COPY . .
RUN rustup target add wasm32-wasi && cargo build --target wasm32-wasi --release

FROM scratch
COPY --from=build /opt/build/spin.toml ./spin.toml
COPY --from=build /opt/build/target/wasm32-wasi/release/spin_keyvalue.wasm ./target/wasm32-wasi/release/spin_keyvalue.wasm
COPY --from=build /opt/build/runtime-config.toml ./runtime-config.toml