use anyhow::{Context, Result};
use containerd_shim as shim;
use containerd_shim_wasm::libcontainer_instance::LibcontainerInstance;
use containerd_shim_wasm::libcontainer_instance::LinuxContainerExecutor;
use containerd_shim_wasm::sandbox::instance::ExitCode;
use containerd_shim_wasm::sandbox::instance_utils::maybe_open_stdio;
use containerd_shim_wasm::sandbox::{error::Error, InstanceConfig, ShimCli};
use executor::WwsExecutor;
use libcontainer::container::builder::ContainerBuilder;
use libcontainer::container::Container;
use libcontainer::syscall::syscall::create_syscall;
use serde::Deserialize;
use serde::Serialize;
use std::fs::File;
use std::io::ErrorKind;
use std::io::Read;
use std::option::Option;
use std::os::fd::IntoRawFd;
use std::path::Path;
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
    bundle: String,
    rootdir: PathBuf,
}
#[derive(Serialize, Deserialize)]
struct Options {
    root: Option<PathBuf>,
}

fn determine_rootdir<P: AsRef<Path>>(bundle: P, namespace: String) -> Result<PathBuf, Error> {
    log::info!(
        "determining rootdir for bundle: {}",
        bundle.as_ref().display()
    );
    let mut file = match File::open(bundle.as_ref().join("options.json")) {
        Ok(f) => f,
        Err(err) => match err.kind() {
            ErrorKind::NotFound => {
                return Ok(<&str as Into<PathBuf>>::into(DEFAULT_CONTAINER_ROOT_DIR).join(namespace))
            }
            _ => return Err(err.into()),
        },
    };
    let mut data = String::new();
    file.read_to_string(&mut data)?;
    let options: Options = serde_json::from_str(&data)?;
    let path = options
        .root
        .unwrap_or(PathBuf::from(DEFAULT_CONTAINER_ROOT_DIR))
        .join(namespace);
    log::info!("youki root path is: {}", path.display());
    Ok(path)
}

/// Implement the "default" interface from runwasi
impl LibcontainerInstance for Workers {
    type Engine = ();
    fn new_libcontainer(id: String, cfg: Option<&InstanceConfig<Self::Engine>>) -> Self {
        log::info!("[wws] new instance");
        let cfg = cfg.unwrap();
        let bundle = cfg.get_bundle().unwrap_or_default();
        let rootdir = determine_rootdir(bundle.as_str(), cfg.get_namespace()).unwrap();
        Workers {
            exit_code: Default::default(),
            id,
            // TODO: set the stdio to redirect the logs to the pod. Currently, we only set the
            // stderr as Wasm Workers use stdin/stdout to pass and receive data. This behavior
            // will change in the future.
            // stdin: cfg.get_stdin().unwrap_or_default(),
            // stdout: cfg.get_stdout().unwrap_or_default(),
            stderr: cfg.get_stderr().unwrap_or_default(),
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
        let syscall = create_syscall();
        let stderr = maybe_open_stdio(&self.stderr)
            .context("could not open stderr")?
            .map(|f| f.into_raw_fd());
        let err_others = |err| Error::Others(format!("failed to create container: {}", err));
        let spin_executor = Box::new(WwsExecutor { stderr });
        let default_executor = Box::<LinuxContainerExecutor>::default();

        let container = ContainerBuilder::new(self.id.clone(), syscall.as_ref())
            .with_executor(vec![default_executor, spin_executor])
            .map_err(err_others)?
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
