PREFIX ?= /usr/local
INSTALL ?= install
TEST_IMG_NAME_SPIN ?= wasmtest_spin:latest

CONTAINERD_NAMESPACE ?= default

.PHONY: build
build:
	cargo build --release

.PHONY: install-cross
install-cross:
	cargo install cross --git https://github.com/cross-rs/cross

build-static-musl: install-cross
	cross build --release --target x86_64-unknown-linux-musl

.PHONY: install
install: build
	sudo $(INSTALL) target/release/containerd-shim-*-v1 $(PREFIX)/bin

.PHONY: update-deps
update-deps:
	cargo update

test/out_spin/img.tar: images/spin/Dockerfile
	mkdir -p $(@D)
	docker build -t $(TEST_IMG_NAME_SPIN) ./images/spin
	docker save -o $@ $(TEST_IMG_NAME_SPIN)

load: test/out_spin/img.tar
	sudo ctr -n $(CONTAINERD_NAMESPACE) image import test/out_spin/img.tar

.PHONY: run_spin
run_spin: install load
	sudo ctr run --net-host --rm --runtime=io.containerd.spin.v1 docker.io/library/$(TEST_IMG_NAME_SPIN) testspin

.PHONY: clean
clean:
	sudo rm -rf $(PREFIX)/bin/containerd-shim-spin-v1
	sudo rm -rf ./test