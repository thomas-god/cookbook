use base64::{Engine as _, engine::general_purpose::STANDARD};
use cookbook_domain::recipe::models::{NewRecipeSourceImage, RecipeContent};
use serde_json::{Value, json};
use ureq::Agent;

use crate::VisionError;
use crate::VisionRecipe;
use crate::config::{VisionConfig, VisionEndpoint};
use crate::image::{PreparedImage, SourcePhoto, prepare_images};
use crate::recipe_parser::{SYSTEM_PROMPT, parse_recipe_json, response_format, user_prompt};

/// An ordered, failover-capable VLM client.
#[derive(Debug, Clone, Default)]
pub struct VisionClient {
    endpoints: Vec<VisionEndpoint>,
}

impl VisionClient {
    pub fn new(config: VisionConfig) -> Self {
        Self {
            endpoints: config.endpoints().to_vec(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        !self.endpoints.is_empty()
    }

    /// Extracts a recipe from the supplied photos.
    ///
    /// The first endpoint that returns a valid recipe wins. Invalid model
    /// output (e.g. cannot be parsed) also triggers failover.
    pub async fn parse_recipe(
        &self,
        photos: Vec<SourcePhoto>,
    ) -> Result<VisionRecipe, VisionError> {
        if !self.is_enabled() {
            return Err(VisionError::Disabled);
        }

        let client = self.clone();
        tokio::task::spawn_blocking(move || client.parse_recipe_blocking(photos))
            .await
            .map_err(|error| VisionError::Internal(error.to_string()))?
    }

    fn parse_recipe_blocking(&self, photos: Vec<SourcePhoto>) -> Result<VisionRecipe, VisionError> {
        let images = prepare_images(photos)?;
        let mut failures = Vec::new();

        for (index, endpoint) in self.endpoints.iter().enumerate() {
            tracing::debug!(
                endpoint = index + 1,
                model = endpoint.model(),
                images = images.len(),
                "trying vision endpoint"
            );

            match self.try_endpoint(endpoint, &images) {
                Ok(content) => {
                    tracing::info!(
                        endpoint = index + 1,
                        model = endpoint.model(),
                        "vision endpoint returned a recipe"
                    );

                    let source_images = images
                        .iter()
                        .map(|image| {
                            NewRecipeSourceImage::new(
                                image.content_type.to_string(),
                                image.original_name.clone(),
                                image.original_bytes.clone(),
                            )
                        })
                        .collect();

                    return Ok(VisionRecipe::new(content, source_images));
                }
                Err(reason) => {
                    tracing::warn!(
                        endpoint = index + 1,
                        model = endpoint.model(),
                        reason,
                        "vision endpoint failed"
                    );
                    failures.push(format!("endpoint {}: {reason}", index + 1));
                }
            }
        }

        Err(VisionError::AllEndpointsFailed(failures))
    }

    fn try_endpoint(
        &self,
        endpoint: &VisionEndpoint,
        images: &[PreparedImage],
    ) -> Result<RecipeContent, String> {
        let body = request_body(endpoint, images);
        let agent: Agent = Agent::config_builder()
            .timeout_global(Some(endpoint.timeout()))
            .build()
            .into();
        let mut request = agent.post(endpoint.url());

        if let Some(api_key) = endpoint.api_key() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }

        let mut response = request
            .send_json(&body)
            .map_err(|error| format!("request failed: {error}"))?;
        let response: Value = response
            .body_mut()
            .read_json()
            .map_err(|error| format!("invalid response body: {error}"))?;
        let content = response_content(&response)
            .ok_or_else(|| "response did not contain message content".to_string())?;

        parse_recipe_json(&content).map_err(|error| format!("invalid recipe output: {error}"))
    }
}

fn request_body(endpoint: &VisionEndpoint, images: &[PreparedImage]) -> Value {
    let mut content = vec![json!({
        "type": "text",
        "text": user_prompt(images.len()),
    })];

    for image in images {
        let data_url = format!(
            "data:{};base64,{}",
            image.vlm_content_type,
            STANDARD.encode(&image.vlm_bytes)
        );

        content.push(json!({
            "type": "image_url",
            "image_url": { "url": data_url },
        }));
    }

    json!({
        "model": endpoint.model(),
        "temperature": 0,
        "response_format": response_format(),
        "messages": [
            {
                "role": "system",
                "content": SYSTEM_PROMPT,
            },
            {
                "role": "user",
                "content": content,
            }
        ],
    })
}

fn response_content(response: &Value) -> Option<String> {
    let content = response.pointer("/choices/0/message/content")?;

    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }

    let text = content
        .as_array()?
        .iter()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("");

    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn endpoint() -> VisionEndpoint {
        VisionEndpoint::new(
            "http://localhost:1234/v1/chat/completions",
            "qwen-vl",
            Some("secret".to_string()),
            Duration::from_secs(30),
        )
    }

    #[test]
    fn disabled_without_endpoints() {
        let client = VisionClient::default();

        assert!(!client.is_enabled());
    }

    #[test]
    fn builds_a_vision_request_with_all_images() {
        let images = vec![
            PreparedImage {
                content_type: "image/jpeg",
                original_name: Some("front.jpg".to_string()),
                original_bytes: b"front".to_vec(),
                vlm_bytes: b"front".to_vec(),
                vlm_content_type: "image/jpeg",
            },
            PreparedImage {
                content_type: "image/png",
                original_name: Some("back.png".to_string()),
                original_bytes: b"back".to_vec(),
                vlm_bytes: b"back".to_vec(),
                vlm_content_type: "image/png",
            },
        ];

        let body = request_body(&endpoint(), &images);
        let parts = body["messages"][1]["content"].as_array().unwrap();

        assert_eq!(body["model"], "qwen-vl");
        assert_eq!(body["response_format"]["type"], "json_schema");
        assert_eq!(body["response_format"]["json_schema"]["name"], "recipe");
        assert_eq!(body["response_format"]["json_schema"]["strict"], true);
        assert_eq!(
            body["response_format"]["json_schema"]["schema"]["type"],
            "object"
        );
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[1]["type"], "image_url");
        assert_eq!(
            parts[1]["image_url"]["url"],
            format!("data:image/jpeg;base64,{}", STANDARD.encode("front"))
        );
        assert_eq!(
            parts[2]["image_url"]["url"],
            format!("data:image/png;base64,{}", STANDARD.encode("back"))
        );
    }

    #[test]
    fn reads_string_and_array_message_content() {
        assert_eq!(
            response_content(&json!({"choices": [{"message": {"content": "hello"}}]})),
            Some("hello".to_string())
        );
        assert_eq!(
            response_content(&json!({
                "choices": [{"message": {"content": [
                    {"type": "text", "text": "hel"},
                    {"type": "text", "text": "lo"}
                ]}}]
            })),
            Some("hello".to_string())
        );
        assert_eq!(response_content(&json!({})), None);
    }

    #[tokio::test]
    async fn disabled_client_rejects_requests_without_calling_a_model() {
        let client = VisionClient::default();

        let result = client.parse_recipe(Vec::new()).await;

        assert!(matches!(result, Err(VisionError::Disabled)));
    }
}
