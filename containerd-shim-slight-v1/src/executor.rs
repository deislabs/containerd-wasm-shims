use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::runtime::Runtime;

use containerd_shim_wasm::libcontainer_instance::LinuxContainerExecutor;
use containerd_shim_wasm::sandbox::Stdio;
use libcontainer::workload::{Executor, ExecutorError, ExecutorValidationError};
use oci_spec::runtime::Spec;
use slight_lib::commands::run::{handle_run, RunArgs};
use utils::is_linux_executable;

#[derive(Clone)]
pub struct SlightExecutor {
    stdio: Stdio,
}

impl SlightExecutor {
    pub fn new(stdio: Stdio) -> Self {
        Self { stdio }
    }

    fn wasm_exec(&self) -> anyhow::Result<()> {
        self.stdio
            .take()
            .redirect()
            .context("failed to redirect stdio")?;
        let mod_path = PathBuf::from("/slightfile.toml");
        let wasm_path = PathBuf::from("/app.wasm");
        let rt = Runtime::new().context("failed to create runtime")?;
        let args = RunArgs {
            module: wasm_path,
            slightfile: mod_path,
            io_redirects: None,
            link_all_capabilities: true,
        };
        rt.block_on(handle_run(args))
    }
}

impl Executor for SlightExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        if is_linux_executable(spec).is_ok() {
            log::info!("executing linux container");
            LinuxContainerExecutor::new(self.stdio.clone()).exec(spec)
        } else {
            if let Err(err) = self.wasm_exec() {
                log::error!(" >>> error: {:?}", err);
                std::process::exit(137);
            }
            log::info!(" >>> slight shut down: exiting");
            std::process::exit(0);
        }
    }

    fn validate(&self, _spec: &Spec) -> Result<(), ExecutorValidationError> {
        Ok(())
    }
}
