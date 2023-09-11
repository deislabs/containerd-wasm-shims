use anyhow::{Context, Result};
use log::{error, info};
use std::path::Path;
use tokio::runtime::Runtime;

use containerd_shim_wasm::{
    container::{Engine, RuntimeContext},
    sandbox::Stdio,
};
use wasm_workers_server::{
    wws_config::Config,
    wws_router::Routes,
    wws_server::{serve, Panel, ServeOptions},
};

/// URL to listen to in wws
const WWS_ADDR: &str = "0.0.0.0";
const WWS_PORT: u16 = 3000;

#[derive(Clone, Default)]
pub struct WwsEngine;

impl WwsEngine {
    async fn wasm_exec_async(&self, root: &Path, routes: Routes) -> Result<()> {
        let server = serve(ServeOptions {
            root_path: root.to_path_buf(),
            base_routes: routes,
            hostname: WWS_ADDR.to_string(),
            port: WWS_PORT,
            panel: Panel::Disabled,
            cors_origins: None,
        })
        .await?;
        info!(" >>> notifying main thread we are about to start");
        Ok(server.await?)
    }
}

impl Engine for WwsEngine {
    fn name() -> &'static str {
        "wws"
    }

    fn run_wasi(&self, _ctx: &impl RuntimeContext, stdio: Stdio) -> Result<i32> {
        log::info!("setting up wasi");
        stdio.redirect()?;
        let path = Path::new("/");

        let config = Config::load(path).unwrap_or_else(|err| {
            error!("[wws] Error reading .wws.toml file. It will be ignored");
            error!("[wws] Error: {err}");
            Config::default()
        });

        // Check if there're missing runtimes
        if config.is_missing_any_runtime(path) {
            error!("[wws] Required language runtimes are not installed. Some files may not be considered workers");
            error!("[wws] You can install the missing runtimes with: wws runtimes install");
        }

        let routes = Routes::new(path, "", Vec::new(), &config);

        let rt = Runtime::new().context("failed to create runtime")?;

        if let Err(e) = rt.block_on(self.wasm_exec_async(path, routes)) {
            log::error!(" >>> error: {:?}", e);
            return Ok(137);
        }
        Ok(0)
    }
}
