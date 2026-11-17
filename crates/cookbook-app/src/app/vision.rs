use std::time::Duration;

use anyhow::{Context, bail};
use cookbook_config::{defaults, env, names};
use cookbook_vision::{VisionClient, VisionConfig as ClientVisionConfig, VisionEndpoint};

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
        let system_prompt = env::optional(names::vision::SYSTEM_PROMPT)?;
        let endpoints = endpoint_from_env(
            names::vision::PRIMARY_PREFIX,
            defaults::VISION_PRIMARY_TIMEOUT_SECONDS,
            system_prompt.as_deref(),
        )?
        .into_iter()
        .chain(endpoint_from_env(
            names::vision::BACKUP_PREFIX,
            defaults::VISION_BACKUP_TIMEOUT_SECONDS,
            system_prompt.as_deref(),
        )?)
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
    default_system_prompt: Option<&str>,
) -> anyhow::Result<Option<VisionEndpoint>> {
    endpoint_from_values(
        prefix,
        default_timeout_seconds,
        env::optional(&format!("{prefix}_{}", names::vision::URL_SUFFIX))?,
        env::optional(&format!("{prefix}_{}", names::vision::MODEL_SUFFIX))?,
        env::optional(&format!("{prefix}_{}", names::vision::API_KEY_SUFFIX))?,
        env::optional(&format!(
            "{prefix}_{}",
            names::vision::TIMEOUT_SECONDS_SUFFIX
        ))?,
        env::optional(&format!("{prefix}_{}", names::vision::SYSTEM_PROMPT_SUFFIX))?,
        default_system_prompt,
    )
}

fn endpoint_from_values(
    prefix: &str,
    default_timeout_seconds: u64,
    url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    timeout: Option<String>,
    system_prompt: Option<String>,
    default_system_prompt: Option<&str>,
) -> anyhow::Result<Option<VisionEndpoint>> {
    let Some(url) = url else {
        if model.is_some() || api_key.is_some() || timeout.is_some() || system_prompt.is_some() {
            bail!(
                "{prefix}_MODEL, {prefix}_API_KEY, {prefix}_TIMEOUT_SECONDS and \
                 {prefix}_SYSTEM_PROMPT require {prefix}_URL"
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

    let system_prompt = system_prompt
        .filter(|prompt| !prompt.trim().is_empty())
        .or_else(|| default_system_prompt.map(str::to_string));

    Ok(Some(
        VisionEndpoint::new(url, model, api_key, Duration::from_secs(timeout_seconds))
            .with_system_prompt(system_prompt),
    ))
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
            names::vision::PRIMARY_PREFIX,
            180,
            url.map(str::to_string),
            model.map(str::to_string),
            None,
            timeout.map(str::to_string),
            None,
            None,
        )
    }

    fn endpoint_with_prompts(
        system_prompt: Option<&str>,
        default_system_prompt: Option<&str>,
    ) -> anyhow::Result<Option<VisionEndpoint>> {
        endpoint_from_values(
            names::vision::PRIMARY_PREFIX,
            180,
            Some("http://localhost:1234/v1/chat/completions".to_string()),
            Some("qwen-vl".to_string()),
            None,
            None,
            system_prompt.map(str::to_string),
            default_system_prompt,
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
    fn a_system_prompt_without_a_url_is_rejected() {
        assert!(
            endpoint_from_values(
                names::vision::PRIMARY_PREFIX,
                180,
                None,
                None,
                None,
                None,
                Some("custom prompt".to_string()),
                None,
            )
            .is_err()
        );
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

    #[test]
    fn an_endpoint_prompt_overrides_the_global_prompt() {
        let endpoint = endpoint_with_prompts(Some("endpoint prompt"), Some("global prompt"))
            .unwrap()
            .unwrap();

        assert_eq!(endpoint.system_prompt(), Some("endpoint prompt"));
    }

    #[test]
    fn the_global_prompt_is_used_as_a_fallback() {
        let endpoint = endpoint_with_prompts(None, Some("global prompt"))
            .unwrap()
            .unwrap();

        assert_eq!(endpoint.system_prompt(), Some("global prompt"));
    }

    #[test]
    fn a_blank_endpoint_prompt_falls_back_to_the_global_prompt() {
        let endpoint = endpoint_with_prompts(Some("   "), Some("global prompt"))
            .unwrap()
            .unwrap();

        assert_eq!(endpoint.system_prompt(), Some("global prompt"));
    }

    #[test]
    fn no_prompt_uses_the_built_in_default() {
        let endpoint = endpoint_with_prompts(None, None).unwrap().unwrap();

        assert_eq!(endpoint.system_prompt(), None);
    }
}
