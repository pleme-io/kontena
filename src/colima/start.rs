use tracing::info;

use crate::error::Error;
use crate::util::process::{CommandRunner, SystemCommandRunner};
use crate::util::validate;
use crate::util::{env_or, env_parse};

/// Configuration for colima start.
#[derive(Debug, Clone)]
pub(crate) struct ColimaConfig {
    pub bin: String,
    pub cpus: u32,
    pub memory: u32,
    pub disk: u32,
    pub vm_type: String,
    pub runtime: String,
    pub rosetta: bool,
}

impl ColimaConfig {
    /// Build a [`ColimaConfig`] from environment variables.
    fn from_env() -> Result<Self, Error> {
        Ok(Self {
            bin: env_or("KONTENA_COLIMA_BIN", "colima"),
            cpus: env_parse("KONTENA_COLIMA_CPUS", 4)?,
            memory: env_parse("KONTENA_COLIMA_MEMORY", 8)?,
            disk: env_parse("KONTENA_COLIMA_DISK", 60)?,
            vm_type: env_or("KONTENA_COLIMA_VM_TYPE", "vz"),
            runtime: env_or("KONTENA_COLIMA_RUNTIME", "docker"),
            rosetta: env_parse("KONTENA_COLIMA_ROSETTA", true)?,
        })
    }

    /// Validate all config values against their allowed ranges / enum sets.
    fn validate(&self) -> Result<(), Error> {
        validate::range("cpus", &self.cpus, &1, &256)?;
        validate::range("memory", &self.memory, &1, &256)?;
        validate::range("disk", &self.disk, &5, &2048)?;
        validate::one_of("vm_type", &self.vm_type, &["vz", "qemu"])?;
        validate::one_of("runtime", &self.runtime, &["docker", "containerd"])?;
        Ok(())
    }

    /// Build the argument list for the colima start command.
    #[must_use]
    pub(crate) fn build_args(&self) -> Vec<String> {
        let mut args = vec![
            "start".to_owned(),
            "--cpu".to_owned(),
            self.cpus.to_string(),
            "--memory".to_owned(),
            self.memory.to_string(),
            "--disk".to_owned(),
            self.disk.to_string(),
            "--vm-type".to_owned(),
            self.vm_type.clone(),
            "--runtime".to_owned(),
            self.runtime.clone(),
            "--foreground".to_owned(),
        ];

        if self.rosetta && self.vm_type == "vz" {
            args.push("--vz-rosetta".to_owned());
        }

        args
    }
}

/// Start colima in foreground mode (production entry point).
///
/// # Errors
///
/// Returns an error if configuration is invalid or `exec` fails.
pub fn run() -> Result<(), Error> {
    let config = ColimaConfig::from_env()?;
    run_with(&SystemCommandRunner, &config)
}

/// Core colima start logic, decoupled from I/O for testability.
pub(crate) fn run_with(runner: &dyn CommandRunner, config: &ColimaConfig) -> Result<(), Error> {
    config.validate()?;

    info!(
        cpus = config.cpus,
        memory = config.memory,
        disk = config.disk,
        vm_type = %config.vm_type,
        runtime = %config.runtime,
        rosetta = config.rosetta,
        "starting colima"
    );

    let args = config.build_args();
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();

    runner.run_exec(&config.bin, &arg_refs)
}

#[cfg(test)]
mod tests {
    use crate::util::mock::testing::{MockCommandRunner, MockResponse};
    use crate::util::validate;

    use super::*;

    fn default_config() -> ColimaConfig {
        ColimaConfig {
            bin: "colima".into(),
            cpus: 4,
            memory: 8,
            disk: 60,
            vm_type: "vz".into(),
            runtime: "docker".into(),
            rosetta: true,
        }
    }

    // --- validation tests ---

    #[test]
    fn colima_default_cpus_within_range() {
        assert!(validate::range("cpus", &4_u32, &1, &256).is_ok());
    }

    #[test]
    fn colima_default_memory_within_range() {
        assert!(validate::range("memory", &8_u32, &1, &256).is_ok());
    }

    #[test]
    fn colima_default_disk_within_range() {
        assert!(validate::range("disk", &60_u32, &5, &2048).is_ok());
    }

    #[test]
    fn colima_cpus_below_minimum() {
        let err = validate::range("cpus", &0_u32, &1, &256).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cpus"), "{msg}");
        assert!(msg.contains('0'), "{msg}");
    }

    #[test]
    fn colima_disk_above_maximum() {
        let err = validate::range("disk", &2049_u32, &5, &2048).unwrap_err();
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

    // --- config tests ---

    #[test]
    fn config_validate_accepts_defaults() {
        assert!(default_config().validate().is_ok());
    }

    #[test]
    fn config_validate_rejects_bad_vm_type() {
        let mut config = default_config();
        config.vm_type = "hyperv".into();
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_validate_rejects_bad_runtime() {
        let mut config = default_config();
        config.runtime = "podman".into();
        assert!(config.validate().is_err());
    }

    // --- build_args tests ---

    #[test]
    fn build_args_includes_rosetta_for_vz() {
        let args = default_config().build_args();
        assert!(args.contains(&"--vz-rosetta".to_owned()));
    }

    #[test]
    fn build_args_excludes_rosetta_for_qemu() {
        let mut config = default_config();
        config.vm_type = "qemu".into();
        let args = config.build_args();
        assert!(!args.contains(&"--vz-rosetta".to_owned()));
    }

    #[test]
    fn build_args_excludes_rosetta_when_disabled() {
        let mut config = default_config();
        config.rosetta = false;
        let args = config.build_args();
        assert!(!args.contains(&"--vz-rosetta".to_owned()));
    }

    #[test]
    fn build_args_contains_foreground() {
        let mut config = default_config();
        config.rosetta = false;
        config.runtime = "containerd".into();
        let args = config.build_args();
        assert!(args.contains(&"--foreground".to_owned()));
        assert!(args.contains(&"--runtime".to_owned()));
    }

    #[test]
    fn build_args_contains_correct_values() {
        let mut config = default_config();
        config.cpus = 2;
        config.memory = 16;
        config.disk = 100;
        let args = config.build_args();
        assert!(args.contains(&"2".to_owned()));
        assert!(args.contains(&"16".to_owned()));
        assert!(args.contains(&"100".to_owned()));
    }

    // --- mock-based lifecycle tests ---

    #[test]
    fn run_with_execs_colima() {
        let config = default_config();
        let mock = MockCommandRunner::new(vec![MockResponse::ExecOk]);

        let result = run_with(&mock, &config);
        assert!(result.is_ok());

        let calls = mock.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].bin, "colima");
        assert!(calls[0].args.contains(&"start".to_owned()));
        assert!(calls[0].args.contains(&"--foreground".to_owned()));
    }

    #[test]
    fn run_with_rejects_invalid_config() {
        let mut config = default_config();
        config.cpus = 0;

        let mock = MockCommandRunner::new(vec![]);

        let result = run_with(&mock, &config);
        assert!(result.is_err());
        assert!(mock.calls().is_empty());
    }

    #[test]
    fn run_with_propagates_exec_error() {
        let config = default_config();
        let mock = MockCommandRunner::new(vec![
            MockResponse::Err(Error::Exec {
                bin: "colima".into(),
                reason: "not found".into(),
            }),
        ]);

        let err = run_with(&mock, &config).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("colima"), "{msg}");
    }

    #[test]
    fn run_with_passes_rosetta_flag() {
        let config = default_config();
        let mock = MockCommandRunner::new(vec![MockResponse::ExecOk]);

        run_with(&mock, &config).unwrap();

        let calls = mock.calls();
        assert!(calls[0].args.contains(&"--vz-rosetta".to_owned()));
    }

    #[test]
    fn run_with_omits_rosetta_for_qemu() {
        let mut config = default_config();
        config.vm_type = "qemu".into();

        let mock = MockCommandRunner::new(vec![MockResponse::ExecOk]);

        run_with(&mock, &config).unwrap();

        let calls = mock.calls();
        assert!(!calls[0].args.contains(&"--vz-rosetta".to_owned()));
    }
}
