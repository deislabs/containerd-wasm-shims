# Quickstart

## Pre-requisites
Before you begin, you need to have the following installed:

- [Docker](https://docs.docker.com/install/)
- [k3d](https://k3d.io/v5.4.6/#installation)
- [kubectl](https://kubernetes.io/docs/tasks/tools/#kubectl)
- [Spin binary and templates](https://spin.fermyon.dev/quickstart/)
- [Rust](https://www.rust-lang.org/tools/install)

## Start and configure a k3d cluster

Start a k3d cluster with the wasm shims already installed:

```bash
k3d cluster create wasm-cluster --image ghcr.io/deislabs/containerd-wasm-shims/examples/k3d:v0.3.2 -p "8081:80@loadbalancer" --agents 2 --registry-create mycluster-registry:12345
```

Apply RuntimeClass for spin applications to use the spin wasm shim:

```bash
kubectl apply -f https://raw.githubusercontent.com/deislabs/containerd-wasm-shims/main/deployments/workloads/runtime.yaml
```

## Deploy an existing sample spin application

Deploy a pre-built sample spin application:

```bash
kubectl apply -f https://raw.githubusercontent.com/deislabs/containerd-wasm-shims/main/deployments/workloads/workload.yaml
echo "waiting 5 seconds for workload to be ready"
sleep 5
curl -v http://0.0.0.0:8081/hello
```

Confirm you see a response from the sample application. For example:

```output
$ curl -v http://0.0.0.0:8081/hello
*   Trying 0.0.0.0:8081...
* TCP_NODELAY set
* Connected to 0.0.0.0 (127.0.0.1) port 8081 (#0)
> GET /hello HTTP/1.1
> Host: 0.0.0.0:8081
> User-Agent: curl/7.68.0
> Accept: */*
> 
* Mark bundle as not supporting multiuse
< HTTP/1.1 200 OK
< Content-Length: 22
< Date: Mon, 10 Oct 2022 20:39:43 GMT
< Content-Type: text/plain; charset=utf-8
< 
* Connection #0 to host 0.0.0.0 left intact
Hello world from Spin!
```

Delete the pre-built sample spin application:

```bash
kubectl delete -f https://raw.githubusercontent.com/deislabs/containerd-wasm-shims/main/deployments/workloads/workload.yaml
```

## Create a new spin sample application

Use `spin` to create a new sample application based on the `http-rust` template:

```bash
spin new http-rust qs-wasm-spin
```

Add the details when prompted. For example:

```bash
$ spin new http-rust qs-wasm-spin
Project description: An example app for the quickstart
HTTP base: /
HTTP path: /hi
```

## Build the application

Navigate to the directory where you created the application:

```bash
cd qs-wasm-spin
```

Use `rustup` to install the `wasm32-wasi` target and `spin build` to build the application. For example:

```bash
rustup target add wasm32-wasi
spin build
```

## Run the application

Use `spin up` to run the application on your development computer. For example:

```bash
spin up
```

The output shows the url for accessing the application. For example:

```output
$ spin up
Serving http://127.0.0.1:3000
Available Routes:
  qs-wasm-spin: http://127.0.0.1:3000/hi
```

Access the `/hi` route. For example, use `curl` in a new terminal window:

```bash
$ curl http://127.0.0.1:3000/hi
Hello, Fermyon
```

Return to the terminal window running `spin up` and stop the application.

## Create a container image for the application

Create a `Dockerfile` at the root of the application directory with the following:

```dockerfile
FROM rust:1.59 AS build
WORKDIR /opt/build
COPY . .
RUN rustup target add wasm32-wasi && cargo build --target wasm32-wasi --release

FROM scratch
COPY --from=build /opt/build/target/wasm32-wasi/release/qs_wasm_spin.wasm .
COPY --from=build /opt/build/spin.toml .
```

Update `spin.toml` to change `source` to `qs_wasm_spin.wasm`:

```toml
...
[[component]]
id = "qs-wasm-spin"
source = "qs_wasm_spin.wasm"
...
```

Use `docker` to build the container image and push it to the k3d registry:

```bash
docker build -t localhost:12345/qs-wasm-spin .
docker push localhost:12345/qs-wasm-spin:latest
```

## Deploy the application

Create a `qs.yaml` file with the following:

```yml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: wasm-spin
spec:
  replicas: 3
  selector:
    matchLabels:
      app: wasm-spin
  template:
    metadata:
      labels:
        app: wasm-spin
    spec:
      runtimeClassName: wasmtime-spin
      containers:
        - name: testwasm
          image: mycluster-registry:12345/qs-wasm-spin:latest
          command: ["/"]
---
apiVersion: v1
kind: Service
metadata:
  name: wasm-spin
spec:
  ports:
    - protocol: TCP
      port: 80
      targetPort: 80
  selector:
    app: wasm-spin
---
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: wasm-spin
  annotations:
    ingress.kubernetes.io/ssl-redirect: "false"
    kubernetes.io/ingress.class: traefik
spec:
  rules:
    - http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: wasm-spin
                port:
                  number: 80
```

Deploy the application and confirm it is running:

```bash
kubectl apply -f qs.yaml
echo "waiting 5 seconds for workload to be ready"
sleep 5
curl -v http://0.0.0.0:8081/hi
```

Confirm you see a response from the sample application. For example:

```output
$ curl -v http://0.0.0.0:8081/hi
*   Trying 0.0.0.0:8081...
* TCP_NODELAY set
* Connected to 0.0.0.0 (127.0.0.1) port 8081 (#0)
> GET /hi HTTP/1.1
> Host: 0.0.0.0:8081
> User-Agent: curl/7.68.0
> Accept: */*
> 
* Mark bundle as not supporting multiuse
< HTTP/1.1 200 OK
< Content-Length: 14
< Date: Tue, 11 Oct 2022 02:23:32 GMT
< Foo: bar
< Content-Type: text/plain; charset=utf-8
< 
* Connection #0 to host 0.0.0.0 left intact
Hello, Fermyon
```

## Clean up

Remove the sample application:

```bash
kubectl delete -f qs.yaml
```

Delete the cluster:

```bash
k3d cluster delete wasm-cluster
```