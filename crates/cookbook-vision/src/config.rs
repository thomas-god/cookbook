use std::time::Duration;

/// One OpenAI-compatible chat-completions endpoint.
#[derive(Debug, Clone)]
pub struct VisionEndpoint {
    url: String,
    model: String,
    api_key: Option<String>,
    timeout: Duration,
    system_prompt: Option<String>,
}

impl VisionEndpoint {
    pub fn new(
        url: impl Into<String>,
        model: impl Into<String>,
        api_key: Option<String>,
        timeout: Duration,
    ) -> Self {
        Self {
            url: url.into(),
            model: model.into(),
            api_key: api_key.filter(|key| !key.trim().is_empty()),
            timeout,
            system_prompt: None,
        }
    }

    /// Sets a custom system prompt for this endpoint, replacing the built-in
    /// one. Blank prompts are treated as unset.
    pub fn with_system_prompt(mut self, prompt: Option<String>) -> Self {
        self.system_prompt = prompt.filter(|prompt| !prompt.trim().is_empty());
        self
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn system_prompt(&self) -> Option<&str> {
        self.system_prompt.as_deref()
    }
}

/// An ordered list of endpoints. The first one is tried first and the rest
/// act as fallbacks.
#[derive(Debug, Clone, Default)]
pub struct VisionConfig {
    endpoints: Vec<VisionEndpoint>,
}

impl VisionConfig {
    pub fn new(endpoints: Vec<VisionEndpoint>) -> Self {
        Self { endpoints }
    }

    pub fn endpoints(&self) -> &[VisionEndpoint] {
        &self.endpoints
    }

    pub fn is_enabled(&self) -> bool {
        !self.endpoints.is_empty()
    }
}
