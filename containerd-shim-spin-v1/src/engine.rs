use anyhow::{anyhow, ensure, Context, Result};
use containerd_shim_wasm::container::{Engine, RuntimeContext, Stdio};
use log::info;
use spin_app::locked::LockedApp;
use spin_loader::FilesMountStrategy;
use spin_manifest::schema::v2::AppManifest;
use spin_redis_engine::RedisTrigger;
use spin_trigger::TriggerHooks;
use spin_trigger::{loader, RuntimeConfig, TriggerExecutor, TriggerExecutorBuilder};
use spin_trigger_http::HttpTrigger;
use std::collections::HashSet;
use std::env;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use tokio::runtime::Runtime;
use url::Url;

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
    fn app_source(&self) -> Result<PathBuf> {
        spin_common::paths::resolve_manifest_file_path("/spin.toml")
    }

    fn resolve_app_source(&self, app_source: PathBuf) -> Result<ResolvedAppSource> {
        Ok(ResolvedAppSource::File {
            manifest_path: app_source.clone(),
            manifest: spin_manifest::manifest_from_file(app_source)?,
        })
    }

    async fn wasm_exec_async(&self) -> Result<()> {
        let app_source = self.app_source()?;
        let resolved_app_source = self.resolve_app_source(app_source.clone())?;
        let trigger_cmd = trigger_command_for_resolved_app_source(&resolved_app_source)
            .with_context(|| format!("Couldn't find trigger executor for {app_source:?}"))?;
        let mut locked_app = self.load_resolved_app_source(resolved_app_source).await?;
        self.update_locked_app(&mut locked_app); // no-op for now
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
            _ => {
                todo!("Only Http and Redis triggers are currently supported.")
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

                // create a cache directory at /.cache
                let cache_dir = PathBuf::from("/.cache");
                env::set_var("XDG_CACHE_HOME", &cache_dir);

                if !cache_dir.exists() {
                    tokio::fs::create_dir_all(&cache_dir)
                        .await
                        .with_context(|| format!("failed to create {:?}", cache_dir))?;
                }

                spin_loader::from_file(&manifest_path, files_mount_strategy).await
            }
            ResolvedAppSource::OciRegistry { locked_app } => Ok(locked_app),
        }
    }

    fn update_locked_app(&self, _locked_app: &mut LockedApp) {
        // TODO: Apply --env to component environments
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
        let runtime_config = RuntimeConfig::new(PathBuf::from("/").into());
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
            ResolvedAppSource::OciRegistry { locked_app: _ } => {
                todo!("OCI not yet supported")
            }
        };

        ensure!(!types.is_empty(), "no triggers in app");
        ensure!(types.len() == 1, "multiple trigger types not yet supported");
        Ok(types.into_iter().next().unwrap())
    }
}

fn trigger_command_for_resolved_app_source(resolved: &ResolvedAppSource) -> Result<String> {
    let trigger_type = resolved.trigger_type()?;

    match trigger_type {
        RedisTrigger::TRIGGER_TYPE | HttpTrigger::TRIGGER_TYPE => Ok(trigger_type.to_owned()),
        _ => {
            todo!("Only Http and Redis triggers are currently supported.")
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
