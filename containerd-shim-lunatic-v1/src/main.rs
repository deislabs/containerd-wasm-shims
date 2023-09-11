use std::{
    env,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
};

use containerd_shim::{parse, run};
use containerd_shim_wasm::sandbox::instance_utils::determine_rootdir;
use containerd_shim_wasm::sandbox::stdio::Stdio;
use containerd_shim_wasm::{
    libcontainer_instance::LibcontainerInstance,
    sandbox::{instance::ExitCode, Error, InstanceConfig, ShimCli},
};
use libcontainer::container::{builder::ContainerBuilder, Container};
use libcontainer::syscall::syscall::SyscallType;

use anyhow::Result;

use crate::executor::LunaticExecutor;

mod common;
mod executor;

static DEFAULT_CONTAINER_ROOT_DIR: &str = "/run/containerd/lunatic";

pub struct Wasi {
    id: String,
    exit_code: ExitCode,
    bundle: String,
    rootdir: PathBuf,
    stdio: Stdio,
}

impl LibcontainerInstance for Wasi {
    type Engine = ();

    fn new_libcontainer(id: String, cfg: Option<&InstanceConfig<Self::Engine>>) -> Self {
        let cfg = cfg.unwrap();
        let bundle = cfg.get_bundle().unwrap_or_default();

        Wasi {
            id,
            exit_code: Arc::new((Mutex::new(None), Condvar::new())),
            rootdir: determine_rootdir(
                bundle.as_str(),
                cfg.get_namespace().as_str(),
                DEFAULT_CONTAINER_ROOT_DIR,
            )
            .unwrap(),
            bundle,
            stdio: Stdio::init_from_cfg(cfg).expect("failed to open stdio"),
        }
    }

    fn get_exit_code(&self) -> ExitCode {
        self.exit_code.clone()
    }

    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn get_root_dir(&self) -> std::result::Result<PathBuf, Error> {
        Ok(self.rootdir.clone())
    }

    fn build_container(&self) -> Result<Container, Error> {
        log::info!("Building container");

        let err_msg = |err| format!("failed to create container: {}", err);
        let container = ContainerBuilder::new(self.id.clone(), SyscallType::Linux)
            .with_executor(LunaticExecutor::new(self.stdio.take()))
            .with_root_path(self.rootdir.clone())
            .map_err(|err| Error::Others(err_msg(err)))?
            .as_init(&self.bundle)
            .with_systemd(false)
            .build()
            .map_err(|err| Error::Others(err_msg(err)))?;
        log::info!(">>> Container built.");
        Ok(container)
    }
}

fn parse_version() {
    let os_args: Vec<_> = env::args_os().collect();
    let flags = parse(&os_args[1..]).unwrap();
    if flags.version {
        println!("{}:", os_args[0].to_string_lossy());
        println!("  Version: {}", env!("CARGO_PKG_VERSION"));
        println!("  Revision: {}", env!("CARGO_GIT_HASH"));
        println!();

        std::process::exit(0);
    }
}

fn main() {
    parse_version();
    run::<ShimCli<Wasi>>("io.containerd.lunatic.v1", None);
}
