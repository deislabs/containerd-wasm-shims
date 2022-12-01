import time
import os

def which(binary_name):
    """Return the path to a binary, or None if it is not found."""
    for path in os.environ["PATH"].split(os.pathsep):
        binary_path = os.path.join(path, binary_name)
        if os.path.exists(binary_path):
            return binary_path
    # panic
    raise RuntimeError("Could not find %s" % binary_name)
    

def setup_test():
    # run this as root 
    which("k3d")
    which("cross")
    which("docker")
    which("kubectl")

    dockerfile_path = "deployments/k3d"
    bin_path = "deployments/k3d/.tmp/"
    slight_shim_path = "deployments/k3d/.tmp/containerd-shim-slight-v1"
    spin_shim_path = "deployments/k3d/.tmp/containerd-shim-spin-v1"
    
    try:
        which(slight_shim_path)
    except RuntimeError:
        print(">>> install containerd-shim-slight-v1")
        os.system("cross build --target x86_64-unknown-linux-musl --release --manifest-path=containerd-shim-slight-v1/Cargo.toml")
        os.system(f"sudo install containerd-shim-slight-v1/target/x86_64-unknown-linux-musl/release/containerd-shim-slight-v1 {bin_path}")
    
    try:
        which(spin_shim_path)
    except RuntimeError:
        print(">>> install containerd-shim-spin-v1")
        os.system("cross build --target x86_64-unknown-linux-musl --release --manifest-path=containerd-shim-spin-v1/Cargo.toml")
        os.system(f"sudo install containerd-shim-spin-v1/target/x86_64-unknown-linux-musl/release/containerd-shim-spin-v1 {bin_path}")

    # build the docker image
    os.system(f"docker build -t k3d-shim-test {dockerfile_path}")

    # create the cluster
    os.system("k3d cluster create test-cluster --image k3d-shim-test --api-port 6551 -p '8082:80@loadbalancer' --agents 2")

    # wait for the cluster to be ready
    os.system("kubectl wait --for=condition=ready node --all --timeout=120s")
    
    # wait for 30 seconds
    time.sleep(30)

    print(">>> apply workloads")
    os.system("kubectl apply -f deployments/workloads")
    
    # wait for 30 seconds
    time.sleep(30)

    print(">>> cluster is ready")

if __name__ == '__main__':
    setup_test()