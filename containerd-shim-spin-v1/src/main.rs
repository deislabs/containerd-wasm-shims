use std::net::{SocketAddr, ToSocketAddrs};
use std::path::Path;

use anyhow::{bail, Context, Result};
use containerd_shim_wasm::container::{RuntimeContext, Stdio};
use spin_core::wasmtime::OptLevel;
use spin_manifest::{Application, ApplicationTrigger};
use spin_redis_engine::RedisTrigger;
use spin_trigger::loader::TriggerLoader;
use spin_trigger::{RuntimeConfig, TriggerExecutorBuilder};
use spin_trigger_http::HttpTrigger;

const SPIN_ADDR: &str = "0.0.0.0:80";
const SPIN_FILE: &str = "/spin.toml";

#[containerd_shim_wasm::validate]
fn validate(_ctx: &impl RuntimeContext) -> bool {
    Path::new(SPIN_FILE).exists()
}

#[containerd_shim_wasm::main("Spin")]
async fn main(_ctx: &impl RuntimeContext, stdio: Stdio) -> Result<()> {
    log::info!(" >>> building spin application");

    stdio.redirect()?;

    let app = spin_loader::from_file(SPIN_FILE, Some("/"))
        .await
        .context("failed to build spin application")?;

    let trigger = &app.info.trigger;
    log::info!(" >>> building spin trigger {:?}", trigger);

    match trigger {
        ApplicationTrigger::Http(_) => {
            let config = spin_trigger_http::CliArgs {
                address: parse_addr(SPIN_ADDR).unwrap(),
                tls_cert: None,
                tls_key: None,
            };
            run_spin_trigger::<HttpTrigger>(app, config).await
        }
        ApplicationTrigger::Redis(_) => {
            let config = spin_trigger::cli::NoArgs;
            run_spin_trigger::<RedisTrigger>(app, config).await
        }
        _ => bail!("Only Http and Redis triggers are currently supported."),
    }
}

async fn run_spin_trigger<T>(app: Application, config: T::RunConfig) -> Result<()>
where
    T: spin_trigger::TriggerExecutor,
    T::TriggerConfig: serde::de::DeserializeOwned,
{
    let working_dir = "/";
    let locked_path = "/spin.lock";
    let locked_uri = "file:///spin.lock";

    // Build and write app lock file
    let app = spin_trigger::locked::build_locked_app(app, working_dir)?;
    let app_json = app.to_json().context("serializing locked app")?;
    std::fs::write(&locked_path, app_json).context("could not write locked app")?;

    // Build trigger config
    let loader = TriggerLoader::new(working_dir, true);
    let runtime_config = RuntimeConfig::new(Some("/".into()));
    let mut builder = TriggerExecutorBuilder::new(loader);
    builder
        .wasmtime_config_mut()
        .cranelift_opt_level(OptLevel::Speed);

    let init_data = Default::default();
    let trigger: T = builder
        .build(locked_uri.into(), runtime_config, init_data)
        .await
        .context("failed to build spin trigger")?;

    log::info!(" >>> running spin redis trigger");
    trigger.run(config).await?;

    Ok(())
}

fn parse_addr(addr: &str) -> Result<SocketAddr> {
    addr.to_socket_addrs()?
        .next()
        .context("could not parse address")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_parse_spin_address() {
        let parsed = parse_addr(SPIN_ADDR).unwrap();
        assert_eq!(parsed.port(), 80);
        assert_eq!(parsed.ip().to_string(), "0.0.0.0");
    }
}
