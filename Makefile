PREFIX ?= /usr/local
INSTALL ?= install
TEST_IMG_NAME_SPIN ?= wasmtest_spin:latest
TEST_IMG_NAME_SLIGHT ?= wasmtest_slight:latest
ARCH ?= x86_64
TARGET ?= $(ARCH)-unknown-linux-musl
PYTHON ?= python3
CONTAINERD_NAMESPACE ?= default

.PHONY: test
test: unit-tests integration-tests

.PHONY: unit-tests
unit-tests:
	cross test --release --manifest-path=containerd-shim-slight-v1/Cargo.toml --target $(TARGET)
	cross test --release --manifest-path=containerd-shim-spin-v1/Cargo.toml --target $(TARGET)

.PHONY: integration-tests
integration-tests:
	$(PYTHON) tests/setup.py
	cargo test -- --nocapture
	$(PYTHON) tests/teardown.py

.PHONY: fmt
fmt: 
	cargo fmt --all --manifest-path=containerd-shim-slight-v1/Cargo.toml -- --check
	cargo fmt --all --manifest-path=containerd-shim-spin-v1/Cargo.toml -- --check
	cargo clippy --all-targets --all-features --workspace --manifest-path=containerd-shim-slight-v1/Cargo.toml -- -D warnings
	cargo clippy --all-targets --all-features --workspace --manifest-path=containerd-shim-spin-v1/Cargo.toml -- -D warnings

.PHONY: build
build: build-spin-cross-$(TARGET) build-slight-cross-$(TARGET)
	echo "Build complete"

.PHONY: install-cross
install-cross:
	@if [ -z $$(which cross) ]; then cargo install cross --git https://github.com/cross-rs/cross; fi

# build-cross can be be used to build any cross supported target (make build-cross-x86_64-unknown-linux-musl)
.PHONY: build-spin-cross-%
build-spin-cross-%: install-cross
	cross build --release --target $* --manifest-path=containerd-shim-spin-v1/Cargo.toml

.PHONY: build-slight-cross-%
build-slight-cross-%: install-cross
	cross build --release --target $* --manifest-path=containerd-shim-slight-v1/Cargo.toml

.PHONY: build-spin
build-spin:
	cargo build --release --manifest-path=containerd-shim-spin-v1/Cargo.toml

.PHONY: build-slight
build-slight:
	cargo build --release --manifest-path=containerd-shim-slight-v1/Cargo.toml

.PHONY: install
install: build-spin build-slight
	sudo $(INSTALL) target/release/containerd-shim-*-v1 $(PREFIX)/bin

.PHONY: update-deps
update-deps:
	cargo update

test/out_spin/img.tar: images/spin/Dockerfile
	mkdir -p $(@D)
	docker buildx build --platform=wasi/wasm -t $(TEST_IMG_NAME_SPIN) ./images/spin
	docker save -o $@ $(TEST_IMG_NAME_SPIN)

test/out_slight/img.tar: images/slight/Dockerfile
	mkdir -p $(@D)
	docker buildx build --platform=wasi/wasm -t $(TEST_IMG_NAME_SLIGHT) ./images/slight
	docker save -o $@ $(TEST_IMG_NAME_SLIGHT)

load: test/out_spin/img.tar test/out_slight/img.tar
	sudo ctr -n $(CONTAINERD_NAMESPACE) image import test/out_spin/img.tar
	sudo ctr -n $(CONTAINERD_NAMESPACE) image import test/out_slight/img.tar

.PHONY: run_spin
run_spin: install load
	sudo ctr run --net-host --rm --runtime=io.containerd.spin.v1 docker.io/library/$(TEST_IMG_NAME_SPIN) testspin

.PHONY: run_slight
run_slight: install load
	sudo ctr run --net-host --rm --runtime=io.containerd.slight.v1 docker.io/library/$(TEST_IMG_NAME_SLIGHT) testslight

.PHONY: clean
clean: clean-slight clean-spin
	test -f $(PREFIX)/bin/containerd-shim-spin-v1 && sudo rm -rf $(PREFIX)/bin/containerd-shim-spin-v1 || true
	test -f  $(PREFIX)/bin/containerd-shim-slight-v1 && sudo rm -rf $(PREFIX)/bin/containerd-shim-slight-v1 || true
	test -d ./test && sudo rm -rf ./test || true

.PHONY: clean-spin
clean-spin:
	cargo clean --manifest-path containerd-shim-spin-v1/Cargo.toml

.PHONY: clean-slight
clean-slight:
	cargo clean --manifest-path containerd-shim-slight-v1/Cargo.toml
