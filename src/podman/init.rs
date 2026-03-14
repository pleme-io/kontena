use anyhow::{Context, Result};
use tracing::{info, warn};

use crate::util::process::{run_check, run_output};
use crate::util::{env_bool, env_or, env_parse};

/// Idempotent podman machine initialisation.
///
/// Checks whether the named machine already exists and, if not, creates it with
/// the configured CPU/memory/disk parameters.  Intended to run as a
/// `nix-darwin` activation script replacement.
pub fn run() -> Result<()> {
    let bin = env_or("KONTENA_PODMAN_BIN", "podman");
    let cpus: u32 = env_parse("KONTENA_PODMAN_CPUS", 4)?;
    let memory: u32 = env_parse("KONTENA_PODMAN_MEMORY", 4096)?;
    let disk: u32 = env_parse("KONTENA_PODMAN_DISK", 60)?;
    let machine = env_or("KONTENA_MACHINE_NAME", "podman-machine-default");
    let rootful = env_bool("KONTENA_PODMAN_ROOTFUL", false);

    validate_range("cpus", cpus, 1, 256)?;
    validate_range("memory", memory, 512, 131_072)?;
    validate_range("disk", disk, 10, 2048)?;

    if machine_exists(&bin, &machine)? {
        info!(%machine, "machine already exists, skipping init");
        return Ok(());
    }

    let cpus_s = cpus.to_string();
    let memory_s = memory.to_string();
    let disk_s = disk.to_string();

    info!(
        %machine, cpus, memory, disk, rootful,
        "initializing podman machine"
    );

    let mut args = vec![
        "machine", "init",
        "--cpus", &cpus_s,
        "--memory", &memory_s,
        "--disk-size", &disk_s,
    ];
    if rootful {
        args.push("--rootful");
    }

    let output = run_output(&bin, &args);

    match output {
        Ok(stdout) => {
            info!(%machine, %stdout, "podman machine initialized");
            Ok(())
        }
        Err(e) => {
            // Podman sometimes races with itself — if the machine appeared
            // between our check and the init call, treat it as success.
            if machine_exists(&bin, &machine)? {
                warn!(%machine, "init returned error but machine exists (race): {e:#}");
                Ok(())
            } else {
                Err(e).context("podman machine init failed")
            }
        }
    }
}

/// Check whether a named podman machine exists by inspecting it.
fn machine_exists(bin: &str, machine: &str) -> Result<bool> {
    run_check(bin, &["machine", "inspect", machine])
}

fn validate_range(name: &str, value: u32, min: u32, max: u32) -> Result<()> {
    if value < min || value > max {
        anyhow::bail!("{name}={value} is out of range [{min}, {max}]");
    }
    Ok(())
}
