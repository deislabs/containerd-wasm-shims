use anyhow::Result;
use log::{error, info};
use nix::unistd::{dup, dup2};
use std::{os::fd::RawFd, path::PathBuf};
use tokio::runtime::Runtime;

use containerd_shim_wasm::sandbox::oci;
use libc::STDERR_FILENO;
use libcontainer::workload::{Executor, ExecutorError};
use oci_spec::runtime::Spec;
use wws_config::Config;
use wws_router::Routes;
use wws_server::serve;

/// URL to listen to in wws
const WWS_ADDR: &str = "0.0.0.0";
const WWS_PORT: u16 = 3000;
const EXECUTOR_NAME: &str = "wws";

pub struct WwsExecutor {
    pub stderr: Option<RawFd>,
}

impl WwsExecutor {}

impl Executor for WwsExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        let args = oci::get_args(spec);
        if args.is_empty() {
            return Err(ExecutorError::InvalidArg);
        }

        prepare_stdio(self.stderr).map_err(|err| {
            ExecutorError::Other(format!("failed to prepare stdio for container: {}", err))
        })?;

        let path = PathBuf::from("/");

        let config = match Config::load(&path) {
            Ok(c) => c,
            Err(err) => {
                error!("[wws] There was an error reading the .wws.toml file. It will be ignored");
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

    fn can_handle(&self, _spec: &Spec) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        EXECUTOR_NAME
    }
}

fn prepare_stdio(stderr: Option<RawFd>) -> Result<()> {
    if let Some(stderr) = stderr {
        dup(STDERR_FILENO)?;
        dup2(stderr, STDERR_FILENO)?;
    }
    Ok(())
}
