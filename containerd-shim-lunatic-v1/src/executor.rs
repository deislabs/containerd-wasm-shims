use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;

use containerd_shim_wasm::libcontainer_instance::LinuxContainerExecutor;
use containerd_shim_wasm::sandbox::Stdio;
use libcontainer::workload::{Executor, ExecutorError, ExecutorValidationError};
use lunatic_process::{
    env::{Environments, LunaticEnvironments},
    runtimes,
};
use oci_spec::runtime::Spec;
use utils::{get_args, is_linux_executable};

use crate::common::{run_wasm, RunWasm};

#[derive(Clone)]
pub struct LunaticExecutor {
    stdio: Stdio,
}

impl LunaticExecutor {
    pub fn new(stdio: Stdio) -> Self {
        Self { stdio }
    }

    fn wasm_exec(&self, spec: &Spec) -> anyhow::Result<()> {
        self.stdio
            .take()
            .redirect()
            .context("failed to redirect stdio")?;
        let cmd = get_args(spec).first().context("no cmd provided")?.clone();
        let rt = Runtime::new().context("failed to create runtime")?;
        rt.block_on(async {
            log::info!(" >>> building lunatic application");
            crate::executor::exec(cmd).await
        })
    }
}

impl Executor for LunaticExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        if is_linux_executable(spec).is_ok() {
            log::info!("executing linux container");
            LinuxContainerExecutor::new(self.stdio.clone()).exec(spec)
        } else {
            if let Err(e) = self.wasm_exec(spec) {
                log::error!(" >>> error: {:?}", e);
                std::process::exit(137);
            }
            std::process::exit(0);
        }
    }

    fn validate(&self, _spec: &Spec) -> Result<(), ExecutorValidationError> {
        Ok(())
    }
}

pub async fn exec(cmd: String) -> Result<()> {
    // Create wasmtime runtime
    let wasmtime_config = runtimes::wasmtime::default_config();
    let runtime = runtimes::wasmtime::WasmtimeRuntime::new(&wasmtime_config)?;
    let envs = Arc::new(LunaticEnvironments::default());

    let env = envs.create(1).await;
    run_wasm(RunWasm {
        path: PathBuf::from(cmd),
        wasm_args: vec![],
        dir: vec![],
        runtime,
        envs,
        env,
        distributed: None,
    })
    .await
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test() {
        if let Err(error) = crate::executor::exec(
            "../images/lunatic/target/wasm32-wasi/release/wasi-hello-world.wasm".to_string(),
        )
        .await
        {
            panic!("Problem opening the file: {:?}", error)
        }
    }
}
