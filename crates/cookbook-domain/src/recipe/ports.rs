use std::fmt;
use std::future::Future;

use crate::recipe::models::{
    NewRecipeImage, NewRecipeSourceDocument, Recipe, RecipeContent, RecipeId, RecipeImage,
    RecipeImageId, RecipeImageMetadata, RecipeRevision, RecipeRevisionId, RecipeScope,
    RecipeSourceDocument, RecipeSourceDocumentId, RecipeSourceDocumentSummary, RecipeStructureKind,
    RecipeWithContent, RevisionMessage, RevisionVerification,
};
use crate::user::models::UserId;

#[derive(Debug, Clone)]
pub struct CreateRecipeRequest {
    pub content: RecipeContent,
    pub author: UserId,
    pub source_documents: Vec<NewRecipeSourceDocument>,
    pub images: Vec<NewRecipeImage>,
    pub verification: RevisionVerification,
}

#[derive(Debug, Clone)]
pub struct AddRecipeSourceDocumentsRequest {
    pub recipe_id: RecipeId,
    pub added_by: UserId,
    pub documents: Vec<NewRecipeSourceDocument>,
}

#[derive(Debug, Clone)]
pub struct UpdateRecipeRequest {
    pub id: RecipeId,
    pub content: RecipeContent,
    pub editor: UserId,
    pub latest_revision: RecipeRevisionId,
    pub message: Option<RevisionMessage>,
    pub new_images: Vec<NewRecipeImage>,
}

#[derive(Debug, Clone)]
pub struct UpdateRecipeScopeRequest {
    pub id: RecipeId,
    pub scope: RecipeScope,
    pub actor: UserId,
}

#[derive(Debug, Clone)]
pub struct ConvertRecipeStructureRequest {
    pub id: RecipeId,
    pub to: RecipeStructureKind,
    pub editor: UserId,
    pub latest_revision: RecipeRevisionId,
}

#[derive(Debug, Clone)]
pub struct RestoreRecipeRevisionRequest {
    pub id: RecipeId,
    pub revision_id: RecipeRevisionId,
    pub editor: UserId,
    pub latest_revision: RecipeRevisionId,
}

/// Inbound (driving) port for the recipe domain service.
pub trait RecipeService: Clone + Send + Sync + 'static {
    fn create_verified_recipe(
        &self,
        request: CreateRecipeRequest,
    ) -> impl Future<Output = Result<RecipeWithContent, CreateRecipeError>> + Send;

    fn create_imported_recipe(
        &self,
        request: CreateRecipeRequest,
    ) -> impl Future<Output = Result<RecipeWithContent, CreateRecipeError>> + Send;

    fn update_recipe(
        &self,
        request: UpdateRecipeRequest,
    ) -> impl Future<Output = Result<RecipeWithContent, UpdateRecipeError>> + Send;

    fn convert_recipe_structure(
        &self,
        request: ConvertRecipeStructureRequest,
    ) -> impl Future<Output = Result<RecipeWithContent, UpdateRecipeError>> + Send;

    fn restore_recipe_revision(
        &self,
        request: RestoreRecipeRevisionRequest,
    ) -> impl Future<Output = Result<RecipeWithContent, RestoreRecipeRevisionError>> + Send;

    fn update_recipe_scope(
        &self,
        request: UpdateRecipeScopeRequest,
    ) -> impl Future<Output = Result<RecipeWithContent, UpdateRecipeScopeError>> + Send;

    fn get_recipes(
        &self,
        viewer: UserId,
    ) -> impl Future<Output = Result<Vec<RecipeWithContent>, GetRecipesError>> + Send;

    fn get_recipe(
        &self,
        id: RecipeId,
        viewer: UserId,
    ) -> impl Future<Output = Result<Option<RecipeWithContent>, GetRecipeError>> + Send;

    fn get_recipe_history(
        &self,
        id: RecipeId,
        viewer: UserId,
    ) -> impl Future<Output = Result<Vec<RecipeRevision>, GetRecipeHistoryError>> + Send;

    fn add_source_documents(
        &self,
        request: AddRecipeSourceDocumentsRequest,
    ) -> impl Future<
        Output = Result<Vec<RecipeSourceDocumentSummary>, AddRecipeSourceDocumentsError>,
    > + Send;

    fn get_source_documents(
        &self,
        recipe_id: RecipeId,
        viewer: UserId,
    ) -> impl Future<
        Output = Result<Vec<RecipeSourceDocumentSummary>, GetRecipeSourceDocumentsError>,
    > + Send;

    fn get_source_document(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeSourceDocumentId,
        viewer: UserId,
    ) -> impl Future<Output = Result<Option<RecipeSourceDocument>, GetRecipeSourceDocumentError>> + Send;

    fn delete_source_document(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeSourceDocumentId,
        actor: UserId,
    ) -> impl Future<Output = Result<(), DeleteRecipeSourceDocumentError>> + Send;

    fn get_recipe_images(
        &self,
        recipe_id: RecipeId,
        viewer: UserId,
    ) -> impl Future<Output = Result<Vec<RecipeImageMetadata>, GetRecipeImagesError>> + Send;

    fn get_recipe_image(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeImageId,
        viewer: UserId,
    ) -> impl Future<Output = Result<Option<RecipeImage>, GetRecipeImageError>> + Send;
}

/// Outbound (driven) port for recipe persistence.
pub trait RecipeRepository: Clone + Send + Sync + 'static {
    /// Saves a new recipe with its first revision, source documents, and
    /// presentation images atomically.
    fn save_new_recipe(
        &self,
        recipe: Recipe,
        revision: RecipeRevision,
        source_documents: Vec<RecipeSourceDocument>,
        images: Vec<RecipeImage>,
    ) -> impl Future<Output = Result<(), SaveRecipeError>> + Send;

    /// Saves the current recipe and appends its revision atomically, together
    /// with presentation image additions.
    fn save_recipe_with_revision(
        &self,
        recipe: Recipe,
        revision: RecipeRevision,
        added_images: Vec<RecipeImage>,
    ) -> impl Future<Output = Result<(), SaveRecipeError>> + Send;

    fn save_recipe(
        &self,
        recipe: Recipe,
    ) -> impl Future<Output = Result<(), SaveRecipeError>> + Send;

    fn get_recipe(
        &self,
        id: RecipeId,
    ) -> impl Future<Output = Result<Option<RecipeWithContent>, GetRecipeError>> + Send;

    fn get_recipes(
        &self,
    ) -> impl Future<Output = Result<Vec<RecipeWithContent>, ListRecipesError>> + Send;

    fn get_recipe_revisions(
        &self,
        id: RecipeId,
    ) -> impl Future<Output = Result<Vec<RecipeRevision>, ListRecipeRevisionsError>> + Send;

    fn add_source_documents(
        &self,
        images: Vec<RecipeSourceDocument>,
    ) -> impl Future<Output = Result<(), AddSourceDocumentsError>> + Send;

    fn get_source_documents(
        &self,
        recipe_id: RecipeId,
    ) -> impl Future<Output = Result<Vec<RecipeSourceDocumentSummary>, ListSourceDocumentsError>> + Send;

    fn get_source_document(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeSourceDocumentId,
    ) -> impl Future<Output = Result<Option<RecipeSourceDocument>, GetSourceDocumentError>> + Send;

    fn delete_source_document(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeSourceDocumentId,
    ) -> impl Future<Output = Result<(), DeleteSourceDocumentError>> + Send;

    fn get_recipe_images(
        &self,
        recipe_id: RecipeId,
    ) -> impl Future<Output = Result<Vec<RecipeImageMetadata>, ListRecipeImagesError>> + Send;

    fn get_recipe_image(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeImageId,
    ) -> impl Future<Output = Result<Option<RecipeImage>, GetRecipeImageError>> + Send;
}

#[derive(Debug)]
pub enum CreateRecipeError {
    /// The request's verification status does not match the service
    /// entrypoint that was called.
    VerificationMismatch,
    /// An image targets a step that is not part of the recipe.
    InvalidImageReference,
    Storage(anyhow::Error),
}

impl fmt::Display for CreateRecipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VerificationMismatch => f.write_str(
                "the recipe entrypoint does not match the requested verification status",
            ),
            Self::InvalidImageReference => {
                f.write_str("an image reference is invalid for this recipe")
            }
            Self::Storage(reason) => write!(f, "failed to store recipe: {reason}"),
        }
    }
}

impl std::error::Error for CreateRecipeError {}

#[derive(Debug)]
pub enum UpdateRecipeError {
    NotFound,
    Forbidden,
    Conflict,
    InvalidImageReference,
    Storage(anyhow::Error),
}

impl fmt::Display for UpdateRecipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe not found"),
            Self::Forbidden => f.write_str("the recipe cannot be edited by this user"),
            Self::Conflict => f.write_str("the recipe was changed since it was loaded"),
            Self::InvalidImageReference => {
                f.write_str("an image reference is invalid for this recipe")
            }
            Self::Storage(reason) => write!(f, "failed to update recipe: {reason}"),
        }
    }
}

impl std::error::Error for UpdateRecipeError {}

#[derive(Debug)]
pub enum RestoreRecipeRevisionError {
    NotFound,
    Forbidden,
    Conflict,
    Storage(anyhow::Error),
}

impl fmt::Display for RestoreRecipeRevisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe or revision not found"),
            Self::Forbidden => f.write_str("the recipe cannot be edited by this user"),
            Self::Conflict => f.write_str("the recipe was changed since it was loaded"),
            Self::Storage(reason) => write!(f, "failed to restore recipe revision: {reason}"),
        }
    }
}

impl std::error::Error for RestoreRecipeRevisionError {}

#[derive(Debug)]
pub enum UpdateRecipeScopeError {
    NotFound,
    Forbidden,
    Storage(anyhow::Error),
}

impl fmt::Display for UpdateRecipeScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe not found"),
            Self::Forbidden => f.write_str("only the recipe author can change its scope"),
            Self::Storage(reason) => write!(f, "failed to update recipe scope: {reason}"),
        }
    }
}

impl std::error::Error for UpdateRecipeScopeError {}

#[derive(Debug)]
pub enum GetRecipesError {
    Storage(anyhow::Error),
}

impl fmt::Display for GetRecipesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read recipes: {reason}"),
        }
    }
}

impl std::error::Error for GetRecipesError {}

#[derive(Debug)]
pub enum GetRecipeHistoryError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for GetRecipeHistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe not found"),
            Self::Storage(reason) => write!(f, "failed to read recipe history: {reason}"),
        }
    }
}

impl std::error::Error for GetRecipeHistoryError {}

#[derive(Debug)]
pub enum SaveRecipeError {
    RecipeIdMismatch,
    RecipeAuthorMismatch,
    RecipeNotFound,
    InvalidImageReference,
    Storage(anyhow::Error),
}

impl fmt::Display for SaveRecipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecipeIdMismatch => f.write_str("revision does not belong to the recipe"),
            Self::RecipeAuthorMismatch => f.write_str("a recipe author cannot be changed"),
            Self::RecipeNotFound => f.write_str("recipe not found"),
            Self::InvalidImageReference => {
                f.write_str("an image reference is invalid for this recipe")
            }
            Self::Storage(reason) => write!(f, "failed to save recipe: {reason}"),
        }
    }
}

impl std::error::Error for SaveRecipeError {}

#[derive(Debug)]
pub enum GetRecipeError {
    Storage(anyhow::Error),
}

impl fmt::Display for GetRecipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read recipe: {reason}"),
        }
    }
}

impl std::error::Error for GetRecipeError {}

#[derive(Debug)]
pub enum ListRecipesError {
    Storage(anyhow::Error),
}

impl fmt::Display for ListRecipesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read recipes: {reason}"),
        }
    }
}

impl std::error::Error for ListRecipesError {}

#[derive(Debug)]
pub enum ListRecipeRevisionsError {
    Storage(anyhow::Error),
}

impl fmt::Display for ListRecipeRevisionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read recipe revisions: {reason}"),
        }
    }
}

impl std::error::Error for ListRecipeRevisionsError {}

#[derive(Debug)]
pub enum AddRecipeSourceDocumentsError {
    RecipeNotFound,
    Forbidden,
    Storage(anyhow::Error),
}

impl fmt::Display for AddRecipeSourceDocumentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecipeNotFound => f.write_str("recipe not found"),
            Self::Forbidden => f.write_str("the recipe cannot be edited by this user"),
            Self::Storage(reason) => write!(f, "failed to store source documents: {reason}"),
        }
    }
}

impl std::error::Error for AddRecipeSourceDocumentsError {}

#[derive(Debug)]
pub enum GetRecipeSourceDocumentsError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for GetRecipeSourceDocumentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe not found"),
            Self::Storage(reason) => write!(f, "failed to read source documents: {reason}"),
        }
    }
}

impl std::error::Error for GetRecipeSourceDocumentsError {}

#[derive(Debug)]
pub enum GetRecipeSourceDocumentError {
    Storage(anyhow::Error),
}

impl fmt::Display for GetRecipeSourceDocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read source document: {reason}"),
        }
    }
}

impl std::error::Error for GetRecipeSourceDocumentError {}

#[derive(Debug)]
pub enum GetRecipeImagesError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for GetRecipeImagesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe not found"),
            Self::Storage(reason) => write!(f, "failed to read recipe images: {reason}"),
        }
    }
}

impl std::error::Error for GetRecipeImagesError {}

#[derive(Debug)]
pub enum ListRecipeImagesError {
    Storage(anyhow::Error),
}

impl fmt::Display for ListRecipeImagesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read recipe images: {reason}"),
        }
    }
}

impl std::error::Error for ListRecipeImagesError {}

#[derive(Debug)]
pub enum GetRecipeImageError {
    Storage(anyhow::Error),
}

impl fmt::Display for GetRecipeImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read recipe image: {reason}"),
        }
    }
}

impl std::error::Error for GetRecipeImageError {}

#[derive(Debug)]
pub enum DeleteRecipeSourceDocumentError {
    NotFound,
    Forbidden,
    Storage(anyhow::Error),
}

impl fmt::Display for DeleteRecipeSourceDocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("source document not found"),
            Self::Forbidden => f.write_str("the recipe cannot be edited by this user"),
            Self::Storage(reason) => write!(f, "failed to delete source document: {reason}"),
        }
    }
}

impl std::error::Error for DeleteRecipeSourceDocumentError {}

#[derive(Debug)]
pub enum AddSourceDocumentsError {
    RecipeNotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for AddSourceDocumentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecipeNotFound => f.write_str("recipe not found"),
            Self::Storage(reason) => write!(f, "failed to store source documents: {reason}"),
        }
    }
}

impl std::error::Error for AddSourceDocumentsError {}

#[derive(Debug)]
pub enum ListSourceDocumentsError {
    Storage(anyhow::Error),
}

impl fmt::Display for ListSourceDocumentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read source documents: {reason}"),
        }
    }
}

impl std::error::Error for ListSourceDocumentsError {}

#[derive(Debug)]
pub enum GetSourceDocumentError {
    Storage(anyhow::Error),
}

impl fmt::Display for GetSourceDocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read source document: {reason}"),
        }
    }
}

impl std::error::Error for GetSourceDocumentError {}

#[derive(Debug)]
pub enum DeleteSourceDocumentError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for DeleteSourceDocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("source document not found"),
            Self::Storage(reason) => write!(f, "failed to delete source document: {reason}"),
        }
    }
}

impl std::error::Error for DeleteSourceDocumentError {}

#[cfg(test)]
mockall::mock! {
    pub RecipeService {}

    impl Clone for RecipeService {
        fn clone(&self) -> Self;
    }

    impl RecipeService for RecipeService {
        async fn create_verified_recipe(
            &self,
            request: CreateRecipeRequest,
        ) -> Result<RecipeWithContent, CreateRecipeError>;

        async fn create_imported_recipe(
            &self,
            request: CreateRecipeRequest,
        ) -> Result<RecipeWithContent, CreateRecipeError>;

        async fn update_recipe(
            &self,
            request: UpdateRecipeRequest,
        ) -> Result<RecipeWithContent, UpdateRecipeError>;

        async fn convert_recipe_structure(
            &self,
            request: ConvertRecipeStructureRequest,
        ) -> Result<RecipeWithContent, UpdateRecipeError>;

        async fn restore_recipe_revision(
            &self,
            request: RestoreRecipeRevisionRequest,
        ) -> Result<RecipeWithContent, RestoreRecipeRevisionError>;

        async fn update_recipe_scope(
            &self,
            request: UpdateRecipeScopeRequest,
        ) -> Result<RecipeWithContent, UpdateRecipeScopeError>;

        async fn get_recipes(
            &self,
            viewer: UserId,
        ) -> Result<Vec<RecipeWithContent>, GetRecipesError>;

        async fn get_recipe(
            &self,
            id: RecipeId,
            viewer: UserId,
        ) -> Result<Option<RecipeWithContent>, GetRecipeError>;

        async fn get_recipe_history(
            &self,
            id: RecipeId,
            viewer: UserId,
        ) -> Result<Vec<RecipeRevision>, GetRecipeHistoryError>;

        async fn add_source_documents(
            &self,
            request: AddRecipeSourceDocumentsRequest,
        ) -> Result<Vec<RecipeSourceDocumentSummary>, AddRecipeSourceDocumentsError>;

        async fn get_source_documents(
            &self,
            recipe_id: RecipeId,
            viewer: UserId,
        ) -> Result<Vec<RecipeSourceDocumentSummary>, GetRecipeSourceDocumentsError>;

        async fn get_source_document(
            &self,
            recipe_id: RecipeId,
            image_id: RecipeSourceDocumentId,
            viewer: UserId,
        ) -> Result<Option<RecipeSourceDocument>, GetRecipeSourceDocumentError>;

        async fn delete_source_document(
            &self,
            recipe_id: RecipeId,
            image_id: RecipeSourceDocumentId,
            actor: UserId,
        ) -> Result<(), DeleteRecipeSourceDocumentError>;

        async fn get_recipe_images(
            &self,
            recipe_id: RecipeId,
            viewer: UserId,
        ) -> Result<Vec<RecipeImageMetadata>, GetRecipeImagesError>;

        async fn get_recipe_image(
            &self,
            recipe_id: RecipeId,
            image_id: RecipeImageId,
            viewer: UserId,
        ) -> Result<Option<RecipeImage>, GetRecipeImageError>;
    }
}

#[cfg(test)]
mockall::mock! {
    pub RecipeRepository {}

    impl Clone for RecipeRepository {
        fn clone(&self) -> Self;
    }

    impl RecipeRepository for RecipeRepository {
        async fn save_new_recipe(
            &self,
            recipe: Recipe,
            revision: RecipeRevision,
            source_documents: Vec<RecipeSourceDocument>,
            images: Vec<RecipeImage>,
        ) -> Result<(), SaveRecipeError>;

        async fn save_recipe_with_revision(
            &self,
            recipe: Recipe,
            revision: RecipeRevision,
            added_images: Vec<RecipeImage>,
        ) -> Result<(), SaveRecipeError>;

        async fn save_recipe(&self, recipe: Recipe) -> Result<(), SaveRecipeError>;

        async fn get_recipe(
            &self,
            id: RecipeId,
        ) -> Result<Option<RecipeWithContent>, GetRecipeError>;

        async fn get_recipes(&self) -> Result<Vec<RecipeWithContent>, ListRecipesError>;

        async fn get_recipe_revisions(
            &self,
            id: RecipeId,
        ) -> Result<Vec<RecipeRevision>, ListRecipeRevisionsError>;

        async fn add_source_documents(
            &self,
            images: Vec<RecipeSourceDocument>,
        ) -> Result<(), AddSourceDocumentsError>;

        async fn get_source_documents(
            &self,
            recipe_id: RecipeId,
        ) -> Result<Vec<RecipeSourceDocumentSummary>, ListSourceDocumentsError>;

        async fn get_source_document(
            &self,
            recipe_id: RecipeId,
            image_id: RecipeSourceDocumentId,
        ) -> Result<Option<RecipeSourceDocument>, GetSourceDocumentError>;

        async fn delete_source_document(
            &self,
            recipe_id: RecipeId,
            image_id: RecipeSourceDocumentId,
        ) -> Result<(), DeleteSourceDocumentError>;

        async fn get_recipe_images(
            &self,
            recipe_id: RecipeId,
        ) -> Result<Vec<RecipeImageMetadata>, ListRecipeImagesError>;

        async fn get_recipe_image(
            &self,
            recipe_id: RecipeId,
            image_id: RecipeImageId,
        ) -> Result<Option<RecipeImage>, GetRecipeImageError>;
    }
}
