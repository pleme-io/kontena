/// Errors produced by the kontena daemon.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A numeric configuration value is outside the acceptable range.
    #[error("{name}={value} is out of range [{min}, {max}]")]
    OutOfRange {
        name: String,
        value: String,
        min: String,
        max: String,
    },

    /// An enum-style configuration value is not one of the allowed variants.
    #[error("{name}={value:?} is not one of [{allowed}]")]
    InvalidEnum {
        name: String,
        value: String,
        allowed: String,
    },

    /// An environment variable is set but cannot be parsed.
    #[error("{key}={value:?} is not valid: {reason}")]
    EnvParse {
        key: String,
        value: String,
        reason: String,
    },

    /// An environment variable cannot be read (e.g. invalid UTF-8).
    #[error("cannot read {key}: {reason}")]
    EnvRead { key: String, reason: String },

    /// A subprocess failed to launch.
    #[error("failed to execute {bin}: {reason}")]
    Spawn { bin: String, reason: String },

    /// A subprocess exited with a non-zero status.
    #[error("{bin} exited with {status}: {stderr}")]
    NonZeroExit {
        bin: String,
        status: String,
        stderr: String,
    },

    /// `exec` failed (unix only).
    #[error("exec({bin}) failed: {reason}")]
    Exec { bin: String, reason: String },

    /// Podman machine init failed after retry.
    #[error("podman machine init failed: {reason}")]
    MachineInit { reason: String },

    /// Podman machine start failed.
    #[error("podman machine start failed: {reason}")]
    MachineStart { reason: String },

    /// Podman machine inspect failed.
    #[error("failed to inspect machine {machine}: {reason}")]
    MachineInspect { machine: String, reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_of_range_display() {
        let err = Error::OutOfRange {
            name: "cpus".into(),
            value: "0".into(),
            min: "1".into(),
            max: "256".into(),
        };
        let msg = err.to_string();
        assert_eq!(msg, "cpus=0 is out of range [1, 256]");
    }

    #[test]
    fn invalid_enum_display() {
        let err = Error::InvalidEnum {
            name: "vm_type".into(),
            value: "hyperv".into(),
            allowed: "\"vz\", \"qemu\"".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("hyperv"), "{msg}");
        assert!(msg.contains("not one of"), "{msg}");
    }

    #[test]
    fn env_parse_display() {
        let err = Error::EnvParse {
            key: "MY_VAR".into(),
            value: "abc".into(),
            reason: "invalid digit".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("MY_VAR"), "{msg}");
        assert!(msg.contains("abc"), "{msg}");
        assert!(msg.contains("invalid digit"), "{msg}");
    }

    #[test]
    fn spawn_display() {
        let err = Error::Spawn {
            bin: "podman".into(),
            reason: "No such file".into(),
        };
        let msg = err.to_string();
        assert_eq!(msg, "failed to execute podman: No such file");
    }

    #[test]
    fn non_zero_exit_display() {
        let err = Error::NonZeroExit {
            bin: "podman".into(),
            status: "exit status: 1".into(),
            stderr: "error msg".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("podman"), "{msg}");
        assert!(msg.contains("exit status: 1"), "{msg}");
        assert!(msg.contains("error msg"), "{msg}");
    }

    #[test]
    fn exec_display() {
        let err = Error::Exec {
            bin: "colima".into(),
            reason: "not found".into(),
        };
        assert_eq!(err.to_string(), "exec(colima) failed: not found");
    }

    #[test]
    fn machine_init_display() {
        let err = Error::MachineInit {
            reason: "disk full".into(),
        };
        assert_eq!(err.to_string(), "podman machine init failed: disk full");
    }

    #[test]
    fn machine_start_display() {
        let err = Error::MachineStart {
            reason: "timeout".into(),
        };
        assert_eq!(err.to_string(), "podman machine start failed: timeout");
    }

    #[test]
    fn machine_inspect_display() {
        let err = Error::MachineInspect {
            machine: "my-machine".into(),
            reason: "not found".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("my-machine"), "{msg}");
        assert!(msg.contains("not found"), "{msg}");
    }

    #[test]
    fn env_read_display() {
        let err = Error::EnvRead {
            key: "BAD_KEY".into(),
            reason: "invalid utf-8".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("BAD_KEY"), "{msg}");
        assert!(msg.contains("invalid utf-8"), "{msg}");
    }

    #[test]
    fn error_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Error>();
    }

    #[test]
    fn error_clone_preserves_variant() {
        let err = Error::OutOfRange {
            name: "cpus".into(),
            value: "0".into(),
            min: "1".into(),
            max: "256".into(),
        };
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn error_debug_is_not_empty() {
        let err = Error::Spawn {
            bin: "podman".into(),
            reason: "not found".into(),
        };
        let debug = format!("{err:?}");
        assert!(!debug.is_empty());
        assert!(debug.contains("Spawn"));
    }
}
