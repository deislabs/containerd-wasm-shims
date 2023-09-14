use std::sync::Arc;

use anyhow::{Context, Result};
use containerd_shim_wasm::container::{RuntimeContext, Stdio};
use lunatic_process::env::{Environment, Environments, LunaticEnvironments};
use lunatic_process::runtimes::{wasmtime, RawWasm};
use lunatic_process::wasm::spawn_wasm;
use lunatic_process_api::ProcessConfigCtx;
use lunatic_runtime::{DefaultProcessConfig, DefaultProcessState};

#[containerd_shim_wasm::main("Lunatic")]
async fn main(ctx: &impl RuntimeContext, stdio: Stdio) -> Result<()> {
    log::info!("setting up wasi");

    stdio.redirect()?;

    let (path, func) = ctx
        .resolved_wasi_entrypoint()
        .context("no cmd provided")?
        .into();

    log::info!(" >>> building lunatic application (binary: {path:?}, entrypoint: {func:?})");
    let wasmtime_config = wasmtime::default_config();
    let runtime = wasmtime::WasmtimeRuntime::new(&wasmtime_config)?;
    let envs = LunaticEnvironments::default();
    let environment = envs.create(1).await;

    let mut config = DefaultProcessConfig::default();
    config.set_can_compile_modules(true);
    config.set_can_create_configs(true);
    config.set_can_spawn_processes(true);
    config.set_command_line_arguments(ctx.args().to_vec());
    config.set_environment_variables(std::env::vars().collect());
    config.preopen_dir("/");

    let module: RawWasm = std::fs::read(&path).context("opening module")?.into();
    let module = Arc::new(runtime.compile_module(module)?);
    let state = DefaultProcessState::new(
        environment.clone(),
        None,
        runtime.clone(),
        module.clone(),
        config.into(),
        Default::default(),
    )?;

    environment.can_spawn_next_process().await?;

    let (task, _) = spawn_wasm(environment, runtime, &module, state, &func, vec![], None)
        .await
        .context("Spawn lunatic process")?;
    task.await??;

    Ok(())
}
