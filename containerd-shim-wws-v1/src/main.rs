use chrono::{DateTime, Utc};
use containerd_shim as shim;
use containerd_shim_wasm::sandbox::error::Error;
use containerd_shim_wasm::sandbox::instance::{EngineGetter, InstanceConfig};
use containerd_shim_wasm::sandbox::oci;
use containerd_shim_wasm::sandbox::{Instance, ShimCli};
use log::{error, info};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::{Condvar, Mutex};
use std::thread;
use tokio::runtime::Runtime;
use wws_config::Config;
use wws_router::Routes;
use wws_server::serve;

/// URL to listen to in wws
const WWS_ADDR: &str = "0.0.0.0:80";

type ExitCode = Arc<(Mutex<Option<(u32, DateTime<Utc>)>>, Condvar)>;

pub struct Workers {
    exit_code: ExitCode,
    id: String,
    stdin: String,
    stdout: String,
    stderr: String,
    bundle: String,
    shutdown_signal: Arc<(Mutex<bool>, Condvar)>,
}

pub fn prepare_module(bundle: String) -> Result<PathBuf, Error> {
    info!("[wws] Preparing module");
    let mut spec = oci::load(Path::new(&bundle).join("config.json").to_str().unwrap())
        .expect("unable to load OCI bundle");

    info!("[wws] Canonicalize roots");
    spec.canonicalize_rootfs(&bundle)
        .map_err(|err| Error::Others(format!("could not canonicalize rootfs: {err}")))?;

    info!("[wws] Get root");
    let working_dir = oci::get_root(&spec);
    info!("[wws] loading project: {}", working_dir.display());

    // change the working directory to the rootfs
    // std::os::unix::fs::chroot(working_dir).unwrap();
    // std::env::set_current_dir("/").unwrap();

    Ok(working_dir.clone())
}

/// Implement the "default" interface from runwasi
impl Instance for Workers {
    type E = ();
    fn new(id: String, cfg: Option<&InstanceConfig<Self::E>>) -> Self {
        info!("[wws] new instance");
        let cfg = cfg.unwrap();

        Workers {
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
        info!("[wws] Starting the wws shim");
        let exit_code = self.exit_code.clone();
        let shutdown_signal = self.shutdown_signal.clone();
        let (tx, rx) = channel::<Result<(), Error>>();
        let bundle = self.bundle.clone();

        // let stdin = self.stdin.clone();
        // let stdout = self.stdout.clone();
        // let stderr = self.stderr.clone();

        thread::Builder::new()
            .name(self.id.clone())
            .spawn(move || {
                info!("[wws] Starting the process!");
                info!("[wws] Let's go!");
                let working_dir = match prepare_module(bundle) {
                    Ok(f) => f,
                    Err(err) => {
                        info!("[wws] Error when preparing the module!");
                        tx.send(Err(err)).unwrap();
                        return;
                    }
                };

                let paths = fs::read_dir(&working_dir).unwrap();

                info!("[wws] working_dir: {}", &working_dir.display());

                for path in paths {
                    info!(
                        "[wws] file in working_dir: {}",
                        path.unwrap().path().display()
                    )
                }

                info!("[wws] starting wws");

                let rt = Runtime::new().unwrap();
                rt.block_on(async {
                    let rx_future = tokio::task::spawn_blocking(move || {
                        let (lock, cvar) = &*shutdown_signal;
                        let mut shutdown = lock.lock().unwrap();
                        while !*shutdown {
                            shutdown = cvar.wait(shutdown).unwrap();
                        }
                    });

                    // Configure and run wws
                    info!("[wws] running wws");

                    let path = working_dir.clone();

                    // Check the runtimes
                    let config = match Config::load(&path) {
                        Ok(c) => c,
                        Err(err) => {
                            info!("[wws] There was an error reading the .wws.toml file. It will be ignored");
                            info!("[wws] Error: {err}");

                            Config::default()
                        }
                    };

                    // Check if there're missing runtimes
                    if config.is_missing_any_runtime(&path) {
                        info!("[wws] Required language runtimes are not installed. Some files may not be considered workers");
                        info!("[wws] You can install the missing runtimes with: wws runtimes install");
                    }

                    info!("[wws] Loading routes from: {}", &path.display());
                    let routes = Routes::new(&path, "", &config);

                    // Final server
                    let f = serve(&path, routes, "0.0.0.0", 80).await.unwrap();

                    info!("[wws] notifying main thread we are about to start");
                    tx.send(Ok(())).unwrap();
                    tokio::select! {
                        res = f => {
                            info!("[wws] server shut down: exiting");

                            if res.is_err() {
                                error!("[wws] error!");
                            }

                            let (lock, cvar) = &*exit_code;
                            let mut ec = lock.lock().unwrap();
                            *ec = Some((137, Utc::now()));
                            cvar.notify_all();
                        },
                        _ = rx_future => {
                            info!("[wws] user requested shutdown: exiting");
                            let (lock, cvar) = &*exit_code;
                            let mut ec = lock.lock().unwrap();
                            *ec = Some((0, Utc::now()));
                            cvar.notify_all();
                        },
                    }
                })
            })?;

        info!("[wws] waiting for start notification");
        match rx.recv().unwrap() {
            Ok(_) => (),
            Err(err) => {
                info!("[wws] error starting instance: {err}");
                let code = self.exit_code.clone();

                let (lock, cvar) = &*code;
                let mut ec = lock.lock().unwrap();
                *ec = Some((139, Utc::now()));
                cvar.notify_all();
                return Err(err);
            }
        }

        // TODO: Can we try to cast and default to 1 when it fails?
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

impl EngineGetter for Workers {
    type E = ();
    fn new_engine() -> Result<Self::E, Error> {
        Ok(())
    }
}

fn main() {
    shim::run::<ShimCli<Workers, _>>("io.containerd.wws.v1", None);
}
