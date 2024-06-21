# Containerd Wasm Shims

This project aims to provide containerd shim implementations that can run [Wasm](https://webassembly.org/) / [WASI](https://github.com/WebAssembly/WASI) workloads using [runwasi](https://github.com/deislabs/runwasi) as a library. This means that by installing these shims onto Kubernetes nodes, we can add a [runtime class](https://kubernetes.io/docs/concepts/containers/runtime-class/) to Kubernetes and schedule Wasm workloads on those nodes. Your Wasm pods and deployments can act just like container workloads!

[runwasi](https://github.com/deislabs/runwasi) is a project that aims to run WASI workloads managed by [containerd](https://containerd.io/).

## Shims

> We are moving the spin shim to a separate repository, follwoing the annoucement of SpinKube project. Please check out the [SpinKube](https://github.com/spinkube) organization for the latest updates on the Spin shim.

This repo currently maintains four shims for Wasm application runtimes/frameworks:

1. [Spin](https://github.com/fermyon/spin) - a developer tool for building and running serverless Wasm applications.
2. [Slight](https://github.com/deislabs/spiderlightning) - a wasmtime-based runtime for running Wasm applications that use SpiderLightning (aks [WASI-Cloud-Core](https://github.com/WebAssembly/wasi-cloud-core)) capabilities
3. [Wasm Workers Server](https://github.com/vmware-labs/wasm-workers-server) - a tool to develop and run serverless applications server on top of Wasm.
4. [Lunatic](https://github.com/lunatic-solutions/lunatic) - an Erlang-inspired runtime for fast, robust and scalable server-side Wasm applications.

Below is a table of the shims and the the most recent versions of the shims that are supported by this project.

| **shim version** | v0.11.1                                                                          | v0.10                                                                            | v0.9                                                                             | v0.8                                                                             | v0.7                                                                             | v0.5.1                                                                    | v0.5.0                                                                    |
| ---------------- | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| **[Spin](https://github.com/fermyon/spin)**         | [v2.2.0](https://github.com/fermyon/spin/releases/tag/v2.2.0)                    | [v2.0.1](https://github.com/fermyon/spin/releases/tag/v2.0.1)                    | [v1.4.1](https://github.com/fermyon/spin/releases/tag/v1.4.1)                    | [v1.4.0](https://github.com/fermyon/spin/releases/tag/v1.4.0)                    | [v1.3.0](https://github.com/fermyon/spin/releases/tag/v1.3.0)                    | [v1.0.0](https://github.com/fermyon/spin/releases/tag/v1.0.0)             | [v0.9.0](https://github.com/fermyon/spin/releases/tag/v0.9.0)             |
| **[Slight](https://github.com/deislabs/spiderlightning)**       | [v0.5.1](https://github.com/deislabs/spiderlightning/releases/tag/v0.5.1)        | [v0.5.1](https://github.com/deislabs/spiderlightning/releases/tag/v0.5.1)        | [v0.5.1](https://github.com/deislabs/spiderlightning/releases/tag/v0.5.1)        | [v0.5.0](https://github.com/deislabs/spiderlightning/releases/tag/v0.5.1)        | [v0.5.0](https://github.com/deislabs/spiderlightning/releases/tag/v0.5.0)        | [v0.4.0](https://github.com/deislabs/spiderlightning/releases/tag/v0.4.0) | [v0.4.0](https://github.com/deislabs/spiderlightning/releases/tag/v0.4.0) |
| **[Wasm Workers Server](https://github.com/vmware-labs/wasm-workers-server)**          | [v1.7.0](https://github.com/vmware-labs/wasm-workers-server/releases/tag/v1.7.0) | [v1.7.0](https://github.com/vmware-labs/wasm-workers-server/releases/tag/v1.7.0) | [v1.5.0](https://github.com/vmware-labs/wasm-workers-server/releases/tag/v1.5.0) | [v1.4.0](https://github.com/vmware-labs/wasm-workers-server/releases/tag/v1.4.0) | [v1.2.0](https://github.com/vmware-labs/wasm-workers-server/releases/tag/v1.2.0) | /                                                                         | /                                                                         |
| **[Lunatic](https://github.com/lunatic-solutions/lunatic)**      | [v0.13.2](https://github.com/lunatic-solutions/lunatic/releases/tag/v0.13.2)     | [v0.13.2](https://github.com/lunatic-solutions/lunatic/releases/tag/v0.13.2)     | /                                                                                | /                                                                                | /                                                                                | /                                                                         | /                                                                         |

## Compare to `runwasi` shims

As mentioned above, this project uses runwasi's `containerd-shim-wasm` to build shim implementations for higher level Wasm application runtimes/frameworks. The `runwasi` shims are more lower level that are intended to run WASI-compatible Wasm modules, instead of Wasm applications that are built on top of a framework. If you are looking for `Wasmtime`, `WasmEdge` or `Wasmer` shims, please check out [runwasi](https://github.com/deislabs/runwasi).

## Quickstarts

- [Start k3d and run a sample WASM application](./deployments/k3d/README.md#how-to-run-the-example).
- [Create a Spin application on k3d](./containerd-shim-spin/quickstart.md)
- [Deploy a SpiderLightning application with k3d](./containerd-shim-slight/quickstart.md)
- [Deploy a Wasm Workers Server application with k3d](./containerd-shim-wws/quickstart.md)

### Building the shims

To build the shims in this project, run `make build`.

### Running the integration tests

To run the integration tests, run `make integration-tests`.

To clean up, run `make tests/clean`.

## Example Kubernetes Cluster Deployments

In [the deployments directory](deployments) you will find examples of deploying the shims to Kubernetes clusters and using them in example Kubernetes workloads.

## Using a shim in Kubernetes

To use one of these containerd shims in Kubernetes, you must do the following:

1. Install the shim binary somewhere on the path of your Kubernetes worker nodes. For example, copy `containerd-shim-spin-v2` to `/bin`.
2. Add the following to the containerd config.toml that maps the runtime type to the shim binary from step 1.

```toml
[plugins."io.containerd.grpc.v1.cri".containerd.runtimes.spin]
  runtime_type = "io.containerd.spin.v2"
```

3. Apply a runtime class that contains a handler that matches the "spin" config runtime name from step 2.

```yaml
apiVersion: node.k8s.io/v1
kind: RuntimeClass
metadata:
  name: wasmtime-spin
handler: spin
```

4. Deploy a Wasm workload to your cluster with the specified runtime class name matching the "wasmtime-spin" runtime class from step 3.

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: wasm-spin
spec:
  replicas: 1
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
        - name: spin-hello
          image: ghcr.io/deislabs/containerd-wasm-shims/examples/spin-rust-hello:latest
          command: ["/"]
```

## Code of Conduct

This project has adopted the [Microsoft Open Source Code of
Conduct](https://opensource.microsoft.com/codeofconduct/).

For more information see the [Code of Conduct
FAQ](https://opensource.microsoft.com/codeofconduct/faq/) or contact
[opencode@microsoft.com](mailto:opencode@microsoft.com) with any additional questions or comments.
