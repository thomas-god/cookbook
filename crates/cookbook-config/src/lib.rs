//! Shared environment configuration for the Cookbook application and its tools.
//!
//! Every environment variable name and default lives here so the web app and the
//! command-line tools cannot drift apart. Load values with [`env::optional`] to get
//! Docker secret (`*_FILE`) support for free.

use anyhow::bail;

pub mod defaults;
pub mod env;
pub mod names;

/// Parses a boolean environment variable value.
///
/// Accepts `1`/`true`/`yes`/`on` and `0`/`false`/`no`/`off`, case-insensitively.
pub fn parse_bool(name: &str, value: &str) -> anyhow::Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => bail!("invalid {name} value `{value}`: expected true or false"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_boolean_spellings() {
        for value in ["1", "true", "TRUE", "yes", "on"] {
            assert!(parse_bool("FLAG", value).unwrap(), "{value}");
        }

        for value in ["0", "false", "FALSE", "no", "off"] {
            assert!(!parse_bool("FLAG", value).unwrap(), "{value}");
        }

        assert!(parse_bool("FLAG", "maybe").is_err());
    }
}
