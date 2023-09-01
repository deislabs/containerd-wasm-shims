use anyhow::{Result, Context, ensure, bail};
use log::{error, info};
use nix::unistd::{dup, dup2};
use std::{os::{fd::RawFd, unix::prelude::PermissionsExt}, path::PathBuf, fs::File, io::Read};
use tokio::runtime::Runtime;

use containerd_shim_wasm::{sandbox::{oci, Stdio}, libcontainer_instance::LinuxContainerExecutor};
use libc::STDERR_FILENO;
use libcontainer::workload::{Executor, ExecutorError, ExecutorValidationError};
use oci_spec::runtime::Spec;
use wws_config::Config;
use wws_router::Routes;
use wws_server::serve;

/// URL to listen to in wws
const WWS_ADDR: &str = "0.0.0.0";
const WWS_PORT: u16 = 3000;
const EXECUTOR_NAME: &str = "wws";

#[derive(Clone)]
pub struct WwsExecutor {
    pub stderr: Option<RawFd>,
    pub stdio: Stdio,
}

impl WwsExecutor {
    pub fn new(stdio: Stdio, stderr: Option<RawFd>) -> Self {
        Self { stdio, stderr }
    }
}

impl Executor for WwsExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        if is_linux_executable(spec).is_ok() {
            log::info!("executing linux container");
            LinuxContainerExecutor::new(self.stdio.clone()).exec(spec)?;
            Ok(())
        } else {
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
    }

    fn validate(&self, _spec: &Spec) -> Result<(), ExecutorValidationError> { 
        Ok(())
    }
}

fn prepare_stdio(stderr: Option<RawFd>) -> Result<()> {
    if let Some(stderr) = stderr {
        dup(STDERR_FILENO)?;
        dup2(stderr, STDERR_FILENO)?;
    }
    Ok(())
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