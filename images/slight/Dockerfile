FROM --platform=${BUILDPLATFORM} rust:1.69 AS build
WORKDIR /opt/build
COPY . .
RUN rustup target add wasm32-wasi && cargo build --target wasm32-wasi --release
RUN apt-get update && apt-get install ca-certificates -y

FROM scratch
COPY --from=build --chmod=0755 /opt/build/target/wasm32-wasi/release/http_server_lib.wasm ./app.wasm
COPY --from=build /opt/build/slightfile.toml .
COPY --from=build /etc/ssl /etc/ssl
ENTRYPOINT ["/app.wasm"]