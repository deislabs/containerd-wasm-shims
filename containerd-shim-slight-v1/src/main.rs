use std::option::Option;
use std::path::PathBuf;

use containerd_shim as shim;
use containerd_shim_wasm::libcontainer_instance::LibcontainerInstance;
use containerd_shim_wasm::sandbox::instance::ExitCode;
use containerd_shim_wasm::sandbox::instance_utils::determine_rootdir;
use containerd_shim_wasm::sandbox::stdio::Stdio;
use containerd_shim_wasm::sandbox::{error::Error, InstanceConfig, ShimCli};
use executor::SlightExecutor;
use libcontainer::container::builder::ContainerBuilder;
use libcontainer::container::Container;
use libcontainer::syscall::syscall::SyscallType;

mod executor;

static DEFAULT_CONTAINER_ROOT_DIR: &str = "/run/containerd/slight";

pub struct Wasi {
    exit_code: ExitCode,
    id: String,
    stdio: Stdio,
    bundle: String,
    rootdir: PathBuf,
}

impl LibcontainerInstance for Wasi {
    type Engine = ();
    fn new_libcontainer(id: String, cfg: Option<&InstanceConfig<Self::Engine>>) -> Self {
        log::info!(">>> new instance");
        let cfg = cfg.unwrap();
        let bundle = cfg.get_bundle().unwrap_or_default();
        let rootdir = determine_rootdir(
            bundle.as_str(),
            cfg.get_namespace().as_str(),
            DEFAULT_CONTAINER_ROOT_DIR,
        )
        .unwrap();
        Wasi {
            exit_code: Default::default(),
            id,
            stdio: Stdio::init_from_cfg(cfg).expect("failed to open stdio"),
            bundle,
            rootdir,
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

    fn build_container(&self) -> std::result::Result<Container, Error> {
        let err_others = |err| Error::Others(format!("failed to create container: {}", err));
        let slight_executor = SlightExecutor::new(self.stdio.take());
        let container = ContainerBuilder::new(self.id.clone(), SyscallType::Linux)
            .with_executor(slight_executor)
            .with_root_path(self.rootdir.clone())
            .map_err(err_others)?
            .as_init(&self.bundle)
            .with_systemd(false)
            .with_detach(true)
            .build()
            .map_err(err_others)?;
        Ok(container)
    }
}

fn main() {
    shim::run::<ShimCli<Wasi>>("io.containerd.slight.v1", None);
}
