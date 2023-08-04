use std::fs::File;
use std::io::ErrorKind;
use std::io::Read;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::option::Option;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use anyhow::Context;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use containerd_shim as shim;
use containerd_shim_wasm::sandbox::instance::Wait;
use containerd_shim_wasm::sandbox::instance_utils::get_instance_root;
use containerd_shim_wasm::sandbox::instance_utils::instance_exists;
use containerd_shim_wasm::sandbox::instance_utils::maybe_open_stdio;
use containerd_shim_wasm::sandbox::{
    error::Error, EngineGetter, Instance, InstanceConfig, ShimCli,
};
use executor::SpinExecutor;
use libc::{SIGINT, SIGKILL};
use libcontainer::container::builder::ContainerBuilder;
use libcontainer::container::Container;
use libcontainer::container::ContainerStatus;
use libcontainer::signal::Signal;
use libcontainer::syscall::syscall::create_syscall;
use linux_executor::LinuxContainerExecutor;
use log::error;
use nix::errno::Errno;
use nix::sys::wait::{waitid, Id as WaitID, WaitPidFlag, WaitStatus};
use serde::Deserialize;
use serde::Serialize;

mod executor;
mod linux_executor;

const SPIN_ADDR: &str = "0.0.0.0:80";
static DEFAULT_CONTAINER_ROOT_DIR: &str = "/run/containerd/spin";

type ExitCode = Arc<(Mutex<Option<(u32, DateTime<Utc>)>>, Condvar)>;

pub struct Wasi {
    exit_code: ExitCode,
    id: String,
    stdin: String,
    stdout: String,
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

impl Wasi {
    fn build_container(
        &self,
        stdin: &str,
        stdout: &str,
        stderr: &str,
    ) -> anyhow::Result<Container> {
        let syscall = create_syscall();
        let stdin = maybe_open_stdio(stdin).context("could not open stdin")?;
        let stdout = maybe_open_stdio(stdout).context("could not open stdout")?;
        let stderr = maybe_open_stdio(stderr).context("could not open stderr")?;

        let spin_executor = Box::new(SpinExecutor {
            stdin,
            stdout,
            stderr,
        });
        let default_executor = Box::<LinuxContainerExecutor>::default();

        let container = ContainerBuilder::new(self.id.clone(), syscall.as_ref())
            .with_executor(vec![default_executor, spin_executor])?
            .with_root_path(self.rootdir.clone())?
            .as_init(&self.bundle)
            .with_systemd(false)
            .with_detach(true)
            .build()?;
        Ok(container)
    }
}

impl Instance for Wasi {
    type E = ();
    fn new(id: String, cfg: Option<&InstanceConfig<Self::E>>) -> Self {
        let cfg = cfg.unwrap();
        let bundle = cfg.get_bundle().unwrap_or_default();
        let rootdir = determine_rootdir(bundle.as_str(), cfg.get_namespace()).unwrap();
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
    fn start(&self) -> Result<u32, Error> {
        log::info!("starting instance: {}", self.id);
        let mut container = self.build_container(
            self.stdin.as_str(),
            self.stdout.as_str(),
            self.stderr.as_str(),
        )?;
        log::info!("created container: {}", self.id);
        let code = self.exit_code.clone();
        let pid = container.pid().unwrap();

        container
            .start()
            .map_err(|err| Error::Any(anyhow::anyhow!("failed to start container: {}", err)))?;
        thread::spawn(move || {
            let (lock, cvar) = &*code;

            let status = match waitid(WaitID::Pid(pid), WaitPidFlag::WEXITED) {
                Ok(WaitStatus::Exited(_, status)) => status,
                Ok(WaitStatus::Signaled(_, sig, _)) => sig as i32,
                Ok(_) => 0,
                Err(e) => {
                    if e == Errno::ECHILD {
                        log::info!("no child process");
                        0
                    } else {
                        panic!("waitpid failed: {}", e);
                    }
                }
            } as u32;
            let mut ec = lock.lock().unwrap();
            *ec = Some((status, Utc::now()));
            drop(ec);
            cvar.notify_all();
        });

        Ok(pid.as_raw() as u32)
    }

    fn kill(&self, signal: u32) -> Result<(), Error> {
        log::info!("killing instance: {}", self.id);
        if signal as i32 != SIGKILL && signal as i32 != SIGINT {
            return Err(Error::InvalidArgument(
                "only SIGKILL and SIGINT are supported".to_string(),
            ));
        }
        let container_root = get_instance_root(&self.rootdir, self.id.as_str())?;
        let mut container = Container::load(container_root).with_context(|| {
            format!(
                "could not load state for container {id}",
                id = self.id.as_str()
            )
        })?;
        let signal = Signal::try_from(signal as i32)
            .map_err(|err| Error::InvalidArgument(format!("invalid signal number: {}", err)))?;
        match container.kill(signal, true) {
            Ok(_) => Ok(()),
            Err(e) => {
                if container.status() == ContainerStatus::Stopped {
                    return Err(Error::Others("container not running".into()));
                }
                Err(Error::Others(e.to_string()))
            }
        }
    }

    fn delete(&self) -> Result<(), Error> {
        log::info!("deleting instance: {}", self.id);
        match instance_exists(&self.rootdir, self.id.as_str()) {
            Ok(exists) => {
                if !exists {
                    return Ok(());
                }
            }
            Err(err) => {
                error!("could not find the container, skipping cleanup: {}", err);
                return Ok(());
            }
        }
        let container_root = get_instance_root(&self.rootdir, self.id.as_str())?;
        let container = Container::load(container_root).with_context(|| {
            format!(
                "could not load state for container {id}",
                id = self.id.as_str()
            )
        });
        match container {
            Ok(mut container) => container.delete(true).map_err(|err| {
                Error::Any(anyhow::anyhow!(
                    "failed to delete container {}: {}",
                    self.id,
                    err
                ))
            })?,
            Err(err) => {
                error!("could not find the container, skipping cleanup: {}", err);
                return Ok(());
            }
        }

        Ok(())
    }

    fn wait(&self, waiter: &Wait) -> Result<(), Error> {
        log::info!("waiting for instance: {}", self.id);
        let code = self.exit_code.clone();
        waiter.set_up_exit_code_wait(code)
    }
}

impl EngineGetter for Wasi {
    type E = ();

    fn new_engine() -> std::result::Result<Self::E, Error> {
        Ok(())
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
    shim::run::<ShimCli<Wasi, ()>>("io.containerd.spin.v1", None);
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
