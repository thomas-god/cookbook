use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use anyhow::anyhow;
use uuid::Uuid;

use cookbook_domain::recipe::models::{
    DurationMinutes, Ingredient, IngredientName, Quantity, Recipe, RecipeBody, RecipeCategory,
    RecipeContent, RecipeDescription, RecipeDurations, RecipeId, RecipeImage, RecipeImageId,
    RecipeImageMetadata, RecipeRevision, RecipeRevisionId, RecipeSourceImage, RecipeSourceImageId,
    RecipeSourceImageSummary, RecipeStep, RecipeStepId, RecipeTitle, RecipeWithContent,
    RevisionVerification, Servings, Unit,
};
use cookbook_domain::recipe::ports::{
    AddSourceImagesError, DeleteSourceImageError, GetRecipeError, GetRecipeImageError,
    GetSourceImageError, ListRecipeImagesError, ListRecipeRevisionsError, ListRecipesError,
    ListSourceImagesError, RecipeRepository, SaveRecipeError,
};
use cookbook_domain::user::models::UserId;

#[derive(Debug, Clone)]
struct RecipeRecord {
    recipe: Recipe,
    revisions: Vec<RecipeRevision>,
    source_images: Vec<RecipeSourceImage>,
    images: Vec<RecipeImage>,
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
                    source_images: Vec::new(),
                    images: Vec::new(),
                },
            );
        }

        Self {
            store: Arc::new(RwLock::new(store)),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn seeded_recipe(
    id: &str,
    title: &str,
    description: &str,
    servings: Option<Servings>,
    category: Option<RecipeCategory>,
    durations: RecipeDurations,
    ingredients: Vec<Ingredient>,
    steps: Vec<RecipeStep>,
) -> RecipeWithContent {
    let recipe = Recipe::new(RecipeId::new(id.to_string()), seed_author());
    let content = RecipeContent::new_simple(
        RecipeTitle::new(title.to_string()),
        Some(RecipeDescription::new(description.to_string())),
        servings,
        category,
        RecipeBody::new(durations, ingredients, Vec::new(), steps),
    )
    .expect("seed recipe content is valid");
    let revision = RecipeRevision::new(
        RecipeRevisionId::new(Uuid::new_v4().to_string()),
        recipe.id().clone(),
        content,
        recipe.author().clone(),
        SystemTime::now(),
        None,
        RevisionVerification::Verified,
    );

    RecipeWithContent::new(recipe, revision).expect("seed revision belongs to its recipe")
}

fn seed_recipes() -> Vec<RecipeWithContent> {
    vec![
        seeded_recipe(
            "seed-tomato-soup",
            "Tomato Soup",
            "A comforting weeknight soup made with ripe tomatoes.",
            Some(Servings::people(4)),
            Some(RecipeCategory::Main),
            RecipeDurations::new(
                Some(DurationMinutes::new(10)),
                None,
                Some(DurationMinutes::new(30)),
            ),
            vec![
                Ingredient::new(
                    IngredientName::new("tomatoes".to_string()),
                    Quantity::from_measure(800.0, Unit::Gram),
                ),
                Ingredient::new(
                    IngredientName::new("vegetable stock".to_string()),
                    Quantity::from_measure(0.5, Unit::Litre),
                ),
                Ingredient::new(
                    IngredientName::new("olive oil".to_string()),
                    Quantity::from_measure(30.0, Unit::MilliLitre),
                ),
            ],
            vec![
                RecipeStep::new(
                    RecipeStepId::generate(),
                    Some("Soften the aromatics".to_string()),
                    "Warm the olive oil in a pot and cook the tomatoes until soft.".to_string(),
                    Vec::new(),
                ),
                RecipeStep::new(
                    RecipeStepId::generate(),
                    None,
                    "Add the stock, simmer for 20 minutes, then blend until smooth.".to_string(),
                    Vec::new(),
                ),
            ],
        ),
        seeded_recipe(
            "seed-lemon-pasta",
            "Lemon Pasta",
            "A bright pasta dish that comes together in minutes.",
            Some(Servings::people(2)),
            Some(RecipeCategory::Main),
            RecipeDurations::new(
                Some(DurationMinutes::new(10)),
                None,
                Some(DurationMinutes::new(12)),
            ),
            vec![
                Ingredient::new(
                    IngredientName::new("spaghetti".to_string()),
                    Quantity::from_measure(400.0, Unit::Gram),
                ),
                Ingredient::new(
                    IngredientName::new("lemon".to_string()),
                    Quantity::from_measure(100.0, Unit::Gram),
                ),
                Ingredient::new(
                    IngredientName::new("olive oil".to_string()),
                    Quantity::from_measure(30.0, Unit::MilliLitre),
                ),
            ],
            vec![
                RecipeStep::new(
                    RecipeStepId::generate(),
                    None,
                    "Cook the spaghetti in salted water until al dente.".to_string(),
                    Vec::new(),
                ),
                RecipeStep::new(
                    RecipeStepId::generate(),
                    None,
                    "Toss with lemon zest, lemon juice and olive oil.".to_string(),
                    Vec::new(),
                ),
            ],
        ),
        seeded_recipe(
            "seed-banana-bread",
            "Banana Bread",
            "A moist loaf that is perfect for using up ripe bananas.",
            Some(Servings::portions_with_label(12, "slices")),
            Some(RecipeCategory::Dessert),
            RecipeDurations::new(
                Some(DurationMinutes::new(15)),
                Some(DurationMinutes::new(10)),
                Some(DurationMinutes::new(55)),
            ),
            vec![
                Ingredient::new(
                    IngredientName::new("bananas".to_string()),
                    Quantity::from_measure(300.0, Unit::Gram),
                ),
                Ingredient::new(
                    IngredientName::new("flour".to_string()),
                    Quantity::from_measure(250.0, Unit::Gram),
                ),
                Ingredient::new(
                    IngredientName::new("sugar".to_string()),
                    Quantity::from_measure(150.0, Unit::Gram),
                ),
                Ingredient::new(
                    IngredientName::new("butter".to_string()),
                    Quantity::from_measure(100.0, Unit::Gram),
                ),
            ],
            vec![
                RecipeStep::new(
                    RecipeStepId::generate(),
                    None,
                    "Mash the bananas and mix them with the melted butter and sugar.".to_string(),
                    Vec::new(),
                ),
                RecipeStep::new(
                    RecipeStepId::generate(),
                    None,
                    "Fold in the flour, pour into a loaf tin and bake until golden.".to_string(),
                    Vec::new(),
                ),
            ],
        ),
    ]
}

fn seed_author() -> UserId {
    UserId::new("seed-author".to_string())
}

impl RecipeRepository for InMemoryRecipeRepository {
    async fn save_new_recipe(
        &self,
        recipe: Recipe,
        revision: RecipeRevision,
        source_images: Vec<RecipeSourceImage>,
        images: Vec<RecipeImage>,
    ) -> Result<(), SaveRecipeError> {
        if revision.recipe_id() != recipe.id() {
            return Err(SaveRecipeError::RecipeIdMismatch);
        }

        if let Some(image) = source_images
            .iter()
            .find(|image| image.recipe_id() != recipe.id())
        {
            return Err(SaveRecipeError::Storage(anyhow!(
                "source image `{}` does not belong to recipe `{}`",
                image.id().as_str(),
                recipe.id().as_str()
            )));
        }

        validate_images(recipe.id(), &images)?;

        let mut store = self
            .store
            .write()
            .map_err(|_| SaveRecipeError::Storage(anyhow!("recipe store lock poisoned")))?;

        if store.recipes.contains_key(recipe.id()) {
            return Err(SaveRecipeError::Storage(anyhow!(
                "recipe `{}` already exists",
                recipe.id().as_str()
            )));
        }

        store.recipes.insert(
            recipe.id().clone(),
            RecipeRecord {
                recipe,
                revisions: vec![revision],
                source_images,
                images,
            },
        );

        Ok(())
    }

    async fn save_recipe_with_revision(
        &self,
        recipe: Recipe,
        revision: RecipeRevision,
        added_images: Vec<RecipeImage>,
    ) -> Result<(), SaveRecipeError> {
        if revision.recipe_id() != recipe.id() {
            return Err(SaveRecipeError::RecipeIdMismatch);
        }

        validate_images(recipe.id(), &added_images)?;

        let mut store = self
            .store
            .write()
            .map_err(|_| SaveRecipeError::Storage(anyhow!("recipe store lock poisoned")))?;

        if let Some(record) = store.recipes.get_mut(recipe.id()) {
            if record.recipe.author() != recipe.author() {
                return Err(SaveRecipeError::RecipeAuthorMismatch);
            }

            if added_images.iter().any(|image| {
                record
                    .images
                    .iter()
                    .any(|existing| existing.id() == image.id())
            }) {
                return Err(SaveRecipeError::Storage(anyhow!(
                    "a recipe image id already exists for recipe `{}`",
                    recipe.id().as_str()
                )));
            }

            record.revisions.push(revision);
            record.images.extend(added_images);
        } else {
            store.recipes.insert(
                recipe.id().clone(),
                RecipeRecord {
                    recipe,
                    revisions: vec![revision],
                    source_images: Vec::new(),
                    images: added_images,
                },
            );
        }

        Ok(())
    }

    async fn save_recipe(&self, recipe: Recipe) -> Result<(), SaveRecipeError> {
        let mut store = self
            .store
            .write()
            .map_err(|_| SaveRecipeError::Storage(anyhow!("recipe store lock poisoned")))?;

        let record = store
            .recipes
            .get_mut(recipe.id())
            .ok_or(SaveRecipeError::RecipeNotFound)?;

        if record.recipe.author() != recipe.author() {
            return Err(SaveRecipeError::RecipeAuthorMismatch);
        }

        record.recipe = record.recipe.clone().update_scope(recipe.scope());

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

    async fn add_source_images(
        &self,
        images: Vec<RecipeSourceImage>,
    ) -> Result<(), AddSourceImagesError> {
        if images.is_empty() {
            return Ok(());
        }

        let mut store = self
            .store
            .write()
            .map_err(|_| AddSourceImagesError::Storage(anyhow!("recipe store lock poisoned")))?;

        let recipe_id = images[0].recipe_id().clone();
        let recipe = store
            .recipes
            .get_mut(&recipe_id)
            .ok_or(AddSourceImagesError::RecipeNotFound)?;

        if let Some(image) = images.iter().find(|image| image.recipe_id() != &recipe_id) {
            return Err(AddSourceImagesError::Storage(anyhow!(
                "source images belong to different recipes: `{}` and `{}`",
                recipe_id.as_str(),
                image.recipe_id().as_str()
            )));
        }

        recipe.source_images.extend(images);

        Ok(())
    }

    async fn get_source_images(
        &self,
        recipe_id: RecipeId,
    ) -> Result<Vec<RecipeSourceImageSummary>, ListSourceImagesError> {
        let store = self
            .store
            .read()
            .map_err(|_| ListSourceImagesError::Storage(anyhow!("recipe store lock poisoned")))?;

        Ok(store
            .recipes
            .get(&recipe_id)
            .map(|record| {
                record
                    .source_images
                    .iter()
                    .map(RecipeSourceImage::summary)
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn get_source_image(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeSourceImageId,
    ) -> Result<Option<RecipeSourceImage>, GetSourceImageError> {
        let store = self
            .store
            .read()
            .map_err(|_| GetSourceImageError::Storage(anyhow!("recipe store lock poisoned")))?;

        Ok(store.recipes.get(&recipe_id).and_then(|record| {
            record
                .source_images
                .iter()
                .find(|image| image.id() == &image_id)
                .cloned()
        }))
    }

    async fn delete_source_image(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeSourceImageId,
    ) -> Result<(), DeleteSourceImageError> {
        let mut store = self
            .store
            .write()
            .map_err(|_| DeleteSourceImageError::Storage(anyhow!("recipe store lock poisoned")))?;

        let recipe = store
            .recipes
            .get_mut(&recipe_id)
            .ok_or(DeleteSourceImageError::NotFound)?;
        let index = recipe
            .source_images
            .iter()
            .position(|image| image.id() == &image_id)
            .ok_or(DeleteSourceImageError::NotFound)?;

        recipe.source_images.remove(index);

        Ok(())
    }

    async fn get_recipe_images(
        &self,
        recipe_id: RecipeId,
    ) -> Result<Vec<RecipeImageMetadata>, ListRecipeImagesError> {
        let store = self
            .store
            .read()
            .map_err(|_| ListRecipeImagesError::Storage(anyhow!("recipe store lock poisoned")))?;

        Ok(store
            .recipes
            .get(&recipe_id)
            .map(|record| {
                record
                    .images
                    .iter()
                    .map(|image| image.metadata().clone())
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn get_recipe_image(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeImageId,
    ) -> Result<Option<RecipeImage>, GetRecipeImageError> {
        let store = self
            .store
            .read()
            .map_err(|_| GetRecipeImageError::Storage(anyhow!("recipe store lock poisoned")))?;

        Ok(store.recipes.get(&recipe_id).and_then(|record| {
            record
                .images
                .iter()
                .find(|image| image.id() == &image_id)
                .cloned()
        }))
    }
}

fn validate_images(recipe_id: &RecipeId, images: &[RecipeImage]) -> Result<(), SaveRecipeError> {
    let mut ids = std::collections::HashSet::with_capacity(images.len());

    for image in images {
        if image.recipe_id() != recipe_id {
            return Err(SaveRecipeError::Storage(anyhow!(
                "image `{}` does not belong to recipe `{}`",
                image.id().as_str(),
                recipe_id.as_str()
            )));
        }

        if !ids.insert(image.id()) {
            return Err(SaveRecipeError::Storage(anyhow!(
                "duplicate image id `{}`",
                image.id().as_str()
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;
    use cookbook_domain::recipe::models::{RecipeDurations, RecipeId, RecipeScope, RecipeTitle};

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
            RecipeContent::new_simple(
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
            .unwrap(),
            recipe.author().clone(),
            SystemTime::UNIX_EPOCH,
            None,
            RevisionVerification::Verified,
        )
    }

    fn current_recipe(id: &str, title: &str) -> RecipeWithContent {
        let recipe = recipe(id);
        let revision = RecipeRevision::new(
            RecipeRevisionId::new(format!("{id}-revision")),
            recipe.id().clone(),
            RecipeContent::new_simple(
                RecipeTitle::new(title.to_string()),
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
            .unwrap(),
            recipe.author().clone(),
            SystemTime::UNIX_EPOCH,
            None,
            RevisionVerification::Verified,
        );

        RecipeWithContent::new(recipe, revision).unwrap()
    }

    async fn save(repository: &InMemoryRecipeRepository, current: RecipeWithContent) {
        repository
            .save_recipe_with_revision(
                current.recipe().clone(),
                current.revision().clone(),
                Vec::new(),
            )
            .await
            .unwrap();
    }

    fn source_image(recipe: &Recipe, id: &str, bytes: &[u8]) -> RecipeSourceImage {
        RecipeSourceImage::new(
            RecipeSourceImageId::new(id.to_string()),
            recipe.id().clone(),
            "image/jpeg".to_string(),
            Some(format!("{id}.jpg")),
            bytes.to_vec(),
            recipe.author().clone(),
            SystemTime::UNIX_EPOCH,
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
            .save_recipe_with_revision(recipe.clone(), first, Vec::new())
            .await
            .unwrap();
        repository
            .save_recipe_with_revision(recipe, second, Vec::new())
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

        let result = repository
            .save_recipe_with_revision(soup, revision, Vec::new())
            .await;

        assert!(matches!(result, Err(SaveRecipeError::RecipeIdMismatch)));
    }

    #[tokio::test]
    async fn rejects_a_different_author_for_an_existing_recipe() {
        let repository = InMemoryRecipeRepository::new();
        let soup = recipe("soup");
        let first_revision = revision("revision-1", &soup);
        repository
            .save_recipe_with_revision(soup, first_revision, Vec::new())
            .await
            .unwrap();

        let other_recipe = Recipe::new(
            RecipeId::new("soup".to_string()),
            UserId::new("user-2".to_string()),
        );
        let second_revision = revision("revision-2", &other_recipe);

        let result = repository
            .save_recipe_with_revision(other_recipe, second_revision, Vec::new())
            .await;

        assert!(matches!(result, Err(SaveRecipeError::RecipeAuthorMismatch)));
    }

    #[tokio::test]
    async fn saves_a_new_recipe_with_its_source_images() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        let image = source_image(&recipe, "image-1", b"front");

        repository
            .save_new_recipe(recipe.clone(), revision, vec![image], Vec::new())
            .await
            .unwrap();

        let images = repository
            .get_source_images(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert_eq!(images.len(), 1);
        assert_eq!(images[0].id().as_str(), "image-1");
        assert_eq!(images[0].content_type(), "image/jpeg");
        assert_eq!(images[0].byte_len(), 5);
    }

    #[tokio::test]
    async fn preserves_the_verification_status_of_a_revision() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = RecipeRevision::new(
            RecipeRevisionId::new("revision-1".to_string()),
            recipe.id().clone(),
            RecipeContent::new_simple(
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
            .unwrap(),
            recipe.author().clone(),
            SystemTime::UNIX_EPOCH,
            None,
            RevisionVerification::NeedsReview,
        );

        repository
            .save_new_recipe(recipe.clone(), revision, Vec::new(), Vec::new())
            .await
            .unwrap();

        let stored = repository
            .get_recipe(RecipeId::new("soup".to_string()))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(stored.verification(), RevisionVerification::NeedsReview);
        assert!(stored.needs_review());
    }

    #[tokio::test]
    async fn rejects_a_new_recipe_image_for_another_recipe() {
        let repository = InMemoryRecipeRepository::new();
        let soup = recipe("soup");
        let revision = revision("revision-1", &soup);
        let salad = recipe("salad");
        let image = source_image(&salad, "image-1", b"front");

        let result = repository
            .save_new_recipe(soup, revision, vec![image], Vec::new())
            .await;

        assert!(matches!(result, Err(SaveRecipeError::Storage(_))));
    }

    #[tokio::test]
    async fn adds_and_lists_source_images() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        repository
            .save_new_recipe(recipe.clone(), revision, Vec::new(), Vec::new())
            .await
            .unwrap();

        repository
            .add_source_images(vec![
                source_image(&recipe, "image-1", b"front"),
                source_image(&recipe, "image-2", b"back"),
            ])
            .await
            .unwrap();

        let images = repository
            .get_source_images(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert_eq!(images.len(), 2);
        assert_eq!(images[0].id().as_str(), "image-1");
        assert_eq!(images[1].id().as_str(), "image-2");
    }

    #[tokio::test]
    async fn gets_a_source_image_by_id() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        repository
            .save_new_recipe(
                recipe.clone(),
                revision,
                vec![source_image(&recipe, "image-1", b"front")],
                Vec::new(),
            )
            .await
            .unwrap();

        let image = repository
            .get_source_image(
                RecipeId::new("soup".to_string()),
                RecipeSourceImageId::new("image-1".to_string()),
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(image.bytes(), b"front");
        assert_eq!(image.original_name(), Some("image-1.jpg"));
    }

    #[tokio::test]
    async fn returns_none_for_an_unknown_source_image() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        repository
            .save_new_recipe(recipe, revision, Vec::new(), Vec::new())
            .await
            .unwrap();

        let image = repository
            .get_source_image(
                RecipeId::new("soup".to_string()),
                RecipeSourceImageId::new("missing".to_string()),
            )
            .await
            .unwrap();

        assert!(image.is_none());
    }

    #[tokio::test]
    async fn deletes_a_source_image() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        repository
            .save_new_recipe(
                recipe.clone(),
                revision,
                vec![source_image(&recipe, "image-1", b"front")],
                Vec::new(),
            )
            .await
            .unwrap();

        repository
            .delete_source_image(
                RecipeId::new("soup".to_string()),
                RecipeSourceImageId::new("image-1".to_string()),
            )
            .await
            .unwrap();

        let images = repository
            .get_source_images(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert!(images.is_empty());
    }

    #[tokio::test]
    async fn rejects_source_images_for_an_unknown_recipe() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let image = source_image(&recipe, "image-1", b"front");

        let result = repository.add_source_images(vec![image]).await;

        assert!(matches!(result, Err(AddSourceImagesError::RecipeNotFound)));
    }

    #[tokio::test]
    async fn deleting_an_unknown_source_image_is_not_found() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        repository
            .save_new_recipe(recipe, revision, Vec::new(), Vec::new())
            .await
            .unwrap();

        let result = repository
            .delete_source_image(
                RecipeId::new("soup".to_string()),
                RecipeSourceImageId::new("missing".to_string()),
            )
            .await;

        assert!(matches!(result, Err(DeleteSourceImageError::NotFound)));
    }

    #[tokio::test]
    async fn unknown_recipes_have_no_source_images() {
        let repository = InMemoryRecipeRepository::new();

        let images = repository
            .get_source_images(RecipeId::new("unknown".to_string()))
            .await
            .unwrap();

        assert!(images.is_empty());
    }

    #[tokio::test]
    async fn saves_the_recipe_scope_without_adding_a_revision() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        repository
            .save_new_recipe(recipe.clone(), revision, Vec::new(), Vec::new())
            .await
            .unwrap();

        let private = recipe.clone().update_scope(RecipeScope::Private);
        repository.save_recipe(private).await.unwrap();

        let stored = repository
            .get_recipe(RecipeId::new("soup".to_string()))
            .await
            .unwrap()
            .unwrap();
        let revisions = repository
            .get_recipe_revisions(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert_eq!(stored.scope(), RecipeScope::Private);
        assert_eq!(revisions.len(), 1);
    }

    #[tokio::test]
    async fn persists_a_scope_change_across_content_revisions() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let first = revision("revision-1", &recipe);
        repository
            .save_new_recipe(recipe.clone(), first, Vec::new(), Vec::new())
            .await
            .unwrap();

        let viewable = recipe.clone().update_scope(RecipeScope::Viewable);
        repository.save_recipe(viewable.clone()).await.unwrap();

        let second = revision("revision-2", &viewable);
        repository
            .save_recipe_with_revision(viewable, second, Vec::new())
            .await
            .unwrap();

        let stored = repository
            .get_recipe(RecipeId::new("soup".to_string()))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(stored.scope(), RecipeScope::Viewable);
        assert_eq!(stored.title().as_str(), "Soup");
    }

    #[tokio::test]
    async fn rejects_a_scope_change_for_an_unknown_recipe() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let recipe = recipe.update_scope(RecipeScope::Private);

        let result = repository.save_recipe(recipe).await;

        assert!(matches!(result, Err(SaveRecipeError::RecipeNotFound)));
    }

    #[tokio::test]
    async fn rejects_a_scope_change_from_a_different_author() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        repository
            .save_new_recipe(recipe, revision, Vec::new(), Vec::new())
            .await
            .unwrap();

        let other = Recipe::new(
            RecipeId::new("soup".to_string()),
            UserId::new("user-2".to_string()),
        )
        .update_scope(RecipeScope::Private);

        let result = repository.save_recipe(other).await;

        assert!(matches!(result, Err(SaveRecipeError::RecipeAuthorMismatch)));
    }

    fn revision_with_step(id: &str, recipe: &Recipe) -> RecipeRevision {
        RecipeRevision::new(
            RecipeRevisionId::new(id.to_string()),
            recipe.id().clone(),
            RecipeContent::new_simple(
                RecipeTitle::new("Soup".to_string()),
                None,
                None,
                None,
                RecipeBody::new(
                    RecipeDurations::new(None, None, None),
                    Vec::new(),
                    Vec::new(),
                    vec![RecipeStep::new(
                        RecipeStepId::generate(),
                        None,
                        "Stir everything together.".to_string(),
                        Vec::new(),
                    )],
                ),
            )
            .unwrap(),
            recipe.author().clone(),
            SystemTime::UNIX_EPOCH,
            None,
            RevisionVerification::Verified,
        )
    }

    fn recipe_image(recipe: &Recipe, id: &str, bytes: &[u8]) -> RecipeImage {
        RecipeImage::new(
            RecipeImageId::new(id.to_string()),
            recipe.id().clone(),
            "image/jpeg".to_string(),
            bytes.to_vec(),
            recipe.author().clone(),
            SystemTime::UNIX_EPOCH,
        )
    }

    #[tokio::test]
    async fn saves_and_lists_recipe_images() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        let image = recipe_image(&recipe, "image-1", b"finished");

        repository
            .save_new_recipe(recipe.clone(), revision, Vec::new(), vec![image])
            .await
            .unwrap();

        let images = repository
            .get_recipe_images(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert_eq!(images.len(), 1);
        assert_eq!(images[0].id().as_str(), "image-1");
        assert_eq!(images[0].byte_len(), 8);
    }

    #[tokio::test]
    async fn rejects_duplicate_image_ids() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        let image = recipe_image(&recipe, "image-1", b"image");

        let result = repository
            .save_new_recipe(recipe, revision, Vec::new(), vec![image.clone(), image])
            .await;

        assert!(matches!(result, Err(SaveRecipeError::Storage(_))));
    }

    #[tokio::test]
    async fn adds_recipe_images_with_a_revision() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let first = revision_with_step("revision-1", &recipe);
        let content = first.content().clone();
        let existing = recipe_image(&recipe, "image-existing", b"existing");
        repository
            .save_new_recipe(recipe.clone(), first, Vec::new(), vec![existing])
            .await
            .unwrap();

        let added = recipe_image(&recipe, "image-added", b"added");
        let second = RecipeRevision::new(
            RecipeRevisionId::new("revision-2".to_string()),
            recipe.id().clone(),
            content,
            recipe.author().clone(),
            SystemTime::UNIX_EPOCH,
            None,
            RevisionVerification::Verified,
        );
        repository
            .save_recipe_with_revision(recipe, second, vec![added])
            .await
            .unwrap();

        let images = repository
            .get_recipe_images(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert_eq!(images.len(), 2);
        assert_eq!(images[1].id().as_str(), "image-added");
    }
}
