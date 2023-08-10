use std::{os::fd::RawFd, path::PathBuf, sync::Arc};

use containerd_shim_wasm::sandbox::oci::Spec;
use libc::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
use libcontainer::workload::{Executor, ExecutorError};
use lunatic_process::{
    env::{Environments, LunaticEnvironments},
    runtimes,
};
use nix::unistd::{dup, dup2};

use anyhow::Result;
use tokio::runtime::Runtime;

use crate::common::{run_wasm, RunWasm};

#[derive(Clone)]
pub struct LunaticExecutor {
    pub stdin: Option<RawFd>,
    pub stdout: Option<RawFd>,
    pub stderr: Option<RawFd>,
}

fn prepare_stdio(stdin: Option<RawFd>, stdout: Option<RawFd>, stderr: Option<RawFd>) -> Result<()> {
    if let Some(stdin) = stdin {
        dup(STDIN_FILENO)?;
        dup2(stdin, STDIN_FILENO)?;
    }
    if let Some(stdout) = stdout {
        dup(STDOUT_FILENO)?;
        dup2(stdout, STDOUT_FILENO)?;
    }
    if let Some(stderr) = stderr {
        dup(STDERR_FILENO)?;
        dup2(stderr, STDERR_FILENO)?;
    }
    Ok(())
}

fn get_args(spec: &Spec) -> &[String] {
    let p = match spec.process() {
        None => return &[],
        Some(p) => p,
    };

    match p.args() {
        None => &[],
        Some(args) => args.as_slice(),
    }
}

impl Executor for LunaticExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        prepare_stdio(self.stdin, self.stdout, self.stderr).map_err(|err| {
            ExecutorError::Other(format!("failed to prepare stdio for container: {}", err))
        })?;

        let args = get_args(spec);
        let cmd = args[0].clone();

        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            log::info!(" >>> building lunatic application");

            match crate::executor::exec(cmd).await {
                Err(error) => log::error!(" >>> error: {:?}", error),
                Ok(_) => std::process::exit(0),
            }
        });
        std::process::exit(137);
    }

    fn can_handle(&self, _spec: &Spec) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "lunatic"
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
