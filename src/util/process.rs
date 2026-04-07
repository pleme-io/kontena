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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_check_returns_true_for_success() {
        let ok = run_check("true", &[]).unwrap();
        assert!(ok);
    }

    #[test]
    fn run_check_returns_false_for_failure() {
        let ok = run_check("false", &[]).unwrap();
        assert!(!ok);
    }

    #[test]
    fn run_check_errors_on_nonexistent_binary() {
        let result = run_check("__no_such_binary_kontena_test__", &[]);
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(msg.contains("failed to execute"), "{msg}");
    }

    #[test]
    fn run_output_captures_stdout() {
        let out = run_output("echo", &["hello world"]).unwrap();
        assert_eq!(out, "hello world");
    }

    #[test]
    fn run_output_trims_whitespace() {
        let out = run_output("echo", &[""]).unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn run_output_errors_on_nonzero_exit() {
        let result = run_output("false", &[]);
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(msg.contains("exited with"), "{msg}");
    }

    #[test]
    fn run_output_errors_on_nonexistent_binary() {
        let result = run_output("__no_such_binary_kontena_test__", &[]);
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(msg.contains("failed to execute"), "{msg}");
    }

    #[test]
    fn run_check_with_arguments() {
        let ok = run_check("test", &["1", "-eq", "1"]).unwrap();
        assert!(ok);
        let ok = run_check("test", &["1", "-eq", "2"]).unwrap();
        assert!(!ok);
    }

    #[test]
    fn run_output_multiline_captures_full_output() {
        let out = run_output("printf", &["line1\nline2\nline3"]).unwrap();
        assert!(out.contains("line1"), "{out}");
        assert!(out.contains("line3"), "{out}");
    }
}
