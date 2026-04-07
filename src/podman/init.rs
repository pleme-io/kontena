use tracing::{info, warn};

use crate::error::Error;
use crate::util::process::{run_check, run_output};
use crate::util::validate;
use crate::util::{env_bool, env_or, env_parse};

/// Idempotent podman machine initialisation.
///
/// Checks whether the named machine already exists and, if not, creates it with
/// the configured CPU/memory/disk parameters.  Intended to run as a
/// `nix-darwin` activation script replacement.
///
/// # Errors
///
/// Returns an error if configuration is invalid, subprocess execution fails,
/// or machine init fails and the machine still does not exist.
pub fn run() -> Result<(), Error> {
    let bin = env_or("KONTENA_PODMAN_BIN", "podman");
    let cpus: u32 = env_parse("KONTENA_PODMAN_CPUS", 4)?;
    let memory: u32 = env_parse("KONTENA_PODMAN_MEMORY", 4096)?;
    let disk: u32 = env_parse("KONTENA_PODMAN_DISK", 60)?;
    let machine = env_or("KONTENA_MACHINE_NAME", "podman-machine-default");
    let rootful = env_bool("KONTENA_PODMAN_ROOTFUL", false);

    validate::range("cpus", cpus, 1, 256)?;
    validate::range("memory", memory, 512, 131_072)?;
    validate::range("disk", disk, 10, 2048)?;

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
                warn!(%machine, "init returned error but machine exists (race): {e}");
                Ok(())
            } else {
                Err(Error::MachineInit {
                    reason: e.to_string(),
                })
            }
        }
    }
}

/// Check whether a named podman machine exists by inspecting it.
fn machine_exists(bin: &str, machine: &str) -> Result<bool, Error> {
    run_check(bin, &["machine", "inspect", machine])
}

#[cfg(test)]
mod tests {
    use crate::util::validate;

    #[test]
    fn podman_default_cpus_within_range() {
        assert!(validate::range("cpus", 4_u32, 1, 256).is_ok());
    }

    #[test]
    fn podman_default_memory_within_range() {
        assert!(validate::range("memory", 4096_u32, 512, 131_072).is_ok());
    }

    #[test]
    fn podman_default_disk_within_range() {
        assert!(validate::range("disk", 60_u32, 10, 2048).is_ok());
    }

    #[test]
    fn podman_memory_below_minimum() {
        let err = validate::range("memory", 256_u32, 512, 131_072).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("memory"), "{msg}");
        assert!(msg.contains("256"), "{msg}");
    }

    #[test]
    fn podman_disk_above_maximum() {
        let err = validate::range("disk", 3000_u32, 10, 2048).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("disk"), "{msg}");
        assert!(msg.contains("3000"), "{msg}");
    }
}
