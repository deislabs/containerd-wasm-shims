use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;

use containerd_shim_wasm::container::{Engine, RuntimeContext, Stdio};
use lunatic_process::{
    env::{Environments, LunaticEnvironments},
    runtimes,
};

use crate::common::{run_wasm, RunWasm};

#[derive(Clone, Default)]
pub struct LunaticEngine;

impl Engine for LunaticEngine {
    fn name() -> &'static str {
        "lunatic"
    }

    fn run_wasi(&self, ctx: &impl RuntimeContext, stdio: Stdio) -> Result<i32> {
        log::info!("setting up wasi");
        stdio.redirect()?;
        let cmd = ctx.entrypoint().context("no cmd provided")?;
        let rt = Runtime::new().context("failed to create runtime")?;
        rt.block_on(exec(cmd.to_owned()))?;
        Ok(0)
    }
}

async fn exec(cmd: PathBuf) -> Result<()> {
    log::info!(" >>> lunatic wasm binary: {:?}", cmd);
    // Create wasmtime runtime
    let wasmtime_config = runtimes::wasmtime::default_config();
    let runtime = runtimes::wasmtime::WasmtimeRuntime::new(&wasmtime_config)?;
    let envs = Arc::new(LunaticEnvironments::default());

    let env = envs.create(1).await;
    run_wasm(RunWasm {
        path: cmd,
        wasm_args: vec![],
        dir: vec![],
        runtime,
        envs,
        env,
        distributed: None,
    })
    .await
}

#[cfg(test)]
mod tests {
    use crate::engine::exec;

    #[tokio::test]
    async fn test() {
        if let Err(error) = exec(
            "../images/lunatic/target/wasm32-wasi/release/wasi-hello-world.wasm"
                .to_string()
                .into(),
        )
        .await
        {
            panic!("Problem opening the file: {:?}", error)
        }
    }
}
