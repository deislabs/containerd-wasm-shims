# K3d Shim Deployment
This example shows how one could deploy the shims and use them locally using k3d. The example consists of the following files.

```
$ tree .
.
├── config.toml.tmpl
├── Dockerfile
├── Makefile
└── README.md
```

- **config.toml.tmpl:** is the containerd config template that k3d uses to generate the containerd config. We have added a line to the template to register the shims, so that containerd will understand how to run our Wasm pod's runtime class.
- **Dockerfile:** is the specification for the image run as a Kubernetes node within the k3d cluster. We add the shims to the `/bin` directory and add the containerd config in the k3s prescribed directory.
- **Makefile**: has some helpful tasks to aid in execution.

## How to use this example
- `make install-k3d`: will install k3d
- `make up`: will build the shims and the k3d kubernetes cluster
- `make deploy`: will deploy our runtime classes, services, and deployments of Wasm workloads. You can find these manifests in [the workloads directory](../workloads).
- `make test`: will make a curl call to our deployed service
- `make clean`: will tear down the cluster