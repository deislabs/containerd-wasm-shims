# Quickstart

## Pre-requisites
Before you begin, you need to have the following installed:

- [Docker](https://docs.docker.com/install/) version 4.13.1 (90346) or later with [containerd enabled](https://docs.docker.com/desktop/containerd/)
- [k3d](https://k3d.io/v5.4.6/#installation)
- [kubectl](https://kubernetes.io/docs/tasks/tools/#kubectl)
- [SpiderLightning and `slight`](https://github.com/deislabs/spiderlightning#spiderlightning-or-slight)
- [Rust](https://www.rust-lang.org/tools/install)

## Start and configure a k3d cluster

Start a k3d cluster with the WASM shims already installed:

```bash
k3d cluster create wasm-cluster --image ghcr.io/deislabs/containerd-wasm-shims/examples/k3d:v0.3.3 -p "8081:80@loadbalancer" --agents 2 --registry-create mycluster-registry:12345
```

Apply RuntimeClass for SpiderLightning applications to use the SpiderLightning WASM shim:

```bash
kubectl apply -f https://raw.githubusercontent.com/deislabs/containerd-wasm-shims/main/deployments/workloads/runtime.yaml
```

## Deploy an existing sample SpiderLightning application

Deploy a pre-built sample SpiderLightning application:

```bash
kubectl apply -f https://raw.githubusercontent.com/deislabs/containerd-wasm-shims/main/deployments/workloads/workload.yaml
echo "waiting 5 seconds for workload to be ready"
sleep 5
curl -v http://0.0.0.0:8081/slight/hello
```

Confirm you see a response from the sample application. For example:

```output
$ curl -v http://0.0.0.0:8081/slight/hello
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
< Accept: */*
< Accept-Encoding: gzip
< Content-Length: 5
< Date: Tue, 11 Oct 2022 19:12:17 GMT
< Host: 0.0.0.0:8081
< User-Agent: curl/7.68.0
< X-Forwarded-For: 10.42.1.1
< X-Forwarded-Host: 0.0.0.0:8081
< X-Forwarded-Port: 8081
< X-Forwarded-Proto: http
< X-Forwarded-Server: traefik-7cd4fcff68-xr2gh
< X-Real-Ip: 10.42.1.1
< Content-Type: text/plain; charset=utf-8
< 
* Connection #0 to host 0.0.0.0 left intact
hello
```

Delete the pre-built sample SpiderLightning application:

```bash
kubectl delete -f https://raw.githubusercontent.com/deislabs/containerd-wasm-shims/main/deployments/workloads/workload.yaml
```

## Build and deploy a SpiderLightning application

Clone the [deislabs/containerd-wasm-shims](https://github.com/deislabs/containerd-wasm-shims) project and navigate to the `containerd-wasm-shims/images/slight` directory. This directory contains the source for the application you deployed in the previous section.

```bash
git clone https://github.com/deislabs/containerd-wasm-shims.git
cd containerd-wasm-shims/images/slight
```

Use `rustup` to install the `wasm32-wasi` target and `cargo` to build the application. For example:

```bash
rustup target add wasm32-wasi
cargo build --target wasm32-wasi
```

## Run the application

Use `slight run` to run the application on your development computer. For example:

```bash
slight -c slightfile.toml run -m target/wasm32-wasi/debug/slight.wasm
```

The application is running at `http://0.0.0.0/`.

Access the `/hello` route. For example, use `curl` in a new terminal window:

```bash
$$ curl -v http://0.0.0.0/hello
*   Trying 0.0.0.0:80...
* TCP_NODELAY set
* Connected to 0.0.0.0 (127.0.0.1) port 80 (#0)
> GET /hello HTTP/1.1
> Host: 0.0.0.0
> User-Agent: curl/7.68.0
> Accept: */*
> 
* Mark bundle as not supporting multiuse
< HTTP/1.1 200 OK
< host: 0.0.0.0
< user-agent: curl/7.68.0
< accept: */*
< content-length: 5
< date: Tue, 11 Oct 2022 20:15:47 GMT
< 
* Connection #0 to host 0.0.0.0 left intact
hello
```

Return to the terminal window running `slight run` and stop the application.

## Create a container image for the application

Use `docker` to build the container image and push it to the k3d registry:

```bash
docker buildx build --platform=wasi/wasm -t localhost:12345/qs-wasm-slight .
docker push localhost:12345/qs-wasm-slight:latest
```

## Deploy the application

Create a `qs.yaml` file with the following:

```yml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: wasm-slight
spec:
  replicas: 1
  selector:
    matchLabels:
      app: wasm-slight
  template:
    metadata:
      labels:
        app: wasm-slight
    spec:
      runtimeClassName: wasmtime-slight
      containers:
        - name: testwasm
          image: mycluster-registry:12345/qs-wasm-slight:latest
          command: ["/"]
---
apiVersion: v1
kind: Service
metadata:
  name: wasm-slight
spec:
  ports:
    - protocol: TCP
      port: 80
      targetPort: 80
  selector:
    app: wasm-slight
---
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: wasm-slight
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
                name: wasm-slight
                port:
                  number: 80
```

Deploy the application and confirm it is running:

```bash
kubectl apply -f qs.yaml
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
< Accept: */*
< Accept-Encoding: gzip
< Content-Length: 5
< Date: Tue, 11 Oct 2022 20:29:12 GMT
< Host: 0.0.0.0:8081
< User-Agent: curl/7.68.0
< X-Forwarded-For: 10.42.0.0
< X-Forwarded-Host: 0.0.0.0:8081
< X-Forwarded-Port: 8081
< X-Forwarded-Proto: http
< X-Forwarded-Server: traefik-7cd4fcff68-xr2gh
< X-Real-Ip: 10.42.0.0
< Content-Type: text/plain; charset=utf-8
< 
* Connection #0 to host 0.0.0.0 left intact
hello
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

## Next steps

Try running Wasm applications on [Docker Desktop](https://docs.docker.com/desktop/wasm/) or on Kubernetes, such as [AKS](https://learn.microsoft.com/en-us/azure/aks/use-wasi-node-pools).

If you prefer tutorials in video format, check out [Deploying Your Wasm WASI App to Kubernetes in 10 minutes](https://youtu.be/czxUVhMpWXg) on Youtube.