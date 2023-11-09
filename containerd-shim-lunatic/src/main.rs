use containerd_shim_wasm::container::Instance;
use containerd_shim_wasm::sandbox::cli::{revision, shim_main, version};

mod common;
mod engine;

fn main() {
    shim_main::<Instance<engine::LunaticEngine>>("lunatic", version!(), revision!(), "v1", None);
}
