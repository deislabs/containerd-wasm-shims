use containerd_shim_wasm::container::Instance;
use containerd_shim_wasm::sandbox::cli::{revision, shim_main, version};

mod engine;

fn main() {
    shim_main::<Instance<engine::SpinEngine>>("spin", version!(), revision!(), Some("v2"), None);
}
