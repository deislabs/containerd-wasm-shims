import time
import os
import sys

def which(binary_name):
    """Return the path to a binary, or None if it is not found."""
    for path in os.environ["PATH"].split(os.pathsep):
        binary_path = os.path.join(path, binary_name)
        if os.path.exists(binary_path):
            return binary_path
    # panic
    raise RuntimeError("Could not find %s" % binary_name)


def setup_test(target):
    # run this as root
    which("k3d")
    which("cross")
    which("docker")
    which("kubectl")

    dockerfile_path = "deployments/k3d"
    bin_path = "deployments/k3d/.tmp/"
    slight_shim_path = "deployments/k3d/.tmp/containerd-shim-slight-v1"
    spin_shim_path = "deployments/k3d/.tmp/containerd-shim-spin-v1"
    wws_shim_path = "deployments/k3d/.tmp/containerd-shim-wws-v1"
    cluster_name = "test-cluster"

    # create bin_path if not exists
    if not os.path.exists(bin_path):
        os.makedirs(bin_path)

    try:
        which(slight_shim_path)
    except RuntimeError:
        print(">>> install containerd-shim-slight-v1")
        os.system(f"cp containerd-shim-slight-v1/target/{target}/release/containerd-shim-slight-v1 {bin_path}/containerd-shim-slight-v1")

    try:
        which(spin_shim_path)
    except RuntimeError:
        print(">>> install containerd-shim-spin-v1")
        os.system(f"cp containerd-shim-spin-v1/target/{target}/release/containerd-shim-spin-v1 {bin_path}/containerd-shim-spin-v1")

    try:
        which(wws_shim_path)
    except RuntimeError:
        print(">>> install containerd-shim-wws-v1")
        os.system(f"cp containerd-shim-wws-v1/target/{target}/release/containerd-shim-wws-v1 {bin_path}/containerd-shim-wws-v1")

    # build the docker image
    os.system(f"docker build -t k3d-shim-test {dockerfile_path}")

    # create the cluster
    os.system(f"k3d cluster create {cluster_name} --image k3d-shim-test --api-port 6551 -p '8082:80@loadbalancer' --agents 2")

    # wait for the cluster to be ready
    os.system("kubectl wait --for=condition=ready node --all --timeout=120s")

    # build slight and spin images locally
    os.system("docker buildx build -t slight-hello-world:latest ./images/slight --load")
    os.system("docker buildx build -t spin-hello-world:latest ./images/spin --load")
    os.system("docker buildx build -t wws-hello-world:latest ./images/wws --load")

    # create dir if not exists
    if not os.path.exists("test/out_slight"):
        os.makedirs("test/out_slight")
    if not os.path.exists("test/out_spin"):
        os.makedirs("test/out_spin")
    if not os.path.exists("test/out_wws"):
        os.makedirs("test/out_wws")

    # save docker images to tar ball
    os.system("docker save -o test/out_slight/img.tar slight-hello-world:latest")
    os.system("docker save -o test/out_spin/img.tar spin-hello-world:latest")
    os.system("docker save -o test/out_wws/img.tar wws-hello-world:latest")

    # load tar ball to k3d cluster
    os.system(f"k3d image import test/out_slight/img.tar -c {cluster_name}")
    os.system(f"k3d image import test/out_spin/img.tar -c {cluster_name}")
    os.system(f"k3d image import test/out_wws/img.tar -c {cluster_name}")

    # wait for 5 seconds
    time.sleep(5)

    print(">>> apply workloads")
    os.system("kubectl apply -f tests/workloads")

    # wait for 25 seconds
    time.sleep(25)

    os.system("kubectl describe pods")

    print(">>> cluster is ready")

if __name__ == '__main__':
    if len(sys.argv) < 2:
        target = "x86_64-unknown-linux-musl"
    else:
        target = sys.argv[1]
    setup_test(target = target)