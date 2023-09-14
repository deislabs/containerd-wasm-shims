use std::path::Path;

use anyhow::Result;
use containerd_shim_wasm::container::{RuntimeContext, Stdio};
use wasm_workers_server::wws_config::Config;
use wasm_workers_server::wws_router::Routes;
use wasm_workers_server::wws_server::{serve, Panel, ServeOptions};

const WWS_ADDR: &str = "0.0.0.0";
const WWS_PORT: u16 = 3000;

#[containerd_shim_wasm::validate]
fn validate(_ctx: &impl RuntimeContext) -> bool {
    true
}

#[containerd_shim_wasm::main("Wws")]
async fn main(_ctx: &impl RuntimeContext, stdio: Stdio) -> Result<()> {
    log::info!("setting up wasi");
    stdio.redirect()?;
    let path = Path::new("/");

    let config = Config::load(path).unwrap_or_else(|err| {
        log::error!("[wws] Error reading .wws.toml file. It will be ignored");
        log::error!("[wws] Error: {err}");
        Config::default()
    });

    // Check if there're missing runtimes
    if config.is_missing_any_runtime(path) {
        log::error!("[wws] Required language runtimes are not installed. Some files may not be considered workers");
        log::error!("[wws] You can install the missing runtimes with: wws runtimes install");
    }

    let routes = Routes::new(path, "", vec![], &config);

    let options = ServeOptions {
        root_path: path.to_path_buf(),
        base_routes: routes,
        hostname: WWS_ADDR.to_string(),
        port: WWS_PORT,
        panel: Panel::Disabled,
        cors_origins: None,
    };

    serve(options).await?.await?;

    Ok(())
}
