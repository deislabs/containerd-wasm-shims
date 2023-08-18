use anyhow::Result;
use log::info;
use nix::unistd::{dup, dup2};
use std::{os::fd::RawFd, path::PathBuf};
use tokio::runtime::Runtime;

use containerd_shim_wasm::sandbox::oci;
use libc::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
use libcontainer::workload::{Executor, ExecutorError};
use oci_spec::runtime::Spec;
use slight_lib::commands::run::{handle_run, RunArgs};

const EXECUTOR_NAME: &str = "slight";

pub struct SlightExecutor {
    pub stdin: Option<RawFd>,
    pub stdout: Option<RawFd>,
    pub stderr: Option<RawFd>,
}

impl SlightExecutor {}

impl Executor for SlightExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        let args = oci::get_args(spec);
        if args.is_empty() {
            return Err(ExecutorError::InvalidArg);
        }

        let mod_path = PathBuf::from("/slightfile.toml");
        let wasm_path = PathBuf::from("/app.wasm");

        prepare_stdio(self.stdin, self.stdout, self.stderr).map_err(|err| {
            ExecutorError::Other(format!("failed to prepare stdio for container: {}", err))
        })?;

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

    fn can_handle(&self, _spec: &Spec) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        EXECUTOR_NAME
    }
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
