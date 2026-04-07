use anyhow::Result;
use tracing::info;

use crate::util::process::run_exec;
use crate::util::{env_or, env_parse};

/// Start colima in foreground mode.
///
/// Validates all configuration from environment variables, builds the argument
/// list, and then `exec`s into colima so that it replaces the current process.
/// Colima runs in `--foreground` mode -- it blocks until stopped and launchd
/// restarts the agent when it exits.
pub fn run() -> Result<()> {
    let bin = env_or("KONTENA_COLIMA_BIN", "colima");
    let cpus: u32 = env_parse("KONTENA_COLIMA_CPUS", 4)?;
    let memory: u32 = env_parse("KONTENA_COLIMA_MEMORY", 8)?;
    let disk: u32 = env_parse("KONTENA_COLIMA_DISK", 60)?;
    let vm_type = env_or("KONTENA_COLIMA_VM_TYPE", "vz");
    let runtime = env_or("KONTENA_COLIMA_RUNTIME", "docker");
    let rosetta: bool = env_parse("KONTENA_COLIMA_ROSETTA", true)?;

    validate_range("cpus", cpus, 1, 256)?;
    validate_range("memory", memory, 1, 256)?;
    validate_range("disk", disk, 5, 2048)?;
    validate_enum("vm_type", &vm_type, &["vz", "qemu"])?;
    validate_enum("runtime", &runtime, &["docker", "containerd"])?;

    info!(
        cpus, memory, disk,
        %vm_type, %runtime, rosetta,
        "starting colima"
    );

    let cpus_s = cpus.to_string();
    let memory_s = memory.to_string();
    let disk_s = disk.to_string();

    let mut args: Vec<&str> = vec![
        "start",
        "--cpu",
        &cpus_s,
        "--memory",
        &memory_s,
        "--disk",
        &disk_s,
        "--vm-type",
        &vm_type,
        "--runtime",
        &runtime,
        "--foreground",
    ];

    if rosetta && vm_type == "vz" {
        args.push("--vz-rosetta");
    }

    // exec replaces this process -- colima runs in foreground.
    run_exec(&bin, &args)
}

fn validate_range(name: &str, value: u32, min: u32, max: u32) -> Result<()> {
    if value < min || value > max {
        anyhow::bail!("{name}={value} is out of range [{min}, {max}]");
    }
    Ok(())
}

fn validate_enum(name: &str, value: &str, allowed: &[&str]) -> Result<()> {
    if !allowed.contains(&value) {
        anyhow::bail!(
            "{name}={value:?} is not one of [{}]",
            allowed
                .iter()
                .map(|s| format!("{s:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_range_accepts_minimum_boundary() {
        assert!(validate_range("cpus", 1, 1, 256).is_ok());
    }

    #[test]
    fn validate_range_accepts_maximum_boundary() {
        assert!(validate_range("cpus", 256, 1, 256).is_ok());
    }

    #[test]
    fn validate_range_accepts_mid_value() {
        assert!(validate_range("memory", 8, 1, 256).is_ok());
    }

    #[test]
    fn validate_range_rejects_below_minimum() {
        let err = validate_range("cpus", 0, 1, 256).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("cpus=0"), "error should identify field and value: {msg}");
        assert!(msg.contains("out of range"), "{msg}");
    }

    #[test]
    fn validate_range_rejects_above_maximum() {
        let err = validate_range("disk", 2049, 5, 2048).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("disk=2049"), "{msg}");
    }

    #[test]
    fn validate_enum_accepts_valid_value() {
        assert!(validate_enum("vm_type", "vz", &["vz", "qemu"]).is_ok());
        assert!(validate_enum("vm_type", "qemu", &["vz", "qemu"]).is_ok());
    }

    #[test]
    fn validate_enum_rejects_invalid_value() {
        let err = validate_enum("runtime", "podman", &["docker", "containerd"]).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("podman"), "error should mention the invalid value: {msg}");
        assert!(msg.contains("docker"), "error should list allowed values: {msg}");
    }

    #[test]
    fn validate_enum_rejects_empty_string() {
        let err = validate_enum("vm_type", "", &["vz", "qemu"]).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("not one of"), "{msg}");
    }

    #[test]
    fn validate_enum_case_sensitive() {
        let err = validate_enum("vm_type", "VZ", &["vz", "qemu"]).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("VZ"), "{msg}");
    }

    #[test]
    fn validate_enum_single_allowed_value() {
        assert!(validate_enum("x", "only", &["only"]).is_ok());
        assert!(validate_enum("x", "other", &["only"]).is_err());
    }
}
