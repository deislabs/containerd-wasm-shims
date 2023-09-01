use anyhow::{bail, ensure, Context, Result};
use log::info;
use std::fs::File;
use std::io::Read;
use std::os::unix::prelude::PermissionsExt;
use std::path::PathBuf;
use tokio::runtime::Runtime;

use containerd_shim_wasm::libcontainer_instance::LinuxContainerExecutor;
use containerd_shim_wasm::sandbox::{oci, Stdio};
use libcontainer::workload::{Executor, ExecutorError, ExecutorValidationError};
use oci_spec::runtime::Spec;
use slight_lib::commands::run::{handle_run, RunArgs};

// const EXECUTOR_NAME: &str = "slight";

#[derive(Clone)]
pub struct SlightExecutor {
    stdio: Stdio,
}

impl SlightExecutor {
    pub fn new(stdio: Stdio) -> Self {
        Self { stdio }
    }
}

impl Executor for SlightExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        if is_linux_executable(spec).is_ok() {
            log::info!("executing linux container");
            LinuxContainerExecutor::new(self.stdio.clone()).exec(spec)?;
            Ok(())
        } else {
            log::info!("executing slight container");
            let args = oci::get_args(spec);
            if args.is_empty() {
                return Err(ExecutorError::InvalidArg);
            }

            let mod_path = PathBuf::from("/slightfile.toml");
            let wasm_path = PathBuf::from("/app.wasm");

            self.stdio.take().redirect().unwrap();

            let rt = Runtime::new().unwrap();
            let args = RunArgs {
                module: wasm_path,
                slightfile: PathBuf::from(&mod_path),
                io_redirects: None,
                link_all_capabilities: true,
            };
            rt.block_on(async {
                let f = handle_run(args);
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
