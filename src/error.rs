/// Errors produced by the kontena daemon.
#[derive(Debug, Clone, thiserror::Error)]
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
