use anyhow::{Context, Result};
use log::{error, info};
use std::path::Path;
use tokio::runtime::Runtime;

use containerd_shim_wasm::{libcontainer_instance::LinuxContainerExecutor, sandbox::Stdio};
use libcontainer::workload::{Executor, ExecutorError, ExecutorValidationError};
use oci_spec::runtime::Spec;
use utils::is_linux_executable;
use wasm_workers_server::{
    wws_config::Config,
    wws_router::Routes,
    wws_server::{serve, Panel, ServeOptions},
};

/// URL to listen to in wws
const WWS_ADDR: &str = "0.0.0.0";
const WWS_PORT: u16 = 3000;

#[derive(Clone)]
pub struct WwsExecutor {
    pub stdio: Stdio,
}

impl WwsExecutor {
    pub fn new(stdio: Stdio) -> Self {
        Self { stdio }
    }

    fn wasm_exec(&self, _spec: &Spec) -> anyhow::Result<()> {
        let stderr = self.stdio.take().stderr;
        stderr.redirect().context("redirecting stdio")?;

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
        rt.block_on(self.wasm_exec_async(path, routes))
    }

    async fn wasm_exec_async(&self, root: &Path, routes: Routes) -> Result<()> {
        let server = serve(ServeOptions {
            root_path: root.to_path_buf(),
            base_routes: routes,
            hostname: WWS_ADDR.to_string(),
            port: WWS_PORT,
            panel: Panel::Disabled,
            cors_origins: None,
        }).await?;
        info!(" >>> notifying main thread we are about to start");
        Ok(server.await?)
    }
}

impl Executor for WwsExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        if is_linux_executable(spec).is_ok() {
            log::info!("executing linux container");
            LinuxContainerExecutor::new(self.stdio.clone()).exec(spec)
        } else {
            if let Err(err) = self.wasm_exec(spec) {
                log::info!(" >>> server shut down due to error: {err}");
                std::process::exit(137);
            }
            log::info!(" >>> server shut down: exiting");
            std::process::exit(0);
        }
    }

    fn validate(&self, _spec: &Spec) -> Result<(), ExecutorValidationError> {
        Ok(())
    }
}
