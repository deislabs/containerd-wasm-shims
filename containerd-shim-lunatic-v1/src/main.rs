use std::{
    fs::File,
    io::{ErrorKind, Read},
    os::fd::IntoRawFd,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
};

use chrono::{DateTime, Utc};
use containerd_shim::run;
use containerd_shim_wasm::{
    libcontainer_instance::LibcontainerInstance,
    sandbox::{instance_utils::maybe_open_stdio, EngineGetter, Error, InstanceConfig, ShimCli},
};
use libcontainer::{
    container::{builder::ContainerBuilder, Container},
    syscall::syscall::create_syscall,
};
use serde::{Deserialize, Serialize};

use anyhow::{Context, Result};

use crate::executor::LunaticExecutor;

type ExitCode = Arc<(Mutex<Option<(u32, DateTime<Utc>)>>, Condvar)>;

mod common;
mod executor;

static DEFAULT_CONTAINER_ROOT_DIR: &str = "/run/containerd/lunatic";

pub struct Wasi {
    id: String,
    exit_code: ExitCode,
    bundle: String,
    rootdir: PathBuf,
    stdin: String,
    stdout: String,
    stderr: String,
}

#[derive(Serialize, Deserialize)]
struct Options {
    root: Option<PathBuf>,
}

fn determine_rootdir<P: AsRef<Path>>(bundle: P, namespace: String) -> Result<PathBuf, Error> {
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
    Ok(options
        .root
        .unwrap_or(PathBuf::from(DEFAULT_CONTAINER_ROOT_DIR))
        .join(namespace))
}

impl LibcontainerInstance for Wasi {
    type E = ();

    fn new_libcontainer(id: String, cfg: Option<&InstanceConfig<Self::E>>) -> Self {
        let cfg = cfg.unwrap();
        let bundle = cfg.get_bundle().unwrap_or_default();

        Wasi {
            id,
            exit_code: Arc::new((Mutex::new(None), Condvar::new())),
            rootdir: determine_rootdir(bundle.as_str(), cfg.get_namespace()).unwrap(),
            bundle,
            stdin: cfg.get_stdin().unwrap_or_default(),
            stdout: cfg.get_stdout().unwrap_or_default(),
            stderr: cfg.get_stderr().unwrap_or_default(),
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

        let stdin = maybe_open_stdio(&self.stdin)
            .context("could not open stdin")?
            .map(|f| f.into_raw_fd());
        let stdout = maybe_open_stdio(&self.stdout)
            .context("could not open stdout")?
            .map(|f| f.into_raw_fd());
        let stderr = maybe_open_stdio(&self.stderr)
            .context("could not open stderr")?
            .map(|f| f.into_raw_fd());

        let syscall = create_syscall();
        let err_msg = |err| format!("failed to create container: {}", err);
        let container = ContainerBuilder::new(self.id.clone(), syscall.as_ref())
            .with_executor(vec![Box::new(LunaticExecutor {
                stdin,
                stdout,
                stderr,
            })])
            .map_err(|err| Error::Others(err_msg(err)))?
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

impl EngineGetter for Wasi {
    type E = ();

    fn new_engine() -> std::result::Result<Self::E, Error> {
        Ok(())
    }
}
fn main() {
    run::<ShimCli<Wasi, ()>>("io.containerd.lunatic.v1", None);
}
