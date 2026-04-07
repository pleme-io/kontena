pub mod backoff;
pub(crate) mod mock;
pub mod process;
pub mod validate;

use std::env;

use crate::error::Error;

/// Read an environment variable or return a default value.
#[must_use]
pub fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}

/// Read a boolean environment variable (true/false/1/0), or return a default.
#[must_use]
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
/// Returns [`Error::EnvParse`] if the variable is set but cannot be parsed.
/// Returns [`Error::EnvRead`] if the variable cannot be read (e.g. invalid UTF-8).
pub fn env_parse<T>(key: &str, default: T) -> Result<T, Error>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(key) {
        Ok(val) => val.parse::<T>().map_err(|e| Error::EnvParse {
            key: key.to_owned(),
            value: val,
            reason: e.to_string(),
        }),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(e) => Err(Error::EnvRead {
            key: key.to_owned(),
            reason: e.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn set(key: &str, val: &str) {
        unsafe { env::set_var(key, val) };
    }

    unsafe fn unset(key: &str) {
        unsafe { env::remove_var(key) };
    }

    #[test]
    fn env_or_returns_value_when_set() {
        unsafe { set("TEST_ENV_OR_SET", "hello") };
        assert_eq!(env_or("TEST_ENV_OR_SET", "fallback"), "hello");
        unsafe { unset("TEST_ENV_OR_SET") };
    }

    #[test]
    fn env_or_returns_default_when_unset() {
        unsafe { unset("TEST_ENV_OR_UNSET") };
        assert_eq!(env_or("TEST_ENV_OR_UNSET", "fallback"), "fallback");
    }

    #[test]
    fn env_or_returns_empty_string_when_set_empty() {
        unsafe { set("TEST_ENV_OR_EMPTY", "") };
        assert_eq!(env_or("TEST_ENV_OR_EMPTY", "fallback"), "");
        unsafe { unset("TEST_ENV_OR_EMPTY") };
    }

    #[test]
    fn env_bool_recognises_true_variants() {
        for (i, val) in ["true", "1", "yes"].iter().enumerate() {
            let key = format!("TEST_ENV_BOOL_TRUE_{i}");
            unsafe { set(&key, val) };
            assert!(env_bool(&key, false), "expected true for {val:?}");
            unsafe { unset(&key) };
        }
    }

    #[test]
    fn env_bool_rejects_non_truthy_strings() {
        for (i, val) in ["false", "0", "no", "TRUE", "Yes", ""].iter().enumerate() {
            let key = format!("TEST_ENV_BOOL_FALSE_{i}");
            unsafe { set(&key, val) };
            assert!(!env_bool(&key, true), "expected false for {val:?}");
            unsafe { unset(&key) };
        }
    }

    #[test]
    fn env_bool_returns_default_when_unset() {
        unsafe { unset("TEST_ENV_BOOL_UNSET_T") };
        unsafe { unset("TEST_ENV_BOOL_UNSET_F") };
        assert!(env_bool("TEST_ENV_BOOL_UNSET_T", true));
        assert!(!env_bool("TEST_ENV_BOOL_UNSET_F", false));
    }

    #[test]
    fn env_parse_returns_parsed_value() {
        unsafe { set("TEST_ENV_PARSE_OK", "42") };
        let v: u32 = env_parse("TEST_ENV_PARSE_OK", 0).unwrap();
        assert_eq!(v, 42);
        unsafe { unset("TEST_ENV_PARSE_OK") };
    }

    #[test]
    fn env_parse_returns_default_when_unset() {
        unsafe { unset("TEST_ENV_PARSE_UNSET") };
        let v: u32 = env_parse("TEST_ENV_PARSE_UNSET", 99).unwrap();
        assert_eq!(v, 99);
    }

    #[test]
    fn env_parse_errors_on_invalid_value() {
        unsafe { set("TEST_ENV_PARSE_BAD", "not_a_number") };
        let result: Result<u32, _> = env_parse("TEST_ENV_PARSE_BAD", 0);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("not_a_number"), "error should mention the bad value: {msg}");
        unsafe { unset("TEST_ENV_PARSE_BAD") };
    }

    #[test]
    fn env_parse_handles_negative_for_unsigned() {
        unsafe { set("TEST_ENV_PARSE_NEG", "-1") };
        let result: Result<u32, _> = env_parse("TEST_ENV_PARSE_NEG", 0);
        assert!(result.is_err());
        unsafe { unset("TEST_ENV_PARSE_NEG") };
    }

    #[test]
    fn env_parse_handles_overflow() {
        unsafe { set("TEST_ENV_PARSE_OVER", "999999999999999") };
        let result: Result<u32, _> = env_parse("TEST_ENV_PARSE_OVER", 0);
        assert!(result.is_err());
        unsafe { unset("TEST_ENV_PARSE_OVER") };
    }

    #[test]
    fn env_parse_handles_bool_type() {
        unsafe { set("TEST_ENV_PARSE_BOOL", "true") };
        let v: bool = env_parse("TEST_ENV_PARSE_BOOL", false).unwrap();
        assert!(v);
        unsafe { unset("TEST_ENV_PARSE_BOOL") };
    }

    #[test]
    fn env_parse_error_is_env_parse_variant() {
        unsafe { set("TEST_ENV_PARSE_TYPED", "abc") };
        let err = env_parse::<u32>("TEST_ENV_PARSE_TYPED", 0).unwrap_err();
        assert!(matches!(err, Error::EnvParse { .. }));
        unsafe { unset("TEST_ENV_PARSE_TYPED") };
    }

    #[test]
    fn env_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Error>();
    }
}
