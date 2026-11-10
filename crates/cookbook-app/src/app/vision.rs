use std::{env, time::Duration};

use anyhow::{Context, bail};
use cookbook_vision::{VisionClient, VisionConfig as ClientVisionConfig, VisionEndpoint};

const PRIMARY_PREFIX: &str = "COOKBOOK_VLM_PRIMARY";
const BACKUP_PREFIX: &str = "COOKBOOK_VLM_BACKUP";

/// Vision availability exposed to the web layer.
#[derive(Debug, Clone, Copy)]
pub struct VisionSettings {
    enabled: bool,
}

impl VisionSettings {
    pub fn enabled(&self) -> bool {
        self.enabled
    }
}

/// Startup configuration for vision-based recipe import.
#[derive(Debug, Clone)]
pub struct VisionConfig {
    settings: VisionSettings,
    client: VisionClient,
}

impl VisionConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let endpoints = endpoint_from_env(PRIMARY_PREFIX, 180)?
            .into_iter()
            .chain(endpoint_from_env(BACKUP_PREFIX, 120)?)
            .collect::<Vec<_>>();
        let settings = VisionSettings {
            enabled: !endpoints.is_empty(),
        };
        let client = VisionClient::new(ClientVisionConfig::new(endpoints));

        Ok(Self { settings, client })
    }

    pub fn settings(&self) -> VisionSettings {
        self.settings
    }

    pub fn client(&self) -> VisionClient {
        self.client.clone()
    }
}

fn endpoint_from_env(
    prefix: &str,
    default_timeout_seconds: u64,
) -> anyhow::Result<Option<VisionEndpoint>> {
    endpoint_from_values(
        prefix,
        default_timeout_seconds,
        optional_env(&format!("{prefix}_URL")),
        optional_env(&format!("{prefix}_MODEL")),
        optional_env(&format!("{prefix}_API_KEY")),
        optional_env(&format!("{prefix}_TIMEOUT_SECONDS")),
    )
}

fn endpoint_from_values(
    prefix: &str,
    default_timeout_seconds: u64,
    url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    timeout: Option<String>,
) -> anyhow::Result<Option<VisionEndpoint>> {
    let Some(url) = url else {
        if model.is_some() || api_key.is_some() || timeout.is_some() {
            bail!(
                "{prefix}_MODEL, {prefix}_API_KEY and {prefix}_TIMEOUT_SECONDS require {prefix}_URL"
            );
        }

        return Ok(None);
    };

    if !(url.starts_with("http://") || url.starts_with("https://")) {
        bail!("{prefix}_URL must start with http:// or https://");
    }

    let model = model.with_context(|| format!("{prefix}_MODEL is required with {prefix}_URL"))?;
    let timeout_seconds = match timeout {
        Some(value) => value.parse::<u64>().with_context(|| {
            format!("{prefix}_TIMEOUT_SECONDS must be a positive number of seconds")
        })?,
        None => default_timeout_seconds,
    };

    if timeout_seconds == 0 {
        bail!("{prefix}_TIMEOUT_SECONDS must be greater than zero");
    }

    Ok(Some(VisionEndpoint::new(
        url,
        model,
        api_key,
        Duration::from_secs(timeout_seconds),
    )))
}

fn optional_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(
        url: Option<&str>,
        model: Option<&str>,
        timeout: Option<&str>,
    ) -> anyhow::Result<Option<VisionEndpoint>> {
        endpoint_from_values(
            PRIMARY_PREFIX,
            180,
            url.map(str::to_string),
            model.map(str::to_string),
            None,
            timeout.map(str::to_string),
        )
    }

    #[test]
    fn an_empty_slot_is_disabled() {
        assert!(endpoint(None, None, None).unwrap().is_none());
    }

    #[test]
    fn a_model_without_a_url_is_rejected() {
        assert!(endpoint(None, Some("qwen-vl"), None).is_err());
    }

    #[test]
    fn a_url_without_a_model_is_rejected() {
        assert!(
            endpoint(
                Some("http://localhost:1234/v1/chat/completions"),
                None,
                None
            )
            .is_err()
        );
    }

    #[test]
    fn a_non_http_url_is_rejected() {
        assert!(endpoint(Some("file:///model"), Some("qwen-vl"), None).is_err());
    }

    #[test]
    fn a_valid_slot_uses_the_default_timeout() {
        let endpoint = endpoint(
            Some("http://localhost:1234/v1/chat/completions"),
            Some("qwen-vl"),
            None,
        )
        .unwrap()
        .unwrap();

        assert_eq!(endpoint.model(), "qwen-vl");
        assert_eq!(endpoint.timeout(), Duration::from_secs(180));
    }

    #[test]
    fn a_valid_slot_uses_an_explicit_timeout() {
        let endpoint = endpoint(
            Some("https://openrouter.ai/api/v1/chat/completions"),
            Some("qwen/qwen-vl"),
            Some("45"),
        )
        .unwrap()
        .unwrap();

        assert_eq!(endpoint.timeout(), Duration::from_secs(45));
    }

    #[test]
    fn a_zero_timeout_is_rejected() {
        assert!(
            endpoint(
                Some("http://localhost:1234/v1/chat/completions"),
                Some("qwen-vl"),
                Some("0"),
            )
            .is_err()
        );
    }
}
