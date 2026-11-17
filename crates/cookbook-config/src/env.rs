//! Environment variable loading with support for Docker secrets.
//!
//! For every variable `NAME`, the helper first looks for `NAME_FILE`. When set, the value is read
//! from the file at that path (trimming the trailing newline commonly added by secret stores).
//! Otherwise it falls back to reading `NAME` directly.

use std::{env::VarError, fs};

use anyhow::Context;

/// Reads an optional environment variable, preferring the value from `{name}_FILE`.
///
/// Returns `None` when neither variable is set, or when the resolved value is empty. An error is
/// returned when `{name}_FILE` is set but the file cannot be read, rather than silently falling
/// back to the direct variable.
pub fn optional(name: &str) -> anyhow::Result<Option<String>> {
    optional_with(name, |key| std::env::var(key))
}

fn optional_with(
    name: &str,
    get: impl Fn(&str) -> Result<String, VarError>,
) -> anyhow::Result<Option<String>> {
    let file_name = format!("{name}_FILE");

    match get(&file_name) {
        Ok(path) => {
            let path = path.trim();
            if path.is_empty() {
                return direct(name, get);
            }

            let value = fs::read_to_string(path)
                .with_context(|| format!("failed to read {file_name} file at {path:?}"))?;

            Ok(normalize(value))
        }
        Err(VarError::NotPresent) => direct(name, get),
        Err(error) => Err(error).with_context(|| format!("failed to read {file_name}")),
    }
}

fn direct(
    name: &str,
    get: impl Fn(&str) -> Result<String, VarError>,
) -> anyhow::Result<Option<String>> {
    match get(name) {
        Ok(value) => Ok(normalize(value)),
        Err(VarError::NotPresent) => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {name}")),
    }
}

fn normalize(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    fn temp_file(contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "cookbook-env-test-{}-{}.txt",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, contents).unwrap();
        path
    }

    fn lookup(vars: &HashMap<String, String>) -> impl Fn(&str) -> Result<String, VarError> + '_ {
        move |name| vars.get(name).cloned().ok_or(VarError::NotPresent)
    }

    #[test]
    fn reads_the_value_from_the_file_first() {
        let path = temp_file("from-file\n");
        let path = path.to_str().unwrap();
        let vars = HashMap::from([
            ("SECRET_FILE".to_string(), path.to_string()),
            ("SECRET".to_string(), "direct".to_string()),
        ]);

        assert_eq!(
            optional_with("SECRET", lookup(&vars)).unwrap().as_deref(),
            Some("from-file")
        );
    }

    #[test]
    fn falls_back_to_the_direct_variable() {
        let vars = HashMap::from([("SECRET".to_string(), "direct".to_string())]);

        assert_eq!(
            optional_with("SECRET", lookup(&vars)).unwrap().as_deref(),
            Some("direct")
        );
    }

    #[test]
    fn a_blank_file_path_falls_back_to_the_direct_variable() {
        let vars = HashMap::from([
            ("SECRET_FILE".to_string(), "  ".to_string()),
            ("SECRET".to_string(), "direct".to_string()),
        ]);

        assert_eq!(
            optional_with("SECRET", lookup(&vars)).unwrap().as_deref(),
            Some("direct")
        );
    }

    #[test]
    fn returns_none_when_neither_variable_is_set() {
        let vars = HashMap::new();

        assert_eq!(optional_with("SECRET", lookup(&vars)).unwrap(), None);
    }

    #[test]
    fn an_empty_value_is_none() {
        let vars = HashMap::from([("SECRET".to_string(), " \n ".to_string())]);

        assert_eq!(optional_with("SECRET", lookup(&vars)).unwrap(), None);
    }

    #[test]
    fn an_unreadable_file_is_an_error() {
        let vars = HashMap::from([("SECRET_FILE".to_string(), "/does/not/exist".to_string())]);

        assert!(optional_with("SECRET", lookup(&vars)).is_err());
    }
}
