use anyhow::{Result, Context, ensure, bail};
use std::path::PathBuf;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use tokio::runtime::Runtime;
use std::os::unix::prelude::PermissionsExt;

use containerd_shim_wasm::sandbox::{oci, Stdio};
use containerd_shim_wasm::libcontainer_instance::LinuxContainerExecutor;
use libcontainer::workload::{Executor, ExecutorError, ExecutorValidationError};
use oci_spec::runtime::Spec;
use lunatic_process::{
    env::{Environments, LunaticEnvironments},
    runtimes,
};

use crate::common::{run_wasm, RunWasm};

#[derive(Clone)]
pub struct LunaticExecutor {
    stdio: Stdio
}

impl LunaticExecutor {
    pub fn new(stdio: Stdio) -> Self {
        Self { stdio }
    }
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
        if is_linux_executable(spec).is_ok() {
            log::info!("executing linux container");
            LinuxContainerExecutor::new(self.stdio.clone()).exec(spec)?;
            Ok(())
        } else {
            self.stdio.take().redirect().unwrap();
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

fn is_linux_executable(spec: &Spec) -> anyhow::Result<()> {
    let args = oci::get_args(spec).to_vec();

    if args.is_empty() {
        bail!("no args provided");
    }

    let executable = args.first().context("no executable provided")?;
    ensure!(!executable.is_empty(), "executable is empty");
    let cwd = std::env::current_dir()?;

    let executable = if executable.contains('/') {
        let path = cwd.join(executable);
        ensure!(path.is_file(), "file not found");
        path
    } else {
        spec.process()
            .as_ref()
            .and_then(|p| p.env().clone())
            .unwrap_or_default()
            .into_iter()
            .map(|v| match v.split_once('=') {
                None => (v, "".to_string()),
                Some((k, v)) => (k.to_string(), v.to_string()),
            })
            .find(|(key, _)| key == "PATH")
            .context("PATH not defined")?
            .1
            .split(':')
            .map(|p| cwd.join(p).join(executable))
            .find(|p| p.is_file())
            .context("file not found")?
    };
    
    let mode = executable.metadata()?.permissions().mode();
    ensure!(mode & 0o001 != 0, "entrypoint is not a executable");

    // check the shebang and ELF magic number
    // https://en.wikipedia.org/wiki/Executable_and_Linkable_Format#File_header
    let mut buffer = [0; 4];
    File::open(&executable)?.read_exact(&mut buffer)?;

    match buffer {
        [0x7f, 0x45, 0x4c, 0x46] => Ok(()), // ELF magic number
        [0x23, 0x21, ..] => Ok(()),         // shebang
        _ => bail!("{executable:?} is not a valid script or elf file"),
    }
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
