# TODO: Remove this step once https://github.com/vmware-labs/wasm-workers-server/issues/120 is closed
FROM --platform=${BUILDPLATFORM} bitnami/minideb AS certs
RUN install_packages ca-certificates

FROM --platform=${BUILDPLATFORM} ghcr.io/vmware-labs/wws:latest AS installer
COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
# Install the Python runtime
RUN ["/wws", "runtimes", "install", "python", "latest"]

FROM scratch
COPY --from=installer /.wws ./.wws
COPY --from=installer /.wws.toml .
COPY --chmod=0755 ./hello.py .
ENTRYPOINT [ "/hello.py" ]