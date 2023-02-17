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
    
    # create bin_path if not exists
    if not os.path.exists(bin_path):
        os.makedirs(bin_path)

    try:
        which(slight_shim_path)
    except RuntimeError:
        print(">>> install containerd-shim-slight-v1")
        os.system("cross build --target x86_64-unknown-linux-musl --release --manifest-path=containerd-shim-slight-v1/Cargo.toml -vvv")
        os.system(f"mv containerd-shim-slight-v1/target/x86_64-unknown-linux-musl/release/containerd-shim-slight-v1 {bin_path}")
    
    try:
        which(spin_shim_path)
    except RuntimeError:
        print(">>> install containerd-shim-spin-v1")
        os.system("cross build --target x86_64-unknown-linux-musl --release --manifest-path=containerd-shim-spin-v1/Cargo.toml -vvv")
        os.system(f"mv containerd-shim-spin-v1/target/x86_64-unknown-linux-musl/release/containerd-shim-spin-v1 {bin_path}")

    # build the docker image
    os.system(f"docker build -t k3d-shim-test {dockerfile_path}")

    # create the cluster
    os.system("k3d cluster create test-cluster --image k3d-shim-test --api-port 6551 -p '8082:80@loadbalancer' --agents 2")

    # wait for the cluster to be ready
    os.system("kubectl wait --for=condition=ready node --all --timeout=120s")
    
    # build slight and spin images locally
    os.system("docker buildx build -t slight-hello-world:latest ./images/slight --load")
    os.system("docker buildx build -t spin-hello-world:latest ./images/spin --load")
    
    # save docker images to tar ball
    os.system("docker save -o test/out_slight/img.tar slight-hello-world:latest")
    os.system("docker save -o test/out_spin/img.tar spin-hello-world:latest")

    # load tar ball to k3d cluster
    os.system("k3d image import test/out_slight/img.tar -c test-cluster")
    os.system("k3d image import test/out_spin/img.tar -c test-cluster")

    # wait for 10 seconds
    time.sleep(10)

    print(">>> apply workloads")
    os.system("kubectl apply -f tests/workloads")
    
    # wait for 30 seconds
    time.sleep(30)

    print(">>> cluster is ready")

if __name__ == '__main__':
    setup_test()