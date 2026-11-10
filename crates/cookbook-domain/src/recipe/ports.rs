use std::fmt;
use std::future::Future;

use crate::recipe::models::{
    NewRecipeSourceImage, Recipe, RecipeContent, RecipeId, RecipeRevision, RecipeRevisionId,
    RecipeSourceImage, RecipeSourceImageId, RecipeSourceImageSummary, RecipeWithContent,
    RevisionMessage,
};
use crate::user::models::UserId;

#[derive(Debug, Clone)]
pub struct CreateRecipeRequest {
    pub content: RecipeContent,
    pub author: UserId,
    pub source_images: Vec<NewRecipeSourceImage>,
}

#[derive(Debug, Clone)]
pub struct AddRecipeSourceImagesRequest {
    pub recipe_id: RecipeId,
    pub added_by: UserId,
    pub images: Vec<NewRecipeSourceImage>,
}

#[derive(Debug, Clone)]
pub struct UpdateRecipeRequest {
    pub id: RecipeId,
    pub content: RecipeContent,
    pub editor: UserId,
    pub latest_revision: RecipeRevisionId,
    pub message: Option<RevisionMessage>,
}

/// Inbound (driving) port for the recipe domain service.
pub trait RecipeService: Clone + Send + Sync + 'static {
    fn create_recipe(
        &self,
        request: CreateRecipeRequest,
    ) -> impl Future<Output = Result<RecipeWithContent, CreateRecipeError>> + Send;

    fn update_recipe(
        &self,
        request: UpdateRecipeRequest,
    ) -> impl Future<Output = Result<RecipeWithContent, UpdateRecipeError>> + Send;

    fn get_recipes(
        &self,
    ) -> impl Future<Output = Result<Vec<RecipeWithContent>, GetRecipesError>> + Send;

    fn get_recipe(
        &self,
        id: RecipeId,
    ) -> impl Future<Output = Result<Option<RecipeWithContent>, GetRecipeError>> + Send;

    fn get_recipe_history(
        &self,
        id: RecipeId,
    ) -> impl Future<Output = Result<Vec<RecipeRevision>, GetRecipeHistoryError>> + Send;

    fn add_source_images(
        &self,
        request: AddRecipeSourceImagesRequest,
    ) -> impl Future<Output = Result<Vec<RecipeSourceImageSummary>, AddRecipeSourceImagesError>> + Send;

    fn get_source_images(
        &self,
        recipe_id: RecipeId,
    ) -> impl Future<Output = Result<Vec<RecipeSourceImageSummary>, GetRecipeSourceImagesError>> + Send;

    fn get_source_image(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeSourceImageId,
    ) -> impl Future<Output = Result<Option<RecipeSourceImage>, GetRecipeSourceImageError>> + Send;

    fn delete_source_image(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeSourceImageId,
    ) -> impl Future<Output = Result<(), DeleteRecipeSourceImageError>> + Send;
}

/// Outbound (driven) port for recipe persistence.
pub trait RecipeRepository: Clone + Send + Sync + 'static {
    /// Saves a new recipe with its first revision and source images atomically.
    fn save_new_recipe(
        &self,
        recipe: Recipe,
        revision: RecipeRevision,
        source_images: Vec<RecipeSourceImage>,
    ) -> impl Future<Output = Result<(), SaveRecipeError>> + Send;

    /// Saves the current recipe and appends its revision atomically.
    fn save_recipe_with_revision(
        &self,
        recipe: Recipe,
        revision: RecipeRevision,
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

    fn add_source_images(
        &self,
        images: Vec<RecipeSourceImage>,
    ) -> impl Future<Output = Result<(), AddSourceImagesError>> + Send;

    fn get_source_images(
        &self,
        recipe_id: RecipeId,
    ) -> impl Future<Output = Result<Vec<RecipeSourceImageSummary>, ListSourceImagesError>> + Send;

    fn get_source_image(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeSourceImageId,
    ) -> impl Future<Output = Result<Option<RecipeSourceImage>, GetSourceImageError>> + Send;

    fn delete_source_image(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeSourceImageId,
    ) -> impl Future<Output = Result<(), DeleteSourceImageError>> + Send;
}

#[derive(Debug)]
pub enum CreateRecipeError {
    Storage(anyhow::Error),
}

impl fmt::Display for CreateRecipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to store recipe: {reason}"),
        }
    }
}

impl std::error::Error for CreateRecipeError {}

#[derive(Debug)]
pub enum UpdateRecipeError {
    NotFound,
    Conflict,
    Storage(anyhow::Error),
}

impl fmt::Display for UpdateRecipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe not found"),
            Self::Conflict => f.write_str("the recipe was changed since it was loaded"),
            Self::Storage(reason) => write!(f, "failed to update recipe: {reason}"),
        }
    }
}

impl std::error::Error for UpdateRecipeError {}

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
    Storage(anyhow::Error),
}

impl fmt::Display for GetRecipeHistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read recipe history: {reason}"),
        }
    }
}

impl std::error::Error for GetRecipeHistoryError {}

#[derive(Debug)]
pub enum SaveRecipeError {
    RecipeIdMismatch,
    RecipeAuthorMismatch,
    Storage(anyhow::Error),
}

impl fmt::Display for SaveRecipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecipeIdMismatch => f.write_str("revision does not belong to the recipe"),
            Self::RecipeAuthorMismatch => f.write_str("a recipe author cannot be changed"),
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
pub enum AddRecipeSourceImagesError {
    RecipeNotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for AddRecipeSourceImagesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecipeNotFound => f.write_str("recipe not found"),
            Self::Storage(reason) => write!(f, "failed to store source images: {reason}"),
        }
    }
}

impl std::error::Error for AddRecipeSourceImagesError {}

#[derive(Debug)]
pub enum GetRecipeSourceImagesError {
    Storage(anyhow::Error),
}

impl fmt::Display for GetRecipeSourceImagesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read source images: {reason}"),
        }
    }
}

impl std::error::Error for GetRecipeSourceImagesError {}

#[derive(Debug)]
pub enum GetRecipeSourceImageError {
    Storage(anyhow::Error),
}

impl fmt::Display for GetRecipeSourceImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read source image: {reason}"),
        }
    }
}

impl std::error::Error for GetRecipeSourceImageError {}

#[derive(Debug)]
pub enum DeleteRecipeSourceImageError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for DeleteRecipeSourceImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("source image not found"),
            Self::Storage(reason) => write!(f, "failed to delete source image: {reason}"),
        }
    }
}

impl std::error::Error for DeleteRecipeSourceImageError {}

#[derive(Debug)]
pub enum AddSourceImagesError {
    RecipeNotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for AddSourceImagesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecipeNotFound => f.write_str("recipe not found"),
            Self::Storage(reason) => write!(f, "failed to store source images: {reason}"),
        }
    }
}

impl std::error::Error for AddSourceImagesError {}

#[derive(Debug)]
pub enum ListSourceImagesError {
    Storage(anyhow::Error),
}

impl fmt::Display for ListSourceImagesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read source images: {reason}"),
        }
    }
}

impl std::error::Error for ListSourceImagesError {}

#[derive(Debug)]
pub enum GetSourceImageError {
    Storage(anyhow::Error),
}

impl fmt::Display for GetSourceImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read source image: {reason}"),
        }
    }
}

impl std::error::Error for GetSourceImageError {}

#[derive(Debug)]
pub enum DeleteSourceImageError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for DeleteSourceImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("source image not found"),
            Self::Storage(reason) => write!(f, "failed to delete source image: {reason}"),
        }
    }
}

impl std::error::Error for DeleteSourceImageError {}

#[cfg(test)]
mockall::mock! {
    pub RecipeService {}

    impl Clone for RecipeService {
        fn clone(&self) -> Self;
    }

    impl RecipeService for RecipeService {
        async fn create_recipe(
            &self,
            request: CreateRecipeRequest,
        ) -> Result<RecipeWithContent, CreateRecipeError>;

        async fn update_recipe(
            &self,
            request: UpdateRecipeRequest,
        ) -> Result<RecipeWithContent, UpdateRecipeError>;

        async fn get_recipes(&self) -> Result<Vec<RecipeWithContent>, GetRecipesError>;

        async fn get_recipe(
            &self,
            id: RecipeId,
        ) -> Result<Option<RecipeWithContent>, GetRecipeError>;

        async fn get_recipe_history(
            &self,
            id: RecipeId,
        ) -> Result<Vec<RecipeRevision>, GetRecipeHistoryError>;

        async fn add_source_images(
            &self,
            request: AddRecipeSourceImagesRequest,
        ) -> Result<Vec<RecipeSourceImageSummary>, AddRecipeSourceImagesError>;

        async fn get_source_images(
            &self,
            recipe_id: RecipeId,
        ) -> Result<Vec<RecipeSourceImageSummary>, GetRecipeSourceImagesError>;

        async fn get_source_image(
            &self,
            recipe_id: RecipeId,
            image_id: RecipeSourceImageId,
        ) -> Result<Option<RecipeSourceImage>, GetRecipeSourceImageError>;

        async fn delete_source_image(
            &self,
            recipe_id: RecipeId,
            image_id: RecipeSourceImageId,
        ) -> Result<(), DeleteRecipeSourceImageError>;
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
            source_images: Vec<RecipeSourceImage>,
        ) -> Result<(), SaveRecipeError>;

        async fn save_recipe_with_revision(
            &self,
            recipe: Recipe,
            revision: RecipeRevision,
        ) -> Result<(), SaveRecipeError>;

        async fn get_recipe(
            &self,
            id: RecipeId,
        ) -> Result<Option<RecipeWithContent>, GetRecipeError>;

        async fn get_recipes(&self) -> Result<Vec<RecipeWithContent>, ListRecipesError>;

        async fn get_recipe_revisions(
            &self,
            id: RecipeId,
        ) -> Result<Vec<RecipeRevision>, ListRecipeRevisionsError>;

        async fn add_source_images(
            &self,
            images: Vec<RecipeSourceImage>,
        ) -> Result<(), AddSourceImagesError>;

        async fn get_source_images(
            &self,
            recipe_id: RecipeId,
        ) -> Result<Vec<RecipeSourceImageSummary>, ListSourceImagesError>;

        async fn get_source_image(
            &self,
            recipe_id: RecipeId,
            image_id: RecipeSourceImageId,
        ) -> Result<Option<RecipeSourceImage>, GetSourceImageError>;

        async fn delete_source_image(
            &self,
            recipe_id: RecipeId,
            image_id: RecipeSourceImageId,
        ) -> Result<(), DeleteSourceImageError>;
    }
}
