use anyhow::{bail, ensure, Context};
use oci_spec::runtime::Spec;
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;

pub fn get_args(spec: &Spec) -> Vec<String> {
    let p = match spec.process() {
        None => return vec![],
        Some(p) => p,
    };

    match p.args() {
        None => vec![],
        Some(args) => args.as_slice().to_vec(),
    }
}

pub fn is_linux_executable(spec: &Spec) -> anyhow::Result<()> {
    let args = get_args(spec).to_vec();

    let executable = args.first().context("no executable provided")?;
    ensure!(!executable.is_empty(), "executable is empty");
    let cwd = std::env::current_dir()?;

    let executable = if executable.contains('/') {
        let path = cwd.join(executable);
        ensure!(path.is_file(), "file not found");
        path
    } else {
        spec.process()
            .as_ref()
            .and_then(|p| p.env().clone())
            .unwrap_or_default()
            .into_iter()
            .map(|v| match v.split_once('=') {
                None => (v, "".to_string()),
                Some((k, v)) => (k.to_string(), v.to_string()),
            })
            .find(|(key, _)| key == "PATH")
            .context("PATH not defined")?
            .1
            .split(':')
            .map(|p| cwd.join(p).join(executable))
            .find(|p| p.is_file())
            .context("file not found")?
    };

    let mode = executable.metadata()?.permissions().mode();
    ensure!(mode & 0o001 != 0, "entrypoint is not a executable");

    // check the shebang and ELF magic number
    // https://en.wikipedia.org/wiki/Executable_and_Linkable_Format#File_header
    let mut buffer = [0; 4];
    File::open(&executable)?.read_exact(&mut buffer)?;

    match buffer {
        [0x7f, 0x45, 0x4c, 0x46] => Ok(()), // ELF magic number
        [0x23, 0x21, ..] => Ok(()),         // shebang
        _ => bail!("{executable:?} is not a valid script or elf file"),
    }
}
