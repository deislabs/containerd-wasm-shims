
use anyhow::{anyhow, Context, Result};
<<<<<<< HEAD
use spin_trigger::TriggerHooks;
=======
use spin_manifest::ApplicationTrigger;
use std::fs::File;
use std::io::Write;
>>>>>>> 1879c99 (oci artifact support)
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::path::PathBuf;

use containerd_shim_wasm::container::{Engine, RuntimeContext, Stdio};
use containerd_shim_wasm::sandbox::oci::WASM_LAYER_MEDIA_TYPE;
use spin_app::locked::LockedApp;
use oci_spec::image::MediaType;
use log::info;
use spin_loader::cache::Cache;
use spin_redis_engine::RedisTrigger;
use spin_trigger::{loader, RuntimeConfig, TriggerExecutor, TriggerExecutorBuilder};
use spin_trigger_http::HttpTrigger;
use tokio::runtime::Runtime;
use url::Url;
use wasmtime::OptLevel;

const SPIN_ADDR: &str = "0.0.0.0:80";
const SPIN_APPLICATION_MEDIA_TYPE: &str = "application/vnd.fermyon.spin.application.v1+config";

enum AppSource {
    Oci,
    File
}


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
    async fn build_spin_application(source: AppSource, cache: &Cache) -> anyhow::Result<LockedApp> {
        let working_dir = PathBuf::from("/");
        match source {
            AppSource::File => {
                log::info!("loading from file");
                let app = spin_loader::from_file(PathBuf::from("/spin.toml"), Some(working_dir.clone())).await.context("unable to find app file")?;
                spin_trigger::locked::build_locked_app(app, &working_dir).context("couldn't build app")
            },
            AppSource::Oci => {
                log::info!("loading from oci");
                let oci_loader = spin_oci::OciLoader::new(working_dir);
                let reference = "docker.io/library/wasmtest_spin:latest"; // todo maybe get that via annotations?
                oci_loader.build_locked_app(PathBuf::from("/spin.json"), reference, cache).await
            },
        }
    }

    async fn build_spin_trigger<T: spin_trigger::TriggerExecutor>(
        working_dir: PathBuf,
        app: LockedApp,
    ) -> Result<T>
    where
        for<'de> <T as TriggerExecutor>::TriggerConfig: serde::de::Deserialize<'de>,
    {
        // Build and write app lock file
        let locked_path = working_dir.join("spin.lock");
        let locked_app_contents =
            serde_json::to_vec_pretty(&app).expect("could not serialize locked app");
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


    async fn wasm_exec_async(&self, ctx: &impl RuntimeContext) -> Result<()> {
        let mut app_source = AppSource::File;
        
        // TODO: can spin load all this without writing to disk? maybe in memory cache?
        let cache = Cache::new(Some(PathBuf::from("/"))).await.context("failed to create cache")?;
        if let Some(artifacts) = ctx.oci_artifacts() {
            info!(" >>> configuring spin oci application {}", artifacts.len());
            app_source = AppSource::Oci;
            for artifact in artifacts.iter() {
                match artifact.config.media_type() {
                    MediaType::Other(name) if name == SPIN_APPLICATION_MEDIA_TYPE => {
                        let path = PathBuf::from("/spin.json");
                        log::info!("writing spin oci config to {:?}", path);
                        File::create(&path)
                            .context("failed to create spin.json")?
                            .write_all(&artifact.layer)
                            .context("failed to write spin.json")?;
                    },
                    MediaType::Other(name) if name == WASM_LAYER_MEDIA_TYPE  => {
                        log::info!("writing artifact config to cache, near {:?}", cache.manifests_dir());
                        cache.write_wasm(&artifact.layer, &artifact.config.digest()).await?;
                    },
                    _ => {}
                }
            }
        }

        info!(" >>> building spin application");
        let app =
            SpinEngine::build_spin_application(app_source, &cache)
                .await
                .with_context(|| "failed to build spin application")?;

        let trigger = &app.metadata["trigger"];
        let trigger: ApplicationTrigger = serde_json::from_str(trigger.to_string().as_ref()).context("not able to parse trigger from locked")?;
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

    fn run_wasi(&self, ctx: &impl RuntimeContext, stdio: Stdio) -> Result<i32> {
        log::info!("setting up wasi");
        stdio.redirect()?;
        let rt = Runtime::new().context("failed to create runtime")?;

        rt.block_on(self.wasm_exec_async(ctx))?;
        
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
