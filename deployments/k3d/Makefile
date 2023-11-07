SHIMS = spin slight wws lunatic
SPIN_VERSION = v2
IMAGE_NAME ?= k3swithshims
CLUSTER_NAME ?= k3s-default
PLATFORM ?= linux/amd64
ARCH ?= x86_64
TARGET ?= $(ARCH)-unknown-linux-musl
TEST_IMG_NAME_lunatic ?= wasmtest_lunatic:latest
TEST_IMG_NAME_spin ?= wasmtest_spin:latest
TEST_IMG_NAME_slight ?= wasmtest_slight:latest
TEST_IMG_NAME_wws ?= wasmtest_wws:latest

compile-musl-%:
	make build-$*-cross-$(TARGET) -C ../..

move-musl-to-tmp: $(addprefix compile-musl-,$(SHIMS))
	mkdir -p ./.tmp
	cp ../../containerd-shim-spin/target/$(TARGET)/release/containerd-shim-spin-$(SPIN_VERSION) ./.tmp/
	cp ../../containerd-shim-slight/target/$(TARGET)/release/containerd-shim-slight-v1 ./.tmp/
	cp ../../containerd-shim-wws/target/$(TARGET)/release/containerd-shim-wws-v1 ./.tmp/
	cp ../../containerd-shim-lunatic/target/$(TARGET)/release/containerd-shim-lunatic-v1 ./.tmp/

build-multi-k3d-image: move-musl-to-tmp
	docker buildx build -t $(IMAGE_NAME) --platform linux/amd64,linux/arm64 .

build-dev-k3d-image: move-musl-to-tmp
	docker buildx build -t $(IMAGE_NAME) --load --platform $(PLATFORM) .

create-k3d: build-dev-k3d-image
	k3d cluster create $(CLUSTER_NAME) --image $(IMAGE_NAME) --api-port 6550 -p "8081:80@loadbalancer" --agents 1

build-workload-images:
	$(foreach shim,$(SHIMS),docker buildx build --platform=wasi/wasm --load -t $(TEST_IMG_NAME_$(shim)) ../../images/$(shim);)

load-workload-images: build-workload-images
	$(foreach shim,$(SHIMS),k3d image load $(TEST_IMG_NAME_$(shim));)

up: create-k3d load-workload-images
	kubectl label nodes k3d-k3s-default-agent-0 spin-enabled=true slight-enabled=true wws-enabled=true lunatic-enabled=true
	kubectl apply -f ./workload

test:
	$(foreach shim,$(SHIMS),curl localhost:8081/$(shim)/hello;)

integration: move-musl-to-tmp
	cd ../.. && cargo test -- --nocapture

clean:
	k3d cluster delete $(CLUSTER_NAME)

install-k3d:
	wget -q -O - https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash

.PHONY: deploy clean test build-image install-k3d up compile-musl move-musl-to-tmp build-multi-k3d-image build-dev-k3d-image build-workload-images load-workload-images
