IMAGE_NAME ?= k3swithshims
CLUSTER_NAME ?= k3s-default
ARCH ?= x86_64
TARGET ?= $(ARCH)-unknown-linux-musl

compile-musl-spin:
	make build-spin-cross-$(TARGET) -C ../..

compile-musl-slight:
	make build-slight-cross-$(TARGET) -C ../..

move-musl-to-tmp: compile-musl-spin compile-musl-slight
	mkdir -p ./.tmp
	cp ../../containerd-shim-slight-v1/target/$(TARGET)/release/containerd-shim-*-v1 ./.tmp/
	cp ../../containerd-shim-spin-v1/target/$(TARGET)/release/containerd-shim-*-v1 ./.tmp/

build-image: move-musl-to-tmp
	docker buildx build -t $(IMAGE_NAME) --platform linux/amd64,linux/arm64 .

up: build-image
	k3d cluster create $(CLUSTER_NAME) --image $(IMAGE_NAME) --api-port 6550 -p "8081:80@loadbalancer" --agents 1

deploy: 
	kubectl apply -f ../workloads

test:
	curl localhost:8081/spin/hello 
	curl localhost:8081/slight/hello

integration: move-musl-to-tmp
	cd ../.. && cargo test -- --nocapture

clean: 
	k3d cluster delete $(CLUSTER_NAME)

install-k3d:
	wget -q -O - https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash

.PHONY: deploy clean test build-image install-k3d up compile-musl move-musl-to-tmp
