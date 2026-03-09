use std::process::Command;

use anyhow::{Context, Result};
use tracing::debug;

/// Run a command and return whether it exited successfully (code 0).
///
/// Stdout and stderr are inherited so the caller sees output in logs.
pub fn run_check(bin: &str, args: &[&str]) -> Result<bool> {
    debug!(bin, ?args, "run_check");
    let status = Command::new(bin)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .with_context(|| format!("failed to execute {bin}"))?;
    debug!(bin, code = status.code(), "run_check finished");
    Ok(status.success())
}

/// Run a command and capture its stdout as a trimmed `String`.
///
/// Returns an error if the process exits with a non-zero code.
pub fn run_output(bin: &str, args: &[&str]) -> Result<String> {
    debug!(bin, ?args, "run_output");
    let output = Command::new(bin)
        .args(args)
        .output()
        .with_context(|| format!("failed to execute {bin}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("{bin} exited with {}: {stderr}", output.status);
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    debug!(bin, %stdout, "run_output finished");
    Ok(stdout)
}

/// Replace the current process with the given command (unix `exec`).
///
/// This function only returns on error — on success the process image is
/// replaced entirely.
#[cfg(unix)]
pub fn run_exec(bin: &str, args: &[&str]) -> Result<()> {
    use std::os::unix::process::CommandExt;

    debug!(bin, ?args, "exec");

    let mut cmd = Command::new(bin);
    cmd.args(args);

    // exec() replaces the process image; only returns on error.
    let err = cmd.exec();
    Err(anyhow::anyhow!("exec({bin}) failed: {err}"))
}
