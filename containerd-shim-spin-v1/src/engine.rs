use anyhow::{anyhow, Context, Result};
use spin_trigger::TriggerHooks;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::path::PathBuf;

use containerd_shim_wasm::container::{Engine, RuntimeContext, Stdio};
use log::info;
use spin_manifest::Application;
use spin_redis_engine::RedisTrigger;
use spin_trigger::{loader, RuntimeConfig, TriggerExecutor, TriggerExecutorBuilder};
use spin_trigger_http::HttpTrigger;
use tokio::runtime::Runtime;
use url::Url;
use wasmtime::OptLevel;

const SPIN_ADDR: &str = "0.0.0.0:80";

#[derive(Clone, Default)]
pub struct SpinEngine;

struct StdioTriggerHook;
impl TriggerHooks for StdioTriggerHook {
    fn app_loaded(&mut self, _app: &spin_app::App, _runtime_config: &RuntimeConfig) -> Result<()> {
        Ok(())
    }

    fn component_store_builder(
        &self,
        _component: &spin_app::AppComponent,
        builder: &mut spin_core::StoreBuilder,
    ) -> Result<()> {
        builder.inherit_stdout();
        builder.inherit_stderr();
        Ok(())
    }
}

impl SpinEngine {
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
        builder
            .hooks(StdioTriggerHook{})
            .config_mut()
            .wasmtime_config()
            .cranelift_opt_level(OptLevel::Speed);
        let init_data = Default::default();
        let executor = builder.build(locked_url, runtime_config, init_data).await?;
        Ok(executor)
    }

    async fn wasm_exec_async(&self) -> Result<()> {
        info!(" >>> building spin application");
        let app =
            SpinEngine::build_spin_application(PathBuf::from("/spin.toml"), PathBuf::from("/"))
                .await
                .context("failed to build spin application")?;

        let trigger = app.info.trigger.clone();
        info!(" >>> building spin trigger {:?}", trigger);

        let f = match trigger {
            spin_manifest::ApplicationTrigger::Http(_config) => {
                let http_trigger: HttpTrigger =
                    SpinEngine::build_spin_trigger(PathBuf::from("/"), app)
                        .await
                        .context("failed to build spin trigger")?;
                info!(" >>> running spin trigger");
                http_trigger.run(spin_trigger_http::CliArgs {
                    address: parse_addr(SPIN_ADDR).unwrap(),
                    tls_cert: None,
                    tls_key: None,
                })
            }
            spin_manifest::ApplicationTrigger::Redis(_config) => {
                let redis_trigger: RedisTrigger =
                    SpinEngine::build_spin_trigger(PathBuf::from("/"), app)
                        .await
                        .context("failed to build spin trigger")?;

                info!(" >>> running spin trigger");
                redis_trigger.run(spin_trigger::cli::NoArgs)
            }
            _ => todo!("Only Http and Redis triggers are currently supported."),
        };

        info!(" >>> notifying main thread we are about to start");
        f.await
    }
}

impl Engine for SpinEngine {
    fn name() -> &'static str {
        "spin"
    }

    fn run_wasi(&self, _ctx: &impl RuntimeContext, stdio: Stdio) -> Result<i32> {
        info!("setting up wasi");
        stdio.redirect()?;
        let rt = Runtime::new().context("failed to create runtime")?;

        rt.block_on(self.wasm_exec_async())?;
        Ok(0)
    }

    fn can_handle(&self, _ctx: &impl RuntimeContext) -> Result<()> {
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
