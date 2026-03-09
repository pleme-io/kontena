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
