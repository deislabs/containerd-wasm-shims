#!/bin/bash

set -euo pipefail

cluster_name="test-cluster"
dockerfile_path="deployments/k3d"
bin_path="${dockerfile_path}/.tmp/"

teardown_test() {
  # delete k3d cluster
  k3d cluster delete "$cluster_name"

  # delete docker image
  docker rmi k3d-shim-test

  # delete binaries
  rm -r "$bin_path"
}

teardown_test