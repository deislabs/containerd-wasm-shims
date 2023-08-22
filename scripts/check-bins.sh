#!/bin/bash

# Description:
# This script checks for the existence of specific binaries on the system.
# It uses a function called which_binary to accomplish this.
# The script first verifies the existence of the binaries and then prints their paths.

# Usage:
# ./check-bins.sh

# Dependencies:
# The script expects the following binaries to be present in the system's PATH:
# k3d, cross, docker, kubectl

set -euo pipefail

# Function: which_binary
# Description:
# Finds and prints the path of the specified binary if it exists in the system's PATH.
# If the binary is not found, it prints an error message.
# Parameters:
# $1 - The name of the binary to locate.
which_binary() {
  local binary_name="$1"
  local binary_path
  binary_path=$(command -v "$binary_name")
  if [[ -n "$binary_path" ]]; then
    echo "$binary_path"
  else
    echo "Could not find $binary_name" >&2
    exit 1
  fi
}


# List of binary names
binaries=("k3d" "cross" "docker" "kubectl")

for binary in "${binaries[@]}"; do
  which_binary "$binary"
done
