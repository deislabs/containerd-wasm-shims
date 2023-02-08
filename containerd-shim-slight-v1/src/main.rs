use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::{Condvar, Mutex};
use std::thread;

use chrono::{DateTime, Utc};
use containerd_shim as shim;
use containerd_shim_wasm::sandbox::error::Error;
use containerd_shim_wasm::sandbox::instance::EngineGetter;
use containerd_shim_wasm::sandbox::oci;
use containerd_shim_wasm::sandbox::Instance;
use containerd_shim_wasm::sandbox::{instance::InstanceConfig, ShimCli};
use log::info;

use slight_lib::commands::run::handle_run;
use tokio::runtime::Runtime;

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

    // change the working directory to the rootfs
    std::os::unix::fs::chroot(working_dir).unwrap();
    std::env::set_current_dir("/").unwrap();

    // add env to current proc
    let env = spec.process().as_ref().unwrap().env().as_ref().unwrap();
    for v in env {
        match v.split_once('=') {
            None => {}
            Some(t) => std::env::set_var(t.0, t.1),
        };
    }

    let mod_path = PathBuf::from("slightfile.toml");
    let wasm_path = PathBuf::from("app.wasm");
    Ok((wasm_path, mod_path))
}

impl Instance for Wasi {
    type E = ();
    fn new(id: String, cfg: Option<&InstanceConfig<Self::E>>) -> Self {
        info!(">>> new instance");
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
        info!(">>> shim starts");
        let exit_code = self.exit_code.clone();
        let shutdown_signal = self.shutdown_signal.clone();
        let (tx, rx) = channel::<Result<(), Error>>();
        let bundle = self.bundle.clone();

        // FIXME: redirect slight stdio to pod stdio
        let _pod_stdin = self.stdin.clone();
        let _pod_stdout = self.stdout.clone();
        let _pod_stderr = self.stderr.clone();

        thread::Builder::new()
            .name(self.id.clone())
            .spawn(move || {
                let (wasm_path, mod_path) = match prepare_module(bundle) {
                    Ok(f) => f,
                    Err(err) => {
                        tx.send(Err(err)).unwrap();
                        return;
                    }
                };

                info!(" >>> loading module: {}", mod_path.display());
                info!(" >>> wasm path: {}", wasm_path.display());
                info!(" >>> starting slight");

                let rt = Runtime::new().unwrap();
                rt.block_on(async {
                    let toml_file_path = mod_path;

                    let rx_future = tokio::task::spawn_blocking(move || {
                        let (lock, cvar) = &*shutdown_signal;
                        let mut shutdown = lock.lock().unwrap();
                        while !*shutdown {
                            shutdown = cvar.wait(shutdown).unwrap();
                        }
                    });

                    let f = handle_run(wasm_path, &toml_file_path);

                    info!(" >>> notifying main thread we are about to start");
                    tx.send(Ok(())).unwrap();
                    tokio::select! {
                        res = f => {
                            log::info!(" >>> server shut down: exiting");
                            if res.is_err() {
                                log::error!(" >>> error: {:?}", res);
                            }
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
                    };
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

impl EngineGetter for Wasi {
    type E = ();
    fn new_engine() -> Result<Self::E, Error> {
        Ok(())
    }
}

fn main() {
    shim::run::<ShimCli<Wasi, _>>("io.containerd.slight.v1", None);
}
