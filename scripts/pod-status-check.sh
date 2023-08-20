#!/bin/bash

set -euo pipefail

# Get the status of all pods
pod_statuses=$(kubectl get pods --no-headers -o custom-columns=":status.phase")

# Check if all pods are running fine
all_running=true
for status in $pod_statuses; do
  if [ "$status" != "Running" ]; then
    all_running=false
    break
  fi
done

if $all_running; then
  echo "All pods are running fine."
else
  echo "Not all pods are running fine. Please check the status."
fi