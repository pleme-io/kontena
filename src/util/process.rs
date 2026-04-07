use std::process::Command;

use tracing::debug;

use crate::error::Error;

/// Abstraction over subprocess execution for testability.
///
/// Production code uses [`SystemCommandRunner`]; tests can swap in a mock
/// implementation to control subprocess results without forking real processes.
pub trait CommandRunner {
    /// Run a command and return whether it exited successfully (code 0).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Spawn`] if the process cannot be started.
    fn run_check(&self, bin: &str, args: &[&str]) -> Result<bool, Error>;

    /// Run a command and capture its stdout as a trimmed `String`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Spawn`] if the process cannot be started, or
    /// [`Error::NonZeroExit`] if it exits with a non-zero status.
    fn run_output(&self, bin: &str, args: &[&str]) -> Result<String, Error>;

    /// Replace the current process with the given command (unix `exec`).
    ///
    /// The default implementation returns an error; only [`SystemCommandRunner`]
    /// actually exec's.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Exec`] if exec fails (or on non-unix platforms).
    fn run_exec(&self, bin: &str, args: &[&str]) -> Result<(), Error>;
}

/// [`CommandRunner`] that delegates to real [`std::process::Command`] calls.
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run_check(&self, bin: &str, args: &[&str]) -> Result<bool, Error> {
        debug!(bin, ?args, "run_check");
        let status = Command::new(bin)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| Error::Spawn {
                bin: bin.to_owned(),
                reason: e.to_string(),
            })?;
        debug!(bin, code = status.code(), "run_check finished");
        Ok(status.success())
    }

    fn run_output(&self, bin: &str, args: &[&str]) -> Result<String, Error> {
        debug!(bin, ?args, "run_output");
        let output = Command::new(bin)
            .args(args)
            .output()
            .map_err(|e| Error::Spawn {
                bin: bin.to_owned(),
                reason: e.to_string(),
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(Error::NonZeroExit {
                bin: bin.to_owned(),
                status: output.status.to_string(),
                stderr,
            });
        }
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        debug!(bin, %stdout, "run_output finished");
        Ok(stdout)
    }

    #[cfg(unix)]
    fn run_exec(&self, bin: &str, args: &[&str]) -> Result<(), Error> {
        use std::os::unix::process::CommandExt;

        debug!(bin, ?args, "exec");

        let mut cmd = Command::new(bin);
        cmd.args(args);

        let err = cmd.exec();
        Err(Error::Exec {
            bin: bin.to_owned(),
            reason: err.to_string(),
        })
    }

    #[cfg(not(unix))]
    fn run_exec(&self, bin: &str, _args: &[&str]) -> Result<(), Error> {
        Err(Error::Exec {
            bin: bin.to_owned(),
            reason: "exec is not supported on this platform".to_owned(),
        })
    }
}

/// Free-function wrappers using the system runner for backward compatibility
/// with code that hasn't been converted to use the trait yet.

/// Run a command and return whether it exited successfully (code 0).
pub fn run_check(bin: &str, args: &[&str]) -> Result<bool, Error> {
    SystemCommandRunner.run_check(bin, args)
}

/// Run a command and capture its stdout as a trimmed `String`.
pub fn run_output(bin: &str, args: &[&str]) -> Result<String, Error> {
    SystemCommandRunner.run_output(bin, args)
}

/// Replace the current process with the given command (unix `exec`).
#[cfg(unix)]
pub fn run_exec(bin: &str, args: &[&str]) -> Result<(), Error> {
    SystemCommandRunner.run_exec(bin, args)
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
        let msg = format!("{}", result.unwrap_err());
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
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("exited with"), "{msg}");
    }

    #[test]
    fn run_output_errors_on_nonexistent_binary() {
        let result = run_output("__no_such_binary_kontena_test__", &[]);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
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

    #[test]
    fn system_runner_check_delegates_correctly() {
        let runner = SystemCommandRunner;
        assert!(runner.run_check("true", &[]).unwrap());
        assert!(!runner.run_check("false", &[]).unwrap());
    }

    #[test]
    fn system_runner_output_delegates_correctly() {
        let runner = SystemCommandRunner;
        let out = runner.run_output("echo", &["test"]).unwrap();
        assert_eq!(out, "test");
    }
}
