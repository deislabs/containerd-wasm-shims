SHIMS := slight spin wws lunatic
BUILD_TARGETS = $(foreach shim,$(SHIMS),build-$(shim)-cross-$(TARGET))

PREFIX ?= /usr/local
INSTALL ?= install
TEST_IMG_NAME_lunatic ?= wasmtest_lunatic:latest
TEST_IMG_NAME_spin ?= wasmtest_spin:latest
TEST_IMG_NAME_slight ?= wasmtest_slight:latest
TEST_IMG_NAME_wws ?= wasmtest_wws:latest
ARCH ?= x86_64
TARGET ?= $(ARCH)-unknown-linux-musl
PYTHON ?= python3
CONTAINERD_NAMESPACE ?= default
ifeq ($(VERBOSE),)
VERBOSE_FLAG :=
else
VERBOSE_FLAG := -vvv
endif

BIN_DIR ?= 

.PHONY: test
test: unit-tests integration-tests

.PHONY: unit-tests
unit-tests: build
	$(foreach shim,$(SHIMS),cross test --release --manifest-path=containerd-shim-$(shim)-v1/Cargo.toml --target $(TARGET);)

.PHONY: check-bins
check-bins:
	./scripts/check-bins.sh

./PHONY: move-bins
move-bins:
	./scripts/move-bins.sh $(BIN_DIR)

./PHONY: up
up:
	./scripts/up.sh

./PHONY: pod-status-check
pod-status-check:
	./scripts/pod-status-check.sh

./PHONY: workloads
workloads:
	./scripts/workloads.sh

.PHONY: integration-tests
integration-tests: install-cross check-bins move-bins up pod-status-check workloads
	cargo test -- --nocapture

.PHONY: tests/clean
test/clean:
	./scripts/down.sh

.PHONY: fmt
fmt:
	$(foreach shim,$(SHIMS),cargo fmt --all --manifest-path=containerd-shim-$(shim)-v1/Cargo.toml;)
	$(foreach shim,$(SHIMS),cargo clippy --all-targets --all-features --workspace --manifest-path=containerd-shim-$(shim)-v1/Cargo.toml -- -D warnings;)	
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features --workspace -- --deny=warnings

.PHONY: build
build: $(foreach shim,$(SHIMS),build-$(shim)-cross-$(TARGET))
	echo "Build complete"

.PHONY: install-cross
install-cross:
	@if [ -z $$(which cross) ]; then cargo install cross --git https://github.com/cross-rs/cross; fi

.PHONY: install-rust-targets
install-rust-targets:
	rustup target add wasm32-wasi && rustup target add wasm32-unknown-unknown

# build-cross can be be used to build any cross supported target (make build-cross-x86_64-unknown-linux-musl)
.PHONY: $(BUILD_TARGETS)
$(BUILD_TARGETS): SHIM = $(word 2,$(subst -, ,$@))
$(BUILD_TARGETS): install-cross
	cross build --release --target $(TARGET) --manifest-path=containerd-shim-$(SHIM)-v1/Cargo.toml $(VERBOSE_FLAG)

.PHONY: build-%
build-%: install-rust-targets
	cargo build --release --manifest-path=containerd-shim-$*-v1/Cargo.toml

.PHONY: install
install: $(foreach shim,$(SHIMS),build-$(shim))
	sudo $(INSTALL) containerd-shim-*/target/release/containerd-shim-*-v1 $(PREFIX)/bin

.PHONY: update-deps
update-deps:
	cargo update

test/out_%/img.tar: images/%/Dockerfile
	mkdir -p $(@D)
	docker buildx build --platform=wasi/wasm --load -t $(TEST_IMG_NAME_$*) ./images/$*
	docker save -o $@ $(TEST_IMG_NAME_$*)

load: $(foreach shim,$(SHIMS),test/out_$(shim)/img.tar)
	$(foreach shim,$(SHIMS),sudo ctr -n $(CONTAINERD_NAMESPACE) image import test/out_$(shim)/img.tar;)

.PHONY: run_%
run_%: install load
	sudo ctr run --net-host --rm --runtime=io.containerd.$*.v1 docker.io/library/$(TEST_IMG_NAME_$*) test$*

.PHONY: clean
clean: $(addprefix clean-,$(SHIMS))
	$(foreach shim,$(SHIMS),test -f $(PREFIX)/bin/containerd-shim-$(shim)-v1 && sudo rm -rf $(PREFIX)/bin/containerd-shim-$(proj)-v1 || true;)
	test -d ./test && sudo rm -rf ./test || true

.PHONY: clean-%
clean-%:
	cargo clean --manifest-path containerd-shim-$*-v1/Cargo.toml