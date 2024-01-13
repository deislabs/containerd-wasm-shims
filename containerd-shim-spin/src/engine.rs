use anyhow::{anyhow, ensure, Context, Result};
use containerd_shim_wasm::container::{Engine, RuntimeContext, Stdio};
use log::info;
use oci_spec::image::MediaType;
use spin_app::locked::LockedApp;
use spin_loader::cache::Cache;
use spin_loader::FilesMountStrategy;
use spin_manifest::schema::v2::AppManifest;
use spin_redis_engine::RedisTrigger;
use spin_trigger::TriggerHooks;
use spin_trigger::{loader, RuntimeConfig, TriggerExecutor, TriggerExecutorBuilder};
use spin_trigger_http::HttpTrigger;
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::Write;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use tokio::runtime::Runtime;
use trigger_sqs::SqsTrigger;
use url::Url;

const SPIN_ADDR: &str = "0.0.0.0:80";
/// RUNTIME_CONFIG_PATH specifies the expected location and name of the runtime
/// config for a Spin application. The runtime config should be loaded into the
/// root `/` of the container.
const RUNTIME_CONFIG_PATH: &str = "/runtime-config.toml";

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

#[derive(Clone)]
enum AppSource {
    File(PathBuf),
    Oci,
}

impl std::fmt::Debug for AppSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppSource::File(path) => write!(f, "File({})", path.display()),
            AppSource::Oci => write!(f, "Oci"),
        }
    }
}

impl SpinEngine {
    async fn app_source(&self, ctx: &impl RuntimeContext, cache: &Cache) -> Result<AppSource> {
        match ctx.wasm_layers() {
            [] => Ok(AppSource::File(
                spin_common::paths::resolve_manifest_file_path("/spin.toml")?,
            )),
            layers => {
                info!(
                    " >>> configuring spin oci application {}",
                    ctx.wasm_layers().len()
                );

                for artifact in layers {
                    match artifact.config.media_type() {
                        MediaType::Other(name)
                            if name == spin_oci::client::SPIN_APPLICATION_MEDIA_TYPE =>
                        {
                            let path = PathBuf::from("/spin.json");
                            log::info!("writing spin oci config to {:?}", path);
                            File::create(&path)
                                .context("failed to create spin.json")?
                                .write_all(&artifact.layer)
                                .context("failed to write spin.json")?;
                        }
                        MediaType::Other(name)
                            if name == "application/vnd.wasm.content.layer.v1+wasm" =>
                        {
                            log::info!(
                                "writing artifact config to cache, near {:?}",
                                cache.manifests_dir()
                            );
                            cache
                                .write_wasm(&artifact.layer, &artifact.config.digest())
                                .await?;
                        }
                        _ => {}
                    }
                }
                Ok(AppSource::Oci)
            }
        }
    }

    async fn resolve_app_source(
        &self,
        app_source: AppSource,
        cache: &Cache,
    ) -> Result<ResolvedAppSource> {
        let resolve_app_source = match app_source {
            AppSource::File(source) => ResolvedAppSource::File {
                manifest_path: source.clone(),
                manifest: spin_manifest::manifest_from_file(source.clone())?,
            },
            AppSource::Oci => {
                let working_dir = PathBuf::from("/");
                let loader = spin_oci::OciLoader::new(working_dir);

                // TODO: what is the best way to get this info? It isn't used only saved in the locked file
                let reference = "docker.io/library/wasmtest_spin:latest";

                let locked_app = loader
                    .load_from_cache(PathBuf::from("/spin.json"), reference, cache)
                    .await?;
                ResolvedAppSource::OciRegistry { locked_app }
            }
        };
        Ok(resolve_app_source)
    }

    async fn wasm_exec_async(&self, ctx: &impl RuntimeContext) -> Result<()> {
        // create a cache directory at /.cache
        // this is needed for the spin LocalLoader to work
        // TODO: spin should provide a more flexible `loader::from_file` that
        // does not assume the existence of a cache directory
        let cache_dir = PathBuf::from("/.cache");
        let cache = Cache::new(Some(cache_dir.clone()))
            .await
            .context("failed to create cache")?;
        env::set_var("XDG_CACHE_HOME", &cache_dir);
        let app_source = self.app_source(ctx, &cache).await?;
        let resolved_app_source = self.resolve_app_source(app_source.clone(), &cache).await?;
        let trigger_cmd = trigger_command_for_resolved_app_source(&resolved_app_source)
            .with_context(|| format!("Couldn't find trigger executor for {app_source:?}"))?;
        let locked_app = self.load_resolved_app_source(resolved_app_source).await?;
        self.run_trigger(&trigger_cmd, locked_app).await
    }

    async fn run_trigger(&self, trigger_type: &str, app: LockedApp) -> Result<()> {
        let working_dir = PathBuf::from("/");
        let f = match trigger_type {
            HttpTrigger::TRIGGER_TYPE => {
                let http_trigger: HttpTrigger = self
                    .build_spin_trigger(working_dir, app)
                    .await
                    .context("failed to build spin trigger")?;
                info!(" >>> running spin trigger");
                http_trigger.run(spin_trigger_http::CliArgs {
                    address: parse_addr(SPIN_ADDR).unwrap(),
                    tls_cert: None,
                    tls_key: None,
                })
            }
            RedisTrigger::TRIGGER_TYPE => {
                let redis_trigger: RedisTrigger = self
                    .build_spin_trigger(working_dir, app)
                    .await
                    .context("failed to build spin trigger")?;

                info!(" >>> running spin trigger");
                redis_trigger.run(spin_trigger::cli::NoArgs)
            }
            SqsTrigger::TRIGGER_TYPE => {
                let sqs_trigger: SqsTrigger = self
                    .build_spin_trigger(working_dir, app)
                    .await
                    .context("failed to build spin trigger")?;

                info!(" >>> running spin trigger");
                sqs_trigger.run(spin_trigger::cli::NoArgs)
            }
            _ => {
                todo!("Only Http, Redis and SQS triggers are currently supported.")
            }
        };
        info!(" >>> notifying main thread we are about to start");
        f.await
    }

    async fn load_resolved_app_source(
        &self,
        resolved: ResolvedAppSource,
    ) -> anyhow::Result<LockedApp> {
        match resolved {
            ResolvedAppSource::File { manifest_path, .. } => {
                // TODO: This should be configurable, see https://github.com/deislabs/containerd-wasm-shims/issues/166
                let files_mount_strategy = FilesMountStrategy::Direct;
                spin_loader::from_file(&manifest_path, files_mount_strategy, None).await
            }
            ResolvedAppSource::OciRegistry { locked_app } => Ok(locked_app),
        }
    }

    async fn write_locked_app(&self, locked_app: &LockedApp, working_dir: &Path) -> Result<String> {
        let locked_path = working_dir.join("spin.lock");
        let locked_app_contents =
            serde_json::to_vec_pretty(&locked_app).context("failed to serialize locked app")?;
        tokio::fs::write(&locked_path, locked_app_contents)
            .await
            .with_context(|| format!("failed to write {:?}", locked_path))?;
        let locked_url = Url::from_file_path(&locked_path)
            .map_err(|_| anyhow!("cannot convert to file URL: {locked_path:?}"))?
            .to_string();

        Ok(locked_url)
    }

    async fn build_spin_trigger<T: spin_trigger::TriggerExecutor>(
        &self,
        working_dir: PathBuf,
        app: LockedApp,
    ) -> Result<T>
    where
        for<'de> <T as TriggerExecutor>::TriggerConfig: serde::de::Deserialize<'de>,
    {
        let locked_url = self.write_locked_app(&app, &working_dir).await?;

        // Build trigger config
        let loader = loader::TriggerLoader::new(working_dir.clone(), true);
        let mut runtime_config = RuntimeConfig::new(PathBuf::from("/").into());
        // Load in runtime config if one exists at expected location
        if Path::new(RUNTIME_CONFIG_PATH).exists() {
            runtime_config.merge_config_file(RUNTIME_CONFIG_PATH)?;
        }
        let mut builder = TriggerExecutorBuilder::new(loader);
        builder
            .hooks(StdioTriggerHook {})
            .config_mut()
            .wasmtime_config()
            .cranelift_opt_level(spin_core::wasmtime::OptLevel::Speed);
        let init_data = Default::default();
        let executor = builder.build(locked_url, runtime_config, init_data).await?;
        Ok(executor)
    }
}

impl Engine for SpinEngine {
    fn name() -> &'static str {
        "spin"
    }

    fn run_wasi(&self, ctx: &impl RuntimeContext, stdio: Stdio) -> Result<i32> {
        stdio.redirect()?;
        info!("setting up wasi");
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

// TODO: we should use spin's ResolvedAppSource
pub enum ResolvedAppSource {
    File {
        manifest_path: PathBuf,
        manifest: AppManifest,
    },
    OciRegistry {
        locked_app: LockedApp,
    },
}

impl ResolvedAppSource {
    pub fn trigger_type(&self) -> anyhow::Result<&str> {
        let types = match self {
            ResolvedAppSource::File { manifest, .. } => {
                manifest.triggers.keys().collect::<HashSet<_>>()
            }
            ResolvedAppSource::OciRegistry { locked_app } => locked_app
                .triggers
                .iter()
                .map(|t| &t.trigger_type)
                .collect::<HashSet<_>>(),
        };

        ensure!(!types.is_empty(), "no triggers in app");
        ensure!(types.len() == 1, "multiple trigger types not yet supported");
        Ok(types.into_iter().next().unwrap())
    }
}

fn trigger_command_for_resolved_app_source(resolved: &ResolvedAppSource) -> Result<String> {
    let trigger_type = resolved.trigger_type()?;

    match trigger_type {
        RedisTrigger::TRIGGER_TYPE | HttpTrigger::TRIGGER_TYPE | SqsTrigger::TRIGGER_TYPE => {
            Ok(trigger_type.to_owned())
        }
        _ => {
            todo!("Only Http, Redis and SQS triggers are currently supported.")
        }
    }
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
