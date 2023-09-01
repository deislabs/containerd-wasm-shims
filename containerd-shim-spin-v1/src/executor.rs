use anyhow::{anyhow, bail, ensure, Context, Result};
use log::info;
use spin_manifest::Application;
use spin_redis_engine::RedisTrigger;
use spin_trigger::{loader, RuntimeConfig, TriggerExecutor, TriggerExecutorBuilder};
use spin_trigger_http::HttpTrigger;
use std::fs::File;
use std::io::Read;
use std::os::unix::prelude::PermissionsExt;
use std::{future::Future, path::PathBuf, pin::Pin};

use tokio::runtime::Runtime;
use url::Url;
use wasmtime::OptLevel;

use containerd_shim_wasm::libcontainer_instance::LinuxContainerExecutor;
use containerd_shim_wasm::sandbox::{oci, Stdio};
use libcontainer::workload::{Executor, ExecutorError, ExecutorValidationError};
use oci_spec::runtime::Spec;

use crate::{parse_addr, SPIN_ADDR};

// const EXECUTOR_NAME: &str = "spin";
// const RUNTIME_CONFIG_FILE_PATH: &str = "runtime_config.toml";

#[derive(Clone)]
pub struct SpinExecutor {
    stdio: Stdio,
}

impl SpinExecutor {
    pub fn new(stdio: Stdio) -> Self {
        Self { stdio }
    }
}

impl SpinExecutor {
    async fn build_spin_application(
        mod_path: PathBuf,
        working_dir: PathBuf,
    ) -> anyhow::Result<Application> {
        spin_loader::from_file(mod_path, Some(working_dir)).await
    }

    async fn build_spin_trigger<T: spin_trigger::TriggerExecutor>(
        working_dir: PathBuf,
        app: Application,
    ) -> Result<T>
    where
        for<'de> <T as TriggerExecutor>::TriggerConfig: serde::de::Deserialize<'de>,
    {
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
        let runtime_config = RuntimeConfig::new(PathBuf::from("/").into());
        let mut builder = TriggerExecutorBuilder::new(loader);
        let config = builder.wasmtime_config_mut();
        config.cranelift_opt_level(OptLevel::Speed);
        let init_data = Default::default();
        let executor = builder.build(locked_url, runtime_config, init_data).await?;
        Ok(executor)
    }
}

impl Executor for SpinExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        if is_linux_executable(spec).is_ok() {
            log::info!("executing linux container");
            LinuxContainerExecutor::new(self.stdio.clone()).exec(spec)?;
            Ok(())
        } else {
            log::info!("executing spin container");
            let args = oci::get_args(spec);
            if args.is_empty() {
                return Err(ExecutorError::InvalidArg);
            }
            let rt = Runtime::new().unwrap();
            let res = rt.block_on(async {
                info!(" >>> building spin application");
                let app = match SpinExecutor::build_spin_application(
                    PathBuf::from("/spin.toml"),
                    PathBuf::from("/"),
                )
                .await
                {
                    Ok(app) => app,
                    Err(err) => {
                        return err;
                    }
                };

                let trigger = app.info.trigger.clone();
                info!(" >>> building spin trigger {:?}", trigger);

                let f: Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send>>;

                match trigger {
                    spin_manifest::ApplicationTrigger::Http(_config) => {
                        let http_trigger: HttpTrigger =
                            match SpinExecutor::build_spin_trigger(PathBuf::from("/"), app).await {
                                Ok(http_trigger) => http_trigger,
                                Err(err) => {
                                    log::error!(" >>> failed to build spin trigger: {:?}", err);
                                    return err;
                                }
                            };

                        info!(" >>> running spin trigger");
                        f = http_trigger.run(spin_trigger_http::CliArgs {
                            address: parse_addr(SPIN_ADDR).unwrap(),
                            tls_cert: None,
                            tls_key: None,
                        });
                    }
                    spin_manifest::ApplicationTrigger::Redis(_config) => {
                        let redis_trigger: RedisTrigger =
                            match SpinExecutor::build_spin_trigger(PathBuf::from("/"), app).await {
                                Ok(redis_trigger) => redis_trigger,
                                Err(err) => {
                                    return err;
                                }
                            };

                        info!(" >>> running spin trigger");
                        f = redis_trigger.run(spin_trigger::cli::NoArgs);
                    }
                    _ => todo!("Only Http and Redis triggers are currently supported."),
                }

                info!(" >>> notifying main thread we are about to start");
                tokio::select! {
                    _ = f => {
                        log::info!(" >>> server shut down: exiting");
                        std::process::exit(0);
                    },
                }
            });
            log::error!(" >>> error: {:?}", res);
            std::process::exit(137);
        }
    }

    fn validate(&self, _spec: &Spec) -> Result<(), ExecutorValidationError> {
        Ok(())
    }
}

fn is_linux_executable(spec: &Spec) -> anyhow::Result<()> {
    let args = oci::get_args(spec).to_vec();

    if args.is_empty() {
        bail!("no args provided");
    }

    let executable = args.first().context("no executable provided")?;
    ensure!(!executable.is_empty(), "executable is empty");
    let cwd = std::env::current_dir()?;

    let executable = if executable.contains('/') {
        let path = cwd.join(executable);
        ensure!(path.is_file(), "file not found");
        path
    } else {
        spec.process()
            .as_ref()
            .and_then(|p| p.env().clone())
            .unwrap_or_default()
            .into_iter()
            .map(|v| match v.split_once('=') {
                None => (v, "".to_string()),
                Some((k, v)) => (k.to_string(), v.to_string()),
            })
            .find(|(key, _)| key == "PATH")
            .context("PATH not defined")?
            .1
            .split(':')
            .map(|p| cwd.join(p).join(executable))
            .find(|p| p.is_file())
            .context("file not found")?
    };

    let mode = executable.metadata()?.permissions().mode();
    ensure!(mode & 0o001 != 0, "entrypoint is not a executable");

    // check the shebang and ELF magic number
    // https://en.wikipedia.org/wiki/Executable_and_Linkable_Format#File_header
    let mut buffer = [0; 4];
    File::open(&executable)?.read_exact(&mut buffer)?;

    match buffer {
        [0x7f, 0x45, 0x4c, 0x46] => Ok(()), // ELF magic number
        [0x23, 0x21, ..] => Ok(()),         // shebang
        _ => bail!("{executable:?} is not a valid script or elf file"),
    }
}
