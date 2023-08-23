use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::option::Option;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};

use anyhow::Context;
use anyhow::{anyhow, Result};
use containerd_shim as shim;
use containerd_shim_wasm::libcontainer_instance::LibcontainerInstance;
use containerd_shim_wasm::libcontainer_instance::LinuxContainerExecutor;
use containerd_shim_wasm::sandbox::instance::ExitCode;
use containerd_shim_wasm::sandbox::instance_utils::{determine_rootdir, maybe_open_stdio};
use containerd_shim_wasm::sandbox::{error::Error, InstanceConfig, ShimCli};
use executor::SpinExecutor;
use libcontainer::container::builder::ContainerBuilder;
use libcontainer::container::Container;
use libcontainer::syscall::syscall::create_syscall;
use std::os::fd::IntoRawFd;

mod executor;

const SPIN_ADDR: &str = "0.0.0.0:80";
static DEFAULT_CONTAINER_ROOT_DIR: &str = "/run/containerd/spin";

pub struct Wasi {
    exit_code: ExitCode,
    id: String,
    stdin: String,
    stdout: String,
    stderr: String,
    bundle: String,
    rootdir: PathBuf,
}

impl LibcontainerInstance for Wasi {
    type Engine = ();

    fn new_libcontainer(id: String, cfg: Option<&InstanceConfig<Self::Engine>>) -> Self {
        let cfg = cfg.unwrap();
        let bundle = cfg.get_bundle().unwrap_or_default();
        let rootdir = determine_rootdir(
            bundle.as_str(),
            cfg.get_namespace().as_str(),
            DEFAULT_CONTAINER_ROOT_DIR,
        )
        .unwrap();
        Wasi {
            exit_code: Arc::new((Mutex::new(None), Condvar::new())),
            id,
            stdin: cfg.get_stdin().unwrap_or_default(),
            stdout: cfg.get_stdout().unwrap_or_default(),
            stderr: cfg.get_stderr().unwrap_or_default(),
            bundle: cfg.get_bundle().unwrap_or_default(),
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
        let stdin = maybe_open_stdio(&self.stdin)
            .context("could not open stdin")?
            .map(|f| f.into_raw_fd());
        let stdout = maybe_open_stdio(&self.stdout)
            .context("could not open stdout")?
            .map(|f| f.into_raw_fd());
        let stderr = maybe_open_stdio(&self.stderr)
            .context("could not open stderr")?
            .map(|f| f.into_raw_fd());
        let err_others = |err| Error::Others(format!("failed to create container: {}", err));
        let spin_executor = Box::new(SpinExecutor {
            stdin,
            stdout,
            stderr,
        });
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

fn parse_addr(addr: &str) -> Result<SocketAddr> {
    let addrs: SocketAddr = addr
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| anyhow!("could not parse address: {}", addr))?;
    Ok(addrs)
}

fn main() {
    shim::run::<ShimCli<Wasi>>("io.containerd.spin.v1", None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_parse_spin_address() {
        let parsed = parse_addr(SPIN_ADDR).unwrap();
        assert_eq!(parsed.clone().port(), 80);
        assert_eq!(parsed.ip().to_string(), "0.0.0.0");
    }
}
