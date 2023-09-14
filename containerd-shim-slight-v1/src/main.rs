use std::path::Path;

use anyhow::Result;
use containerd_shim_wasm::container::{RuntimeContext, Stdio};
use slight_lib::commands::run::{handle_run, RunArgs};

const SLIGHT_FILE: &str = "/slightfile.toml";
const MODULE_FILE: &str = "/app.wasm";

#[containerd_shim_wasm::validate]
fn validate(_ctx: &impl RuntimeContext) -> bool {
    Path::new(SLIGHT_FILE).exists() && Path::new(MODULE_FILE).exists()
}

#[containerd_shim_wasm::main("Slight")]
async fn main(_ctx: &impl RuntimeContext, stdio: Stdio) -> Result<()> {
    log::info!("setting up wasi");

    stdio.redirect()?;

    handle_run(RunArgs {
        module: MODULE_FILE.into(),
        slightfile: SLIGHT_FILE.into(),
        io_redirects: None,
        link_all_capabilities: true,
    })
    .await
}
