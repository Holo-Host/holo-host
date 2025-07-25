
use anyhow::Context;
use std::process::Stdio;

pub(crate) async fn bash(cmd: &str) -> anyhow::Result<()> {
    let mut bash_cmd = tokio::process::Command::new("/usr/bin/env");
    bash_cmd.args(["bash", "-c", cmd]);

    log::trace!("Running bash command: {cmd}...");

    let output = bash_cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context(format!("Spawning {cmd}..."))?
        .wait_with_output()
        .await
        .context(format!("Waiting for spawned command: {cmd}"))?;

    if !output.status.success() {
        anyhow::bail!("Error running {bash_cmd:?} yielded non-success status:\n{output:?}");
    }

    log::info!("Nixos channel update result:\n{output:#?}");

    Ok(())
}
