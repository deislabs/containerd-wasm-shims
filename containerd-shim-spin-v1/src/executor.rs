use anyhow::{anyhow, Context, Result};
use log::info;
use spin_manifest::Application;
use spin_redis_engine::RedisTrigger;
use spin_trigger::{loader, RuntimeConfig, TriggerExecutor, TriggerExecutorBuilder};
use spin_trigger_http::HttpTrigger;
use std::{future::Future, path::PathBuf, pin::Pin};

use tokio::runtime::Runtime;
use url::Url;
use wasmtime::OptLevel;

use containerd_shim_wasm::libcontainer_instance::LinuxContainerExecutor;
use containerd_shim_wasm::sandbox::Stdio;
use libcontainer::workload::{Executor, ExecutorError, ExecutorValidationError};
use oci_spec::runtime::Spec;
use utils::is_linux_executable;

use crate::{parse_addr, SPIN_ADDR};

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

    fn wasm_exec(&self, _spec: &Spec) -> Result<()> {
        log::info!("executing spin container");
        let rt = Runtime::new().context("failed to create runtime")?;
        rt.block_on(self.wasm_exec_async())
    }

    async fn wasm_exec_async(&self) -> Result<()> {
        info!(" >>> building spin application");
        let app =
            SpinExecutor::build_spin_application(PathBuf::from("/spin.toml"), PathBuf::from("/"))
                .await
                .context("failed to build spin application")?;

        let trigger = app.info.trigger.clone();
        info!(" >>> building spin trigger {:?}", trigger);

        let f: Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send>>;

        match trigger {
            spin_manifest::ApplicationTrigger::Http(_config) => {
                let http_trigger: HttpTrigger =
                    SpinExecutor::build_spin_trigger(PathBuf::from("/"), app)
                        .await
                        .context("failed to build spin trigger")?;
                info!(" >>> running spin trigger");
                f = http_trigger.run(spin_trigger_http::CliArgs {
                    address: parse_addr(SPIN_ADDR).unwrap(),
                    tls_cert: None,
                    tls_key: None,
                });
            }
            spin_manifest::ApplicationTrigger::Redis(_config) => {
                let redis_trigger: RedisTrigger =
                    SpinExecutor::build_spin_trigger(PathBuf::from("/"), app)
                        .await
                        .context("failed to build spin trigger")?;

                info!(" >>> running spin trigger");
                f = redis_trigger.run(spin_trigger::cli::NoArgs);
            }
            _ => todo!("Only Http and Redis triggers are currently supported."),
        }

        info!(" >>> notifying main thread we are about to start");
        f.await
    }
}

impl Executor for SpinExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        if is_linux_executable(spec).is_ok() {
            log::info!("executing linux container");
            LinuxContainerExecutor::new(self.stdio.clone()).exec(spec)
        } else {
            if let Err(err) = self.wasm_exec(spec) {
                log::info!(" >>> server shut down due to error: {err}");
                std::process::exit(137);
            }
            log::info!(" >>> server shut down: exiting");
            std::process::exit(0);
        }
    }

    fn validate(&self, _spec: &Spec) -> Result<(), ExecutorValidationError> {
        Ok(())
    }
}
