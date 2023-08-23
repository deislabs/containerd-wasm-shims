FROM --platform=${BUILDPLATFORM} rust:1.69 AS build
COPY . /build
WORKDIR /build
RUN rustup target add wasm32-wasi
RUN cargo build --release --target wasm32-wasi

FROM scratch
COPY --link --chmod=0755 --from=build /build/target/wasm32-wasi/release/wasi-hello-world.wasm /
ENTRYPOINT [ "./wasi-hello-world.wasm" ]