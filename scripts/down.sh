#!/bin/bash

set -euo pipefail

cluster_name="test-cluster"

teardown_test() {
  # delete k3d cluster
  k3d cluster delete "$cluster_name"

  # delete docker image
  docker rmi k3d-shim-test
}

teardown_test