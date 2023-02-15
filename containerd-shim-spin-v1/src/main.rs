use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::option::Option;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use clap::Parser;
use containerd_shim as shim;
use containerd_shim_wasm::sandbox::{
    error::Error,
    instance::{EngineGetter, InstanceConfig},
    oci, Instance, ShimCli,
};
use log::info;
use reqwest::Url;
use spin_http::HttpTrigger;
use spin_manifest::Application;
use spin_trigger::{
    config::TriggerExecutorBuilderConfig, loader, TriggerExecutor, TriggerExecutorBuilder,
};
use tokio::runtime::Runtime;
use wasmtime::OptLevel;

mod podio;

const SPIN_ADDR: &str = "0.0.0.0:80";
const RUNTIME_CONFIG_FILE_PATH: &str = "runtime_config.toml";

/// Helper for passing VERSION to opt.
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

type ExitCode = Arc<(Mutex<Option<(u32, DateTime<Utc>)>>, Condvar)>;

pub struct Wasi {
    exit_code: ExitCode,
    id: String,
    stdin: String,
    stdout: String,
    stderr: String,
    bundle: String,
    shutdown_signal: Arc<(Mutex<bool>, Condvar)>,
}

pub fn prepare_module(bundle: String) -> Result<(PathBuf, PathBuf), Error> {
    let mut spec = oci::load(Path::new(&bundle).join("config.json").to_str().unwrap())
        .expect("unable to load OCI bundle");

    spec.canonicalize_rootfs(&bundle)
        .map_err(|err| Error::Others(format!("could not canonicalize rootfs: {err}")))?;

    let working_dir = oci::get_root(&spec);
    let mod_path = working_dir.join("spin.toml");
    Ok((working_dir.to_path_buf(), mod_path))
}

impl Wasi {
    async fn build_spin_application(
        mod_path: PathBuf,
        working_dir: PathBuf,
    ) -> Result<Application, Error> {
        Ok(spin_loader::from_file(mod_path, Some(working_dir), &None).await?)
    }

    async fn build_spin_trigger(
        working_dir: PathBuf,
        app: Application,
        stdout_pipe_path: PathBuf,
        stderr_pipe_path: PathBuf,
        stdin_pipe_path: PathBuf,
    ) -> Result<HttpTrigger> {
        // Build and write app lock file
        let locked_app = spin_trigger::locked::build_locked_app(app, &working_dir)?;
        let locked_path = working_dir.join("spin.lock");
        let locked_app_contents =
            serde_json::to_vec_pretty(&locked_app).expect("could not serialize locked app");
        std::fs::write(&locked_path, locked_app_contents).expect("could not write locked app");
        let locked_url = Url::from_file_path(&locked_path)
            .map_err(|_| anyhow!("cannot convert to file URL: {locked_path:?}"))?
            .to_string();

        // Build trigger config
        let loader = loader::TriggerLoader::new(working_dir.clone(), true);

        let executor: HttpTrigger = {
            let mut builder = TriggerExecutorBuilder::<HttpTrigger>::new(loader);
            let config = builder.wasmtime_config_mut();
            config
                .cache_config_load_default()?
                .cranelift_opt_level(OptLevel::Speed);

            let logging_hooks = podio::PodioLoggingTriggerHooks::new(
                stdout_pipe_path,
                stderr_pipe_path,
                stdin_pipe_path,
            );
            builder.hooks(logging_hooks);
            let runtime_config = working_dir.clone().join(RUNTIME_CONFIG_FILE_PATH);
            let trigger_config = match runtime_config.as_path().try_exists() {
                Ok(true) => TriggerExecutorBuilderConfig::load_from_file(Some(runtime_config))?,
                _ => TriggerExecutorBuilderConfig::load_from_file(None)?,
            };
            builder.build(locked_url, trigger_config).await?
        };

        Ok(executor)
    }
}

impl Instance for Wasi {
    type E = ();
    fn new(id: String, cfg: Option<&InstanceConfig<Self::E>>) -> Self {
        let cfg = cfg.unwrap();
        Wasi {
            exit_code: Arc::new((Mutex::new(None), Condvar::new())),
            id,
            stdin: cfg.get_stdin().unwrap_or_default(),
            stdout: cfg.get_stdout().unwrap_or_default(),
            stderr: cfg.get_stderr().unwrap_or_default(),
            bundle: cfg.get_bundle().unwrap_or_default(),
            shutdown_signal: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }
    fn start(&self) -> Result<u32, Error> {
        let exit_code = self.exit_code.clone();
        let shutdown_signal = self.shutdown_signal.clone();
        let (tx, rx) = channel::<Result<(), Error>>();
        let bundle = self.bundle.clone();
        let stdin = self.stdin.clone();
        let stdout = self.stdout.clone();
        let stderr = self.stderr.clone();

        info!(
            " >>> stdin: {:#?}, stdout: {:#?}, stderr: {:#?}",
            stdin, stdout, stderr
        );

        thread::Builder::new()
            .name(self.id.clone())
            .spawn(move || {
                let (working_dir, mod_path) = match prepare_module(bundle) {
                    Ok(f) => f,
                    Err(err) => {
                        tx.send(Err(err)).unwrap();
                        return;
                    }
                };

                info!(" >>> loading module: {}", mod_path.display());
                info!(" >>> working dir: {}", working_dir.display());
                info!(" >>> starting spin");

                let rt = Runtime::new().unwrap();
                rt.block_on(async {
                    info!(" >>> building spin application");
                    let app =
                        match Wasi::build_spin_application(mod_path, working_dir.clone()).await {
                            Ok(app) => app,
                            Err(err) => {
                                tx.send(Err(err)).unwrap();
                                return;
                            }
                        };

                    info!(" >>> building spin trigger");
                    let http_trigger = match Wasi::build_spin_trigger(
                        working_dir,
                        app,
                        PathBuf::from(stdout),
                        PathBuf::from(stderr),
                        PathBuf::from(stdin),
                    )
                    .await
                    {
                        Ok(http_trigger) => http_trigger,
                        Err(err) => {
                            tx.send(Err(Error::Others(format!(
                                "could not build spin trigger: {err}"
                            ))))
                            .unwrap();
                            return;
                        }
                    };

                    let rx_future = tokio::task::spawn_blocking(move || {
                        let (lock, cvar) = &*shutdown_signal;
                        let mut shutdown = lock.lock().unwrap();
                        while !*shutdown {
                            shutdown = cvar.wait(shutdown).unwrap();
                        }
                    });

                    info!(" >>> running spin trigger");
                    let f = http_trigger.run(spin_http::CliArgs {
                        address: parse_addr(SPIN_ADDR).unwrap(),
                        tls_cert: None,
                        tls_key: None,
                    });

                    info!(" >>> notifying main thread we are about to start");
                    tx.send(Ok(())).unwrap();
                    tokio::select! {
                        _ = f => {
                            log::info!(" >>> server shut down: exiting");

                            let (lock, cvar) = &*exit_code;
                            let mut ec = lock.lock().unwrap();
                            *ec = Some((137, Utc::now()));
                            cvar.notify_all();
                        },
                        _ = rx_future => {
                            log::info!(" >>> user requested shutdown: exiting");
                            let (lock, cvar) = &*exit_code;
                            let mut ec = lock.lock().unwrap();
                            *ec = Some((0, Utc::now()));
                            cvar.notify_all();
                        },
                    }
                })
            })?;

        info!(" >>> waiting for start notification");
        match rx.recv().unwrap() {
            Ok(_) => (),
            Err(err) => {
                info!(" >>> error starting instance: {err}");
                let code = self.exit_code.clone();

                let (lock, cvar) = &*code;
                let mut ec = lock.lock().unwrap();
                *ec = Some((139, Utc::now()));
                cvar.notify_all();
                return Err(err);
            }
        }

        Ok(1) // TODO: PID: I wanted to use a thread ID here, but threads use a u64, the API wants a u32
    }

    fn kill(&self, signal: u32) -> Result<(), Error> {
        if signal != 9 && signal != 2 {
            return Err(Error::InvalidArgument(
                "only SIGKILL and SIGINT are supported".to_string(),
            ));
        }

        let (lock, cvar) = &*self.shutdown_signal;
        let mut shutdown = lock.lock().unwrap();
        *shutdown = true;
        cvar.notify_all();

        Ok(())
    }

    fn delete(&self) -> Result<(), Error> {
        Ok(())
    }

    fn wait(&self, channel: Sender<(u32, DateTime<Utc>)>) -> Result<(), Error> {
        let code = self.exit_code.clone();
        thread::spawn(move || {
            let (lock, cvar) = &*code;
            let mut exit = lock.lock().unwrap();
            while (*exit).is_none() {
                exit = cvar.wait(exit).unwrap();
            }
            let ec = (*exit).unwrap();
            channel.send(ec).unwrap();
        });

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

impl EngineGetter for Wasi {
    type E = ();
    fn new_engine() -> Result<Self::E, Error> {
        Ok(())
    }
}

/// The spin shim
#[derive(Parser, Debug)]
#[command(version = version())]
struct Args {}

fn main() {
    Args::parse();
    shim::run::<ShimCli<Wasi, _>>("io.containerd.spin.v1", None);
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
