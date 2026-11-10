//! Vision-based recipe extraction.
//!
//! This crate owns everything related to turning photos into a structured
//! recipe draft: image validation and preprocessing, VLM endpoints, prompt
//! construction, failover, and model-output parsing. The application layer
//! wires it and passes the resulting create request to the recipe service.

mod client;
mod config;
mod image;
mod recipe_parser;

use std::fmt;

use cookbook_domain::recipe::models::{NewRecipeSourceImage, RecipeContent};
use cookbook_domain::recipe::ports::CreateRecipeRequest;
use cookbook_domain::user::models::UserId;

pub use client::VisionClient;
pub use config::{VisionConfig, VisionEndpoint};
pub use image::{SourcePhoto, detect_image_content_type};

/// A parsed recipe and the source photos it was created from.
#[derive(Debug, Clone)]
pub struct VisionRecipe {
    content: RecipeContent,
    source_images: Vec<NewRecipeSourceImage>,
}

impl VisionRecipe {
    pub(crate) fn new(content: RecipeContent, source_images: Vec<NewRecipeSourceImage>) -> Self {
        Self {
            content,
            source_images,
        }
    }

    pub fn content(&self) -> &RecipeContent {
        &self.content
    }

    pub fn source_images(&self) -> &[NewRecipeSourceImage] {
        &self.source_images
    }

    /// Builds the request consumed by the recipe service.
    pub fn into_create_request(self, author: UserId) -> CreateRecipeRequest {
        CreateRecipeRequest {
            content: self.content,
            author,
            source_images: self.source_images,
        }
    }
}

#[derive(Debug, Clone)]
pub enum VisionError {
    Disabled,
    NoImages,
    EmptyImage {
        index: usize,
    },
    UnsupportedImage {
        index: usize,
        reason: String,
    },
    /// A source photo could not be decoded or prepared.
    ImageProcessing {
        index: usize,
        reason: String,
    },
    AllEndpointsFailed(Vec<String>),
    Internal(String),
}

impl fmt::Display for VisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => f.write_str("vision support is disabled"),
            Self::NoImages => f.write_str("no photos were provided"),
            Self::EmptyImage { index } => write!(f, "photo {} is empty", index + 1),
            Self::UnsupportedImage { index, reason } => {
                write!(f, "photo {} is not supported: {reason}", index + 1)
            }
            Self::ImageProcessing { index, reason } => {
                write!(f, "photo {} could not be processed: {reason}", index + 1)
            }
            Self::AllEndpointsFailed(failures) => {
                write!(f, "all vision endpoints failed: {}", failures.join("; "))
            }
            Self::Internal(reason) => write!(f, "vision processing failed: {reason}"),
        }
    }
}

impl std::error::Error for VisionError {}
