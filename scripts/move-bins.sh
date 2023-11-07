#!/bin/bash

# Containerd Shim Installer Script
#
# This script automates the installation of specific containerd shim versions (slight, spin, wws, lunatic)
# by checking their existence and copying them to a desired location if not found.
#
# Usage:
# ./move-bins.sh [release_pattern] [target]
#
# Arguments:
# 1. release_pattern (Optional): The pattern used to locate the shim binaries.
# 2. target (Optional): The target architecture used in the release path.
#    Default value is `x86_64-unknown-linux-musl`.
#
# Example:
# ./move-bins.sh
#

set -euo pipefail

target="${2:-x86_64-unknown-linux-musl}"
release_pattern="${1:-containerd-shim-%s-v1/target/$target/release}"

dockerfile_path="deployments/k3d"
bin_path="${dockerfile_path}/.tmp/"
cluster_name="test-cluster"
default_shim_path="${bin_path}containerd-shim-"

declare -A shims=(
    [slight]="${default_shim_path}slight-v1"
    [spin]="${default_shim_path}spin-v2"
    [wws]="${default_shim_path}wws-v1"
    [lunatic]="${default_shim_path}lunatic-v1"
)

mkdir -p "$bin_path"

for shim_key in "${!shims[@]}"; do
    shim_path=${shims[$shim_key]}
    release_path=$(printf "$release_pattern" "$shim_key")

    if [ ! -f "$shim_path" ]; then
        echo ">>> install containerd-shim-${shim_key}-v1 from $release_path"
        cp "$(eval echo $release_path)/containerd-shim-${shim_key}-v1" "${bin_path}containerd-shim-${shim_key}-v1"
    fi
done
