# Setup Docker + Wasm

If you are using Docker Desktop for Windoes/MacOS/Linux, please refer to this [document](https://docs.docker.com/desktop/wasm/) on turning on wasm intergration in Docker Desktop. This is the easiest way to get started with wasm shims. 

The more important part is to enable `containerd` for pulling and storing images in Docker Desktop. Refer this [document](https://docs.docker.com/desktop/containerd/#enabling-the-containerd-image-store-feature) for more details.

This document is primarily for those who are using Docker daemon on Linux. 

## Install Docker 24.0.0-beta.2

Before you install Docker 24.0.0-beta.2, please uninstall docker tools you have installed on your machine. Please see this [document](https://docs.docker.com/engine/install/ubuntu/) for more details.

```shell
sudo apt-get purge docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin docker-ce-rootless-extras
```

After you have uninstalled docker tools, please install Docker 24.0.0-beta.2.

```shell
curl -fsSL https://get.docker.com -o get-docker.sh

sudo CHANNEL=test sh get-docker.sh
```

This will install Docker 24.0.0-beta.2 on your machine.

To verify the installation, please run the following command.

```shell
docker version

> Docker version 24.0.0-beta.2
```

Now you have Docker 24.0.0-beta.2 installed on your machine. Next step, we will need to edit the `/etc/docker/daemon.json` file to enable `containerd` for pulling and storing images in Docker Desktop.

If you don't have `/etc/docker/daemon.json` file, please create one.
```json
{ 
  "features": { 
    "containerd-snapshotter": true 
  }
}
```

Otherwise, please append the above content to the file.

## Build wasm images

Now you have Docker 24.0.0-beta.2 installed on your machine, you can build wasm images using the following command.

```shell
cd deployments/k3d
docker buildx build --platform=wasi/wasm --load -t wasmtest_spin:latest ../../images/spin
```

The `wasi/wasm` platform specifies that the image is a wasm image. The major benefit of using this platform is that you don't need to build each image for a different computer architecture. 

The `--load` flag tells Docker to load the image into the local Docker daemon. As described in this [issue](https://github.com/deislabs/containerd-wasm-shims/issues/87), this flag actually doesn't work for wasi/wasm platform. You will need to save the image to a tar file and load it into the local Docker daemon.

```shell
docker save wasmtest_spin:latest -o wasmtest_spin.tar
```

### Load the image to k3d

Refer to this [document](https://github.com/deislabs/containerd-wasm-shims/tree/main/deployments/k3d#how-build-get-started-from-source) to create a k3d cluster.

Assume you have a k3d cluster named `k3d-default`, you can load the image to the cluster using the following command.

```shell
k3d image load wasmtest_spin.tar
```
