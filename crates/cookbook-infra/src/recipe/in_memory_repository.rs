use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use anyhow::anyhow;
use uuid::Uuid;

use cookbook_domain::recipe::models::{
    DurationMinutes, IngredientName, Quantity, Recipe, RecipeContent, RecipeDescription, RecipeId,
    RecipeMetadata, RecipeRevision, RecipeRevisionId, RecipeStep, RecipeTitle, RecipeWithContent,
    Unit,
};
use cookbook_domain::recipe::ports::{
    GetRecipeError, ListRecipeRevisionsError, ListRecipesError, RecipeRepository, SaveRecipeError,
};
use cookbook_domain::user::models::UserId;

#[derive(Debug, Clone)]
struct RecipeRecord {
    recipe: Recipe,
    revisions: Vec<RecipeRevision>,
}

impl RecipeRecord {
    fn current(&self) -> anyhow::Result<RecipeWithContent> {
        let revision = self
            .revisions
            .last()
            .cloned()
            .ok_or_else(|| anyhow!("recipe has no revisions"))?;

        Ok(RecipeWithContent::new(self.recipe.clone(), revision)?)
    }
}

#[derive(Debug, Clone, Default)]
struct RecipeStore {
    recipes: HashMap<RecipeId, RecipeRecord>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryRecipeRepository {
    store: Arc<RwLock<RecipeStore>>,
}

impl InMemoryRecipeRepository {
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a repository pre-populated with a few sample recipes.
    pub fn seeded() -> Self {
        Self::from_current_recipes(seed_recipes())
    }

    fn from_current_recipes(recipes: impl IntoIterator<Item = RecipeWithContent>) -> Self {
        let mut store = RecipeStore::default();

        for current in recipes {
            store.recipes.insert(
                current.id().clone(),
                RecipeRecord {
                    recipe: current.recipe().clone(),
                    revisions: vec![current.revision().clone()],
                },
            );
        }

        Self {
            store: Arc::new(RwLock::new(store)),
        }
    }
}

fn seeded_recipe(
    id: &str,
    title: &str,
    description: &str,
    metadata: RecipeMetadata,
    ingredients: HashMap<IngredientName, Quantity>,
    steps: Vec<RecipeStep>,
) -> RecipeWithContent {
    let recipe = Recipe::new(RecipeId::new(id.to_string()), seed_author());
    let content = RecipeContent::new(
        RecipeTitle::new(title.to_string()),
        Some(RecipeDescription::new(description.to_string())),
        metadata,
        ingredients,
        steps,
    );
    let revision = RecipeRevision::new(
        RecipeRevisionId::new(Uuid::new_v4().to_string()),
        recipe.id().clone(),
        content,
        recipe.author().clone(),
        SystemTime::now(),
        None,
    );

    RecipeWithContent::new(recipe, revision).expect("seed revision belongs to its recipe")
}

fn seed_recipes() -> Vec<RecipeWithContent> {
    vec![
        seeded_recipe(
            "seed-tomato-soup",
            "Tomato Soup",
            "A comforting weeknight soup made with ripe tomatoes.",
            RecipeMetadata::new(
                Some(DurationMinutes::new(10)),
                None,
                Some(DurationMinutes::new(30)),
            ),
            HashMap::from([
                (
                    IngredientName::new("tomatoes".to_string()),
                    Quantity::from_measure(800.0, Unit::Gram),
                ),
                (
                    IngredientName::new("vegetable stock".to_string()),
                    Quantity::from_measure(0.5, Unit::Litre),
                ),
                (
                    IngredientName::new("olive oil".to_string()),
                    Quantity::from_measure(30.0, Unit::MilliLitre),
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
        seeded_recipe(
            "seed-lemon-pasta",
            "Lemon Pasta",
            "A bright pasta dish that comes together in minutes.",
            RecipeMetadata::new(
                Some(DurationMinutes::new(10)),
                None,
                Some(DurationMinutes::new(12)),
            ),
            HashMap::from([
                (
                    IngredientName::new("spaghetti".to_string()),
                    Quantity::from_measure(400.0, Unit::Gram),
                ),
                (
                    IngredientName::new("lemon".to_string()),
                    Quantity::from_measure(100.0, Unit::Gram),
                ),
                (
                    IngredientName::new("olive oil".to_string()),
                    Quantity::from_measure(30.0, Unit::MilliLitre),
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
        seeded_recipe(
            "seed-banana-bread",
            "Banana Bread",
            "A moist loaf that is perfect for using up ripe bananas.",
            RecipeMetadata::new(
                Some(DurationMinutes::new(15)),
                Some(DurationMinutes::new(10)),
                Some(DurationMinutes::new(55)),
            ),
            HashMap::from([
                (
                    IngredientName::new("bananas".to_string()),
                    Quantity::from_measure(300.0, Unit::Gram),
                ),
                (
                    IngredientName::new("flour".to_string()),
                    Quantity::from_measure(250.0, Unit::Gram),
                ),
                (
                    IngredientName::new("sugar".to_string()),
                    Quantity::from_measure(150.0, Unit::Gram),
                ),
                (
                    IngredientName::new("butter".to_string()),
                    Quantity::from_measure(100.0, Unit::Gram),
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

fn seed_author() -> UserId {
    UserId::new("seed-author".to_string())
}

impl RecipeRepository for InMemoryRecipeRepository {
    async fn save_recipe_with_revision(
        &self,
        recipe: Recipe,
        revision: RecipeRevision,
    ) -> Result<(), SaveRecipeError> {
        if revision.recipe_id() != recipe.id() {
            return Err(SaveRecipeError::RecipeIdMismatch);
        }

        let mut store = self
            .store
            .write()
            .map_err(|_| SaveRecipeError::Storage(anyhow!("recipe store lock poisoned")))?;

        if let Some(record) = store.recipes.get_mut(recipe.id()) {
            if record.recipe != recipe {
                return Err(SaveRecipeError::RecipeAuthorMismatch);
            }

            record.revisions.push(revision);
        } else {
            store.recipes.insert(
                recipe.id().clone(),
                RecipeRecord {
                    recipe,
                    revisions: vec![revision],
                },
            );
        }

        Ok(())
    }

    async fn get_recipe(&self, id: RecipeId) -> Result<Option<RecipeWithContent>, GetRecipeError> {
        let store = self
            .store
            .read()
            .map_err(|_| GetRecipeError::Storage(anyhow!("recipe store lock poisoned")))?;

        store
            .recipes
            .get(&id)
            .map(RecipeRecord::current)
            .transpose()
            .map_err(GetRecipeError::Storage)
    }

    async fn get_recipes(&self) -> Result<Vec<RecipeWithContent>, ListRecipesError> {
        let store = self
            .store
            .read()
            .map_err(|_| ListRecipesError::Storage(anyhow!("recipe store lock poisoned")))?;

        store
            .recipes
            .values()
            .map(RecipeRecord::current)
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(ListRecipesError::Storage)
    }

    async fn get_recipe_revisions(
        &self,
        id: RecipeId,
    ) -> Result<Vec<RecipeRevision>, ListRecipeRevisionsError> {
        let store = self.store.read().map_err(|_| {
            ListRecipeRevisionsError::Storage(anyhow!("recipe store lock poisoned"))
        })?;

        Ok(store
            .recipes
            .get(&id)
            .map(|record| record.revisions.clone())
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::SystemTime;

    use super::*;
    use cookbook_domain::recipe::models::{RecipeId, RecipeMetadata, RecipeTitle};

    fn recipe(id: &str) -> Recipe {
        Recipe::new(
            RecipeId::new(id.to_string()),
            UserId::new("user-1".to_string()),
        )
    }

    fn revision(id: &str, recipe: &Recipe) -> RecipeRevision {
        RecipeRevision::new(
            RecipeRevisionId::new(id.to_string()),
            recipe.id().clone(),
            RecipeContent::new(
                RecipeTitle::new("Soup".to_string()),
                None,
                RecipeMetadata::new(None, None, None),
                HashMap::new(),
                Vec::new(),
            ),
            recipe.author().clone(),
            SystemTime::UNIX_EPOCH,
            None,
        )
    }

    fn current_recipe(id: &str, title: &str) -> RecipeWithContent {
        let recipe = recipe(id);
        let revision = RecipeRevision::new(
            RecipeRevisionId::new(format!("{id}-revision")),
            recipe.id().clone(),
            RecipeContent::new(
                RecipeTitle::new(title.to_string()),
                None,
                RecipeMetadata::new(None, None, None),
                HashMap::new(),
                Vec::new(),
            ),
            recipe.author().clone(),
            SystemTime::UNIX_EPOCH,
            None,
        );

        RecipeWithContent::new(recipe, revision).unwrap()
    }

    async fn save(repository: &InMemoryRecipeRepository, current: RecipeWithContent) {
        repository
            .save_recipe_with_revision(current.recipe().clone(), current.revision().clone())
            .await
            .unwrap();
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
    async fn seeded_recipes_have_an_initial_revision() {
        let repository = InMemoryRecipeRepository::seeded();

        for recipe in repository.get_recipes().await.unwrap() {
            let revisions = repository
                .get_recipe_revisions(recipe.id().clone())
                .await
                .unwrap();

            assert_eq!(revisions.len(), 1);
            assert_eq!(revisions[0].editor(), recipe.author());
            assert!(revisions[0].message().is_none());
        }
    }

    #[tokio::test]
    async fn saves_and_gets_a_recipe_by_id() {
        let repository = InMemoryRecipeRepository::new();
        let id = RecipeId::new("soup".to_string());

        save(&repository, current_recipe("soup", "Soup")).await;

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

        save(&repository, current_recipe("soup", "Soup")).await;
        save(&repository, current_recipe("stew", "Stew")).await;

        let stored = repository.get_recipes().await.unwrap();

        assert_eq!(stored.len(), 2);
    }

    #[tokio::test]
    async fn saving_an_existing_recipe_replaces_its_content() {
        let repository = InMemoryRecipeRepository::new();
        let id = RecipeId::new("soup".to_string());

        save(&repository, current_recipe("soup", "Soup")).await;
        save(&repository, current_recipe("soup", "Stew")).await;

        let stored = repository.get_recipe(id.clone()).await.unwrap().unwrap();

        assert_eq!(stored.id(), &id);
        assert_eq!(stored.title().as_str(), "Stew");
    }

    #[tokio::test]
    async fn appends_revisions_in_order() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let first = revision("revision-1", &recipe);
        let second = revision("revision-2", &recipe);

        repository
            .save_recipe_with_revision(recipe.clone(), first)
            .await
            .unwrap();
        repository
            .save_recipe_with_revision(recipe, second)
            .await
            .unwrap();

        let revisions = repository
            .get_recipe_revisions(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert_eq!(revisions.len(), 2);
        assert_eq!(revisions[0].id().as_str(), "revision-1");
        assert_eq!(revisions[1].id().as_str(), "revision-2");
    }

    #[tokio::test]
    async fn returns_no_revisions_for_an_unknown_recipe() {
        let repository = InMemoryRecipeRepository::new();

        let revisions = repository
            .get_recipe_revisions(RecipeId::new("unknown".to_string()))
            .await
            .unwrap();

        assert!(revisions.is_empty());
    }

    #[tokio::test]
    async fn rejects_a_revision_for_another_recipe() {
        let repository = InMemoryRecipeRepository::new();
        let soup = recipe("soup");
        let revision = revision("salad-revision", &recipe("salad"));

        let result = repository.save_recipe_with_revision(soup, revision).await;

        assert!(matches!(result, Err(SaveRecipeError::RecipeIdMismatch)));
    }

    #[tokio::test]
    async fn rejects_a_different_author_for_an_existing_recipe() {
        let repository = InMemoryRecipeRepository::new();
        let soup = recipe("soup");
        let first_revision = revision("revision-1", &soup);
        repository
            .save_recipe_with_revision(soup, first_revision)
            .await
            .unwrap();

        let other_recipe = Recipe::new(
            RecipeId::new("soup".to_string()),
            UserId::new("user-2".to_string()),
        );
        let second_revision = revision("revision-2", &other_recipe);

        let result = repository
            .save_recipe_with_revision(other_recipe, second_revision)
            .await;

        assert!(matches!(result, Err(SaveRecipeError::RecipeAuthorMismatch)));
    }
}
