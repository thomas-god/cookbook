use std::fmt;
use std::future::Future;

use crate::domain::recipe::models::Recipe;

/// Inbound (driving) port for the recipe domain service.
pub trait RecipeService: Clone + Send + Sync + 'static {
    fn create_recipe(
        &self,
        recipe: Recipe,
    ) -> impl Future<Output = Result<Recipe, CreateRecipeError>> + Send;

    fn get_recipes(&self) -> impl Future<Output = Result<Vec<Recipe>, GetRecipesError>> + Send;
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
