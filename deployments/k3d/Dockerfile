ARG STAGE="test"

FROM scratch AS bin-test
COPY ./.tmp /

FROM scratch AS bin-release
ARG TARGETPLATFORM
COPY ./.tmp/${TARGETPLATFORM} /

FROM bin-${STAGE} AS bin

FROM rancher/k3s:v1.27.8-k3s2

# copy shims from target directory into the /bin
COPY --link --from=bin / /bin/

# copy in an containerd config into /etc/containerd/config.toml
COPY config.toml.tmpl /var/lib/rancher/k3s/agent/etc/containerd/config.toml.tmpl