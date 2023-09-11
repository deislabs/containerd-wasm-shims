use anyhow::{Context, Result};
use containerd_shim_wasm::container::RuntimeContext;
use std::path::PathBuf;
use tokio::runtime::Runtime;

use containerd_shim_wasm::container::Engine;
use containerd_shim_wasm::sandbox::Stdio;
use slight_lib::commands::run::{handle_run, RunArgs};

#[derive(Clone, Default)]
pub struct SlightEngine;

impl Engine for SlightEngine {
    fn name() -> &'static str {
        "slight"
    }

    fn run_wasi(&self, _ctx: &impl RuntimeContext, stdio: Stdio) -> Result<i32> {
        log::info!("setting up wasi");
        stdio.redirect()?;
        let mod_path = PathBuf::from("/slightfile.toml");
        let wasm_path = PathBuf::from("/app.wasm");
        let rt = Runtime::new().context("failed to create runtime")?;
        let args = RunArgs {
            module: wasm_path,
            slightfile: mod_path,
            io_redirects: None,
            link_all_capabilities: true,
        };

        if let Err(err) = rt.block_on(handle_run(args)) {
            log::error!(" >>> error: {:?}", err);
            return Ok(137);
        }
        Ok(0)
    }
}
