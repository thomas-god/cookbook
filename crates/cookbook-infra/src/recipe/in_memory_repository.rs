use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use anyhow::anyhow;

use cookbook_domain::recipe::models::{Recipe, RecipeId};
use cookbook_domain::recipe::ports::{
    GetRecipeError, ListRecipesError, RecipeRepository, SaveRecipeError,
};

#[derive(Debug, Clone, Default)]
pub struct InMemoryRecipeRepository {
    recipes: Arc<RwLock<HashMap<RecipeId, Recipe>>>,
}

impl InMemoryRecipeRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RecipeRepository for InMemoryRecipeRepository {
    async fn save_recipe(&self, recipe: Recipe) -> Result<(), SaveRecipeError> {
        let mut recipes = self
            .recipes
            .write()
            .map_err(|_| SaveRecipeError::Storage(anyhow!("recipe store lock poisoned")))?;

        recipes.insert(recipe.id().clone(), recipe);

        Ok(())
    }

    async fn get_recipe(&self, id: RecipeId) -> Result<Option<Recipe>, GetRecipeError> {
        let recipes = self
            .recipes
            .read()
            .map_err(|_| GetRecipeError::Storage(anyhow!("recipe store lock poisoned")))?;

        Ok(recipes.get(&id).cloned())
    }

    async fn get_recipes(&self) -> Result<Vec<Recipe>, ListRecipesError> {
        let recipes = self
            .recipes
            .read()
            .map_err(|_| ListRecipesError::Storage(anyhow!("recipe store lock poisoned")))?;

        Ok(recipes.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use cookbook_domain::recipe::models::{RecipeId, RecipeMetadata, RecipeTitle};

    fn recipe(id: &str) -> Recipe {
        Recipe::new(
            RecipeId::new(id.to_string()),
            RecipeTitle::new("Soup".to_string()),
            None,
            RecipeMetadata::new(None, None, None),
            HashMap::new(),
            Vec::new(),
        )
    }

    #[tokio::test]
    async fn saves_and_gets_a_recipe_by_id() {
        let repository = InMemoryRecipeRepository::new();
        let id = RecipeId::new("soup".to_string());

        repository.save_recipe(recipe("soup")).await.unwrap();

        let stored = repository.get_recipe(id.clone()).await.unwrap();

        assert_eq!(stored.map(|recipe| recipe.id().clone()), Some(id));
    }

    #[tokio::test]
    async fn returns_none_for_an_unknown_recipe() {
        let repository = InMemoryRecipeRepository::new();

        let stored = repository
            .get_recipe(RecipeId::new("unknown".to_string()))
            .await
            .unwrap();

        assert!(stored.is_none());
    }

    #[tokio::test]
    async fn returns_every_saved_recipe() {
        let repository = InMemoryRecipeRepository::new();

        repository.save_recipe(recipe("soup")).await.unwrap();
        repository.save_recipe(recipe("stew")).await.unwrap();

        let stored = repository.get_recipes().await.unwrap();

        assert_eq!(stored.len(), 2);
    }

    #[tokio::test]
    async fn saving_an_existing_recipe_replaces_it() {
        let repository = InMemoryRecipeRepository::new();
        let id = RecipeId::new("soup".to_string());

        repository.save_recipe(recipe("soup")).await.unwrap();
        repository.save_recipe(recipe("soup")).await.unwrap();

        let stored = repository.get_recipes().await.unwrap();

        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].id(), &id);
    }
}
