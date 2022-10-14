FROM --platform=${BUILDPLATFORM} rust:1.64 AS build
WORKDIR /opt/build
COPY . .
RUN rustup target add wasm32-wasi && cargo build --target wasm32-wasi --release
RUN apt-get update && apt-get install ca-certificates -y

FROM scratch
COPY --from=build /opt/build/target/wasm32-wasi/release/slight.wasm ./app.wasm
COPY --from=build /opt/build/slightfile.toml .
COPY --from=build /etc/ssl /etc/ssl