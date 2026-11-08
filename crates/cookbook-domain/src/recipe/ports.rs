use std::collections::HashMap;
use std::fmt;
use std::future::Future;

use crate::recipe::models::{
    IngredientName, Quantity, Recipe, RecipeId, RecipeMetadata, RecipeStep, RecipeTitle,
};

#[derive(Debug, Clone)]
pub struct CreateRecipeRequest {
    pub title: RecipeTitle,
    pub metadata: RecipeMetadata,
    pub ingredients: HashMap<IngredientName, Quantity>,
    pub steps: Vec<RecipeStep>,
}

/// Inbound (driving) port for the recipe domain service.
pub trait RecipeService: Clone + Send + Sync + 'static {
    fn create_recipe(
        &self,
        request: CreateRecipeRequest,
    ) -> impl Future<Output = Result<Recipe, CreateRecipeError>> + Send;

    fn get_recipes(&self) -> impl Future<Output = Result<Vec<Recipe>, GetRecipesError>> + Send;
}

/// Outbound (driven) port for recipe persistence.
pub trait RecipeRepository: Clone + Send + Sync + 'static {
    fn save_recipe(
        &self,
        recipe: Recipe,
    ) -> impl Future<Output = Result<(), SaveRecipeError>> + Send;

    fn get_recipe(
        &self,
        id: RecipeId,
    ) -> impl Future<Output = Result<Option<Recipe>, GetRecipeError>> + Send;

    fn get_recipes(&self) -> impl Future<Output = Result<Vec<Recipe>, ListRecipesError>> + Send;
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
pub enum SaveRecipeError {
    Storage(anyhow::Error),
}

impl fmt::Display for SaveRecipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
        ) -> Result<Recipe, CreateRecipeError>;

        async fn get_recipes(&self) -> Result<Vec<Recipe>, GetRecipesError>;
    }
}

#[cfg(test)]
mockall::mock! {
    pub RecipeRepository {}

    impl Clone for RecipeRepository {
        fn clone(&self) -> Self;
    }

    impl RecipeRepository for RecipeRepository {
        async fn save_recipe(&self, recipe: Recipe) -> Result<(), SaveRecipeError>;

        async fn get_recipe(
            &self,
            id: RecipeId,
        ) -> Result<Option<Recipe>, GetRecipeError>;

        async fn get_recipes(&self) -> Result<Vec<Recipe>, ListRecipesError>;
    }
}
