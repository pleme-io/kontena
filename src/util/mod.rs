pub mod backoff;
pub mod process;

use std::env;

/// Read an environment variable or return a default value.
pub fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}

/// Read a boolean environment variable (true/false/1/0), or return a default.
pub fn env_bool(key: &str, default: bool) -> bool {
    match env::var(key) {
        Ok(val) => matches!(val.as_str(), "true" | "1" | "yes"),
        Err(_) => default,
    }
}

/// Read an environment variable and parse it, or return a default value.
///
/// # Errors
///
/// Returns an error if the variable is set but cannot be parsed.
pub fn env_parse<T>(key: &str, default: T) -> anyhow::Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(key) {
        Ok(val) => val
            .parse::<T>()
            .map_err(|e| anyhow::anyhow!("{key}={val:?} is not valid: {e}")),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(e) => Err(anyhow::anyhow!("cannot read {key}: {e}")),
    }
}
