FROM rancher/k3s:v1.24.6-k3s1

# copy shims from target directory into the /bin
COPY ./.tmp /bin/

# copy in an containerd config into /etc/containerd/config.toml
COPY config.toml.tmpl /var/lib/rancher/k3s/agent/etc/containerd/config.toml.tmpl