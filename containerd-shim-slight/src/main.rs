use containerd_shim_wasm::container::Instance;
use containerd_shim_wasm::sandbox::cli::{revision, shim_main, version};

mod engine;

fn main() {
    shim_main::<Instance<engine::SlightEngine>>("slight", version!(), revision!(), None, None);
}
