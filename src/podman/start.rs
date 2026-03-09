use std::time::Duration;

use anyhow::{Context, Result};
use tracing::{debug, info, warn};

use crate::util::backoff::ExponentialBackoff;
use crate::util::process::{run_check, run_output};
use crate::util::env_or;

/// Start the podman machine and monitor it until it stops.
///
/// **Phase 1 -- wait:** The activation script that initialises the machine may
/// still be running when launchd starts this agent.  We use exponential backoff
/// (100 ms initial, x1.5, 5 s cap, 30 attempts) to discover the machine
/// quickly once it appears.
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

    wait_for_machine(&bin, &machine)?;
    start_machine(&bin)?;
    monitor_machine(&bin, &machine)?;

    info!(%machine, "podman monitor exiting (launchd will restart)");
    Ok(())
}

/// Block until the named machine exists, using exponential backoff.
fn wait_for_machine(bin: &str, machine: &str) -> Result<()> {
    // Fast path: machine already exists.
    if machine_exists(bin, machine)? {
        info!(%machine, "machine already exists");
        return Ok(());
    }

    info!(%machine, "waiting for machine to appear");

    let mut backoff = ExponentialBackoff::new(
        Duration::from_millis(100),
        1.5,
        Duration::from_secs(5),
        30,
    );

    loop {
        match backoff.next_delay() {
            Some(delay) => {
                debug!(?delay, attempt = backoff.attempts(), "sleeping before next check");
                std::thread::sleep(delay);
            }
            None => {
                anyhow::bail!(
                    "machine {machine} did not appear after {} attempts",
                    backoff.attempts()
                );
            }
        }

        if machine_exists(bin, machine)? {
            info!(
                %machine,
                attempts = backoff.attempts(),
                "machine appeared"
            );
            return Ok(());
        }
    }
}

/// Issue `podman machine start`.
///
/// Podman exits 0 even if the machine is already running, so we treat any
/// non-zero exit as a real error.
fn start_machine(bin: &str) -> Result<()> {
    info!("starting podman machine");

    match run_output(bin, &["machine", "start"]) {
        Ok(stdout) => {
            if !stdout.is_empty() {
                info!(%stdout, "podman machine start output");
            }
            info!("podman machine started");
            Ok(())
        }
        Err(e) => {
            // `podman machine start` may fail if the machine is already
            // running.  Check state before bailing.
            warn!("podman machine start returned error: {e:#}");
            let state = get_machine_state(bin, "podman-machine-default")
                .unwrap_or_default();
            if state == "running" {
                info!("machine is already running, continuing");
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
