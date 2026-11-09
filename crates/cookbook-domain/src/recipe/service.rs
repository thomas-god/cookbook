use uuid::Uuid;

use crate::recipe::models::{Recipe, RecipeId};
use crate::recipe::ports::{
    CreateRecipeError, CreateRecipeRequest, GetRecipesError, RecipeRepository, RecipeService,
};

#[derive(Debug, Clone)]
pub struct RecipeServiceImpl<R> {
    repository: R,
}

impl<R> RecipeServiceImpl<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
}

impl<R> RecipeService for RecipeServiceImpl<R>
where
    R: RecipeRepository,
{
    async fn create_recipe(
        &self,
        request: CreateRecipeRequest,
    ) -> Result<Recipe, CreateRecipeError> {
        let recipe = Recipe::new(
            RecipeId::new(Uuid::new_v4().to_string()),
            request.title,
            request.description,
            request.metadata,
            request.ingredients,
            request.steps,
        );

        self.repository
            .save_recipe(recipe.clone())
            .await
            .map_err(|error| CreateRecipeError::Storage(error.into()))?;

        Ok(recipe)
    }

    async fn get_recipes(&self) -> Result<Vec<Recipe>, GetRecipesError> {
        self.repository
            .get_recipes()
            .await
            .map_err(|error| GetRecipesError::Storage(error.into()))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use anyhow::anyhow;

    use super::*;
    use crate::recipe::models::{RecipeMetadata, RecipeTitle};
    use crate::recipe::ports::{MockRecipeRepository, SaveRecipeError};

    fn request() -> CreateRecipeRequest {
        CreateRecipeRequest {
            title: RecipeTitle::new("Soup".to_string()),
            description: None,
            metadata: RecipeMetadata::new(None, None, None),
            ingredients: HashMap::new(),
            steps: Vec::new(),
        }
    }

    #[tokio::test]
    async fn create_recipe_saves_and_returns_the_recipe() {
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_save_recipe()
            .times(1)
            .returning(|_| Ok(()));

        let service = RecipeServiceImpl::new(repository);

        service.create_recipe(request()).await.unwrap();
    }

    #[tokio::test]
    async fn create_recipe_generates_a_unique_id_for_each_recipe() {
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_save_recipe()
            .times(2)
            .returning(|_| Ok(()));

        let service = RecipeServiceImpl::new(repository);

        let first = service.create_recipe(request()).await.unwrap();
        let second = service.create_recipe(request()).await.unwrap();

        assert_ne!(first.id(), second.id());
    }

    #[tokio::test]
    async fn create_recipe_maps_save_failures_to_storage_errors() {
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_save_recipe()
            .returning(|_| Err(SaveRecipeError::Storage(anyhow!("store unavailable"))));

        let service = RecipeServiceImpl::new(repository);

        let result = service.create_recipe(request()).await;

        assert!(matches!(result, Err(CreateRecipeError::Storage(_))));
    }

    #[tokio::test]
    async fn get_recipes_returns_the_repository_recipes() {
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_get_recipes()
            .times(1)
            .returning(|| Ok(Vec::new()));

        let service = RecipeServiceImpl::new(repository);

        let recipes = service.get_recipes().await.unwrap();

        assert!(recipes.is_empty());
    }

    #[tokio::test]
    async fn get_recipes_maps_listing_failures_to_storage_errors() {
        let mut repository = MockRecipeRepository::new();
        repository.expect_get_recipes().returning(|| {
            Err(crate::recipe::ports::ListRecipesError::Storage(anyhow!(
                "boom"
            )))
        });

        let service = RecipeServiceImpl::new(repository);

        let result = service.get_recipes().await;

        assert!(matches!(result, Err(GetRecipesError::Storage(_))));
    }
}
