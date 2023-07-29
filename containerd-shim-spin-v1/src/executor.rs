use log::info;
use nix::unistd::{dup, dup2};
use spin_manifest::Application;
use spin_redis_engine::RedisTrigger;
use spin_trigger::{loader, RuntimeConfig, TriggerExecutor, TriggerExecutorBuilder};
use spin_trigger_http::HttpTrigger;
use std::{future::Future, os::fd::RawFd, path::PathBuf, pin::Pin};
use tokio::runtime::Runtime;
use url::Url;
use wasmtime::OptLevel;

use anyhow::{anyhow, Result};
use containerd_shim_wasm::sandbox::oci;
use libc::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
use libcontainer::workload::{Executor, ExecutorError};
use oci_spec::runtime::Spec;

use crate::{parse_addr, SPIN_ADDR};

const EXECUTOR_NAME: &str = "spin";
// const RUNTIME_CONFIG_FILE_PATH: &str = "runtime_config.toml";

pub struct SpinExecutor {
    pub stdin: Option<RawFd>,
    pub stdout: Option<RawFd>,
    pub stderr: Option<RawFd>,
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
        config
            .cranelift_opt_level(OptLevel::Speed);
        let init_data = Default::default();
        let executor = builder.build(locked_url, runtime_config, init_data).await?;
        Ok(executor)
    }
}

impl Executor for SpinExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        let args = oci::get_args(spec);
        if args.is_empty() {
            return Err(ExecutorError::InvalidArg);
        }

        prepare_stdio(self.stdin, self.stdout, self.stderr).map_err(|err| {
            ExecutorError::Other(format!("failed to prepare stdio for container: {}", err))
        })?;

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
                    let http_trigger: HttpTrigger = match SpinExecutor::build_spin_trigger(
                        PathBuf::from("/"),
                        app,
                    )
                    .await
                    {
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
                    let redis_trigger: RedisTrigger = match SpinExecutor::build_spin_trigger(
                        PathBuf::from("/"),
                        app,
                    )
                    .await
                    {
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

    fn can_handle(&self, _spec: &Spec) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        EXECUTOR_NAME
    }
}

fn prepare_stdio(stdin: Option<RawFd>, stdout: Option<RawFd>, stderr: Option<RawFd>) -> Result<()> {
    if let Some(stdin) = stdin {
        dup(STDIN_FILENO)?;
        dup2(stdin, STDIN_FILENO)?;
    }
    if let Some(stdout) = stdout {
        dup(STDOUT_FILENO)?;
        dup2(stdout, STDOUT_FILENO)?;
    }
    if let Some(stderr) = stderr {
        dup(STDERR_FILENO)?;
        dup2(stderr, STDERR_FILENO)?;
    }
    Ok(())
}
