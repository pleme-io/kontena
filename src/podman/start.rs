use std::time::Duration;

use anyhow::{Context, Result};
use tracing::{debug, info, warn};

use crate::util::process::{run_check, run_output};
use crate::util::{env_bool, env_or, env_parse};

/// Start the podman machine and monitor it until it stops.
///
/// **Phase 1 -- init:** If the machine doesn't exist, create it with the
/// configured CPU/memory/disk parameters.  This replaces the old activation
/// script approach (which ran as root and failed because `podman machine init`
/// refuses to run as root).
///
/// **Phase 2 -- start:** Issue `podman machine start`.  If the machine is
/// already running this is a harmless no-op (podman exits 0 in that case).
///
/// **Phase 3 -- monitor:** Poll machine state with adaptive intervals (1 s
/// initial, +10 % per check, 30 s cap).  When the machine is stable, checks
/// are infrequent (low CPU).  On state change the function returns and launchd
/// restarts the agent.
pub fn run() -> Result<()> {
    let bin = env_or("KONTENA_PODMAN_BIN", "podman");
    let machine = env_or("KONTENA_MACHINE_NAME", "podman-machine-default");

    info!(%machine, "podman start sequence beginning");

    ensure_machine_exists(&bin, &machine)?;
    start_machine(&bin, &machine)?;
    monitor_machine(&bin, &machine)?;

    info!(%machine, "podman monitor exiting (launchd will restart)");
    Ok(())
}

/// Ensure the named machine exists, creating it if necessary.
fn ensure_machine_exists(bin: &str, machine: &str) -> Result<()> {
    if machine_exists(bin, machine)? {
        info!(%machine, "machine already exists");
        return Ok(());
    }

    let cpus: u32 = env_parse("KONTENA_PODMAN_CPUS", 4)?;
    let memory: u32 = env_parse("KONTENA_PODMAN_MEMORY", 4096)?;
    let disk: u32 = env_parse("KONTENA_PODMAN_DISK", 60)?;
    let rootful = env_bool("KONTENA_PODMAN_ROOTFUL", false);

    let cpus_s = cpus.to_string();
    let memory_s = memory.to_string();
    let disk_s = disk.to_string();

    info!(%machine, cpus, memory, disk, rootful, "initializing podman machine");

    let mut args = vec![
        "machine", "init",
        "--cpus", &cpus_s,
        "--memory", &memory_s,
        "--disk-size", &disk_s,
    ];
    if rootful {
        args.push("--rootful");
    }

    match run_output(bin, &args) {
        Ok(stdout) => {
            info!(%machine, %stdout, "podman machine initialized");
            Ok(())
        }
        Err(e) => {
            // Race: machine may have appeared between check and init.
            if machine_exists(bin, machine)? {
                warn!(%machine, "init error but machine exists (race): {e:#}");
                Ok(())
            } else {
                Err(e).context("podman machine init failed")
            }
        }
    }
}

/// Issue `podman machine start`.
///
/// Podman exits 0 even if the machine is already running, so we treat any
/// non-zero exit as a real error.
fn start_machine(bin: &str, machine: &str) -> Result<()> {
    info!(%machine, "starting podman machine");

    match run_output(bin, &["machine", "start"]) {
        Ok(stdout) => {
            if !stdout.is_empty() {
                info!(%stdout, "podman machine start output");
            }
            info!(%machine, "podman machine started");
            Ok(())
        }
        Err(e) => {
            // `podman machine start` may fail if the machine is already
            // running.  Check state before bailing.
            warn!(%machine, "podman machine start returned error: {e:#}");
            let state = get_machine_state(bin, machine).unwrap_or_default();
            if state == "running" {
                info!(%machine, "machine is already running, continuing");
                Ok(())
            } else {
                Err(e).context("podman machine start failed")
            }
        }
    }
}

/// Poll the machine state with adaptive intervals.
///
/// Starts at 1 s and increases by 10 % each check up to 30 s.  Returns when
/// the machine is no longer in the "running" state so that launchd can restart
/// the agent.
fn monitor_machine(bin: &str, machine: &str) -> Result<()> {
    let mut interval = Duration::from_secs(1);
    let max_interval = Duration::from_secs(30);

    info!(%machine, "monitoring machine state");

    loop {
        std::thread::sleep(interval);

        let state = get_machine_state(bin, machine)?;

        if state != "running" {
            info!(%machine, %state, "machine is no longer running");
            return Ok(());
        }

        debug!(%machine, ?interval, "machine is running");

        // Adaptive: increase interval by 10 % each tick, cap at max.
        let next_secs = (interval.as_secs_f64() * 1.1).min(max_interval.as_secs_f64());
        interval = Duration::from_secs_f64(next_secs);
    }
}

/// Check whether a named podman machine exists.
fn machine_exists(bin: &str, machine: &str) -> Result<bool> {
    run_check(bin, &["machine", "inspect", machine])
}

/// Query the state of a machine via `podman machine inspect --format`.
fn get_machine_state(bin: &str, machine: &str) -> Result<String> {
    run_output(
        bin,
        &["machine", "inspect", machine, "--format", "{{.State}}"],
    )
    .with_context(|| format!("failed to inspect machine {machine}"))
}
