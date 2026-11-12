//! Vision-based recipe extraction.
//!
//! This crate owns everything related to turning source documents into a
//! structured recipe draft: image and PDF validation and preprocessing, VLM
//! endpoints, prompt construction, failover, and model-output parsing. The
//! application layer wires it and passes the resulting create request to the
//! recipe service.

mod client;
mod config;
mod image;
mod recipe_parser;

use std::fmt;

use cookbook_domain::recipe::models::{
    NewRecipeSourceDocument, RecipeContent, RevisionVerification,
};
use cookbook_domain::recipe::ports::CreateRecipeRequest;
use cookbook_domain::user::models::UserId;

pub use client::VisionClient;
pub use config::{VisionConfig, VisionEndpoint};
pub use image::{
    MAX_PDF_PAGES, SourceDocument, detect_image_content_type, detect_source_document_content_type,
};

/// A parsed recipe and the source documents it was created from.
#[derive(Debug, Clone)]
pub struct VisionRecipe {
    content: RecipeContent,
    source_documents: Vec<NewRecipeSourceDocument>,
}

impl VisionRecipe {
    pub(crate) fn new(
        content: RecipeContent,
        source_documents: Vec<NewRecipeSourceDocument>,
    ) -> Self {
        Self {
            content,
            source_documents,
        }
    }

    pub fn content(&self) -> &RecipeContent {
        &self.content
    }

    pub fn source_documents(&self) -> &[NewRecipeSourceDocument] {
        &self.source_documents
    }

    /// Builds the request consumed by the recipe service.
    pub fn into_create_request(self, author: UserId) -> CreateRecipeRequest {
        CreateRecipeRequest {
            content: self.content,
            author,
            source_documents: self.source_documents,
            images: Vec::new(),
            // Imported recipes always need review
            verification: RevisionVerification::NeedsReview,
        }
    }
}

#[derive(Debug, Clone)]
pub enum VisionError {
    Disabled,
    NoDocuments,
    EmptyDocument {
        index: usize,
    },
    UnsupportedDocument {
        index: usize,
        reason: String,
    },
    /// A source document could not be decoded or prepared.
    DocumentProcessing {
        index: usize,
        reason: String,
    },
    EncryptedPdf {
        index: usize,
    },
    TooManyPages {
        index: usize,
        count: usize,
        max: usize,
    },
    MultiplePdfDocuments,
    MixedDocumentTypes,
    AllEndpointsFailed(Vec<String>),
    Internal(String),
}

impl fmt::Display for VisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => f.write_str("vision support is disabled"),
            Self::NoDocuments => f.write_str("no documents were provided"),
            Self::EmptyDocument { index } => write!(f, "document {} is empty", index + 1),
            Self::UnsupportedDocument { index, reason } => {
                write!(f, "document {} is not supported: {reason}", index + 1)
            }
            Self::DocumentProcessing { index, reason } => {
                write!(f, "document {} could not be processed: {reason}", index + 1)
            }
            Self::EncryptedPdf { index } => {
                write!(f, "document {} is a password-protected PDF", index + 1)
            }
            Self::TooManyPages { count, max, .. } => {
                write!(f, "PDF has {count} pages, but at most {max} are supported")
            }
            Self::MultiplePdfDocuments => f.write_str("only one PDF can be imported at a time"),
            Self::MixedDocumentTypes => {
                f.write_str("a PDF cannot be imported together with photos")
            }
            Self::AllEndpointsFailed(failures) => {
                write!(f, "all vision endpoints failed: {}", failures.join("; "))
            }
            Self::Internal(reason) => write!(f, "vision processing failed: {reason}"),
        }
    }
}

impl std::error::Error for VisionError {}

#[cfg(test)]
mod tests {
    use cookbook_domain::recipe::models::{
        RecipeBody, RecipeContent, RecipeDurations, RecipeTitle, RevisionVerification,
    };

    use super::*;

    #[test]
    fn imported_recipes_need_review() {
        let content = RecipeContent::new_simple(
            RecipeTitle::new("Soup".to_string()),
            None,
            None,
            None,
            RecipeBody::new(
                RecipeDurations::new(None, None, None),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
        )
        .unwrap();
        let recipe = VisionRecipe::new(content, Vec::new());

        let request = recipe.into_create_request(UserId::new("user-1".to_string()));

        assert_eq!(request.verification, RevisionVerification::NeedsReview);
    }
}
