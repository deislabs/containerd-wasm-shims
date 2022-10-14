# Containerd Wasm Shims
This project aims to provide containerd shim implementations that can run Wasm / WASI workloads using [runwasi](https://github.com/deislabs/runwasi) as a library. This means that by installing these shims onto Kubernetes nodes, we can add a runtime class to Kubernetes and schedule Wasm workloads on those nodes. Your Wasm pods and deployments can act just like container workloads!

[runwasi](https://github.com/deislabs/runwasi) is a project that aims to run wasm workloads running on [Wasmtime](https://wasmtime.dev/), a fast and secure runtime for WebAssembly, which is managed by containerd.

## Quickstarts

- [Start k3d and run a sample WASM application](./deployments/k3d/README.md#how-to-run-the-example).
- [Create a Spin application on k3d](./containerd-shim-spin-v1/quickstart.md)
- [Deploy a SpiderLightning application with k3d](./containerd-shim-slight-v1/quickstart.md)

## Containerd Wasm Shims
Each of the shims below leverage runwasi to provide the bridge between K8s and containerd. 

### Spin shim
The Spin shim, as the name implies, is powered by the [Fermyon Spin](https://github.com/fermyon/spin) engine. Spin is an open source framework for building and running fast, secure, and composable cloud microservices with WebAssembly.

If you are curious, [here is the Spin shim source code](./containerd-shim-spin-v1).

### Building the shims
To build the shims in this project, run `make`.

### Running a shim locally on Linux
To run the spin shim using [a hello world Spin example](./images/spin), run `make run_spin`. This will use `ctr` to simulate the same call that would be made from containerd to run a local OCI container image.

The "hello world" image contains only 2 files, the [`spin.toml`](./images/spin/spin.toml) file and the `spin_rust_hello.wasm` file created by compiling the "hello world" spin example by running `cargo build --target wasm32-wasi --release` in [the example directory](./images/spin). **The image is only 1.9MB!**

### Cleaning up
To clean up, run `make clean`.

## Example Kubernetes Cluster Deployments
In [the deployments directory](deployments) you will find examples of deploying the shims to Kubernetes clusters and using them in example Kubernetes workloads.

## Using a shim in Kubernetes
To use one of these containerd shims in Kubernetes, you must do the following:
1. Install the shim binary somewhere on the path of your Kubernetes worker nodes. For example, copy `containerd-shim-spin-v1` to  `/bin`.
2. Add the following to the containerd config.toml that maps the runtime type to the shim binary from step 1.
  ```toml
    [plugins.cri.containerd.runtimes.spin]
      runtime_type = "io.containerd.spin.v1"
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
