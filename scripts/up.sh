#!/bin/bash

set -euo pipefail

cluster_name="test-cluster"       # name of the k3d cluster
dockerfile_path="deployments/k3d" # path to the Dockerfile

DOCKER_IMAGES=("slight" "wws" "lunatic-submillisecond")
OUT_DIRS=("test/out_slight" "test/out_wws" "test/out_lunatic")
IMAGES=("slight-hello-world" "wws-hello-world" "lunatic-submillisecond-hello-world")

# build the Docker image for the k3d cluster
docker build -t k3d-shim-test "$dockerfile_path"

k3d cluster create "$cluster_name" --image k3d-shim-test --api-port 6551 -p '8082:80@loadbalancer' --agents 2

kubectl wait --for=condition=ready node --all --timeout=120s

# Iterate through the Docker images and build them
for i in "${!DOCKER_IMAGES[@]}"; do
  docker buildx build -t "${IMAGES[$i]}:latest" "./images/${DOCKER_IMAGES[$i]}" --load
  mkdir -p "${OUT_DIRS[$i]}"
  docker save -o "${OUT_DIRS[$i]}/img.tar" "${IMAGES[$i]}:latest"
  k3d image import "${OUT_DIRS[$i]}/img.tar" -c "$cluster_name"
done

sleep 5

echo ">>> cluster is ready"
