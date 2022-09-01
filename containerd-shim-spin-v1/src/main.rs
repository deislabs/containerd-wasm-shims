use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use std::thread;

use chrono::{DateTime, Utc};
use containerd_shim as shim;
use containerd_shim_wasm::sandbox::{instance::InstanceConfig, ShimCli};
use containerd_shim_wasm::sandbox::error::Error;
use containerd_shim_wasm::sandbox::Instance;
use containerd_shim_wasm::sandbox::instance::EngineGetter;
use containerd_shim_wasm::sandbox::oci;
use log::info;
use spin_engine::io::CustomLogPipes;
use spin_engine::io::ModuleIoRedirectsTypes;
use spin_engine::io::PipeFile;
use spin_http_engine::{HttpTrigger, HttpTriggerConfig};
use spin_trigger::TriggerExecutor;
use tokio::runtime::Runtime;
use wasmtime::OptLevel;

static SPIN_ADDR: &str = "0.0.0.0:80";

pub struct Wasi {
    exit_code: Arc<(Mutex<Option<(u32, DateTime<Utc>)>>, Condvar)>,
    engine: spin_engine::Engine,
    id: String,
    stdin: String,
    stdout: String,
    stderr: String,
    bundle: String,
    shutdown_signal: Arc<(Mutex<bool>, Condvar)>,
}

pub fn prepare_module(bundle: String) -> Result<(PathBuf, PathBuf), Error> {
    let mut spec = oci::load(Path::new(&bundle)
        .join("config.json")
        .to_str()
        .unwrap())
        .expect("unable to load OCI bundle");

    spec.canonicalize_rootfs(&bundle)
        .map_err(|err| Error::Others(format!("could not canonicalize rootfs: {}", err)))?;

    let working_dir = oci::get_root(&spec);
    let mod_path = working_dir.join("spin.toml");
    Ok((working_dir.to_path_buf(), mod_path))
}

pub fn maybe_open_stdio(pipe_path: &PathBuf) -> Option<PipeFile> {
    if pipe_path.as_os_str().is_empty() {
        None
    } else {
        Some(PipeFile::new(
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(pipe_path.clone())
                .unwrap(),
            pipe_path.clone(),
        ))
    }
}

impl Wasi {
    async fn build_spin_application(
        mod_path: PathBuf,
        working_dir: PathBuf,
    ) -> Result<spin_manifest::Application, Error> {
        Ok(spin_loader::from_file(mod_path, working_dir, &None).await?)
    }

    async fn build_spin_trigger(
        engine: spin_engine::Engine,
        app: spin_manifest::Application,
        stdout_pipe_path: PathBuf,
        stderr_pipe_path: PathBuf,
        stdin_pipe_path: PathBuf,
    ) -> Result<HttpTrigger, Error> {
        let custom_log_pipes = CustomLogPipes::new(
            maybe_open_stdio(&stdin_pipe_path),
            maybe_open_stdio(&stdout_pipe_path),
            maybe_open_stdio(&stderr_pipe_path),
        );
        let config = spin_engine::ExecutionContextConfiguration {
            components: app.components,
            label: app.info.name,
            config_resolver: app.config_resolver,
            module_io_redirects: ModuleIoRedirectsTypes::FromFiles(custom_log_pipes),
            ..Default::default()
        };

        let mut builder = spin_engine::Builder::with_engine(config, engine)
            .expect("can create a builder with engine");
        builder
            .link_defaults()
            .expect("can link defaults for builder");
        HttpTrigger::configure_execution_context(&mut builder)?;
        let execution_ctx = builder.build().await?;
        let global_config = app.info.trigger.try_into().unwrap();
        let trigger_configs = app
            .component_triggers
            .into_iter()
            .map(|(id, config)| (id, config).try_into().unwrap())
            .collect::<Vec<HttpTriggerConfig>>();

        let trigger = spin_http_engine::HttpTrigger::new(
            execution_ctx,
            global_config,
            trigger_configs,
        )?;

        Ok(trigger)
    }
}

impl Instance for Wasi {
    type E = spin_engine::Engine;
    fn new(id: String, cfg: Option<&InstanceConfig<Self::E>>) -> Self {
        let cfg = cfg.unwrap();
        Wasi {
            exit_code: Arc::new((Mutex::new(None), Condvar::new())),
            engine: cfg.get_engine(),
            id,
            stdin: cfg.get_stdin().unwrap_or_default(),
            stdout: cfg.get_stdout().unwrap_or_default(),
            stderr: cfg.get_stderr().unwrap_or_default(),
            bundle: cfg.get_bundle().unwrap_or_default(),
            shutdown_signal: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }
    fn start(&self) -> Result<u32, Error> {
        let engine = self.engine.clone();
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
                    let app = match Wasi::build_spin_application(mod_path, working_dir).await {
                        Ok(app) => app,
                        Err(err) => {
                            tx.send(Err(err)).unwrap();
                            return;
                        }
                    };

                    let http_trigger = match Wasi::build_spin_trigger(
                        engine.clone(),
                        app,
                        PathBuf::from(stdout),
                        PathBuf::from(stderr),
                        PathBuf::from(stdin),
                    )
                        .await
                    {
                        Ok(http_trigger) => http_trigger,
                        Err(err) => {
                            tx.send(Err(err)).unwrap();
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

                    let f = http_trigger.run(spin_http_engine::CliArgs {
                        address: SPIN_ADDR.to_string(),
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
                    ;
                })
            })?;

        info!(" >>> waiting for start notification");
        match rx.recv().unwrap() {
            Ok(_) => (),
            Err(err) => {
                info!(" >>> error starting instance: {}", err);
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
        if signal != 9 {
            return Err(Error::InvalidArgument(
                "only SIGKILL is supported".to_string(),
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

impl EngineGetter for Wasi {
    type E = spin_engine::Engine;
    fn new_engine() -> Result<Self::E, Error> {
        let mut config = wasmtime::Config::new();
        config
            .cache_config_load_default()?
            .interruptable(true)
            .cranelift_opt_level(OptLevel::Speed);
        let engine = Self::E::new(config)?;
        Ok(engine)
    }
}

fn main() {
    shim::run::<ShimCli<Wasi, _>>("io.containerd.spin.v1", None);
}
