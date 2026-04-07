use tracing::{info, warn};

use crate::error::Error;
use crate::util::process::{CommandRunner, SystemCommandRunner};
use crate::util::validate;
use crate::util::{env_bool, env_or, env_parse};

/// Configuration for podman machine initialisation.
#[derive(Debug, Clone)]
pub(crate) struct InitConfig {
    pub bin: String,
    pub cpus: u32,
    pub memory: u32,
    pub disk: u32,
    pub machine: String,
    pub rootful: bool,
}

impl InitConfig {
    /// Build an [`InitConfig`] from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error if any numeric env var is set but unparseable.
    fn from_env() -> Result<Self, Error> {
        Ok(Self {
            bin: env_or("KONTENA_PODMAN_BIN", "podman"),
            cpus: env_parse("KONTENA_PODMAN_CPUS", 4)?,
            memory: env_parse("KONTENA_PODMAN_MEMORY", 4096)?,
            disk: env_parse("KONTENA_PODMAN_DISK", 60)?,
            machine: env_or("KONTENA_MACHINE_NAME", "podman-machine-default"),
            rootful: env_bool("KONTENA_PODMAN_ROOTFUL", false),
        })
    }

    /// Validate all config values against their allowed ranges.
    fn validate(&self) -> Result<(), Error> {
        validate::range("cpus", self.cpus, 1, 256)?;
        validate::range("memory", self.memory, 512, 131_072)?;
        validate::range("disk", self.disk, 10, 2048)?;
        Ok(())
    }
}

/// Idempotent podman machine initialisation (production entry point).
///
/// Reads configuration from environment variables and runs the init sequence
/// using real subprocess calls.
///
/// # Errors
///
/// Returns an error if configuration is invalid, subprocess execution fails,
/// or machine init fails and the machine still does not exist.
pub fn run() -> Result<(), Error> {
    let config = InitConfig::from_env()?;
    run_with(&SystemCommandRunner, &config)
}

/// Core init logic, decoupled from I/O for testability.
pub(crate) fn run_with(runner: &dyn CommandRunner, config: &InitConfig) -> Result<(), Error> {
    config.validate()?;

    if machine_exists(runner, &config.bin, &config.machine)? {
        info!(machine = %config.machine, "machine already exists, skipping init");
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
                warn!(machine = %config.machine, "init returned error but machine exists (race): {e}");
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
fn machine_exists(runner: &dyn CommandRunner, bin: &str, machine: &str) -> Result<bool, Error> {
    runner.run_check(bin, &["machine", "inspect", machine])
}

#[cfg(test)]
mod tests {
    use crate::util::validate;

    use super::*;

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

    #[test]
    fn config_validate_accepts_defaults() {
        let config = InitConfig {
            bin: "podman".into(),
            cpus: 4,
            memory: 4096,
            disk: 60,
            machine: "default".into(),
            rootful: false,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn config_validate_rejects_zero_cpus() {
        let config = InitConfig {
            bin: "podman".into(),
            cpus: 0,
            memory: 4096,
            disk: 60,
            machine: "default".into(),
            rootful: false,
        };
        assert!(config.validate().is_err());
    }
}
