use anyhow::Context;
use containerd_shim as shim;
use containerd_shim_wasm::libcontainer_instance::LibcontainerInstance;
use containerd_shim_wasm::sandbox::instance::ExitCode;
use containerd_shim_wasm::sandbox::instance_utils::{determine_rootdir, maybe_open_stdio};
use containerd_shim_wasm::sandbox::Stdio;
use containerd_shim_wasm::sandbox::{error::Error, InstanceConfig, ShimCli};
use executor::WwsExecutor;
use libcontainer::container::builder::ContainerBuilder;
use libcontainer::container::Container;
use libcontainer::syscall::syscall::SyscallType;
use std::option::Option;
use std::os::fd::IntoRawFd;
use std::path::PathBuf;

mod executor;

static DEFAULT_CONTAINER_ROOT_DIR: &str = "/run/containerd/wws";

pub struct Workers {
    exit_code: ExitCode,
    id: String,
    // TODO: set the stdio to redirect the logs to the pod. Currently, we only set the
    // stderr as Wasm Workers use stdin/stdout to pass and receive data. This behavior
    // will change in the future.
    // stdin: String,
    // stdout: String,
    stderr: String,
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
            // TODO: set the stdio to redirect the logs to the pod. Currently, we only set the
            // stderr as Wasm Workers use stdin/stdout to pass and receive data. This behavior
            // will change in the future.
            // stdin: cfg.get_stdin().unwrap_or_default(),
            // stdout: cfg.get_stdout().unwrap_or_default(),
            stderr: cfg.get_stderr().unwrap_or_default(),
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
        let stderr = maybe_open_stdio(&self.stderr)
            .context("could not open stderr")?
            .map(|f| f.into_raw_fd());
        let err_others = |err| Error::Others(format!("failed to create container: {}", err));
        let wws_executor = WwsExecutor::new(self.stdio.take(), stderr);

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

fn main() {
    shim::run::<ShimCli<Workers>>("io.containerd.wws.v1", None);
}
