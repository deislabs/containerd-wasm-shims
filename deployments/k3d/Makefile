IMAGE_NAME ?= k3swithshims
CLUSTER_NAME ?= k3s-default

compile-musl:
	make build-cross-x86_64-unknown-linux-musl -C ../..

move-musl-to-tmp: compile-musl
	mkdir -p ./.tmp
	cp ../../target/x86_64-unknown-linux-musl/release/containerd-shim-*-v1 ./.tmp/

build-image: move-musl-to-tmp
	docker build -t $(IMAGE_NAME) .

up: build-image
	k3d cluster create $(CLUSTER_NAME) --image $(IMAGE_NAME) --api-port 6550 -p "8081:80@loadbalancer" --agents 2

deploy: 
	kubectl apply -f ../workloads

test:
	curl localhost:8081/hello

clean: 
	k3d cluster delete $(CLUSTER_NAME)

install-k3d:
	wget -q -O - https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash

.PHONY: deploy clean test build-image install-k3d up compile-musl move-musl-to-tmp
