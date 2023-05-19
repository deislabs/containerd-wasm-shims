FROM --platform=${BUILDPLATFORM} rust:1.69 AS build
WORKDIR /opt/build
COPY . .
RUN rustup target add wasm32-wasi && cargo build --target wasm32-wasi --release

FROM --platform=linux/amd64 golang:1.19.5-bullseye AS build-go
WORKDIR /opt/build
COPY . .
RUN curl -LO https://github.com/tinygo-org/tinygo/releases/download/v0.25.0/tinygo_0.25.0_amd64.deb && dpkg -i tinygo_0.25.0_amd64.deb
RUN cd go-hello && tinygo build -wasm-abi=generic -target=wasi -gc=leaking -o spin_go_hello.wasm main.go

FROM scratch
COPY --from=build /opt/build/target/wasm32-wasi/release/spin_rust_hello.wasm .
COPY --from=build /opt/build/spin.toml .
COPY --from=build-go /opt/build/go-hello/spin_go_hello.wasm .