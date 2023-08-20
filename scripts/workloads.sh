#!/bin/bash

set -euo pipefail

# apply the workloads
echo ">>> apply workloads"
kubectl apply -f tests/workloads


# wait for all the pods to be ready
kubectl wait --for=condition=ready --timeout=50s pod --all

# get and describe all the pods
echo ">>> Pods:"
kubectl get pods -o wide
kubectl describe pods

# get and describe all the deployments
echo ">>> Deployments:"
kubectl get deployments -o wide
kubectl describe deployments

# get and describe all the services
echo ">>> Services:"
kubectl get services -o wide
kubectl describe services