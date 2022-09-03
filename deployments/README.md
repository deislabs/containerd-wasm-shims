# Shim Deployment Examples
This directory contains examples of how to deploy the shims.

## K3ds Deployment
[This deployment](k3d) uses k3d to deploy a local Kubernetes cluster. It illustrates how to customize the k3ds image that is deployed. The image used to run the k3ds Kubernetes nodes has the shims copied into the `/bin` directory and the containerd config updated to include runtime bindings for the shims.

## Cluster API Deployment
Coming soon...

## Workloads
In [the workloads directory](./workloads) you will find common workloads that we deploy to the clusters to register runtime classes and deploy Wasm enabled pod workloads.