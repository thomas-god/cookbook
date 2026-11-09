use uuid::Uuid;

use crate::clock::Clock;
use crate::recipe::models::{
    Recipe, RecipeId, RecipeRevision, RecipeRevisionId, RecipeWithContent,
};
use crate::recipe::ports::{
    CreateRecipeError, CreateRecipeRequest, GetRecipeError, GetRecipeHistoryError, GetRecipesError,
    RecipeRepository, RecipeService, SaveRecipeError, UpdateRecipeError, UpdateRecipeRequest,
};

#[derive(Debug, Clone)]
pub struct RecipeServiceImpl<R, C> {
    repository: R,
    clock: C,
}

impl<R, C> RecipeServiceImpl<R, C> {
    pub fn new(repository: R, clock: C) -> Self {
        Self { repository, clock }
    }
}

impl<R, C> RecipeService for RecipeServiceImpl<R, C>
where
    R: RecipeRepository,
    C: Clock,
{
    async fn create_recipe(
        &self,
        request: CreateRecipeRequest,
    ) -> Result<RecipeWithContent, CreateRecipeError> {
        let content = request.content;
        let author = request.author;
        let recipe = Recipe::new(RecipeId::new(Uuid::now_v7().to_string()), author.clone());

        let revision = RecipeRevision::new(
            RecipeRevisionId::new(Uuid::now_v7().to_string()),
            recipe.id().clone(),
            content,
            author,
            self.clock.now(),
            None,
        );

        let current = RecipeWithContent::new(recipe.clone(), revision.clone())
            .map_err(|error| CreateRecipeError::Storage(error.into()))?;

        self.repository
            .save_recipe_with_revision(recipe, revision)
            .await
            .map_err(|error| CreateRecipeError::Storage(error.into()))?;

        Ok(current)
    }

    async fn update_recipe(
        &self,
        request: UpdateRecipeRequest,
    ) -> Result<RecipeWithContent, UpdateRecipeError> {
        let current = self
            .repository
            .get_recipe(request.id.clone())
            .await
            .map_err(|error| UpdateRecipeError::Storage(error.into()))?
            .ok_or(UpdateRecipeError::NotFound)?;

        if current.revision().id() != &request.latest_revision {
            return Err(UpdateRecipeError::Conflict);
        }

        // TODO: detect no-op edits by comparing the new content with the latest revision,
        // and skip saving a new revision when they are identical.
        let recipe = current.recipe().clone();
        let revision = RecipeRevision::new(
            RecipeRevisionId::new(Uuid::now_v7().to_string()),
            recipe.id().clone(),
            request.content,
            request.editor,
            self.clock.now(),
            request.message,
        );

        let updated = RecipeWithContent::new(recipe.clone(), revision.clone())
            .map_err(|error| UpdateRecipeError::Storage(error.into()))?;

        self.repository
            .save_recipe_with_revision(recipe, revision)
            .await
            .map_err(|error| match error {
                SaveRecipeError::RecipeIdMismatch | SaveRecipeError::RecipeAuthorMismatch => {
                    UpdateRecipeError::Conflict
                }
                SaveRecipeError::Storage(reason) => UpdateRecipeError::Storage(reason),
            })?;

        Ok(updated)
    }

    async fn get_recipes(&self) -> Result<Vec<RecipeWithContent>, GetRecipesError> {
        self.repository
            .get_recipes()
            .await
            .map_err(|error| GetRecipesError::Storage(error.into()))
    }

    async fn get_recipe(&self, id: RecipeId) -> Result<Option<RecipeWithContent>, GetRecipeError> {
        self.repository
            .get_recipe(id)
            .await
            .map_err(|error| GetRecipeError::Storage(error.into()))
    }

    async fn get_recipe_history(
        &self,
        id: RecipeId,
    ) -> Result<Vec<RecipeRevision>, GetRecipeHistoryError> {
        self.repository
            .get_recipe_revisions(id)
            .await
            .map_err(|error| GetRecipeHistoryError::Storage(error.into()))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::{Duration, SystemTime};

    use anyhow::anyhow;

    use super::*;
    use crate::recipe::models::{
        RecipeContent, RecipeMetadata, RecipeRevision, RecipeTitle, RevisionMessage,
    };
    use crate::recipe::ports::{MockRecipeRepository, SaveRecipeError, UpdateRecipeRequest};
    use crate::user::models::UserId;

    const EDIT_TIME_SECS: u64 = 1_700_000_000;

    #[derive(Debug, Clone, Copy)]
    struct FixedClock(SystemTime);

    impl FixedClock {
        fn new() -> Self {
            Self(SystemTime::UNIX_EPOCH + Duration::from_secs(EDIT_TIME_SECS))
        }
    }

    impl Clock for FixedClock {
        fn now(&self) -> SystemTime {
            self.0
        }
    }

    fn create_request() -> CreateRecipeRequest {
        CreateRecipeRequest {
            content: content("Soup"),
            author: UserId::new("user-1".to_string()),
        }
    }

    fn content(title: &str) -> RecipeContent {
        RecipeContent::new(
            RecipeTitle::new(title.to_string()),
            None,
            RecipeMetadata::new(None, None, None),
            HashMap::new(),
            Vec::new(),
        )
    }

    fn recipe(id: &str) -> Recipe {
        Recipe::new(
            RecipeId::new(id.to_string()),
            UserId::new("user-1".to_string()),
        )
    }

    fn revision(id: &str, recipe: &Recipe, editor: &str, edited_at: SystemTime) -> RecipeRevision {
        RecipeRevision::new(
            RecipeRevisionId::new(id.to_string()),
            recipe.id().clone(),
            content("Soup"),
            UserId::new(editor.to_string()),
            edited_at,
            None,
        )
    }

    fn current_recipe(id: &str, title: &str) -> RecipeWithContent {
        let recipe = recipe(id);
        let revision = RecipeRevision::new(
            RecipeRevisionId::new("revision-1".to_string()),
            recipe.id().clone(),
            content(title),
            recipe.author().clone(),
            FixedClock::new().0,
            None,
        );

        RecipeWithContent::new(recipe, revision).unwrap()
    }

    #[tokio::test]
    async fn create_recipe_saves_and_returns_the_recipe() {
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_save_recipe_with_revision()
            .times(1)
            .returning(|_, _| Ok(()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let recipe = service.create_recipe(create_request()).await.unwrap();

        assert_eq!(recipe.author().as_str(), "user-1");
    }

    #[tokio::test]
    async fn create_recipe_records_an_initial_revision() {
        let edited_at = FixedClock::new().0;
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_save_recipe_with_revision()
            .times(1)
            .withf(move |recipe: &Recipe, revision: &RecipeRevision| {
                revision.recipe_id() == recipe.id()
                    && revision.content().title().as_str() == "Soup"
                    && revision.editor() == recipe.author()
                    && revision.edited_at() == edited_at
                    && revision.message().is_none()
            })
            .returning(|_, _| Ok(()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let recipe = service.create_recipe(create_request()).await.unwrap();

        assert_eq!(recipe.title().as_str(), "Soup");
        assert_eq!(recipe.revision().editor(), recipe.author());
    }

    #[tokio::test]
    async fn create_recipe_generates_a_unique_id_for_each_recipe() {
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_save_recipe_with_revision()
            .times(2)
            .returning(|_, _| Ok(()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let first = service.create_recipe(create_request()).await.unwrap();
        let second = service.create_recipe(create_request()).await.unwrap();

        assert_ne!(first.id(), second.id());
    }

    #[tokio::test]
    async fn create_recipe_maps_save_failures_to_storage_errors() {
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_save_recipe_with_revision()
            .returning(|_, _| Err(SaveRecipeError::Storage(anyhow!("store unavailable"))));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service.create_recipe(create_request()).await;

        assert!(matches!(result, Err(CreateRecipeError::Storage(_))));
    }

    #[tokio::test]
    async fn update_recipe_saves_a_new_revision_and_preserves_the_author() {
        let current = current_recipe("soup", "Soup");
        let latest_revision = current.revision().id().clone();
        let edited_at = FixedClock::new().0;

        let mut repository = MockRecipeRepository::new();
        repository.expect_get_recipe().times(1).returning({
            let current = current.clone();
            move |_| Ok(Some(current.clone()))
        });
        let expected_id = current.id().clone();
        repository
            .expect_save_recipe_with_revision()
            .times(1)
            .withf(move |recipe: &Recipe, revision: &RecipeRevision| {
                recipe.id() == &expected_id
                    && recipe.author().as_str() == "user-1"
                    && revision.recipe_id() == &expected_id
                    && revision.content().title().as_str() == "Updated Soup"
                    && revision.editor().as_str() == "editor-2"
                    && revision.edited_at() == edited_at
                    && revision.message().map(RevisionMessage::as_str) == Some("Changed the title")
            })
            .returning(|_, _| Ok(()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let updated = service
            .update_recipe(UpdateRecipeRequest {
                id: current.id().clone(),
                content: content("Updated Soup"),
                editor: UserId::new("editor-2".to_string()),
                latest_revision,
                message: Some(RevisionMessage::new("Changed the title").unwrap()),
            })
            .await
            .unwrap();

        assert_eq!(updated.id(), current.id());
        assert_eq!(updated.author().as_str(), "user-1");
        assert_eq!(updated.title().as_str(), "Updated Soup");
    }

    #[tokio::test]
    async fn update_recipe_rejects_a_stale_revision() {
        let current = current_recipe("soup", "Soup");

        let mut repository = MockRecipeRepository::new();
        repository.expect_get_recipe().times(1).returning({
            let current = current.clone();
            move |_| Ok(Some(current.clone()))
        });
        // No save expectation: a stale edit must not be persisted.

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service
            .update_recipe(UpdateRecipeRequest {
                id: current.id().clone(),
                content: content("Updated Soup"),
                editor: UserId::new("editor-2".to_string()),
                latest_revision: RecipeRevisionId::new("stale-revision".to_string()),
                message: None,
            })
            .await;

        assert!(matches!(result, Err(UpdateRecipeError::Conflict)));
    }

    #[tokio::test]
    async fn update_recipe_maps_save_identity_errors_to_conflict() {
        let current = current_recipe("soup", "Soup");
        let latest_revision = current.revision().id().clone();

        let mut repository = MockRecipeRepository::new();
        repository.expect_get_recipe().times(1).returning({
            let current = current.clone();
            move |_| Ok(Some(current.clone()))
        });
        repository
            .expect_save_recipe_with_revision()
            .returning(|_, _| Err(SaveRecipeError::RecipeAuthorMismatch));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service
            .update_recipe(UpdateRecipeRequest {
                id: current.id().clone(),
                content: content("Updated Soup"),
                editor: UserId::new("editor-2".to_string()),
                latest_revision,
                message: None,
            })
            .await;

        assert!(matches!(result, Err(UpdateRecipeError::Conflict)));
    }

    #[tokio::test]
    async fn update_recipe_returns_not_found_for_an_unknown_recipe() {
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_get_recipe()
            .times(1)
            .returning(|_| Ok(None));
        // No revision or save expectation: the recipe does not exist.

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service
            .update_recipe(UpdateRecipeRequest {
                id: RecipeId::new("unknown".to_string()),
                content: content("Soup"),
                editor: UserId::new("editor-2".to_string()),
                latest_revision: RecipeRevisionId::new("revision-1".to_string()),
                message: None,
            })
            .await;

        assert!(matches!(result, Err(UpdateRecipeError::NotFound)));
    }

    #[tokio::test]
    async fn update_recipe_maps_read_failures_to_storage_errors() {
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_get_recipe()
            .returning(|_| Err(GetRecipeError::Storage(anyhow!("read unavailable"))));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service
            .update_recipe(UpdateRecipeRequest {
                id: RecipeId::new("soup".to_string()),
                content: content("Soup"),
                editor: UserId::new("editor-2".to_string()),
                latest_revision: RecipeRevisionId::new("revision-1".to_string()),
                message: None,
            })
            .await;

        assert!(matches!(result, Err(UpdateRecipeError::Storage(_))));
    }

    #[tokio::test]
    async fn update_recipe_maps_save_failures_to_storage_errors() {
        let current = current_recipe("soup", "Soup");
        let latest_revision = current.revision().id().clone();

        let mut repository = MockRecipeRepository::new();
        repository.expect_get_recipe().times(1).returning({
            let current = current.clone();
            move |_| Ok(Some(current.clone()))
        });
        repository
            .expect_save_recipe_with_revision()
            .returning(|_, _| Err(SaveRecipeError::Storage(anyhow!("store unavailable"))));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service
            .update_recipe(UpdateRecipeRequest {
                id: current.id().clone(),
                content: content("Updated Soup"),
                editor: UserId::new("editor-2".to_string()),
                latest_revision,
                message: None,
            })
            .await;

        assert!(matches!(result, Err(UpdateRecipeError::Storage(_))));
    }

    #[tokio::test]
    async fn get_recipes_returns_the_repository_recipes() {
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_get_recipes()
            .times(1)
            .returning(|| Ok(Vec::new()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

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

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service.get_recipes().await;

        assert!(matches!(result, Err(GetRecipesError::Storage(_))));
    }

    #[tokio::test]
    async fn get_recipe_returns_the_repository_recipe() {
        let id = RecipeId::new("soup".to_string());
        let mut repository = MockRecipeRepository::new();
        let current = current_recipe("soup", "Soup");
        repository
            .expect_get_recipe()
            .times(1)
            .returning(move |_| Ok(Some(current.clone())));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let recipe = service.get_recipe(id).await.unwrap();

        assert_eq!(
            recipe.map(|recipe| recipe.id().clone()),
            Some(RecipeId::new("soup".to_string()))
        );
    }

    #[tokio::test]
    async fn get_recipe_maps_read_failures_to_storage_errors() {
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_get_recipe()
            .returning(|_| Err(GetRecipeError::Storage(anyhow!("read unavailable"))));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service.get_recipe(RecipeId::new("soup".to_string())).await;

        assert!(matches!(result, Err(GetRecipeError::Storage(_))));
    }

    #[tokio::test]
    async fn get_recipe_history_returns_the_repository_revisions() {
        let id = RecipeId::new("soup".to_string());
        let current = recipe("soup");
        let latest = revision("revision-1", &current, "user-1", FixedClock::new().0);
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_get_recipe_revisions()
            .times(1)
            .returning(move |_| Ok(vec![latest.clone()]));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let history = service.get_recipe_history(id).await.unwrap();

        assert_eq!(history.len(), 1);
    }

    #[tokio::test]
    async fn get_recipe_history_maps_read_failures_to_storage_errors() {
        let mut repository = MockRecipeRepository::new();
        repository.expect_get_recipe_revisions().returning(|_| {
            Err(crate::recipe::ports::ListRecipeRevisionsError::Storage(
                anyhow!("read unavailable"),
            ))
        });

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service
            .get_recipe_history(RecipeId::new("soup".to_string()))
            .await;

        assert!(matches!(result, Err(GetRecipeHistoryError::Storage(_))));
    }
}
