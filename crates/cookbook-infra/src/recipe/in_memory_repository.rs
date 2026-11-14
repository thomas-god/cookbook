use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use anyhow::anyhow;

use cookbook_domain::recipe::models::{
    Recipe, RecipeId, RecipeImage, RecipeImageId, RecipeImageMetadata, RecipeRevision,
    RecipeSourceDocument, RecipeSourceDocumentId, RecipeSourceDocumentSummary, RecipeWithContent,
};
use cookbook_domain::recipe::ports::{
    AddRecipeImagesError, AddSourceDocumentsError, DeleteRecipeAssetsError,
    DeleteRecipeRecordError, DeleteSourceDocumentError, GetRecipeError, GetRecipeImageError,
    GetSourceDocumentError, ListRecipeImagesError, ListRecipeRevisionsError, ListRecipesError,
    ListSourceDocumentsError, RecipeDocumentRepository, RecipeRepository, RecipeSearchCriteria,
    RecipeSort, SaveRecipeError, SearchRecipesError, SortDirection,
};

#[derive(Debug, Clone, Default)]
struct RecipeStore {
    recipes: HashMap<RecipeId, Recipe>,
    revisions: HashMap<RecipeId, Vec<RecipeRevision>>,
    source_documents: HashMap<RecipeId, Vec<RecipeSourceDocument>>,
    images: HashMap<RecipeId, Vec<RecipeImage>>,
}

impl RecipeStore {
    fn current(&self, id: &RecipeId) -> anyhow::Result<RecipeWithContent> {
        let recipe = self
            .recipes
            .get(id)
            .ok_or_else(|| anyhow!("recipe not found"))?;
        let revision = self
            .revisions
            .get(id)
            .and_then(|revisions| revisions.last())
            .cloned()
            .ok_or_else(|| anyhow!("recipe has no revisions"))?;

        Ok(RecipeWithContent::new(recipe.clone(), revision)?)
    }

    fn current_recipes(&self) -> anyhow::Result<Vec<RecipeWithContent>> {
        self.recipes.keys().map(|id| self.current(id)).collect()
    }
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
        Self::from_current_recipes(crate::recipe::seed::sample_recipes())
    }

    fn from_current_recipes(recipes: impl IntoIterator<Item = RecipeWithContent>) -> Self {
        let mut store = RecipeStore::default();

        for current in recipes {
            let id = current.id().clone();
            store.recipes.insert(id.clone(), current.recipe().clone());
            store.revisions.insert(id, vec![current.revision().clone()]);
        }

        Self {
            store: Arc::new(RwLock::new(store)),
        }
    }
}

impl RecipeRepository for InMemoryRecipeRepository {
    async fn save_recipe(&self, recipe: Recipe) -> Result<(), SaveRecipeError> {
        let mut store = self
            .store
            .write()
            .map_err(|_| SaveRecipeError::Storage(anyhow!("recipe store lock poisoned")))?;

        let stored = store
            .recipes
            .get_mut(recipe.id())
            .ok_or(SaveRecipeError::RecipeNotFound)?;

        if stored.author() != recipe.author() {
            return Err(SaveRecipeError::RecipeAuthorMismatch);
        }

        *stored = stored.clone().update_scope(recipe.scope());

        Ok(())
    }

    async fn save_recipe_revision(
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

        let id = recipe.id().clone();

        if let Some(stored) = store.recipes.get(&id) {
            if *stored != recipe {
                return Err(SaveRecipeError::InvalidRecipeReference);
            }

            store
                .revisions
                .entry(id.clone())
                .or_default()
                .push(revision);
            store.recipes.insert(id, recipe);
        } else {
            store.revisions.insert(id.clone(), vec![revision]);
            store.recipes.insert(id, recipe);
        }

        Ok(())
    }

    async fn get_recipe(&self, id: RecipeId) -> Result<Option<RecipeWithContent>, GetRecipeError> {
        let store = self
            .store
            .read()
            .map_err(|_| GetRecipeError::Storage(anyhow!("recipe store lock poisoned")))?;

        if !store.recipes.contains_key(&id) {
            return Ok(None);
        }

        store
            .current(&id)
            .map(Some)
            .map_err(GetRecipeError::Storage)
    }

    async fn get_recipes(&self) -> Result<Vec<RecipeWithContent>, ListRecipesError> {
        let store = self
            .store
            .read()
            .map_err(|_| ListRecipesError::Storage(anyhow!("recipe store lock poisoned")))?;

        store.current_recipes().map_err(ListRecipesError::Storage)
    }

    async fn search_recipes(
        &self,
        criteria: RecipeSearchCriteria,
    ) -> Result<Vec<RecipeWithContent>, SearchRecipesError> {
        let store = self
            .store
            .read()
            .map_err(|_| SearchRecipesError::Storage(anyhow!("recipe store lock poisoned")))?;

        let mut recipes = store
            .current_recipes()
            .map_err(SearchRecipesError::Storage)?;

        let pattern = criteria
            .pattern
            .as_deref()
            .map(str::trim)
            .filter(|pattern| !pattern.is_empty())
            .map(str::to_lowercase);

        recipes.retain(|recipe| {
            if let Some(pattern) = &pattern
                && !recipe_matches_pattern(recipe, pattern)
            {
                return false;
            }

            if !criteria.categories.is_empty()
                && !recipe
                    .category()
                    .is_some_and(|category| criteria.categories.contains(&category))
            {
                return false;
            }

            !(criteria.exclude_needs_review && recipe.needs_review())
        });

        match criteria.sort {
            RecipeSort::LastEdited(direction) => recipes.sort_by(|a, b| {
                let ordering = a.revision().edited_at().cmp(&b.revision().edited_at());

                directed(ordering, direction)
                    .then_with(|| title_ordering(a, b))
                    .then_with(|| a.id().as_str().cmp(b.id().as_str()))
            }),
            RecipeSort::Title(direction) => recipes.sort_by(|a, b| {
                directed(title_ordering(a, b), direction)
                    .then_with(|| a.id().as_str().cmp(b.id().as_str()))
            }),
        }

        Ok(recipes)
    }

    async fn get_recipe_revisions(
        &self,
        id: RecipeId,
    ) -> Result<Vec<RecipeRevision>, ListRecipeRevisionsError> {
        let store = self.store.read().map_err(|_| {
            ListRecipeRevisionsError::Storage(anyhow!("recipe store lock poisoned"))
        })?;

        Ok(store.revisions.get(&id).cloned().unwrap_or_default())
    }

    async fn delete_recipe(&self, id: RecipeId) -> Result<(), DeleteRecipeRecordError> {
        let mut store = self
            .store
            .write()
            .map_err(|_| DeleteRecipeRecordError::Storage(anyhow!("recipe store lock poisoned")))?;

        if store.recipes.remove(&id).is_none() {
            return Err(DeleteRecipeRecordError::NotFound);
        }

        store.revisions.remove(&id);
        store.source_documents.remove(&id);
        store.images.remove(&id);

        Ok(())
    }
}

impl RecipeDocumentRepository for InMemoryRecipeRepository {
    async fn add_source_documents(
        &self,
        images: Vec<RecipeSourceDocument>,
    ) -> Result<(), AddSourceDocumentsError> {
        if images.is_empty() {
            return Ok(());
        }

        let mut store = self
            .store
            .write()
            .map_err(|_| AddSourceDocumentsError::Storage(anyhow!("recipe store lock poisoned")))?;

        let recipe_id = images[0].recipe_id().clone();
        if !store.recipes.contains_key(&recipe_id) {
            return Err(AddSourceDocumentsError::RecipeNotFound);
        }

        if let Some(image) = images.iter().find(|image| image.recipe_id() != &recipe_id) {
            return Err(AddSourceDocumentsError::Storage(anyhow!(
                "source documents belong to different recipes: `{}` and `{}`",
                recipe_id.as_str(),
                image.recipe_id().as_str()
            )));
        }

        store
            .source_documents
            .entry(recipe_id)
            .or_default()
            .extend(images);

        Ok(())
    }

    async fn get_source_documents(
        &self,
        recipe_id: RecipeId,
    ) -> Result<Vec<RecipeSourceDocumentSummary>, ListSourceDocumentsError> {
        let store = self.store.read().map_err(|_| {
            ListSourceDocumentsError::Storage(anyhow!("recipe store lock poisoned"))
        })?;

        Ok(store
            .source_documents
            .get(&recipe_id)
            .map(|documents| {
                documents
                    .iter()
                    .map(RecipeSourceDocument::summary)
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn get_source_document(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeSourceDocumentId,
    ) -> Result<Option<RecipeSourceDocument>, GetSourceDocumentError> {
        let store = self
            .store
            .read()
            .map_err(|_| GetSourceDocumentError::Storage(anyhow!("recipe store lock poisoned")))?;

        Ok(store
            .source_documents
            .get(&recipe_id)
            .and_then(|documents| {
                documents
                    .iter()
                    .find(|document| document.id() == &image_id)
                    .cloned()
            }))
    }

    async fn delete_source_document(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeSourceDocumentId,
    ) -> Result<(), DeleteSourceDocumentError> {
        let mut store = self.store.write().map_err(|_| {
            DeleteSourceDocumentError::Storage(anyhow!("recipe store lock poisoned"))
        })?;

        if !store.recipes.contains_key(&recipe_id) {
            return Err(DeleteSourceDocumentError::NotFound);
        }

        let documents = store
            .source_documents
            .get_mut(&recipe_id)
            .ok_or(DeleteSourceDocumentError::NotFound)?;
        let index = documents
            .iter()
            .position(|document| document.id() == &image_id)
            .ok_or(DeleteSourceDocumentError::NotFound)?;

        documents.remove(index);

        Ok(())
    }

    async fn add_recipe_images(
        &self,
        images: Vec<RecipeImage>,
    ) -> Result<(), AddRecipeImagesError> {
        if images.is_empty() {
            return Ok(());
        }

        let recipe_id = images[0].recipe_id().clone();

        let mut store = self
            .store
            .write()
            .map_err(|_| AddRecipeImagesError::Storage(anyhow!("recipe store lock poisoned")))?;

        if !store.recipes.contains_key(&recipe_id) {
            return Err(AddRecipeImagesError::RecipeNotFound);
        }

        if let Some(image) = images.iter().find(|image| image.recipe_id() != &recipe_id) {
            return Err(AddRecipeImagesError::Storage(anyhow!(
                "images belong to different recipes: `{}` and `{}`",
                recipe_id.as_str(),
                image.recipe_id().as_str()
            )));
        }

        let mut ids = std::collections::HashSet::with_capacity(images.len());
        for image in &images {
            if !ids.insert(image.id()) {
                return Err(AddRecipeImagesError::Storage(anyhow!(
                    "duplicate image id `{}`",
                    image.id().as_str()
                )));
            }
        }

        let stored = store.images.entry(recipe_id).or_default();
        if images
            .iter()
            .any(|image| stored.iter().any(|existing| existing.id() == image.id()))
        {
            return Err(AddRecipeImagesError::Storage(anyhow!(
                "a recipe image id already exists for recipe `{}`",
                images[0].recipe_id().as_str()
            )));
        }

        stored.extend(images);

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
            .images
            .get(&recipe_id)
            .map(|images| {
                images
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

        Ok(store
            .images
            .get(&recipe_id)
            .and_then(|images| images.iter().find(|image| image.id() == &image_id).cloned()))
    }

    async fn delete_recipe_assets(
        &self,
        _recipe_id: RecipeId,
    ) -> Result<(), DeleteRecipeAssetsError> {
        Ok(())
    }
}

fn directed(ordering: Ordering, direction: SortDirection) -> Ordering {
    match direction {
        SortDirection::Ascending => ordering,
        SortDirection::Descending => ordering.reverse(),
    }
}

fn title_ordering(a: &RecipeWithContent, b: &RecipeWithContent) -> Ordering {
    a.title()
        .as_str()
        .to_lowercase()
        .cmp(&b.title().as_str().to_lowercase())
}

fn recipe_matches_pattern(recipe: &RecipeWithContent, pattern: &str) -> bool {
    let matches = |value: &str| value.to_lowercase().contains(pattern);

    matches(recipe.title().as_str())
        || recipe
            .description()
            .is_some_and(|description| matches(description.as_str()))
        || recipe.bodies().any(|body| {
            body.ingredients()
                .iter()
                .any(|ingredient| matches(ingredient.name().as_str()))
                || body
                    .steps()
                    .iter()
                    .any(|step| step.title().is_some_and(matches) || matches(step.instruction()))
        })
        || recipe.parts().iter().any(|part| {
            matches(part.title().as_str())
                || part
                    .description()
                    .is_some_and(|description| matches(description.as_str()))
        })
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;
    use cookbook_domain::recipe::models::{
        Ingredient, IngredientName, Quantity, RecipeBody, RecipeCategory, RecipeContent,
        RecipeDurations, RecipeRevisionId, RecipeScope, RecipeStep, RecipeStepId, RecipeTitle,
        RevisionOrigin, RevisionVerification,
    };
    use cookbook_domain::user::models::UserId;

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
            RevisionOrigin::Created,
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
            RevisionOrigin::Created,
            RevisionVerification::Verified,
        );

        RecipeWithContent::new(recipe, revision).unwrap()
    }

    #[allow(clippy::too_many_arguments)]
    fn searchable_recipe(
        id: &str,
        title: &str,
        category: Option<RecipeCategory>,
        verification: RevisionVerification,
        edited_at: SystemTime,
        ingredients: Vec<Ingredient>,
        steps: Vec<RecipeStep>,
    ) -> RecipeWithContent {
        let recipe = recipe(id);
        let content = RecipeContent::new_simple(
            RecipeTitle::new(title.to_string()),
            None,
            None,
            category,
            RecipeBody::new(
                RecipeDurations::new(None, None, None),
                ingredients,
                Vec::new(),
                steps,
            ),
        )
        .unwrap();
        let revision = RecipeRevision::new(
            RecipeRevisionId::new(format!("{id}-revision")),
            recipe.id().clone(),
            content,
            recipe.author().clone(),
            edited_at,
            RevisionOrigin::Created,
            verification,
        );

        RecipeWithContent::new(recipe, revision).unwrap()
    }

    fn titles(recipes: &[RecipeWithContent]) -> Vec<&str> {
        recipes
            .iter()
            .map(|recipe| recipe.title().as_str())
            .collect()
    }

    async fn save(repository: &InMemoryRecipeRepository, current: RecipeWithContent) {
        repository
            .save_recipe_revision(current.recipe().clone(), current.revision().clone())
            .await
            .unwrap();
    }

    fn source_document(recipe: &Recipe, id: &str, bytes: &[u8]) -> RecipeSourceDocument {
        RecipeSourceDocument::new(
            RecipeSourceDocumentId::new(id.to_string()),
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

        assert_eq!(
            titles,
            [
                "Banana Bread",
                "Lemon Pasta",
                "Quick Pickled Onions",
                "Sunday Roast Chicken",
                "Tomato Soup",
            ]
        );
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
            assert!(matches!(revisions[0].origin(), RevisionOrigin::Created));
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
    async fn search_returns_every_recipe_by_default() {
        let repository = InMemoryRecipeRepository::new();

        save(&repository, current_recipe("soup", "Tomato Soup")).await;
        save(&repository, current_recipe("stew", "Beef Stew")).await;

        let results = repository
            .search_recipes(RecipeSearchCriteria::default())
            .await
            .unwrap();

        assert_eq!(titles(&results), ["Beef Stew", "Tomato Soup"]);
    }

    #[tokio::test]
    async fn search_matches_title_ingredients_and_steps() {
        let repository = InMemoryRecipeRepository::new();
        save(
            &repository,
            searchable_recipe(
                "soup",
                "Tomato Soup",
                None,
                RevisionVerification::Verified,
                SystemTime::UNIX_EPOCH,
                vec![Ingredient::new(
                    IngredientName::new("basil".to_string()),
                    Quantity::from_text("a handful"),
                )],
                vec![RecipeStep::new(
                    RecipeStepId::generate(),
                    None,
                    "Simmer gently.".to_string(),
                    Vec::new(),
                )],
            ),
        )
        .await;

        for pattern in ["tomato", "BASIL", "simmer"] {
            let results = repository
                .search_recipes(RecipeSearchCriteria {
                    pattern: Some(pattern.to_string()),
                    ..Default::default()
                })
                .await
                .unwrap();

            assert_eq!(titles(&results), ["Tomato Soup"], "pattern `{pattern}`");
        }

        let results = repository
            .search_recipes(RecipeSearchCriteria {
                pattern: Some("missing".to_string()),
                ..Default::default()
            })
            .await
            .unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn search_treats_a_blank_pattern_as_no_filter() {
        let repository = InMemoryRecipeRepository::new();
        save(&repository, current_recipe("soup", "Soup")).await;

        let results = repository
            .search_recipes(RecipeSearchCriteria {
                pattern: Some("   ".to_string()),
                ..Default::default()
            })
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn search_filters_by_category_and_excludes_uncategorized_recipes() {
        let repository = InMemoryRecipeRepository::new();
        let verified = RevisionVerification::Verified;
        save(
            &repository,
            searchable_recipe(
                "soup",
                "Soup",
                Some(RecipeCategory::Main),
                verified,
                SystemTime::UNIX_EPOCH,
                Vec::new(),
                Vec::new(),
            ),
        )
        .await;
        save(
            &repository,
            searchable_recipe(
                "cake",
                "Cake",
                Some(RecipeCategory::Dessert),
                verified,
                SystemTime::UNIX_EPOCH,
                Vec::new(),
                Vec::new(),
            ),
        )
        .await;
        save(
            &repository,
            searchable_recipe(
                "mystery",
                "Mystery",
                None,
                verified,
                SystemTime::UNIX_EPOCH,
                Vec::new(),
                Vec::new(),
            ),
        )
        .await;

        let results = repository
            .search_recipes(RecipeSearchCriteria {
                categories: vec![RecipeCategory::Main],
                ..Default::default()
            })
            .await
            .unwrap();

        assert_eq!(titles(&results), ["Soup"]);
    }

    #[tokio::test]
    async fn search_can_exclude_recipes_that_need_review() {
        let repository = InMemoryRecipeRepository::new();
        save(
            &repository,
            searchable_recipe(
                "verified",
                "Verified",
                None,
                RevisionVerification::Verified,
                SystemTime::UNIX_EPOCH,
                Vec::new(),
                Vec::new(),
            ),
        )
        .await;
        save(
            &repository,
            searchable_recipe(
                "review",
                "Needs Review",
                None,
                RevisionVerification::NeedsReview,
                SystemTime::UNIX_EPOCH,
                Vec::new(),
                Vec::new(),
            ),
        )
        .await;

        let all = repository
            .search_recipes(RecipeSearchCriteria::default())
            .await
            .unwrap();
        let verified = repository
            .search_recipes(RecipeSearchCriteria {
                exclude_needs_review: true,
                ..Default::default()
            })
            .await
            .unwrap();

        assert_eq!(all.len(), 2);
        assert_eq!(titles(&verified), ["Verified"]);
    }

    #[tokio::test]
    async fn search_sorts_by_latest_revision_date_in_both_directions() {
        let repository = InMemoryRecipeRepository::new();
        let older = SystemTime::UNIX_EPOCH;
        let newer = SystemTime::UNIX_EPOCH + Duration::from_secs(60);
        save(
            &repository,
            searchable_recipe(
                "old",
                "Older",
                None,
                RevisionVerification::Verified,
                older,
                Vec::new(),
                Vec::new(),
            ),
        )
        .await;
        save(
            &repository,
            searchable_recipe(
                "new",
                "Newer",
                None,
                RevisionVerification::Verified,
                newer,
                Vec::new(),
                Vec::new(),
            ),
        )
        .await;

        let newest_first = repository
            .search_recipes(RecipeSearchCriteria {
                sort: RecipeSort::LastEdited(SortDirection::Descending),
                ..Default::default()
            })
            .await
            .unwrap();
        let oldest_first = repository
            .search_recipes(RecipeSearchCriteria {
                sort: RecipeSort::LastEdited(SortDirection::Ascending),
                ..Default::default()
            })
            .await
            .unwrap();

        assert_eq!(titles(&newest_first), ["Newer", "Older"]);
        assert_eq!(titles(&oldest_first), ["Older", "Newer"]);
    }

    #[tokio::test]
    async fn search_sorts_titles_case_insensitively_in_both_directions() {
        let repository = InMemoryRecipeRepository::new();
        save(&repository, current_recipe("pie", "apple pie")).await;
        save(&repository, current_recipe("bread", "Banana Bread")).await;

        let ascending = repository
            .search_recipes(RecipeSearchCriteria {
                sort: RecipeSort::Title(SortDirection::Ascending),
                ..Default::default()
            })
            .await
            .unwrap();
        let descending = repository
            .search_recipes(RecipeSearchCriteria {
                sort: RecipeSort::Title(SortDirection::Descending),
                ..Default::default()
            })
            .await
            .unwrap();

        assert_eq!(titles(&ascending), ["apple pie", "Banana Bread"]);
        assert_eq!(titles(&descending), ["Banana Bread", "apple pie"]);
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
            .save_recipe_revision(recipe.clone(), first)
            .await
            .unwrap();
        repository
            .save_recipe_revision(recipe, second)
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

        let result = repository.save_recipe_revision(soup, revision).await;

        assert!(matches!(result, Err(SaveRecipeError::RecipeIdMismatch)));
    }

    #[tokio::test]
    async fn rejects_a_revision_when_the_recipe_author_differs_from_the_stored_recipe() {
        let repository = InMemoryRecipeRepository::new();
        let soup = recipe("soup");
        let first_revision = revision("revision-1", &soup);
        repository
            .save_recipe_revision(soup, first_revision)
            .await
            .unwrap();

        let other_recipe = Recipe::new(
            RecipeId::new("soup".to_string()),
            UserId::new("user-2".to_string()),
        );
        let second_revision = revision("revision-2", &other_recipe);

        let result = repository
            .save_recipe_revision(other_recipe, second_revision)
            .await;

        assert!(matches!(
            result,
            Err(SaveRecipeError::InvalidRecipeReference)
        ));

        let revisions = repository
            .get_recipe_revisions(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert_eq!(revisions.len(), 1);
    }

    #[tokio::test]
    async fn rejects_a_revision_when_the_recipe_scope_differs_from_the_stored_recipe() {
        let repository = InMemoryRecipeRepository::new();
        let soup = recipe("soup");
        let first_revision = revision("revision-1", &soup);
        repository
            .save_recipe_revision(soup.clone(), first_revision)
            .await
            .unwrap();

        let private_soup = soup.update_scope(RecipeScope::Private);
        let second_revision = revision("revision-2", &private_soup);

        let result = repository
            .save_recipe_revision(private_soup, second_revision)
            .await;

        assert!(matches!(
            result,
            Err(SaveRecipeError::InvalidRecipeReference)
        ));

        let stored = repository
            .get_recipe(RecipeId::new("soup".to_string()))
            .await
            .unwrap()
            .unwrap();
        let revisions = repository
            .get_recipe_revisions(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert_eq!(stored.scope(), RecipeScope::default());
        assert_eq!(revisions.len(), 1);
    }

    #[tokio::test]
    async fn saves_a_new_recipe_with_its_source_documents() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        let image = source_document(&recipe, "image-1", b"front");

        repository
            .save_recipe_revision(recipe.clone(), revision)
            .await
            .unwrap();
        repository.add_source_documents(vec![image]).await.unwrap();

        let images = repository
            .get_source_documents(RecipeId::new("soup".to_string()))
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
            RevisionOrigin::Created,
            RevisionVerification::NeedsReview,
        );

        repository
            .save_recipe_revision(recipe.clone(), revision)
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
    async fn adds_and_lists_source_documents() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        repository
            .save_recipe_revision(recipe.clone(), revision)
            .await
            .unwrap();

        repository
            .add_source_documents(vec![
                source_document(&recipe, "image-1", b"front"),
                source_document(&recipe, "image-2", b"back"),
            ])
            .await
            .unwrap();

        let images = repository
            .get_source_documents(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert_eq!(images.len(), 2);
        assert_eq!(images[0].id().as_str(), "image-1");
        assert_eq!(images[1].id().as_str(), "image-2");
    }

    #[tokio::test]
    async fn gets_a_source_document_by_id() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        repository
            .save_recipe_revision(recipe.clone(), revision)
            .await
            .unwrap();
        repository
            .add_source_documents(vec![source_document(&recipe, "image-1", b"front")])
            .await
            .unwrap();

        let image = repository
            .get_source_document(
                RecipeId::new("soup".to_string()),
                RecipeSourceDocumentId::new("image-1".to_string()),
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(image.bytes(), b"front");
        assert_eq!(image.original_name(), Some("image-1.jpg"));
    }

    #[tokio::test]
    async fn returns_none_for_an_unknown_source_document() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        repository
            .save_recipe_revision(recipe, revision)
            .await
            .unwrap();

        let image = repository
            .get_source_document(
                RecipeId::new("soup".to_string()),
                RecipeSourceDocumentId::new("missing".to_string()),
            )
            .await
            .unwrap();

        assert!(image.is_none());
    }

    #[tokio::test]
    async fn deletes_a_source_document() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        repository
            .save_recipe_revision(recipe.clone(), revision)
            .await
            .unwrap();
        repository
            .add_source_documents(vec![source_document(&recipe, "image-1", b"front")])
            .await
            .unwrap();

        repository
            .delete_source_document(
                RecipeId::new("soup".to_string()),
                RecipeSourceDocumentId::new("image-1".to_string()),
            )
            .await
            .unwrap();

        let images = repository
            .get_source_documents(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert!(images.is_empty());
    }

    #[tokio::test]
    async fn rejects_source_documents_for_an_unknown_recipe() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let image = source_document(&recipe, "image-1", b"front");

        let result = repository.add_source_documents(vec![image]).await;

        assert!(matches!(
            result,
            Err(AddSourceDocumentsError::RecipeNotFound)
        ));
    }

    #[tokio::test]
    async fn deleting_an_unknown_source_document_is_not_found() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        repository
            .save_recipe_revision(recipe, revision)
            .await
            .unwrap();

        let result = repository
            .delete_source_document(
                RecipeId::new("soup".to_string()),
                RecipeSourceDocumentId::new("missing".to_string()),
            )
            .await;

        assert!(matches!(result, Err(DeleteSourceDocumentError::NotFound)));
    }

    #[tokio::test]
    async fn unknown_recipes_have_no_source_documents() {
        let repository = InMemoryRecipeRepository::new();

        let images = repository
            .get_source_documents(RecipeId::new("unknown".to_string()))
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
            .save_recipe_revision(recipe.clone(), revision)
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
            .save_recipe_revision(recipe.clone(), first)
            .await
            .unwrap();

        let viewable = recipe.clone().update_scope(RecipeScope::Viewable);
        repository.save_recipe(viewable.clone()).await.unwrap();

        let second = revision("revision-2", &viewable);
        repository
            .save_recipe_revision(viewable, second)
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
            .save_recipe_revision(recipe, revision)
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
            .save_recipe_revision(recipe.clone(), revision)
            .await
            .unwrap();
        repository.add_recipe_images(vec![image]).await.unwrap();

        let images = repository
            .get_recipe_images(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert_eq!(images.len(), 1);
        assert_eq!(images[0].id().as_str(), "image-1");
        assert_eq!(images[0].byte_len(), 8);
    }

    #[tokio::test]
    async fn adds_recipe_images_to_an_existing_recipe() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        repository
            .save_recipe_revision(recipe.clone(), revision)
            .await
            .unwrap();

        repository
            .add_recipe_images(vec![
                recipe_image(&recipe, "image-existing", b"existing"),
                recipe_image(&recipe, "image-1", b"front"),
                recipe_image(&recipe, "image-2", b"back"),
            ])
            .await
            .unwrap();

        let images = repository
            .get_recipe_images(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert_eq!(images.len(), 3);
        assert_eq!(images[0].id().as_str(), "image-existing");
        assert_eq!(images[1].id().as_str(), "image-1");
        assert_eq!(images[2].id().as_str(), "image-2");

        let image = repository
            .get_recipe_image(
                RecipeId::new("soup".to_string()),
                RecipeImageId::new("image-2".to_string()),
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(image.bytes(), b"back");
    }

    #[tokio::test]
    async fn adding_no_recipe_images_is_a_no_op() {
        let repository = InMemoryRecipeRepository::new();

        repository.add_recipe_images(Vec::new()).await.unwrap();
    }

    #[tokio::test]
    async fn rejects_recipe_images_for_an_unknown_recipe() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");

        let result = repository
            .add_recipe_images(vec![recipe_image(&recipe, "image-1", b"front")])
            .await;

        assert!(matches!(result, Err(AddRecipeImagesError::RecipeNotFound)));
    }

    #[tokio::test]
    async fn rejects_added_recipe_images_for_another_recipe() {
        let repository = InMemoryRecipeRepository::new();
        let soup = recipe("soup");
        let revision = revision("revision-1", &soup);
        repository
            .save_recipe_revision(soup.clone(), revision)
            .await
            .unwrap();

        let salad = recipe("salad");

        let result = repository
            .add_recipe_images(vec![
                recipe_image(&soup, "image-1", b"front"),
                recipe_image(&salad, "image-2", b"side"),
            ])
            .await;

        assert!(matches!(result, Err(AddRecipeImagesError::Storage(_))));
    }

    #[tokio::test]
    async fn rejects_duplicate_ids_in_added_recipe_images() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        repository
            .save_recipe_revision(recipe.clone(), revision)
            .await
            .unwrap();

        let image = recipe_image(&recipe, "image-1", b"front");
        let result = repository
            .add_recipe_images(vec![image.clone(), image])
            .await;

        assert!(matches!(result, Err(AddRecipeImagesError::Storage(_))));

        let images = repository
            .get_recipe_images(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert!(images.is_empty());
    }

    #[tokio::test]
    async fn rejects_added_recipe_images_that_already_exist() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        repository
            .save_recipe_revision(recipe.clone(), revision)
            .await
            .unwrap();
        repository
            .add_recipe_images(vec![recipe_image(&recipe, "image-1", b"front")])
            .await
            .unwrap();

        let result = repository
            .add_recipe_images(vec![recipe_image(&recipe, "image-1", b"back")])
            .await;

        assert!(matches!(result, Err(AddRecipeImagesError::Storage(_))));

        let images = repository
            .get_recipe_images(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert_eq!(images.len(), 1);
        assert_eq!(images[0].id().as_str(), "image-1");
    }

    #[tokio::test]
    async fn deletes_a_recipe_and_its_related_rows() {
        let repository = InMemoryRecipeRepository::new();
        let recipe = recipe("soup");
        let revision = revision("revision-1", &recipe);
        repository
            .save_recipe_revision(recipe.clone(), revision)
            .await
            .unwrap();
        repository
            .add_source_documents(vec![source_document(&recipe, "document-1", b"front")])
            .await
            .unwrap();
        repository
            .add_recipe_images(vec![recipe_image(&recipe, "image-1", b"front")])
            .await
            .unwrap();

        repository
            .delete_recipe(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert!(
            repository
                .get_recipe(RecipeId::new("soup".to_string()))
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            repository
                .get_recipe_revisions(RecipeId::new("soup".to_string()))
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            repository
                .get_source_documents(RecipeId::new("soup".to_string()))
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            repository
                .get_recipe_images(RecipeId::new("soup".to_string()))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn deleting_an_unknown_recipe_is_not_found() {
        let repository = InMemoryRecipeRepository::new();

        let result = repository
            .delete_recipe(RecipeId::new("missing".to_string()))
            .await;

        assert!(matches!(result, Err(DeleteRecipeRecordError::NotFound)));
    }
}
