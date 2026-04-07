use std::fmt;

use crate::error::Error;

/// Validate that `value` falls within `[min, max]` (inclusive).
///
/// # Errors
///
/// Returns [`Error::OutOfRange`] when the value is outside the bounds.
#[must_use = "validation result must be checked"]
pub fn range<T: PartialOrd + fmt::Display>(name: &str, value: &T, min: &T, max: &T) -> Result<(), Error> {
    if value < min || value > max {
        return Err(Error::OutOfRange {
            name: name.to_owned(),
            value: value.to_string(),
            min: min.to_string(),
            max: max.to_string(),
        });
    }
    Ok(())
}

/// Validate that `value` is one of `allowed`.
///
/// # Errors
///
/// Returns [`Error::InvalidEnum`] when the value is not found in the list.
#[must_use = "validation result must be checked"]
pub fn one_of(name: &str, value: &str, allowed: &[&str]) -> Result<(), Error> {
    if !allowed.contains(&value) {
        return Err(Error::InvalidEnum {
            name: name.to_owned(),
            value: value.to_owned(),
            allowed: format_quoted_list(allowed),
        });
    }
    Ok(())
}

/// Format a slice of strings as a quoted, comma-separated list.
#[must_use]
fn format_quoted_list(items: &[&str]) -> String {
    items
        .iter()
        .map(|s| format!("{s:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_accepts_boundaries() {
        assert!(range("cpus", &1_u32, &1, &256).is_ok());
        assert!(range("cpus", &256_u32, &1, &256).is_ok());
    }

    #[test]
    fn range_accepts_mid_value() {
        assert!(range("memory", &8_u32, &1, &256).is_ok());
    }

    #[test]
    fn range_rejects_below_minimum() {
        let err = range("cpus", &0_u32, &1, &256).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cpus"), "{msg}");
        assert!(msg.contains('0'), "{msg}");
    }

    #[test]
    fn range_rejects_above_maximum() {
        let err = range("disk", &3000_u32, &10, &2048).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("disk"), "{msg}");
        assert!(msg.contains("3000"), "{msg}");
    }

    #[test]
    fn range_min_equals_max() {
        assert!(range("x", &5_u32, &5, &5).is_ok());
        assert!(range("x", &4_u32, &5, &5).is_err());
        assert!(range("x", &6_u32, &5, &5).is_err());
    }

    #[test]
    fn range_works_with_f64() {
        assert!(range("ratio", &0.5_f64, &0.0, &1.0).is_ok());
        assert!(range("ratio", &1.5_f64, &0.0, &1.0).is_err());
    }

    #[test]
    fn one_of_accepts_valid_value() {
        assert!(one_of("vm_type", "vz", &["vz", "qemu"]).is_ok());
        assert!(one_of("vm_type", "qemu", &["vz", "qemu"]).is_ok());
    }

    #[test]
    fn one_of_rejects_invalid_value() {
        let err = one_of("runtime", "podman", &["docker", "containerd"]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("podman"), "{msg}");
        assert!(msg.contains("docker"), "{msg}");
    }

    #[test]
    fn one_of_rejects_empty_string() {
        let err = one_of("vm_type", "", &["vz", "qemu"]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not one of"), "{msg}");
    }

    #[test]
    fn one_of_case_sensitive() {
        let err = one_of("vm_type", "VZ", &["vz", "qemu"]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("VZ"), "{msg}");
    }

    #[test]
    fn one_of_single_allowed_value() {
        assert!(one_of("x", "only", &["only"]).is_ok());
        assert!(one_of("x", "other", &["only"]).is_err());
    }

    #[test]
    fn format_quoted_list_formats_correctly() {
        assert_eq!(format_quoted_list(&["a", "b"]), r#""a", "b""#);
        assert_eq!(format_quoted_list(&["x"]), r#""x""#);
        assert_eq!(format_quoted_list(&[]), "");
    }
}
