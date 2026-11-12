use std::collections::HashSet;

use uuid::Uuid;

use crate::clock::Clock;
use crate::recipe::models::{
    NewRecipeImage, NewRecipeSourceDocument, Recipe, RecipeContent, RecipeId, RecipeImage,
    RecipeImageId, RecipeImageMetadata, RecipeRevision, RecipeRevisionId, RecipeSourceDocument,
    RecipeSourceDocumentId, RecipeSourceDocumentSummary, RecipeStructureKind, RecipeWithContent,
    RevisionOrigin, RevisionVerification,
};
use crate::recipe::ports::{
    AddRecipeSourceDocumentsError, AddRecipeSourceDocumentsRequest, AddSourceDocumentsError,
    ConvertRecipeStructureRequest, CreateRecipeError, CreateRecipeRequest,
    DeleteRecipeSourceDocumentError, DeleteSourceDocumentError, GetRecipeError,
    GetRecipeHistoryError, GetRecipeImageError, GetRecipeImagesError, GetRecipeSourceDocumentError,
    GetRecipeSourceDocumentsError, GetRecipesError, GetSourceDocumentError, ListRecipeImagesError,
    ListSourceDocumentsError, RecipeRepository, RecipeService, RestoreRecipeRevisionError,
    RestoreRecipeRevisionRequest, SaveRecipeError, UpdateRecipeError, UpdateRecipeRequest,
    UpdateRecipeScopeError, UpdateRecipeScopeRequest,
};
use crate::user::models::UserId;

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

impl<R, C> RecipeServiceImpl<R, C>
where
    R: RecipeRepository,
    C: Clock,
{
    async fn create_recipe(
        &self,
        request: CreateRecipeRequest,
        expected_verification: RevisionVerification,
        origin: RevisionOrigin,
    ) -> Result<RecipeWithContent, CreateRecipeError> {
        if request.verification != expected_verification {
            return Err(CreateRecipeError::VerificationMismatch);
        }

        let CreateRecipeRequest {
            content,
            author,
            source_documents,
            images,
            ..
        } = request;
        let recipe = Recipe::new(RecipeId::new(Uuid::now_v7().to_string()), author.clone());
        let now = self.clock.now();

        let new_image_ids: HashSet<_> = images.iter().map(|image| image.id().clone()).collect();
        let ids_are_unique = new_image_ids.len() == images.len();
        if !ids_are_unique || !image_references_are_valid(&content, &new_image_ids) {
            return Err(CreateRecipeError::InvalidImageReference);
        }

        let revision = RecipeRevision::new(
            RecipeRevisionId::new(Uuid::now_v7().to_string()),
            recipe.id().clone(),
            content,
            author.clone(),
            now,
            origin,
            expected_verification,
        );

        let current = RecipeWithContent::new(recipe.clone(), revision.clone())
            .map_err(|error| CreateRecipeError::Storage(error.into()))?;
        let source_documents = build_source_documents(recipe.id(), &author, source_documents, now);
        let images = build_recipe_images(recipe.id(), &author, images, now);

        self.repository
            .save_new_recipe(recipe, revision, source_documents, images)
            .await
            .map_err(|error| match error {
                SaveRecipeError::InvalidImageReference => CreateRecipeError::InvalidImageReference,
                error => CreateRecipeError::Storage(error.into()),
            })?;

        Ok(current)
    }
}

impl<R, C> RecipeService for RecipeServiceImpl<R, C>
where
    R: RecipeRepository,
    C: Clock,
{
    async fn create_verified_recipe(
        &self,
        request: CreateRecipeRequest,
    ) -> Result<RecipeWithContent, CreateRecipeError> {
        self.create_recipe(
            request,
            RevisionVerification::Verified,
            RevisionOrigin::Created,
        )
        .await
    }

    async fn create_imported_recipe(
        &self,
        request: CreateRecipeRequest,
    ) -> Result<RecipeWithContent, CreateRecipeError> {
        self.create_recipe(
            request,
            RevisionVerification::NeedsReview,
            RevisionOrigin::Imported,
        )
        .await
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
            .filter(|recipe| recipe.can_view(&request.editor))
            .ok_or(UpdateRecipeError::NotFound)?;

        if !current.can_edit(&request.editor) {
            return Err(UpdateRecipeError::Forbidden);
        }

        if current.revision().id() != &request.latest_revision {
            return Err(UpdateRecipeError::Conflict);
        }

        let UpdateRecipeRequest {
            content,
            editor,
            message,
            new_images,
            ..
        } = request;

        let added_image_ids: HashSet<_> =
            new_images.iter().map(|image| image.id().clone()).collect();
        if added_image_ids.len() != new_images.len() {
            return Err(UpdateRecipeError::InvalidImageReference);
        }

        let existing_images = self
            .repository
            .get_recipe_images(current.id().clone())
            .await
            .map_err(|error| UpdateRecipeError::Storage(error.into()))?;
        let known_image_ids: HashSet<_> = existing_images
            .iter()
            .map(|image| image.id().clone())
            .chain(added_image_ids)
            .collect();

        if !image_references_are_valid(&content, &known_image_ids) {
            return Err(UpdateRecipeError::InvalidImageReference);
        }

        // TODO: detect no-op edits by comparing the new content and verification
        // status with the latest revision, and skip saving a new revision only
        // when both are unchanged. Content equality alone is not enough: a
        // verification save may keep the content identical while still needing
        // to record the new verification status.
        let recipe = current.recipe().clone();
        let revision = RecipeRevision::new(
            RecipeRevisionId::new(Uuid::now_v7().to_string()),
            recipe.id().clone(),
            content,
            editor.clone(),
            self.clock.now(),
            RevisionOrigin::UserEdit { message },
            RevisionVerification::Verified,
        );

        let updated = RecipeWithContent::new(recipe.clone(), revision.clone())
            .map_err(|error| UpdateRecipeError::Storage(error.into()))?;
        let added_images = build_recipe_images(recipe.id(), &editor, new_images, self.clock.now());

        self.repository
            .save_recipe_with_revision(recipe, revision, added_images)
            .await
            .map_err(|error| match error {
                SaveRecipeError::RecipeIdMismatch | SaveRecipeError::RecipeAuthorMismatch => {
                    UpdateRecipeError::Conflict
                }
                SaveRecipeError::RecipeNotFound => UpdateRecipeError::NotFound,
                SaveRecipeError::InvalidImageReference => UpdateRecipeError::InvalidImageReference,
                SaveRecipeError::Storage(reason) => UpdateRecipeError::Storage(reason),
            })?;

        Ok(updated)
    }

    async fn convert_recipe_structure(
        &self,
        request: ConvertRecipeStructureRequest,
    ) -> Result<RecipeWithContent, UpdateRecipeError> {
        let current = self
            .repository
            .get_recipe(request.id.clone())
            .await
            .map_err(|error| UpdateRecipeError::Storage(error.into()))?
            .filter(|recipe| recipe.can_view(&request.editor))
            .ok_or(UpdateRecipeError::NotFound)?;

        if !current.can_edit(&request.editor) {
            return Err(UpdateRecipeError::Forbidden);
        }

        if current.revision().id() != &request.latest_revision {
            return Err(UpdateRecipeError::Conflict);
        }

        let content = match request.to {
            RecipeStructureKind::Simple => current.content().clone().into_simple(),
            RecipeStructureKind::Multipart => current.content().clone().into_multipart(),
        };
        let recipe = current.recipe().clone();
        let revision = RecipeRevision::new(
            RecipeRevisionId::new(Uuid::now_v7().to_string()),
            recipe.id().clone(),
            content,
            request.editor,
            self.clock.now(),
            RevisionOrigin::StructureConverted { to: request.to },
            RevisionVerification::Verified,
        );

        let updated = RecipeWithContent::new(recipe.clone(), revision.clone())
            .map_err(|error| UpdateRecipeError::Storage(error.into()))?;

        self.repository
            .save_recipe_with_revision(recipe, revision, Vec::new())
            .await
            .map_err(|error| match error {
                SaveRecipeError::RecipeIdMismatch | SaveRecipeError::RecipeAuthorMismatch => {
                    UpdateRecipeError::Conflict
                }
                SaveRecipeError::RecipeNotFound => UpdateRecipeError::NotFound,
                SaveRecipeError::InvalidImageReference => UpdateRecipeError::InvalidImageReference,
                SaveRecipeError::Storage(reason) => UpdateRecipeError::Storage(reason),
            })?;

        Ok(updated)
    }

    async fn restore_recipe_revision(
        &self,
        request: RestoreRecipeRevisionRequest,
    ) -> Result<RecipeWithContent, RestoreRecipeRevisionError> {
        let current = self
            .repository
            .get_recipe(request.id.clone())
            .await
            .map_err(|error| RestoreRecipeRevisionError::Storage(error.into()))?
            .filter(|recipe| recipe.can_view(&request.editor))
            .ok_or(RestoreRecipeRevisionError::NotFound)?;

        if !current.can_edit(&request.editor) {
            return Err(RestoreRecipeRevisionError::Forbidden);
        }

        if current.revision().id() != &request.latest_revision {
            return Err(RestoreRecipeRevisionError::Conflict);
        }

        let source = self
            .repository
            .get_recipe_revisions(current.id().clone())
            .await
            .map_err(|error| RestoreRecipeRevisionError::Storage(error.into()))?
            .into_iter()
            .find(|revision| revision.id() == &request.revision_id)
            .ok_or(RestoreRecipeRevisionError::NotFound)?;

        let recipe = current.recipe().clone();
        let revision = RecipeRevision::new(
            RecipeRevisionId::new(Uuid::now_v7().to_string()),
            recipe.id().clone(),
            source.content().clone(),
            request.editor,
            self.clock.now(),
            RevisionOrigin::Restored {
                revision_id: source.id().clone(),
            },
            source.verification(),
        );

        let updated = RecipeWithContent::new(recipe.clone(), revision.clone())
            .map_err(|error| RestoreRecipeRevisionError::Storage(error.into()))?;

        self.repository
            .save_recipe_with_revision(recipe, revision, Vec::new())
            .await
            .map_err(|error| match error {
                SaveRecipeError::RecipeIdMismatch | SaveRecipeError::RecipeAuthorMismatch => {
                    RestoreRecipeRevisionError::Conflict
                }
                SaveRecipeError::RecipeNotFound => RestoreRecipeRevisionError::NotFound,
                error @ SaveRecipeError::InvalidImageReference => {
                    RestoreRecipeRevisionError::Storage(error.into())
                }
                SaveRecipeError::Storage(reason) => RestoreRecipeRevisionError::Storage(reason),
            })?;

        Ok(updated)
    }

    async fn update_recipe_scope(
        &self,
        request: UpdateRecipeScopeRequest,
    ) -> Result<RecipeWithContent, UpdateRecipeScopeError> {
        let current = self
            .repository
            .get_recipe(request.id.clone())
            .await
            .map_err(|error| UpdateRecipeScopeError::Storage(error.into()))?
            .filter(|recipe| recipe.can_view(&request.actor))
            .ok_or(UpdateRecipeScopeError::NotFound)?;

        if current.author() != &request.actor {
            return Err(UpdateRecipeScopeError::Forbidden);
        }

        let updated_recipe = current.recipe().clone().update_scope(request.scope);

        self.repository
            .save_recipe(updated_recipe.clone())
            .await
            .map_err(|error| match error {
                SaveRecipeError::RecipeNotFound => UpdateRecipeScopeError::NotFound,
                SaveRecipeError::RecipeAuthorMismatch => UpdateRecipeScopeError::Forbidden,
                SaveRecipeError::RecipeIdMismatch
                | SaveRecipeError::InvalidImageReference
                | SaveRecipeError::Storage(_) => UpdateRecipeScopeError::Storage(error.into()),
            })?;

        RecipeWithContent::new(updated_recipe, current.revision().clone())
            .map_err(|error| UpdateRecipeScopeError::Storage(error.into()))
    }

    async fn get_recipes(&self, viewer: UserId) -> Result<Vec<RecipeWithContent>, GetRecipesError> {
        let recipes = self
            .repository
            .get_recipes()
            .await
            .map_err(|error| GetRecipesError::Storage(error.into()))?;

        Ok(recipes
            .into_iter()
            .filter(|recipe| recipe.can_view(&viewer))
            .collect())
    }

    async fn get_recipe(
        &self,
        id: RecipeId,
        viewer: UserId,
    ) -> Result<Option<RecipeWithContent>, GetRecipeError> {
        let recipe = self
            .repository
            .get_recipe(id)
            .await
            .map_err(|error| GetRecipeError::Storage(error.into()))?;

        Ok(recipe.filter(|recipe| recipe.can_view(&viewer)))
    }

    async fn get_recipe_history(
        &self,
        id: RecipeId,
        viewer: UserId,
    ) -> Result<Vec<RecipeRevision>, GetRecipeHistoryError> {
        self.repository
            .get_recipe(id.clone())
            .await
            .map_err(|error| GetRecipeHistoryError::Storage(error.into()))?
            .filter(|recipe| recipe.can_view(&viewer))
            .ok_or(GetRecipeHistoryError::NotFound)?;

        self.repository
            .get_recipe_revisions(id)
            .await
            .map_err(|error| GetRecipeHistoryError::Storage(error.into()))
    }

    async fn add_source_documents(
        &self,
        request: AddRecipeSourceDocumentsRequest,
    ) -> Result<Vec<RecipeSourceDocumentSummary>, AddRecipeSourceDocumentsError> {
        let current = self
            .repository
            .get_recipe(request.recipe_id.clone())
            .await
            .map_err(|error| AddRecipeSourceDocumentsError::Storage(error.into()))?
            .filter(|recipe| recipe.can_view(&request.added_by))
            .ok_or(AddRecipeSourceDocumentsError::RecipeNotFound)?;

        if !current.can_edit(&request.added_by) {
            return Err(AddRecipeSourceDocumentsError::Forbidden);
        }

        if request.documents.is_empty() {
            return Ok(Vec::new());
        }

        let AddRecipeSourceDocumentsRequest {
            recipe_id,
            added_by,
            documents,
        } = request;
        let documents = build_source_documents(&recipe_id, &added_by, documents, self.clock.now());
        let summaries = documents
            .iter()
            .map(RecipeSourceDocument::summary)
            .collect();

        self.repository
            .add_source_documents(documents)
            .await
            .map_err(|error| match error {
                AddSourceDocumentsError::RecipeNotFound => {
                    AddRecipeSourceDocumentsError::RecipeNotFound
                }
                AddSourceDocumentsError::Storage(reason) => {
                    AddRecipeSourceDocumentsError::Storage(reason)
                }
            })?;

        Ok(summaries)
    }

    async fn get_source_documents(
        &self,
        recipe_id: RecipeId,
        viewer: UserId,
    ) -> Result<Vec<RecipeSourceDocumentSummary>, GetRecipeSourceDocumentsError> {
        self.repository
            .get_recipe(recipe_id.clone())
            .await
            .map_err(|error| GetRecipeSourceDocumentsError::Storage(error.into()))?
            .filter(|recipe| recipe.can_view(&viewer))
            .ok_or(GetRecipeSourceDocumentsError::NotFound)?;

        self.repository
            .get_source_documents(recipe_id)
            .await
            .map_err(|error| match error {
                ListSourceDocumentsError::Storage(reason) => {
                    GetRecipeSourceDocumentsError::Storage(reason)
                }
            })
    }

    async fn get_source_document(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeSourceDocumentId,
        viewer: UserId,
    ) -> Result<Option<RecipeSourceDocument>, GetRecipeSourceDocumentError> {
        let visible = self
            .repository
            .get_recipe(recipe_id.clone())
            .await
            .map_err(|error| GetRecipeSourceDocumentError::Storage(error.into()))?
            .is_some_and(|recipe| recipe.can_view(&viewer));

        if !visible {
            return Ok(None);
        }

        self.repository
            .get_source_document(recipe_id, image_id)
            .await
            .map_err(|error| match error {
                GetSourceDocumentError::Storage(reason) => {
                    GetRecipeSourceDocumentError::Storage(reason)
                }
            })
    }

    async fn delete_source_document(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeSourceDocumentId,
        actor: UserId,
    ) -> Result<(), DeleteRecipeSourceDocumentError> {
        let current = self
            .repository
            .get_recipe(recipe_id.clone())
            .await
            .map_err(|error| DeleteRecipeSourceDocumentError::Storage(error.into()))?
            .filter(|recipe| recipe.can_view(&actor))
            .ok_or(DeleteRecipeSourceDocumentError::NotFound)?;

        if !current.can_edit(&actor) {
            return Err(DeleteRecipeSourceDocumentError::Forbidden);
        }

        self.repository
            .delete_source_document(recipe_id, image_id)
            .await
            .map_err(|error| match error {
                DeleteSourceDocumentError::NotFound => DeleteRecipeSourceDocumentError::NotFound,
                DeleteSourceDocumentError::Storage(reason) => {
                    DeleteRecipeSourceDocumentError::Storage(reason)
                }
            })
    }

    async fn get_recipe_images(
        &self,
        recipe_id: RecipeId,
        viewer: UserId,
    ) -> Result<Vec<RecipeImageMetadata>, GetRecipeImagesError> {
        self.repository
            .get_recipe(recipe_id.clone())
            .await
            .map_err(|error| GetRecipeImagesError::Storage(error.into()))?
            .filter(|recipe| recipe.can_view(&viewer))
            .ok_or(GetRecipeImagesError::NotFound)?;

        self.repository
            .get_recipe_images(recipe_id)
            .await
            .map_err(|error| match error {
                ListRecipeImagesError::Storage(reason) => GetRecipeImagesError::Storage(reason),
            })
    }

    async fn get_recipe_image(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeImageId,
        viewer: UserId,
    ) -> Result<Option<RecipeImage>, GetRecipeImageError> {
        let visible = self
            .repository
            .get_recipe(recipe_id.clone())
            .await
            .map_err(|error| GetRecipeImageError::Storage(error.into()))?
            .is_some_and(|recipe| recipe.can_view(&viewer));

        if !visible {
            return Ok(None);
        }

        self.repository
            .get_recipe_image(recipe_id, image_id)
            .await
            .map_err(|error| match error {
                GetRecipeImageError::Storage(reason) => GetRecipeImageError::Storage(reason),
            })
    }
}

fn build_source_documents(
    recipe_id: &RecipeId,
    added_by: &UserId,
    images: Vec<NewRecipeSourceDocument>,
    added_at: std::time::SystemTime,
) -> Vec<RecipeSourceDocument> {
    images
        .into_iter()
        .map(|image| {
            RecipeSourceDocument::new(
                RecipeSourceDocumentId::new(Uuid::now_v7().to_string()),
                recipe_id.clone(),
                image.content_type().to_string(),
                image.original_name().map(str::to_string),
                image.bytes().to_vec(),
                added_by.clone(),
                added_at,
            )
        })
        .collect()
}

fn build_recipe_images(
    recipe_id: &RecipeId,
    added_by: &UserId,
    images: Vec<NewRecipeImage>,
    added_at: std::time::SystemTime,
) -> Vec<RecipeImage> {
    images
        .into_iter()
        .map(|image| {
            RecipeImage::new(
                image.id().clone(),
                recipe_id.clone(),
                image.content_type().to_string(),
                image.bytes().to_vec(),
                added_by.clone(),
                added_at,
            )
        })
        .collect()
}

fn image_references_are_valid(
    content: &RecipeContent,
    known_image_ids: &HashSet<RecipeImageId>,
) -> bool {
    content.bodies().all(|body| {
        body.images()
            .iter()
            .all(|image_id| known_image_ids.contains(image_id))
            && body.steps().iter().all(|step| {
                step.images()
                    .iter()
                    .all(|image_id| known_image_ids.contains(image_id))
            })
    })
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use anyhow::anyhow;

    use super::*;
    use crate::recipe::models::{
        RecipeBody, RecipeContent, RecipeDurations, RecipeImageId, RecipeImageMetadata, RecipePart,
        RecipePartId, RecipePartTitle, RecipeRevision, RecipeScope, RecipeStep, RecipeStepId,
        RecipeTitle, RevisionMessage,
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
            source_documents: Vec::new(),
            images: Vec::new(),
            verification: RevisionVerification::Verified,
        }
    }

    fn new_source_document(name: &str, bytes: &[u8]) -> NewRecipeSourceDocument {
        NewRecipeSourceDocument::new(
            "image/jpeg".to_string(),
            Some(name.to_string()),
            bytes.to_vec(),
        )
    }

    fn recipe_id() -> RecipeId {
        RecipeId::new("soup".to_string())
    }

    fn user_id() -> UserId {
        UserId::new("user-1".to_string())
    }

    fn content(title: &str) -> RecipeContent {
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
        .unwrap()
    }

    fn recipe(id: &str) -> Recipe {
        Recipe::new(
            RecipeId::new(id.to_string()),
            UserId::new("user-1".to_string()),
        )
    }

    fn scoped_recipe(id: &str, author: &str, scope: RecipeScope) -> Recipe {
        Recipe::with_scope(
            RecipeId::new(id.to_string()),
            UserId::new(author.to_string()),
            scope,
        )
    }

    fn revision(id: &str, recipe: &Recipe, editor: &str, edited_at: SystemTime) -> RecipeRevision {
        RecipeRevision::new(
            RecipeRevisionId::new(id.to_string()),
            recipe.id().clone(),
            content("Soup"),
            UserId::new(editor.to_string()),
            edited_at,
            RevisionOrigin::Created,
            RevisionVerification::Verified,
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
            RevisionOrigin::Created,
            RevisionVerification::Verified,
        );

        RecipeWithContent::new(recipe, revision).unwrap()
    }

    fn expect_visible_recipe(repository: &mut MockRecipeRepository, current: RecipeWithContent) {
        repository
            .expect_get_recipe()
            .times(1)
            .returning(move |_| Ok(Some(current.clone())));
    }

    fn current_recipe_with_scope(
        id: &str,
        title: &str,
        author: &str,
        scope: RecipeScope,
    ) -> RecipeWithContent {
        let recipe = scoped_recipe(id, author, scope);
        let revision = RecipeRevision::new(
            RecipeRevisionId::new(format!("{id}-revision")),
            recipe.id().clone(),
            content(title),
            recipe.author().clone(),
            FixedClock::new().0,
            RevisionOrigin::Created,
            RevisionVerification::Verified,
        );

        RecipeWithContent::new(recipe, revision).unwrap()
    }

    #[tokio::test]
    async fn create_recipe_saves_and_returns_the_recipe() {
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_save_new_recipe()
            .times(1)
            .returning(|_, _, _, _| Ok(()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let recipe = service
            .create_verified_recipe(create_request())
            .await
            .unwrap();

        assert_eq!(recipe.author().as_str(), "user-1");
    }

    #[tokio::test]
    async fn create_recipe_records_an_initial_revision() {
        let edited_at = FixedClock::new().0;
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_save_new_recipe()
            .times(1)
            .withf(
                move |recipe: &Recipe,
                      revision: &RecipeRevision,
                      source_documents: &Vec<RecipeSourceDocument>,
                      _: &Vec<RecipeImage>| {
                    revision.recipe_id() == recipe.id()
                        && revision.content().title().as_str() == "Soup"
                        && revision.editor() == recipe.author()
                        && revision.edited_at() == edited_at
                        && revision.origin() == &RevisionOrigin::Created
                        && source_documents.is_empty()
                },
            )
            .returning(|_, _, _, _| Ok(()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let recipe = service
            .create_verified_recipe(create_request())
            .await
            .unwrap();

        assert_eq!(recipe.title().as_str(), "Soup");
        assert_eq!(recipe.revision().editor(), recipe.author());
    }

    #[tokio::test]
    async fn create_imported_recipe_records_an_imported_needs_review_revision() {
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_save_new_recipe()
            .times(1)
            .withf(
                |_: &Recipe,
                 revision: &RecipeRevision,
                 _: &Vec<RecipeSourceDocument>,
                 _: &Vec<RecipeImage>| {
                    revision.verification() == RevisionVerification::NeedsReview
                        && revision.origin() == &RevisionOrigin::Imported
                },
            )
            .returning(|_, _, _, _| Ok(()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let mut request = create_request();
        request.verification = RevisionVerification::NeedsReview;

        let recipe = service.create_imported_recipe(request).await.unwrap();

        assert!(recipe.needs_review());
    }

    #[tokio::test]
    async fn create_recipe_rejects_a_verification_mismatch() {
        // No save expectation: a mismatched entrypoint must not persist anything.
        let repository = MockRecipeRepository::new();
        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service.create_imported_recipe(create_request()).await;

        assert!(matches!(
            result,
            Err(CreateRecipeError::VerificationMismatch)
        ));
    }

    #[tokio::test]
    async fn create_recipe_generates_a_unique_id_for_each_recipe() {
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_save_new_recipe()
            .times(2)
            .returning(|_, _, _, _| Ok(()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let first = service
            .create_verified_recipe(create_request())
            .await
            .unwrap();
        let second = service
            .create_verified_recipe(create_request())
            .await
            .unwrap();

        assert_ne!(first.id(), second.id());
    }

    #[tokio::test]
    async fn create_recipe_maps_save_failures_to_storage_errors() {
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_save_new_recipe()
            .returning(|_, _, _, _| Err(SaveRecipeError::Storage(anyhow!("store unavailable"))));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service.create_verified_recipe(create_request()).await;

        assert!(matches!(result, Err(CreateRecipeError::Storage(_))));
    }

    #[tokio::test]
    async fn create_recipe_attaches_source_documents() {
        let added_at = FixedClock::new().0;
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_save_new_recipe()
            .times(1)
            .withf(
                move |recipe: &Recipe,
                      _: &RecipeRevision,
                      images: &Vec<RecipeSourceDocument>,
                      _: &Vec<RecipeImage>| {
                    images.len() == 1
                        && images[0].recipe_id() == recipe.id()
                        && images[0].content_type() == "image/jpeg"
                        && images[0].original_name() == Some("front.jpg")
                        && images[0].bytes() == b"front"
                        && images[0].added_by() == recipe.author()
                        && images[0].added_at() == added_at
                },
            )
            .returning(|_, _, _, _| Ok(()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let mut request = create_request();
        request.source_documents = vec![new_source_document("front.jpg", b"front")];

        let recipe = service.create_verified_recipe(request).await.unwrap();

        assert_eq!(recipe.title().as_str(), "Soup");
    }

    #[tokio::test]
    async fn add_source_documents_assigns_identity_and_metadata() {
        let added_at = FixedClock::new().0;
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current_recipe("soup", "Soup"));
        repository
            .expect_add_source_documents()
            .times(1)
            .withf(move |images: &Vec<RecipeSourceDocument>| {
                images.len() == 2
                    && images.iter().all(|image| {
                        image.recipe_id() == &recipe_id()
                            && image.added_by() == &user_id()
                            && image.added_at() == added_at
                            && image.content_type() == "image/jpeg"
                    })
                    && images[0].original_name() == Some("front.jpg")
                    && images[0].bytes() == b"front"
                    && images[1].original_name() == Some("back.jpg")
                    && images[1].bytes() == b"back"
                    && images[0].id() != images[1].id()
            })
            .returning(|_| Ok(()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let summaries = service
            .add_source_documents(AddRecipeSourceDocumentsRequest {
                recipe_id: recipe_id(),
                added_by: user_id(),
                documents: vec![
                    new_source_document("front.jpg", b"front"),
                    new_source_document("back.jpg", b"back"),
                ],
            })
            .await
            .unwrap();

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].recipe_id(), &recipe_id());
        assert_eq!(summaries[0].content_type(), "image/jpeg");
        assert_eq!(summaries[0].original_name(), Some("front.jpg"));
        assert_eq!(summaries[0].byte_len(), 5);
        assert_eq!(summaries[0].added_by(), &user_id());
        assert_eq!(summaries[0].added_at(), added_at);
        assert_ne!(summaries[0].id(), summaries[1].id());
    }

    #[tokio::test]
    async fn add_source_documents_is_a_no_op_for_an_empty_request() {
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current_recipe("soup", "Soup"));
        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let summaries = service
            .add_source_documents(AddRecipeSourceDocumentsRequest {
                recipe_id: recipe_id(),
                added_by: user_id(),
                documents: Vec::new(),
            })
            .await
            .unwrap();

        assert!(summaries.is_empty());
    }

    #[tokio::test]
    async fn add_source_documents_maps_a_missing_recipe() {
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_get_recipe()
            .times(1)
            .returning(|_| Ok(None));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service
            .add_source_documents(AddRecipeSourceDocumentsRequest {
                recipe_id: recipe_id(),
                added_by: user_id(),
                documents: vec![new_source_document("front.jpg", b"front")],
            })
            .await;

        assert!(matches!(
            result,
            Err(AddRecipeSourceDocumentsError::RecipeNotFound)
        ));
    }

    #[tokio::test]
    async fn get_source_documents_returns_the_repository_list() {
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current_recipe("soup", "Soup"));
        repository
            .expect_get_source_documents()
            .times(1)
            .returning(|_| Ok(Vec::new()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let images = service
            .get_source_documents(recipe_id(), user_id())
            .await
            .unwrap();

        assert!(images.is_empty());
    }

    #[tokio::test]
    async fn get_source_document_returns_the_repository_image() {
        let image = RecipeSourceDocument::new(
            RecipeSourceDocumentId::new("image-1".to_string()),
            recipe_id(),
            "image/png".to_string(),
            Some("front.png".to_string()),
            b"front".to_vec(),
            user_id(),
            FixedClock::new().0,
        );
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current_recipe("soup", "Soup"));
        repository.expect_get_source_document().times(1).returning({
            let image = image.clone();
            move |_, _| Ok(Some(image.clone()))
        });

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let stored = service
            .get_source_document(
                recipe_id(),
                RecipeSourceDocumentId::new("image-1".to_string()),
                user_id(),
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(stored.id().as_str(), "image-1");
        assert_eq!(stored.content_type(), "image/png");
        assert_eq!(stored.bytes(), b"front");
    }

    #[tokio::test]
    async fn delete_source_document_maps_a_missing_image() {
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current_recipe("soup", "Soup"));
        repository
            .expect_delete_source_document()
            .returning(|_, _| Err(DeleteSourceDocumentError::NotFound));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service
            .delete_source_document(
                recipe_id(),
                RecipeSourceDocumentId::new("missing".to_string()),
                user_id(),
            )
            .await;

        assert!(matches!(
            result,
            Err(DeleteRecipeSourceDocumentError::NotFound)
        ));
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
        repository
            .expect_get_recipe_images()
            .times(1)
            .returning(|_| Ok(Vec::new()));
        let expected_id = current.id().clone();
        repository
            .expect_save_recipe_with_revision()
            .times(1)
            .withf(
                move |recipe: &Recipe, revision: &RecipeRevision, _: &Vec<RecipeImage>| {
                    recipe.id() == &expected_id
                        && recipe.author().as_str() == "user-1"
                        && revision.recipe_id() == &expected_id
                        && revision.content().title().as_str() == "Updated Soup"
                        && revision.editor().as_str() == "editor-2"
                        && revision.edited_at() == edited_at
                        && matches!(
                            revision.origin(),
                            RevisionOrigin::UserEdit { message: Some(message) }
                                if message.as_str() == "Changed the title"
                        )
                        && revision.verification() == RevisionVerification::Verified
                },
            )
            .returning(|_, _, _| Ok(()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let updated = service
            .update_recipe(UpdateRecipeRequest {
                id: current.id().clone(),
                content: content("Updated Soup"),
                editor: UserId::new("editor-2".to_string()),
                latest_revision,
                message: Some(RevisionMessage::new("Changed the title").unwrap()),
                new_images: Vec::new(),
            })
            .await
            .unwrap();

        assert_eq!(updated.id(), current.id());
        assert_eq!(updated.author().as_str(), "user-1");
        assert_eq!(updated.title().as_str(), "Updated Soup");
    }

    #[tokio::test]
    async fn convert_recipe_structure_saves_a_conversion_revision() {
        let current = current_recipe("soup", "Soup");
        let latest_revision = current.revision().id().clone();
        let expected_id = current.id().clone();

        let mut repository = MockRecipeRepository::new();
        repository.expect_get_recipe().times(1).returning({
            let current = current.clone();
            move |_| Ok(Some(current.clone()))
        });
        repository
            .expect_save_recipe_with_revision()
            .times(1)
            .withf(
                move |recipe: &Recipe, revision: &RecipeRevision, images: &Vec<RecipeImage>| {
                    recipe.id() == &expected_id
                        && revision.origin()
                            == &RevisionOrigin::StructureConverted {
                                to: RecipeStructureKind::Multipart,
                            }
                        && revision.content().is_multipart()
                        && revision.verification() == RevisionVerification::Verified
                        && images.is_empty()
                },
            )
            .returning(|_, _, _| Ok(()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let converted = service
            .convert_recipe_structure(ConvertRecipeStructureRequest {
                id: current.id().clone(),
                to: RecipeStructureKind::Multipart,
                editor: user_id(),
                latest_revision,
            })
            .await
            .unwrap();

        assert!(converted.content().is_multipart());
    }

    #[tokio::test]
    async fn restore_recipe_revision_appends_the_source_content() {
        let current = current_recipe("soup", "Current Soup");
        let source = RecipeRevision::new(
            RecipeRevisionId::new("revision-0".to_string()),
            current.id().clone(),
            content("Old Soup"),
            UserId::new("author-1".to_string()),
            FixedClock::new().0,
            RevisionOrigin::Created,
            RevisionVerification::NeedsReview,
        );
        let latest_revision = current.revision().id().clone();
        let expected_id = current.id().clone();
        let expected_source_id = source.id().clone();
        let revisions = vec![source.clone(), current.revision().clone()];

        let mut repository = MockRecipeRepository::new();
        repository.expect_get_recipe().times(1).returning({
            let current = current.clone();
            move |_| Ok(Some(current.clone()))
        });
        repository
            .expect_get_recipe_revisions()
            .times(1)
            .returning(move |_| Ok(revisions.clone()));
        repository
            .expect_save_recipe_with_revision()
            .times(1)
            .withf(
                move |recipe: &Recipe, revision: &RecipeRevision, images: &Vec<RecipeImage>| {
                    recipe.id() == &expected_id
                        && revision.content().title().as_str() == "Old Soup"
                        && revision.origin()
                            == &RevisionOrigin::Restored {
                                revision_id: expected_source_id.clone(),
                            }
                        && revision.verification() == RevisionVerification::NeedsReview
                        && images.is_empty()
                },
            )
            .returning(|_, _, _| Ok(()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let restored = service
            .restore_recipe_revision(RestoreRecipeRevisionRequest {
                id: current.id().clone(),
                revision_id: source.id().clone(),
                editor: user_id(),
                latest_revision,
            })
            .await
            .unwrap();

        assert_eq!(restored.title().as_str(), "Old Soup");
        assert!(restored.needs_review());
    }

    #[tokio::test]
    async fn restore_recipe_revision_rejects_a_stale_revision() {
        let current = current_recipe("soup", "Soup");

        let mut repository = MockRecipeRepository::new();
        repository.expect_get_recipe().times(1).returning({
            let current = current.clone();
            move |_| Ok(Some(current.clone()))
        });

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service
            .restore_recipe_revision(RestoreRecipeRevisionRequest {
                id: current.id().clone(),
                revision_id: current.revision().id().clone(),
                editor: user_id(),
                latest_revision: RecipeRevisionId::new("stale-revision".to_string()),
            })
            .await;

        assert!(matches!(result, Err(RestoreRecipeRevisionError::Conflict)));
    }

    #[tokio::test]
    async fn restore_recipe_revision_forbids_a_viewer_without_edit_access() {
        let current = current_recipe_with_scope("soup", "Soup", "author-1", RecipeScope::Viewable);

        let mut repository = MockRecipeRepository::new();
        repository.expect_get_recipe().times(1).returning({
            let current = current.clone();
            move |_| Ok(Some(current.clone()))
        });

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service
            .restore_recipe_revision(RestoreRecipeRevisionRequest {
                id: current.id().clone(),
                revision_id: current.revision().id().clone(),
                editor: user_id(),
                latest_revision: current.revision().id().clone(),
            })
            .await;

        assert!(matches!(result, Err(RestoreRecipeRevisionError::Forbidden)));
    }

    #[tokio::test]
    async fn restore_recipe_revision_rejects_an_unknown_source_revision() {
        let current = current_recipe("soup", "Soup");
        let revisions = vec![current.revision().clone()];

        let mut repository = MockRecipeRepository::new();
        repository.expect_get_recipe().times(1).returning({
            let current = current.clone();
            move |_| Ok(Some(current.clone()))
        });
        repository
            .expect_get_recipe_revisions()
            .times(1)
            .returning(move |_| Ok(revisions.clone()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service
            .restore_recipe_revision(RestoreRecipeRevisionRequest {
                id: current.id().clone(),
                revision_id: RecipeRevisionId::new("missing-revision".to_string()),
                editor: user_id(),
                latest_revision: current.revision().id().clone(),
            })
            .await;

        assert!(matches!(result, Err(RestoreRecipeRevisionError::NotFound)));
    }

    #[tokio::test]
    async fn restore_recipe_revision_maps_save_failures_to_storage_errors() {
        let current = current_recipe("soup", "Soup");
        let revisions = vec![current.revision().clone()];

        let mut repository = MockRecipeRepository::new();
        repository.expect_get_recipe().times(1).returning({
            let current = current.clone();
            move |_| Ok(Some(current.clone()))
        });
        repository
            .expect_get_recipe_revisions()
            .times(1)
            .returning(move |_| Ok(revisions.clone()));
        repository
            .expect_save_recipe_with_revision()
            .times(1)
            .returning(|_, _, _| Err(SaveRecipeError::Storage(anyhow!("write failed"))));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service
            .restore_recipe_revision(RestoreRecipeRevisionRequest {
                id: current.id().clone(),
                revision_id: current.revision().id().clone(),
                editor: user_id(),
                latest_revision: current.revision().id().clone(),
            })
            .await;

        assert!(matches!(
            result,
            Err(RestoreRecipeRevisionError::Storage(_))
        ));
    }

    #[tokio::test]
    async fn update_recipe_verifies_a_needs_review_recipe() {
        let recipe = recipe("soup");
        let latest = RecipeRevision::new(
            RecipeRevisionId::new("revision-1".to_string()),
            recipe.id().clone(),
            content("Soup"),
            recipe.author().clone(),
            FixedClock::new().0,
            RevisionOrigin::Created,
            RevisionVerification::NeedsReview,
        );
        let current = RecipeWithContent::new(recipe, latest).unwrap();
        let latest_revision = current.revision().id().clone();

        let mut repository = MockRecipeRepository::new();
        repository.expect_get_recipe().times(1).returning({
            let current = current.clone();
            move |_| Ok(Some(current.clone()))
        });
        repository
            .expect_get_recipe_images()
            .times(1)
            .returning(|_| Ok(Vec::new()));
        repository
            .expect_save_recipe_with_revision()
            .times(1)
            .withf(
                |_: &Recipe, revision: &RecipeRevision, _: &Vec<RecipeImage>| {
                    revision.verification() == RevisionVerification::Verified
                },
            )
            .returning(|_, _, _| Ok(()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let updated = service
            .update_recipe(UpdateRecipeRequest {
                id: current.id().clone(),
                content: content("Soup"),
                editor: UserId::new("editor-2".to_string()),
                latest_revision,
                message: None,
                new_images: Vec::new(),
            })
            .await
            .unwrap();

        assert!(!updated.needs_review());
        assert_eq!(updated.verification(), RevisionVerification::Verified);
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
                new_images: Vec::new(),
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
            .expect_get_recipe_images()
            .times(1)
            .returning(|_| Ok(Vec::new()));
        repository
            .expect_save_recipe_with_revision()
            .returning(|_, _, _| Err(SaveRecipeError::RecipeAuthorMismatch));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service
            .update_recipe(UpdateRecipeRequest {
                id: current.id().clone(),
                content: content("Updated Soup"),
                editor: UserId::new("editor-2".to_string()),
                latest_revision,
                message: None,
                new_images: Vec::new(),
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
                new_images: Vec::new(),
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
                new_images: Vec::new(),
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
            .expect_get_recipe_images()
            .times(1)
            .returning(|_| Ok(Vec::new()));
        repository
            .expect_save_recipe_with_revision()
            .returning(|_, _, _| Err(SaveRecipeError::Storage(anyhow!("store unavailable"))));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service
            .update_recipe(UpdateRecipeRequest {
                id: current.id().clone(),
                content: content("Updated Soup"),
                editor: UserId::new("editor-2".to_string()),
                latest_revision,
                message: None,
                new_images: Vec::new(),
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

        let recipes = service.get_recipes(user_id()).await.unwrap();

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

        let result = service.get_recipes(user_id()).await;

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

        let recipe = service.get_recipe(id, user_id()).await.unwrap();

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

        let result = service
            .get_recipe(RecipeId::new("soup".to_string()), user_id())
            .await;

        assert!(matches!(result, Err(GetRecipeError::Storage(_))));
    }

    #[tokio::test]
    async fn get_recipe_history_returns_the_repository_revisions() {
        let id = RecipeId::new("soup".to_string());
        let current = recipe("soup");
        let latest = revision("revision-1", &current, "user-1", FixedClock::new().0);
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current_recipe("soup", "Soup"));
        repository
            .expect_get_recipe_revisions()
            .times(1)
            .returning(move |_| Ok(vec![latest.clone()]));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let history = service.get_recipe_history(id, user_id()).await.unwrap();

        assert_eq!(history.len(), 1);
    }

    #[tokio::test]
    async fn get_recipe_history_maps_read_failures_to_storage_errors() {
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current_recipe("soup", "Soup"));
        repository.expect_get_recipe_revisions().returning(|_| {
            Err(crate::recipe::ports::ListRecipeRevisionsError::Storage(
                anyhow!("read unavailable"),
            ))
        });

        let service = RecipeServiceImpl::new(repository, FixedClock::new());

        let result = service
            .get_recipe_history(RecipeId::new("soup".to_string()), user_id())
            .await;

        assert!(matches!(result, Err(GetRecipeHistoryError::Storage(_))));
    }

    #[tokio::test]
    async fn get_recipes_filters_private_recipes_owned_by_others() {
        let mut repository = MockRecipeRepository::new();
        repository.expect_get_recipes().times(1).returning(|| {
            Ok(vec![
                current_recipe_with_scope("private", "Private", "author-1", RecipeScope::Private),
                current_recipe_with_scope(
                    "viewable",
                    "Viewable",
                    "author-1",
                    RecipeScope::Viewable,
                ),
                current_recipe_with_scope(
                    "editable",
                    "Editable",
                    "author-1",
                    RecipeScope::Editable,
                ),
                current_recipe_with_scope("mine", "Mine", "viewer", RecipeScope::Private),
            ])
        });

        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let recipes = service
            .get_recipes(UserId::new("viewer".to_string()))
            .await
            .unwrap();
        let ids: Vec<_> = recipes.iter().map(|recipe| recipe.id().as_str()).collect();

        assert_eq!(ids, ["viewable", "editable", "mine"]);
    }

    #[tokio::test]
    async fn get_recipe_hides_a_private_recipe_from_other_users() {
        let current = current_recipe_with_scope("soup", "Soup", "author-1", RecipeScope::Private);
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current);

        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let recipe = service
            .get_recipe(
                RecipeId::new("soup".to_string()),
                UserId::new("viewer".to_string()),
            )
            .await
            .unwrap();

        assert!(recipe.is_none());
    }

    #[tokio::test]
    async fn get_recipe_returns_a_private_recipe_to_its_author() {
        let current = current_recipe_with_scope("soup", "Soup", "author-1", RecipeScope::Private);
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current);

        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let recipe = service
            .get_recipe(
                RecipeId::new("soup".to_string()),
                UserId::new("author-1".to_string()),
            )
            .await
            .unwrap();

        assert!(recipe.is_some());
    }

    #[tokio::test]
    async fn update_recipe_forbids_a_non_author_on_a_viewable_recipe() {
        let current = current_recipe_with_scope("soup", "Soup", "author-1", RecipeScope::Viewable);
        let latest_revision = current.revision().id().clone();
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current);
        // No save expectation: a forbidden edit must not be persisted.

        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let result = service
            .update_recipe(UpdateRecipeRequest {
                id: RecipeId::new("soup".to_string()),
                content: content("Updated Soup"),
                editor: UserId::new("editor-2".to_string()),
                latest_revision,
                message: None,
                new_images: Vec::new(),
            })
            .await;

        assert!(matches!(result, Err(UpdateRecipeError::Forbidden)));
    }

    #[tokio::test]
    async fn update_recipe_hides_a_private_recipe_from_a_non_author() {
        let current = current_recipe_with_scope("soup", "Soup", "author-1", RecipeScope::Private);
        let latest_revision = current.revision().id().clone();
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current);

        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let result = service
            .update_recipe(UpdateRecipeRequest {
                id: RecipeId::new("soup".to_string()),
                content: content("Updated Soup"),
                editor: UserId::new("editor-2".to_string()),
                latest_revision,
                message: None,
                new_images: Vec::new(),
            })
            .await;

        assert!(matches!(result, Err(UpdateRecipeError::NotFound)));
    }

    #[tokio::test]
    async fn update_recipe_scope_saves_the_new_scope_for_the_author() {
        let current = current_recipe_with_scope("soup", "Soup", "author-1", RecipeScope::Editable);
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current.clone());
        repository
            .expect_save_recipe()
            .times(1)
            .withf(|recipe: &Recipe| {
                recipe.id().as_str() == "soup"
                    && recipe.author().as_str() == "author-1"
                    && recipe.scope() == RecipeScope::Private
            })
            .returning(|_| Ok(()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let updated = service
            .update_recipe_scope(UpdateRecipeScopeRequest {
                id: RecipeId::new("soup".to_string()),
                scope: RecipeScope::Private,
                actor: UserId::new("author-1".to_string()),
            })
            .await
            .unwrap();

        assert_eq!(updated.scope(), RecipeScope::Private);
        assert_eq!(updated.revision().id(), current.revision().id());
    }

    #[tokio::test]
    async fn update_recipe_scope_rejects_a_non_author() {
        let current = current_recipe_with_scope("soup", "Soup", "author-1", RecipeScope::Editable);
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current);
        // No save expectation: only the author may change the scope.

        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let result = service
            .update_recipe_scope(UpdateRecipeScopeRequest {
                id: RecipeId::new("soup".to_string()),
                scope: RecipeScope::Private,
                actor: UserId::new("editor-2".to_string()),
            })
            .await;

        assert!(matches!(result, Err(UpdateRecipeScopeError::Forbidden)));
    }

    #[tokio::test]
    async fn update_recipe_scope_hides_a_private_recipe_from_a_non_author() {
        let current = current_recipe_with_scope("soup", "Soup", "author-1", RecipeScope::Private);
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current);

        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let result = service
            .update_recipe_scope(UpdateRecipeScopeRequest {
                id: RecipeId::new("soup".to_string()),
                scope: RecipeScope::Editable,
                actor: UserId::new("editor-2".to_string()),
            })
            .await;

        assert!(matches!(result, Err(UpdateRecipeScopeError::NotFound)));
    }

    #[tokio::test]
    async fn get_recipe_history_hides_a_private_recipe_from_a_non_author() {
        let current = current_recipe_with_scope("soup", "Soup", "author-1", RecipeScope::Private);
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current);

        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let result = service
            .get_recipe_history(
                RecipeId::new("soup".to_string()),
                UserId::new("viewer".to_string()),
            )
            .await;

        assert!(matches!(result, Err(GetRecipeHistoryError::NotFound)));
    }

    #[tokio::test]
    async fn get_source_documents_hides_a_private_recipe_from_a_non_author() {
        let current = current_recipe_with_scope("soup", "Soup", "author-1", RecipeScope::Private);
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current);

        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let result = service
            .get_source_documents(
                RecipeId::new("soup".to_string()),
                UserId::new("viewer".to_string()),
            )
            .await;

        assert!(matches!(
            result,
            Err(GetRecipeSourceDocumentsError::NotFound)
        ));
    }

    #[tokio::test]
    async fn get_source_document_hides_a_private_recipe_from_a_non_author() {
        let current = current_recipe_with_scope("soup", "Soup", "author-1", RecipeScope::Private);
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current);
        // No image expectation: hidden recipes must not read their images.

        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let image = service
            .get_source_document(
                RecipeId::new("soup".to_string()),
                RecipeSourceDocumentId::new("image-1".to_string()),
                UserId::new("viewer".to_string()),
            )
            .await
            .unwrap();

        assert!(image.is_none());
    }

    #[tokio::test]
    async fn add_source_documents_forbids_a_non_author_on_a_viewable_recipe() {
        let current = current_recipe_with_scope("soup", "Soup", "author-1", RecipeScope::Viewable);
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current);

        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let result = service
            .add_source_documents(AddRecipeSourceDocumentsRequest {
                recipe_id: recipe_id(),
                added_by: UserId::new("editor-2".to_string()),
                documents: vec![new_source_document("front.jpg", b"front")],
            })
            .await;

        assert!(matches!(
            result,
            Err(AddRecipeSourceDocumentsError::Forbidden)
        ));
    }

    fn new_recipe_image(id: &str, bytes: &[u8]) -> NewRecipeImage {
        NewRecipeImage::new(
            RecipeImageId::new(id.to_string()),
            "image/jpeg".to_string(),
            bytes.to_vec(),
        )
    }

    fn image_metadata(id: &str) -> RecipeImageMetadata {
        RecipeImageMetadata::new(
            RecipeImageId::new(id.to_string()),
            recipe_id(),
            "image/jpeg".to_string(),
            4,
            user_id(),
            FixedClock::new().0,
        )
    }

    fn recipe_with_step(id: &str, instruction: &str) -> RecipeWithContent {
        let recipe = recipe(id);
        let revision = RecipeRevision::new(
            RecipeRevisionId::new(format!("{id}-revision")),
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
                        instruction.to_string(),
                        Vec::new(),
                    )],
                ),
            )
            .unwrap(),
            recipe.author().clone(),
            FixedClock::new().0,
            RevisionOrigin::Created,
            RevisionVerification::Verified,
        );

        RecipeWithContent::new(recipe, revision).unwrap()
    }

    #[tokio::test]
    async fn create_recipe_attaches_presentation_images() {
        let added_at = FixedClock::new().0;
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_save_new_recipe()
            .times(1)
            .withf(
                move |recipe: &Recipe,
                      _: &RecipeRevision,
                      _: &Vec<RecipeSourceDocument>,
                      images: &Vec<RecipeImage>| {
                    images.len() == 1
                        && images[0].id().as_str() == "image-1"
                        && images[0].recipe_id() == recipe.id()
                        && images[0].bytes() == b"image"
                        && images[0].added_by() == recipe.author()
                        && images[0].added_at() == added_at
                },
            )
            .returning(|_, _, _, _| Ok(()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let mut request = create_request();
        request.images = vec![new_recipe_image("image-1", b"image")];
        request.content = RecipeContent::new_simple(
            RecipeTitle::new("Soup".to_string()),
            None,
            None,
            None,
            RecipeBody::new(
                RecipeDurations::new(None, None, None),
                Vec::new(),
                vec![RecipeImageId::new("image-1".to_string())],
                Vec::new(),
            ),
        )
        .unwrap();

        service.create_verified_recipe(request).await.unwrap();
    }

    #[tokio::test]
    async fn create_recipe_rejects_an_unknown_image_reference() {
        let repository = MockRecipeRepository::new();
        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let mut request = create_request();
        request.content = RecipeContent::new_simple(
            RecipeTitle::new("Soup".to_string()),
            None,
            None,
            None,
            RecipeBody::new(
                RecipeDurations::new(None, None, None),
                Vec::new(),
                vec![RecipeImageId::new("missing".to_string())],
                Vec::new(),
            ),
        )
        .unwrap();

        let result = service.create_verified_recipe(request).await;

        assert!(matches!(
            result,
            Err(CreateRecipeError::InvalidImageReference)
        ));
    }

    #[tokio::test]
    async fn create_recipe_accepts_images_referenced_by_multipart_parts() {
        let mut repository = MockRecipeRepository::new();
        repository
            .expect_save_new_recipe()
            .times(1)
            .returning(|_, _, _, _| Ok(()));
        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let mut request = create_request();
        request.images = vec![new_recipe_image("part-image", b"image")];
        request.content = RecipeContent::new_multipart(
            RecipeTitle::new("Cake".to_string()),
            None,
            None,
            None,
            vec![
                RecipePart::new(
                    RecipePartId::generate(),
                    RecipePartTitle::new("Sponge".to_string()),
                    None,
                    RecipeBody::new(
                        RecipeDurations::new(None, None, None),
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    ),
                ),
                RecipePart::new(
                    RecipePartId::generate(),
                    RecipePartTitle::new("Filling".to_string()),
                    None,
                    RecipeBody::new(
                        RecipeDurations::new(None, None, None),
                        Vec::new(),
                        vec![RecipeImageId::new("part-image".to_string())],
                        Vec::new(),
                    ),
                ),
            ],
        )
        .unwrap();

        service.create_verified_recipe(request).await.unwrap();
    }

    #[tokio::test]
    async fn update_recipe_adds_images_referenced_by_the_revision() {
        let current = recipe_with_step("soup", "Stir.");
        let retained_step_id = current.simple_body().unwrap().steps()[0].id().clone();
        let latest_revision = current.revision().id().clone();
        let mut repository = MockRecipeRepository::new();
        let current_for_mock = current.clone();
        repository
            .expect_get_recipe()
            .returning(move |_| Ok(Some(current_for_mock.clone())));
        repository
            .expect_get_recipe_images()
            .returning(move |_| Ok(vec![image_metadata("kept")]));
        repository
            .expect_save_recipe_with_revision()
            .times(1)
            .withf(
                move |_: &Recipe, _: &RecipeRevision, added: &Vec<RecipeImage>| {
                    added.len() == 1 && added[0].id().as_str() == "step-image"
                },
            )
            .returning(|_, _, _| Ok(()));

        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let content = RecipeContent::new_simple(
            RecipeTitle::new("Soup".to_string()),
            None,
            None,
            None,
            RecipeBody::new(
                RecipeDurations::new(None, None, None),
                Vec::new(),
                vec![RecipeImageId::new("kept".to_string())],
                vec![RecipeStep::new(
                    retained_step_id,
                    None,
                    "Stir.".to_string(),
                    vec![RecipeImageId::new("step-image".to_string())],
                )],
            ),
        )
        .unwrap();

        service
            .update_recipe(UpdateRecipeRequest {
                id: current.id().clone(),
                content,
                editor: user_id(),
                latest_revision,
                message: None,
                new_images: vec![new_recipe_image("step-image", b"image")],
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn get_recipe_image_hides_a_private_recipe_from_a_non_author() {
        let current = current_recipe_with_scope("soup", "Soup", "author-1", RecipeScope::Private);
        let mut repository = MockRecipeRepository::new();
        expect_visible_recipe(&mut repository, current);

        let service = RecipeServiceImpl::new(repository, FixedClock::new());
        let image = service
            .get_recipe_image(
                RecipeId::new("soup".to_string()),
                RecipeImageId::new("image-1".to_string()),
                UserId::new("viewer".to_string()),
            )
            .await
            .unwrap();

        assert!(image.is_none());
    }
}
