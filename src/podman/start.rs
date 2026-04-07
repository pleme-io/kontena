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
fn ensure_machine_exists(runner: &dyn CommandRunner, config: &StartConfig) -> Result<(), Error> {
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
fn start_machine(runner: &dyn CommandRunner, bin: &str, machine: &str) -> Result<(), Error> {
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
fn machine_exists(runner: &dyn CommandRunner, bin: &str, machine: &str) -> Result<bool, Error> {
    runner.run_check(bin, &["machine", "inspect", machine])
}

/// Query the state of a machine via `podman machine inspect --format`.
fn get_machine_state(runner: &dyn CommandRunner, bin: &str, machine: &str) -> Result<String, Error> {
    runner.run_output(
        bin,
        &["machine", "inspect", machine, "--format", "{{.State}}"],
    )
    .map_err(|e| Error::MachineInspect {
        machine: machine.to_owned(),
        reason: e.to_string(),
    })
}
