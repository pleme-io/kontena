use std::time::Duration;

use tracing::{debug, info, warn};

use crate::error::Error;
use crate::util::process::{CommandRunner, SystemCommandRunner};
use crate::util::{env_bool, env_or, env_parse};

/// Configuration for the podman start + monitor lifecycle.
#[derive(Debug, Clone)]
pub(crate) struct StartConfig {
    pub bin: String,
    pub machine: String,
    pub cpus: u32,
    pub memory: u32,
    pub disk: u32,
    pub rootful: bool,
}

impl StartConfig {
    /// Build a [`StartConfig`] from environment variables.
    fn from_env() -> Result<Self, Error> {
        Ok(Self {
            bin: env_or("KONTENA_PODMAN_BIN", "podman"),
            machine: env_or("KONTENA_MACHINE_NAME", "podman-machine-default"),
            cpus: env_parse("KONTENA_PODMAN_CPUS", 4)?,
            memory: env_parse("KONTENA_PODMAN_MEMORY", 4096)?,
            disk: env_parse("KONTENA_PODMAN_DISK", 60)?,
            rootful: env_bool("KONTENA_PODMAN_ROOTFUL", false),
        })
    }
}

/// Start the podman machine and monitor it until it stops (production entry point).
///
/// # Errors
///
/// Returns an error if machine initialisation, start, or state inspection fails.
pub fn run() -> Result<(), Error> {
    let config = StartConfig::from_env()?;
    run_with(&SystemCommandRunner, &config)
}

/// Core start + monitor logic, decoupled from I/O for testability.
pub(crate) fn run_with(runner: &dyn CommandRunner, config: &StartConfig) -> Result<(), Error> {
    info!(machine = %config.machine, "podman start sequence beginning");

    ensure_machine_exists(runner, config)?;
    start_machine(runner, &config.bin, &config.machine)?;
    monitor_machine(runner, &config.bin, &config.machine)?;

    info!(machine = %config.machine, "podman monitor exiting (launchd will restart)");
    Ok(())
}

/// Ensure the named machine exists, creating it if necessary.
pub(crate) fn ensure_machine_exists(runner: &dyn CommandRunner, config: &StartConfig) -> Result<(), Error> {
    if machine_exists(runner, &config.bin, &config.machine)? {
        info!(machine = %config.machine, "machine already exists");
        return Ok(());
    }

    let cpus_s = config.cpus.to_string();
    let memory_s = config.memory.to_string();
    let disk_s = config.disk.to_string();

    info!(
        machine = %config.machine,
        cpus = config.cpus,
        memory = config.memory,
        disk = config.disk,
        rootful = config.rootful,
        "initializing podman machine"
    );

    let mut args = vec![
        "machine", "init",
        "--cpus", &cpus_s,
        "--memory", &memory_s,
        "--disk-size", &disk_s,
    ];
    if config.rootful {
        args.push("--rootful");
    }

    match runner.run_output(&config.bin, &args) {
        Ok(stdout) => {
            info!(machine = %config.machine, %stdout, "podman machine initialized");
            Ok(())
        }
        Err(e) => {
            if machine_exists(runner, &config.bin, &config.machine)? {
                warn!(machine = %config.machine, "init error but machine exists (race): {e}");
                Ok(())
            } else {
                Err(Error::MachineInit {
                    reason: e.to_string(),
                })
            }
        }
    }
}

/// Issue `podman machine start`.
pub(crate) fn start_machine(runner: &dyn CommandRunner, bin: &str, machine: &str) -> Result<(), Error> {
    info!(%machine, "starting podman machine");

    match runner.run_output(bin, &["machine", "start"]) {
        Ok(stdout) => {
            if !stdout.is_empty() {
                info!(%stdout, "podman machine start output");
            }
            info!(%machine, "podman machine started");
            Ok(())
        }
        Err(e) => {
            warn!(%machine, "podman machine start returned error: {e}");
            let state = get_machine_state(runner, bin, machine).unwrap_or_default();
            if state == "running" {
                info!(%machine, "machine is already running, continuing");
                Ok(())
            } else {
                Err(Error::MachineStart {
                    reason: e.to_string(),
                })
            }
        }
    }
}

/// Poll the machine state with adaptive intervals.
///
/// Starts at 1 s and increases by 10 % each check up to 30 s.  Returns when
/// the machine is no longer in the "running" state so that launchd can restart
/// the agent.
fn monitor_machine(runner: &dyn CommandRunner, bin: &str, machine: &str) -> Result<(), Error> {
    let mut interval = Duration::from_secs(1);
    let max_interval = Duration::from_secs(30);

    info!(%machine, "monitoring machine state");

    loop {
        std::thread::sleep(interval);

        let state = get_machine_state(runner, bin, machine)?;

        if state != "running" {
            info!(%machine, %state, "machine is no longer running");
            return Ok(());
        }

        debug!(%machine, ?interval, "machine is running");

        let next_secs = (interval.as_secs_f64() * 1.1).min(max_interval.as_secs_f64());
        interval = Duration::from_secs_f64(next_secs);
    }
}

/// Check whether a named podman machine exists.
pub(crate) fn machine_exists(runner: &dyn CommandRunner, bin: &str, machine: &str) -> Result<bool, Error> {
    runner.run_check(bin, &["machine", "inspect", machine])
}

/// Query the state of a machine via `podman machine inspect --format`.
pub(crate) fn get_machine_state(runner: &dyn CommandRunner, bin: &str, machine: &str) -> Result<String, Error> {
    runner.run_output(
        bin,
        &["machine", "inspect", machine, "--format", "{{.State}}"],
    )
    .map_err(|e| Error::MachineInspect {
        machine: machine.to_owned(),
        reason: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use crate::util::mock::testing::{Call, MockCommandRunner, MockResponse};

    use super::*;

    fn default_config() -> StartConfig {
        StartConfig {
            bin: "podman".into(),
            machine: "test-machine".into(),
            cpus: 4,
            memory: 4096,
            disk: 60,
            rootful: false,
        }
    }

    // --- ensure_machine_exists tests ---

    #[test]
    fn ensure_machine_exists_skips_init_when_present() {
        let mock = MockCommandRunner::new(vec![
            MockResponse::Check(true), // machine exists
        ]);

        let result = ensure_machine_exists(&mock, &default_config());
        assert!(result.is_ok());
        assert_eq!(mock.calls().len(), 1);
    }

    #[test]
    fn ensure_machine_exists_creates_when_absent() {
        let mock = MockCommandRunner::new(vec![
            MockResponse::Check(false),                  // machine doesn't exist
            MockResponse::Output("initialized".into()), // init succeeds
        ]);

        let result = ensure_machine_exists(&mock, &default_config());
        assert!(result.is_ok());
        assert_eq!(mock.calls().len(), 2);
        assert_eq!(mock.calls()[1].args[1], "init");
    }

    #[test]
    fn ensure_machine_exists_includes_rootful() {
        let mut config = default_config();
        config.rootful = true;

        let mock = MockCommandRunner::new(vec![
            MockResponse::Check(false),
            MockResponse::Output("ok".into()),
        ]);

        ensure_machine_exists(&mock, &config).unwrap();
        assert!(mock.calls()[1].args.contains(&"--rootful".to_owned()));
    }

    #[test]
    fn ensure_machine_exists_handles_race() {
        let mock = MockCommandRunner::new(vec![
            MockResponse::Check(false),
            MockResponse::Err(Error::NonZeroExit {
                bin: "podman".into(),
                status: "exit status: 125".into(),
                stderr: "already exists".into(),
            }),
            MockResponse::Check(true), // now exists
        ]);

        assert!(ensure_machine_exists(&mock, &default_config()).is_ok());
    }

    #[test]
    fn ensure_machine_exists_fails_on_init_error() {
        let mock = MockCommandRunner::new(vec![
            MockResponse::Check(false),
            MockResponse::Err(Error::Spawn {
                bin: "podman".into(),
                reason: "not found".into(),
            }),
            MockResponse::Check(false), // still missing
        ]);

        let err = ensure_machine_exists(&mock, &default_config()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("machine init failed"), "{msg}");
    }

    // --- start_machine tests ---

    #[test]
    fn start_machine_succeeds() {
        let mock = MockCommandRunner::new(vec![
            MockResponse::Output("Machine started".into()),
        ]);

        assert!(start_machine(&mock, "podman", "test-machine").is_ok());

        let calls = mock.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0],
            Call {
                bin: "podman".into(),
                args: vec!["machine".into(), "start".into()],
            }
        );
    }

    #[test]
    fn start_machine_tolerates_already_running() {
        let mock = MockCommandRunner::new(vec![
            MockResponse::Err(Error::NonZeroExit {
                bin: "podman".into(),
                status: "exit status: 125".into(),
                stderr: "already running".into(),
            }),
            MockResponse::Output("running".into()), // get_machine_state
        ]);

        assert!(start_machine(&mock, "podman", "test-machine").is_ok());
    }

    #[test]
    fn start_machine_fails_when_not_running() {
        let mock = MockCommandRunner::new(vec![
            MockResponse::Err(Error::NonZeroExit {
                bin: "podman".into(),
                status: "exit status: 1".into(),
                stderr: "fatal".into(),
            }),
            MockResponse::Output("stopped".into()), // state != running
        ]);

        let err = start_machine(&mock, "podman", "test-machine").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("machine start failed"), "{msg}");
    }

    // --- get_machine_state tests ---

    #[test]
    fn get_machine_state_returns_state() {
        let mock = MockCommandRunner::new(vec![
            MockResponse::Output("running".into()),
        ]);

        let state = get_machine_state(&mock, "podman", "test-machine").unwrap();
        assert_eq!(state, "running");

        let calls = mock.calls();
        assert_eq!(calls[0].args, vec![
            "machine", "inspect", "test-machine", "--format", "{{.State}}"
        ]);
    }

    #[test]
    fn get_machine_state_wraps_error() {
        let mock = MockCommandRunner::new(vec![
            MockResponse::Err(Error::Spawn {
                bin: "podman".into(),
                reason: "not found".into(),
            }),
        ]);

        let err = get_machine_state(&mock, "podman", "my-machine").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("my-machine"), "{msg}");
    }

    // --- machine_exists tests ---

    #[test]
    fn machine_exists_returns_true_on_success() {
        let mock = MockCommandRunner::new(vec![MockResponse::Check(true)]);
        assert!(machine_exists(&mock, "podman", "m").unwrap());
    }

    #[test]
    fn machine_exists_returns_false_on_failure() {
        let mock = MockCommandRunner::new(vec![MockResponse::Check(false)]);
        assert!(!machine_exists(&mock, "podman", "m").unwrap());
    }
}
