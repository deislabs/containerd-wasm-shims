use containerd_shim as shim;
use containerd_shim_wasm::libcontainer_instance::LibcontainerInstance;
use containerd_shim_wasm::sandbox::instance::ExitCode;
use containerd_shim_wasm::sandbox::instance_utils::determine_rootdir;
use containerd_shim_wasm::sandbox::Stdio;
use containerd_shim_wasm::sandbox::{error::Error, InstanceConfig, ShimCli};
use executor::WwsExecutor;
use libcontainer::container::builder::ContainerBuilder;
use libcontainer::container::Container;
use libcontainer::syscall::syscall::SyscallType;
use std::env;
use std::option::Option;
use std::path::PathBuf;

mod executor;

static DEFAULT_CONTAINER_ROOT_DIR: &str = "/run/containerd/wws";

pub struct Workers {
    exit_code: ExitCode,
    id: String,
    stdio: Stdio,
    bundle: String,
    rootdir: PathBuf,
}

/// Implement the "default" interface from runwasi
impl LibcontainerInstance for Workers {
    type Engine = ();
    fn new_libcontainer(id: String, cfg: Option<&InstanceConfig<Self::Engine>>) -> Self {
        log::info!("[wws] new instance");
        let cfg = cfg.unwrap();
        let bundle = cfg.get_bundle().unwrap_or_default();
        let rootdir = determine_rootdir(
            bundle.as_str(),
            cfg.get_namespace().as_str(),
            DEFAULT_CONTAINER_ROOT_DIR,
        )
        .unwrap();
        Workers {
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
        let wws_executor = WwsExecutor::new(self.stdio.take());

        let container = ContainerBuilder::new(self.id.clone(), SyscallType::Linux)
            .with_executor(wws_executor)
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

fn parse_version() {
    let os_args: Vec<_> = env::args_os().collect();
    let flags = shim::parse(&os_args[1..]).unwrap();
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
    shim::run::<ShimCli<Workers>>("io.containerd.wws.v1", None);
}
