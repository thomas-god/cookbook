use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use anyhow::anyhow;

use cookbook_domain::recipe::models::{
    DurationMinutes, IngredientName, Quantity, Recipe, RecipeDescription, RecipeId, RecipeMetadata,
    RecipeStep, RecipeTitle, Unit,
};
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

    /// Creates a repository pre-populated with a few sample recipes.
    pub fn seeded() -> Self {
        Self::from_recipes(seed_recipes())
    }

    fn from_recipes(recipes: impl IntoIterator<Item = Recipe>) -> Self {
        let recipes = recipes
            .into_iter()
            .map(|recipe| (recipe.id().clone(), recipe))
            .collect();

        Self {
            recipes: Arc::new(RwLock::new(recipes)),
        }
    }
}

fn seed_recipes() -> Vec<Recipe> {
    vec![
        Recipe::new(
            RecipeId::new("seed-tomato-soup".to_string()),
            RecipeTitle::new("Tomato Soup".to_string()),
            Some(RecipeDescription::new(
                "A comforting weeknight soup made with ripe tomatoes.".to_string(),
            )),
            RecipeMetadata::new(
                Some(DurationMinutes::new(10)),
                None,
                Some(DurationMinutes::new(30)),
            ),
            HashMap::from([
                (
                    IngredientName::new("tomatoes".to_string()),
                    Quantity::new(800.0, Unit::Gram),
                ),
                (
                    IngredientName::new("vegetable stock".to_string()),
                    Quantity::new(0.5, Unit::Litre),
                ),
                (
                    IngredientName::new("olive oil".to_string()),
                    Quantity::new(30.0, Unit::MilliLitre),
                ),
            ]),
            vec![
                RecipeStep::new(
                    Some("Soften the aromatics".to_string()),
                    Some(HashMap::from([("tomatoes".to_string(), 800.0)])),
                    "Warm the olive oil in a pot and cook the tomatoes until soft.".to_string(),
                ),
                RecipeStep::new(
                    None,
                    None,
                    "Add the stock, simmer for 20 minutes, then blend until smooth.".to_string(),
                ),
            ],
        ),
        Recipe::new(
            RecipeId::new("seed-lemon-pasta".to_string()),
            RecipeTitle::new("Lemon Pasta".to_string()),
            Some(RecipeDescription::new(
                "A bright pasta dish that comes together in minutes.".to_string(),
            )),
            RecipeMetadata::new(
                Some(DurationMinutes::new(10)),
                None,
                Some(DurationMinutes::new(12)),
            ),
            HashMap::from([
                (
                    IngredientName::new("spaghetti".to_string()),
                    Quantity::new(400.0, Unit::Gram),
                ),
                (
                    IngredientName::new("lemon".to_string()),
                    Quantity::new(100.0, Unit::Gram),
                ),
                (
                    IngredientName::new("olive oil".to_string()),
                    Quantity::new(30.0, Unit::MilliLitre),
                ),
            ]),
            vec![
                RecipeStep::new(
                    None,
                    None,
                    "Cook the spaghetti in salted water until al dente.".to_string(),
                ),
                RecipeStep::new(
                    None,
                    Some(HashMap::from([("lemon".to_string(), 100.0)])),
                    "Toss with lemon zest, lemon juice and olive oil.".to_string(),
                ),
            ],
        ),
        Recipe::new(
            RecipeId::new("seed-banana-bread".to_string()),
            RecipeTitle::new("Banana Bread".to_string()),
            Some(RecipeDescription::new(
                "A moist loaf that is perfect for using up ripe bananas.".to_string(),
            )),
            RecipeMetadata::new(
                Some(DurationMinutes::new(15)),
                Some(DurationMinutes::new(10)),
                Some(DurationMinutes::new(55)),
            ),
            HashMap::from([
                (
                    IngredientName::new("bananas".to_string()),
                    Quantity::new(300.0, Unit::Gram),
                ),
                (
                    IngredientName::new("flour".to_string()),
                    Quantity::new(250.0, Unit::Gram),
                ),
                (
                    IngredientName::new("sugar".to_string()),
                    Quantity::new(150.0, Unit::Gram),
                ),
                (
                    IngredientName::new("butter".to_string()),
                    Quantity::new(100.0, Unit::Gram),
                ),
            ]),
            vec![
                RecipeStep::new(
                    None,
                    Some(HashMap::from([("bananas".to_string(), 300.0)])),
                    "Mash the bananas and mix them with the melted butter and sugar.".to_string(),
                ),
                RecipeStep::new(
                    None,
                    None,
                    "Fold in the flour, pour into a loaf tin and bake until golden.".to_string(),
                ),
            ],
        ),
    ]
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
    async fn seeded_repository_contains_the_sample_recipes() {
        let repository = InMemoryRecipeRepository::seeded();

        let mut titles: Vec<_> = repository
            .get_recipes()
            .await
            .unwrap()
            .iter()
            .map(|recipe| recipe.title().as_str().to_string())
            .collect();
        titles.sort();

        assert_eq!(titles, ["Banana Bread", "Lemon Pasta", "Tomato Soup"]);
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
