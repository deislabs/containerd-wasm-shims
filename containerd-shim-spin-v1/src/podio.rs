use std::{
    fs::{File, OpenOptions},
    path::PathBuf,
};

use spin_trigger::TriggerHooks;

pub struct PodioLoggingTriggerHooks {
    stdout_pipe: Option<File>,
    stderr_pipe: Option<File>,
    stdin_pipe: Option<File>,
}

fn maybe_open_stdio(pipe_path: &PathBuf) -> Option<std::fs::File> {
    if pipe_path.as_os_str().is_empty() {
        None
    } else {
        Some(
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(pipe_path.clone())
                .expect("could not open pipe"),
        )
    }
}

impl PodioLoggingTriggerHooks {
    pub fn new(
        stdout_pipe_path: PathBuf,
        stderr_pipe_path: PathBuf,
        stdin_pipe_path: PathBuf,
    ) -> Self {
        let stdout_pipe = maybe_open_stdio(&stdout_pipe_path);
        let stderr_pipe = maybe_open_stdio(&stderr_pipe_path);
        let stdin_pipe = maybe_open_stdio(&stdin_pipe_path);
        Self {
            stdout_pipe,
            stderr_pipe,
            stdin_pipe,
        }
    }
}

impl TriggerHooks for PodioLoggingTriggerHooks {
    fn app_loaded(&mut self, _app: &spin_app::App) -> anyhow::Result<()> {
        Ok(())
    }

    fn component_store_builder(
        &self,
        _component: spin_app::AppComponent,
        builder: &mut spin_core::StoreBuilder,
    ) -> anyhow::Result<()> {
        if let Some(stdout_pipe) = &self.stdout_pipe {
            builder.stdout_pipe(stdout_pipe.try_clone().unwrap());
        }
        if let Some(stderr_pipe) = &self.stderr_pipe {
            builder.stderr_pipe(stderr_pipe.try_clone().unwrap());
        }
        if let Some(stdin_pipe) = &self.stdin_pipe {
            builder.stdin_pipe(stdin_pipe.try_clone().unwrap());
        }
        Ok(())
    }
}
