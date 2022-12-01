import os

def teardown_test():
    # delete k3d cluster
    os.system("k3d cluster delete test-cluster")

    # delete docker image
    os.system("docker rmi k3d-shim-test")


if __name__ == '__main__':
    teardown_test()