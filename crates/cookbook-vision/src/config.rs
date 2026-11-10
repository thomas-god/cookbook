use std::time::Duration;

/// One OpenAI-compatible chat-completions endpoint.
#[derive(Debug, Clone)]
pub struct VisionEndpoint {
    url: String,
    model: String,
    api_key: Option<String>,
    timeout: Duration,
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
        }
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
