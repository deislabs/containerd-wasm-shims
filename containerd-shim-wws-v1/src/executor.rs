use anyhow::Result;
use log::{error, info};
use std::path::PathBuf;
use tokio::runtime::Runtime;

use containerd_shim_wasm::{
    libcontainer_instance::LinuxContainerExecutor,
    sandbox::Stdio,
};
use libcontainer::workload::{Executor, ExecutorError, ExecutorValidationError};
use oci_spec::runtime::Spec;
use utils::is_linux_executable;
use wws_config::Config;
use wws_router::Routes;
use wws_server::serve;

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
}

impl Executor for WwsExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        if is_linux_executable(spec).is_ok() {
            log::info!("executing linux container");
            LinuxContainerExecutor::new(self.stdio.clone()).exec(spec)
        } else {
            self.stdio.take().stderr.redirect().map_err(|err| {
                error!(" >>> error: {:?}", err);
                ExecutorError::Other(format!("failed to redirect stderr: {:?}", err))
            })?;
            let path = PathBuf::from("/");

            let config = match Config::load(&path) {
                Ok(c) => c,
                Err(err) => {
                    error!(
                        "[wws] There was an error reading the .wws.toml file. It will be ignored"
                    );
                    error!("[wws] Error: {err}");

                    Config::default()
                }
            };

            // Check if there're missing runtimes
            if config.is_missing_any_runtime(&path) {
                error!("[wws] Required language runtimes are not installed. Some files may not be considered workers");
                error!("[wws] You can install the missing runtimes with: wws runtimes install");
            }

            let routes = Routes::new(&path, "", Vec::new(), &config);

            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let f = serve(&path, routes, WWS_ADDR, WWS_PORT, false, None)
                    .await
                    .unwrap();
                info!(" >>> notifying main thread we are about to start");
                tokio::select! {
                    _ = f => {
                        log::info!(" >>> server shut down: exiting");
                        std::process::exit(0);
                    },
                };
            });
            std::process::exit(137);
        }
    }

    fn validate(&self, _spec: &Spec) -> Result<(), ExecutorValidationError> {
        Ok(())
    }
}
