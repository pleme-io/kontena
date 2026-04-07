use tracing::info;

use crate::error::Error;
use crate::util::process::run_exec;
use crate::util::validate;
use crate::util::{env_or, env_parse};

/// Start colima in foreground mode.
///
/// Validates all configuration from environment variables, builds the argument
/// list, and then `exec`s into colima so that it replaces the current process.
/// Colima runs in `--foreground` mode — it blocks until stopped and launchd
/// restarts the agent when it exits.
///
/// # Errors
///
/// Returns an error if configuration is invalid or `exec` fails.
pub fn run() -> Result<(), Error> {
    let bin = env_or("KONTENA_COLIMA_BIN", "colima");
    let cpus: u32 = env_parse("KONTENA_COLIMA_CPUS", 4)?;
    let memory: u32 = env_parse("KONTENA_COLIMA_MEMORY", 8)?;
    let disk: u32 = env_parse("KONTENA_COLIMA_DISK", 60)?;
    let vm_type = env_or("KONTENA_COLIMA_VM_TYPE", "vz");
    let runtime = env_or("KONTENA_COLIMA_RUNTIME", "docker");
    let rosetta: bool = env_parse("KONTENA_COLIMA_ROSETTA", true)?;

    validate::range("cpus", cpus, 1, 256)?;
    validate::range("memory", memory, 1, 256)?;
    validate::range("disk", disk, 5, 2048)?;
    validate::one_of("vm_type", &vm_type, &["vz", "qemu"])?;
    validate::one_of("runtime", &runtime, &["docker", "containerd"])?;

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

    run_exec(&bin, &args)
}

#[cfg(test)]
mod tests {
    use crate::util::validate;

    #[test]
    fn colima_default_cpus_within_range() {
        assert!(validate::range("cpus", 4_u32, 1, 256).is_ok());
    }

    #[test]
    fn colima_default_memory_within_range() {
        assert!(validate::range("memory", 8_u32, 1, 256).is_ok());
    }

    #[test]
    fn colima_default_disk_within_range() {
        assert!(validate::range("disk", 60_u32, 5, 2048).is_ok());
    }

    #[test]
    fn colima_cpus_below_minimum() {
        let err = validate::range("cpus", 0_u32, 1, 256).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cpus"), "{msg}");
        assert!(msg.contains('0'), "{msg}");
    }

    #[test]
    fn colima_disk_above_maximum() {
        let err = validate::range("disk", 2049_u32, 5, 2048).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("disk"), "{msg}");
    }

    #[test]
    fn colima_valid_vm_types() {
        assert!(validate::one_of("vm_type", "vz", &["vz", "qemu"]).is_ok());
        assert!(validate::one_of("vm_type", "qemu", &["vz", "qemu"]).is_ok());
    }

    #[test]
    fn colima_invalid_runtime() {
        let err = validate::one_of("runtime", "podman", &["docker", "containerd"]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("podman"), "{msg}");
        assert!(msg.contains("docker"), "{msg}");
    }

    #[test]
    fn colima_vm_type_case_sensitive() {
        let err = validate::one_of("vm_type", "VZ", &["vz", "qemu"]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("VZ"), "{msg}");
    }
}
