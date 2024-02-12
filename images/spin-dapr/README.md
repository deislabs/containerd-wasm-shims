# Spin Dapr Demo

## Description
This demo application is a simple Spin app that is triggered by the daprs [kubernetes input binding](https://docs.dapr.io/reference/components-reference/supported-bindings/kubernetes-binding/) when it is called with the path `/kevents` it writes the body to a redis running at `redis://localhost:6379` to the key `lastEvent`. All other paths just return the value of the `lastEvent` key.

### Prerequisites
Install dapr cli
```sh
curl -fsSL https://raw.githubusercontent.com/dapr/cli/master/install/install.sh | bash
```

Install spin cli:
```sh
curl -fsSL https://developer.fermyon.com/downloads/install.sh | bash
sudo mv ./spin /usr/local/bin/
```

### Run example with K3d:
```sh
# start the K3d cluster
k3d cluster create wasm-cluster --image ghcr.io/deislabs/containerd-wasm-shims/examples/k3d:v0.11.0 -p "8081:80@loadbalancer"  
# Install Dapr
dapr init -k --wait
# or via helm
# helm repo add dapr https://dapr.github.io/helm-charts/
# helm repo update
# helm upgrade --install dapr dapr/dapr --namespace dapr-system --create-namespace --wait

# build the application
cd images/spin-dapr
spin build
cd -
# create an image and load it into K3d
docker build images/spin-dapr -t spin-dapr:latest --load
mkdir -p test/out_spin-dapr/
docker save spin-dapr:latest -o test/out_spin-dapr/img.tar
k3d image load -c wasm-cluster spin-dapr:latest test/out_spin-dapr/img.tar 
# Apply the manifest
kubectl apply -f https://github.com/deislabs/containerd-wasm-shims/raw/main/deployments/workloads/runtime.yaml
kubectl apply -f images/spin-dapr/deploy.yaml

# When everythin is up, forward the port and get the last kubernetes event
kubectl port-forward svc/spin-dapr 8080:80 &
curl localhost:8080 | jq
```