use std::fmt;
use std::future::Future;
use std::time::SystemTime;

use chrono::NaiveDate;

use crate::recipe::models::{
    CookedRecipeNote, NewRecipeImage, NewRecipeSourceDocument, Recipe, RecipeCategory,
    RecipeContent, RecipeCookedEvent, RecipeCookedEventEntry, RecipeCookedEventId, RecipeForViewer,
    RecipeId, RecipeImage, RecipeImageId, RecipeImageMetadata, RecipeRevision, RecipeRevisionId,
    RecipeScope, RecipeSourceDocument, RecipeSourceDocumentId, RecipeSourceDocumentSummary,
    RecipeStructureKind, RecipeWithContent, RevisionMessage, RevisionVerification,
};
use crate::user::models::UserId;

#[derive(Debug, Clone)]
pub struct CreateRecipeRequest {
    pub content: RecipeContent,
    pub author: UserId,
    pub source_documents: Vec<NewRecipeSourceDocument>,
    pub images: Vec<NewRecipeImage>,
    pub verification: RevisionVerification,
}

#[derive(Debug, Clone)]
pub struct AddRecipeSourceDocumentsRequest {
    pub recipe_id: RecipeId,
    pub added_by: UserId,
    pub documents: Vec<NewRecipeSourceDocument>,
}

#[derive(Debug, Clone)]
pub struct UpdateRecipeRequest {
    pub id: RecipeId,
    pub content: RecipeContent,
    pub editor: UserId,
    pub latest_revision: RecipeRevisionId,
    pub message: Option<RevisionMessage>,
    pub new_images: Vec<NewRecipeImage>,
}

#[derive(Debug, Clone)]
pub struct UpdateRecipeScopeRequest {
    pub id: RecipeId,
    pub scope: RecipeScope,
    pub actor: UserId,
}

#[derive(Debug, Clone)]
pub struct ConvertRecipeStructureRequest {
    pub id: RecipeId,
    pub to: RecipeStructureKind,
    pub editor: UserId,
    pub latest_revision: RecipeRevisionId,
}

#[derive(Debug, Clone)]
pub struct RestoreRecipeRevisionRequest {
    pub id: RecipeId,
    pub revision_id: RecipeRevisionId,
    pub editor: UserId,
    pub latest_revision: RecipeRevisionId,
}

#[derive(Debug, Clone)]
pub struct AddRecipeCookedEventRequest {
    pub recipe_id: RecipeId,
    pub user_id: UserId,
    pub cooked_on: NaiveDate,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct UpdateRecipeCookedEventRequest {
    pub id: RecipeCookedEventId,
    pub user_id: UserId,
    pub cooked_on: NaiveDate,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct ListUserCookedEventsQuery {
    pub user_id: UserId,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct ListUserCookedEventsResult {
    pub events: Vec<RecipeCookedEventEntry>,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CategorySelection {
    All,
    Only(Vec<RecipeCategory>),
}

impl Default for CategorySelection {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeSearchCriteria {
    pub pattern: Option<String>,
    pub categories: CategorySelection,
    pub exclude_needs_review: bool,
    pub favorites_only: bool,
    pub sort: RecipeSort,
}

impl Default for RecipeSearchCriteria {
    fn default() -> Self {
        Self {
            pattern: None,
            categories: CategorySelection::default(),
            exclude_needs_review: false,
            favorites_only: false,
            sort: RecipeSort::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRecipesQuery {
    pub criteria: RecipeSearchCriteria,
    pub offset: usize,
    pub limit: Option<usize>,
}

impl Default for SearchRecipesQuery {
    fn default() -> Self {
        Self {
            criteria: RecipeSearchCriteria::default(),
            offset: 0,
            limit: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchRecipesResult {
    pub recipes: Vec<RecipeForViewer>,
    pub total: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipeSort {
    LastEdited(SortDirection),
    Title(SortDirection),
}

impl Default for RecipeSort {
    fn default() -> Self {
        Self::LastEdited(SortDirection::Descending)
    }
}

/// Inbound (driving) port for the recipe domain service.
pub trait RecipeService: Clone + Send + Sync + 'static {
    fn create_verified_recipe(
        &self,
        request: CreateRecipeRequest,
    ) -> impl Future<Output = Result<RecipeWithContent, CreateRecipeError>> + Send;

    fn create_imported_recipe(
        &self,
        request: CreateRecipeRequest,
    ) -> impl Future<Output = Result<RecipeWithContent, CreateRecipeError>> + Send;

    fn update_recipe(
        &self,
        request: UpdateRecipeRequest,
    ) -> impl Future<Output = Result<RecipeWithContent, UpdateRecipeError>> + Send;

    fn convert_recipe_structure(
        &self,
        request: ConvertRecipeStructureRequest,
    ) -> impl Future<Output = Result<RecipeWithContent, UpdateRecipeError>> + Send;

    fn restore_recipe_revision(
        &self,
        request: RestoreRecipeRevisionRequest,
    ) -> impl Future<Output = Result<RecipeWithContent, RestoreRecipeRevisionError>> + Send;

    fn update_recipe_scope(
        &self,
        request: UpdateRecipeScopeRequest,
    ) -> impl Future<Output = Result<RecipeWithContent, UpdateRecipeScopeError>> + Send;

    fn delete_recipe(
        &self,
        id: RecipeId,
        actor: UserId,
    ) -> impl Future<Output = Result<(), DeleteRecipeError>> + Send;

    fn purge_orphaned_private_recipes(
        &self,
    ) -> impl Future<Output = Result<usize, PurgeOrphanedRecipesError>> + Send;

    fn get_recipes(
        &self,
        viewer: UserId,
    ) -> impl Future<Output = Result<Vec<RecipeWithContent>, GetRecipesError>> + Send;

    fn search_recipes(
        &self,
        query: SearchRecipesQuery,
        viewer: UserId,
    ) -> impl Future<Output = Result<SearchRecipesResult, SearchRecipesError>> + Send;

    fn get_recipe(
        &self,
        id: RecipeId,
        viewer: UserId,
    ) -> impl Future<Output = Result<Option<RecipeWithContent>, GetRecipeError>> + Send;

    fn get_recipe_for_viewer(
        &self,
        id: RecipeId,
        viewer: UserId,
    ) -> impl Future<Output = Result<Option<RecipeForViewer>, GetRecipeForViewerError>> + Send;

    fn set_recipe_favorite(
        &self,
        id: RecipeId,
        viewer: UserId,
        favorite: bool,
    ) -> impl Future<Output = Result<(), SetRecipeFavoriteError>> + Send;

    fn add_recipe_cooked_event(
        &self,
        request: AddRecipeCookedEventRequest,
    ) -> impl Future<Output = Result<RecipeCookedEvent, AddRecipeCookedEventError>> + Send;

    fn update_recipe_cooked_event(
        &self,
        request: UpdateRecipeCookedEventRequest,
    ) -> impl Future<Output = Result<(), UpdateRecipeCookedEventError>> + Send;

    fn delete_recipe_cooked_event(
        &self,
        id: RecipeCookedEventId,
        actor: UserId,
    ) -> impl Future<Output = Result<(), DeleteRecipeCookedEventError>> + Send;

    fn list_recipe_cooked_events(
        &self,
        recipe_id: RecipeId,
        viewer: UserId,
    ) -> impl Future<Output = Result<Vec<RecipeCookedEvent>, ListRecipeCookedEventsError>> + Send;

    fn list_user_cooked_events(
        &self,
        query: ListUserCookedEventsQuery,
    ) -> impl Future<Output = Result<ListUserCookedEventsResult, ListUserCookedEventsError>> + Send;

    fn get_recipe_history(
        &self,
        id: RecipeId,
        viewer: UserId,
    ) -> impl Future<Output = Result<Vec<RecipeRevision>, GetRecipeHistoryError>> + Send;

    fn add_source_documents(
        &self,
        request: AddRecipeSourceDocumentsRequest,
    ) -> impl Future<
        Output = Result<Vec<RecipeSourceDocumentSummary>, AddRecipeSourceDocumentsError>,
    > + Send;

    fn get_source_documents(
        &self,
        recipe_id: RecipeId,
        viewer: UserId,
    ) -> impl Future<
        Output = Result<Vec<RecipeSourceDocumentSummary>, GetRecipeSourceDocumentsError>,
    > + Send;

    fn get_source_document(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeSourceDocumentId,
        viewer: UserId,
    ) -> impl Future<Output = Result<Option<RecipeSourceDocument>, GetRecipeSourceDocumentError>> + Send;

    fn delete_source_document(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeSourceDocumentId,
        actor: UserId,
    ) -> impl Future<Output = Result<(), DeleteRecipeSourceDocumentError>> + Send;

    fn get_recipe_images(
        &self,
        recipe_id: RecipeId,
        viewer: UserId,
    ) -> impl Future<Output = Result<Vec<RecipeImageMetadata>, GetRecipeImagesError>> + Send;

    fn get_recipe_image(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeImageId,
        viewer: UserId,
    ) -> impl Future<Output = Result<Option<RecipeImage>, GetRecipeImageError>> + Send;
}

/// Outbound (driven) port for recipe persistence.
pub trait RecipeRepository: Clone + Send + Sync + 'static {
    fn save_recipe(
        &self,
        recipe: Recipe,
    ) -> impl Future<Output = Result<(), SaveRecipeError>> + Send;

    fn save_recipe_revision(
        &self,
        recipe: Recipe,
        revision: RecipeRevision,
    ) -> impl Future<Output = Result<(), SaveRecipeError>> + Send;

    fn get_recipe(
        &self,
        id: RecipeId,
    ) -> impl Future<Output = Result<Option<RecipeWithContent>, GetRecipeError>> + Send;

    fn get_recipes(
        &self,
    ) -> impl Future<Output = Result<Vec<RecipeWithContent>, ListRecipesError>> + Send;

    fn find_orphaned_private_recipe_ids(
        &self,
    ) -> impl Future<Output = Result<Vec<RecipeId>, ListRecipesError>> + Send;

    fn search_recipes(
        &self,
        criteria: RecipeSearchCriteria,
        viewer: UserId,
    ) -> impl Future<Output = Result<Vec<RecipeForViewer>, SearchRecipesError>> + Send;

    fn get_recipe_revisions(
        &self,
        id: RecipeId,
    ) -> impl Future<Output = Result<Vec<RecipeRevision>, ListRecipeRevisionsError>> + Send;

    fn delete_recipe(
        &self,
        id: RecipeId,
    ) -> impl Future<Output = Result<(), DeleteRecipeRecordError>> + Send;

    fn add_favorite(
        &self,
        recipe_id: RecipeId,
        user_id: UserId,
        created_at: SystemTime,
    ) -> impl Future<Output = Result<(), AddFavoriteError>> + Send;

    fn remove_favorite(
        &self,
        recipe_id: RecipeId,
        user_id: UserId,
    ) -> impl Future<Output = Result<(), RemoveFavoriteError>> + Send;

    fn is_favorite(
        &self,
        recipe_id: RecipeId,
        user_id: UserId,
    ) -> impl Future<Output = Result<bool, GetFavoriteError>> + Send;

    fn add_cooked_event(
        &self,
        event: RecipeCookedEvent,
    ) -> impl Future<Output = Result<(), AddCookedEventError>> + Send;

    fn update_cooked_event(
        &self,
        id: RecipeCookedEventId,
        user_id: UserId,
        cooked_on: NaiveDate,
        note: CookedRecipeNote,
    ) -> impl Future<Output = Result<(), UpdateCookedEventError>> + Send;

    fn delete_cooked_event(
        &self,
        id: RecipeCookedEventId,
        user_id: UserId,
    ) -> impl Future<Output = Result<(), DeleteCookedEventError>> + Send;

    fn list_recipe_cooked_events(
        &self,
        recipe_id: RecipeId,
        user_id: UserId,
    ) -> impl Future<Output = Result<Vec<RecipeCookedEvent>, ListCookedEventsError>> + Send;

    fn list_user_cooked_events(
        &self,
        user_id: UserId,
        offset: usize,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<RecipeCookedEventEntry>, ListCookedEventsError>> + Send;

    fn count_user_cooked_events(
        &self,
        user_id: UserId,
    ) -> impl Future<Output = Result<usize, ListCookedEventsError>> + Send;
}

/// Outbound (driven) port for recipe documents (images, source) persistence.
pub trait RecipeDocumentRepository: Clone + Send + Sync + 'static {
    fn add_source_documents(
        &self,
        documents: Vec<RecipeSourceDocument>,
    ) -> impl Future<Output = Result<(), AddSourceDocumentsError>> + Send;

    fn get_source_documents(
        &self,
        recipe_id: RecipeId,
    ) -> impl Future<Output = Result<Vec<RecipeSourceDocumentSummary>, ListSourceDocumentsError>> + Send;

    fn get_source_document(
        &self,
        recipe_id: RecipeId,
        document_id: RecipeSourceDocumentId,
    ) -> impl Future<Output = Result<Option<RecipeSourceDocument>, GetSourceDocumentError>> + Send;

    fn delete_source_document(
        &self,
        recipe_id: RecipeId,
        document_id: RecipeSourceDocumentId,
    ) -> impl Future<Output = Result<(), DeleteSourceDocumentError>> + Send;

    fn add_recipe_images(
        &self,
        images: Vec<RecipeImage>,
    ) -> impl Future<Output = Result<(), AddRecipeImagesError>> + Send;

    fn get_recipe_images(
        &self,
        recipe_id: RecipeId,
    ) -> impl Future<Output = Result<Vec<RecipeImageMetadata>, ListRecipeImagesError>> + Send;

    fn get_recipe_image(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeImageId,
    ) -> impl Future<Output = Result<Option<RecipeImage>, GetRecipeImageError>> + Send;

    fn delete_recipe_assets(
        &self,
        recipe_id: RecipeId,
    ) -> impl Future<Output = Result<(), DeleteRecipeAssetsError>> + Send;
}

#[derive(Debug)]
pub enum CreateRecipeError {
    /// The request's verification status does not match the service
    /// entrypoint that was called.
    VerificationMismatch,
    /// An image targets a step that is not part of the recipe.
    InvalidImageReference,
    Storage(anyhow::Error),
}

impl fmt::Display for CreateRecipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VerificationMismatch => f.write_str(
                "the recipe entrypoint does not match the requested verification status",
            ),
            Self::InvalidImageReference => {
                f.write_str("an image reference is invalid for this recipe")
            }
            Self::Storage(reason) => write!(f, "failed to store recipe: {reason}"),
        }
    }
}

impl std::error::Error for CreateRecipeError {}

#[derive(Debug)]
pub enum UpdateRecipeError {
    NotFound,
    Forbidden,
    Conflict,
    InvalidImageReference,
    Storage(anyhow::Error),
}

impl fmt::Display for UpdateRecipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe not found"),
            Self::Forbidden => f.write_str("the recipe cannot be edited by this user"),
            Self::Conflict => f.write_str("the recipe was changed since it was loaded"),
            Self::InvalidImageReference => {
                f.write_str("an image reference is invalid for this recipe")
            }
            Self::Storage(reason) => write!(f, "failed to update recipe: {reason}"),
        }
    }
}

impl std::error::Error for UpdateRecipeError {}

#[derive(Debug)]
pub enum RestoreRecipeRevisionError {
    NotFound,
    Forbidden,
    Conflict,
    Storage(anyhow::Error),
}

impl fmt::Display for RestoreRecipeRevisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe or revision not found"),
            Self::Forbidden => f.write_str("the recipe cannot be edited by this user"),
            Self::Conflict => f.write_str("the recipe was changed since it was loaded"),
            Self::Storage(reason) => write!(f, "failed to restore recipe revision: {reason}"),
        }
    }
}

impl std::error::Error for RestoreRecipeRevisionError {}

#[derive(Debug)]
pub enum UpdateRecipeScopeError {
    NotFound,
    Forbidden,
    Storage(anyhow::Error),
}

impl fmt::Display for UpdateRecipeScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe not found"),
            Self::Forbidden => f.write_str("only the recipe author can change its scope"),
            Self::Storage(reason) => write!(f, "failed to update recipe scope: {reason}"),
        }
    }
}

impl std::error::Error for UpdateRecipeScopeError {}

#[derive(Debug)]
pub enum DeleteRecipeError {
    NotFound,
    Forbidden,
    Storage(anyhow::Error),
}

impl fmt::Display for DeleteRecipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe not found"),
            Self::Forbidden => f.write_str("only the recipe author can delete it"),
            Self::Storage(reason) => write!(f, "failed to delete recipe: {reason}"),
        }
    }
}

impl std::error::Error for DeleteRecipeError {}

#[derive(Debug)]
pub enum PurgeOrphanedRecipesError {
    Storage(anyhow::Error),
}

impl fmt::Display for PurgeOrphanedRecipesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to purge orphaned recipes: {reason}"),
        }
    }
}

impl std::error::Error for PurgeOrphanedRecipesError {}

#[derive(Debug)]
pub enum DeleteRecipeRecordError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for DeleteRecipeRecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe not found"),
            Self::Storage(reason) => write!(f, "failed to delete recipe: {reason}"),
        }
    }
}

impl std::error::Error for DeleteRecipeRecordError {}

#[derive(Debug)]
pub enum DeleteRecipeAssetsError {
    Storage(anyhow::Error),
}

impl fmt::Display for DeleteRecipeAssetsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to delete recipe assets: {reason}"),
        }
    }
}

impl std::error::Error for DeleteRecipeAssetsError {}

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
pub enum SearchRecipesError {
    Storage(anyhow::Error),
}

impl fmt::Display for SearchRecipesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to search recipes: {reason}"),
        }
    }
}

impl std::error::Error for SearchRecipesError {}

#[derive(Debug)]
pub enum GetRecipeHistoryError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for GetRecipeHistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe not found"),
            Self::Storage(reason) => write!(f, "failed to read recipe history: {reason}"),
        }
    }
}

impl std::error::Error for GetRecipeHistoryError {}

#[derive(Debug)]
pub enum SaveRecipeError {
    RecipeIdMismatch,
    RecipeAuthorMismatch,
    RecipeNotFound,
    InvalidRecipeReference,
    InvalidImageReference,
    Storage(anyhow::Error),
}

impl fmt::Display for SaveRecipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecipeIdMismatch => f.write_str("revision does not belong to the recipe"),
            Self::RecipeAuthorMismatch => f.write_str("a recipe author cannot be changed"),
            Self::RecipeNotFound => f.write_str("recipe not found"),
            Self::InvalidRecipeReference => {
                f.write_str("recipe information do not match the stored information")
            }
            Self::InvalidImageReference => {
                f.write_str("an image reference is invalid for this recipe")
            }
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
pub enum SetRecipeFavoriteError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for SetRecipeFavoriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe not found"),
            Self::Storage(reason) => write!(f, "failed to update recipe favorite: {reason}"),
        }
    }
}

impl std::error::Error for SetRecipeFavoriteError {}

#[derive(Debug)]
pub enum GetRecipeForViewerError {
    Storage(anyhow::Error),
}

impl fmt::Display for GetRecipeForViewerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read recipe for viewer: {reason}"),
        }
    }
}

impl std::error::Error for GetRecipeForViewerError {}

#[derive(Debug)]
pub enum AddFavoriteError {
    Storage(anyhow::Error),
}

impl fmt::Display for AddFavoriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to add recipe favorite: {reason}"),
        }
    }
}

impl std::error::Error for AddFavoriteError {}

#[derive(Debug)]
pub enum RemoveFavoriteError {
    Storage(anyhow::Error),
}

impl fmt::Display for RemoveFavoriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to remove recipe favorite: {reason}"),
        }
    }
}

impl std::error::Error for RemoveFavoriteError {}

#[derive(Debug)]
pub enum GetFavoriteError {
    Storage(anyhow::Error),
}

impl fmt::Display for GetFavoriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read recipe favorite: {reason}"),
        }
    }
}

impl std::error::Error for GetFavoriteError {}

#[derive(Debug)]
pub enum AddRecipeCookedEventError {
    NotFound,
    InvalidNote,
    Storage(anyhow::Error),
}

impl fmt::Display for AddRecipeCookedEventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe not found"),
            Self::InvalidNote => f.write_str("the cooked event note is invalid"),
            Self::Storage(reason) => write!(f, "failed to add cooked event: {reason}"),
        }
    }
}

impl std::error::Error for AddRecipeCookedEventError {}

#[derive(Debug)]
pub enum UpdateRecipeCookedEventError {
    NotFound,
    InvalidNote,
    Storage(anyhow::Error),
}

impl fmt::Display for UpdateRecipeCookedEventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("cooked event not found"),
            Self::InvalidNote => f.write_str("the cooked event note is invalid"),
            Self::Storage(reason) => write!(f, "failed to update cooked event: {reason}"),
        }
    }
}

impl std::error::Error for UpdateRecipeCookedEventError {}

#[derive(Debug)]
pub enum DeleteRecipeCookedEventError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for DeleteRecipeCookedEventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("cooked event not found"),
            Self::Storage(reason) => write!(f, "failed to delete cooked event: {reason}"),
        }
    }
}

impl std::error::Error for DeleteRecipeCookedEventError {}

#[derive(Debug)]
pub enum ListRecipeCookedEventsError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for ListRecipeCookedEventsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe not found"),
            Self::Storage(reason) => write!(f, "failed to read cooked events: {reason}"),
        }
    }
}

impl std::error::Error for ListRecipeCookedEventsError {}

#[derive(Debug)]
pub enum ListUserCookedEventsError {
    Storage(anyhow::Error),
}

impl fmt::Display for ListUserCookedEventsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read cooked events: {reason}"),
        }
    }
}

impl std::error::Error for ListUserCookedEventsError {}

#[derive(Debug)]
pub enum AddCookedEventError {
    RecipeNotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for AddCookedEventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecipeNotFound => f.write_str("recipe not found"),
            Self::Storage(reason) => write!(f, "failed to add cooked event: {reason}"),
        }
    }
}

impl std::error::Error for AddCookedEventError {}

#[derive(Debug)]
pub enum UpdateCookedEventError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for UpdateCookedEventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("cooked event not found"),
            Self::Storage(reason) => write!(f, "failed to update cooked event: {reason}"),
        }
    }
}

impl std::error::Error for UpdateCookedEventError {}

#[derive(Debug)]
pub enum DeleteCookedEventError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for DeleteCookedEventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("cooked event not found"),
            Self::Storage(reason) => write!(f, "failed to delete cooked event: {reason}"),
        }
    }
}

impl std::error::Error for DeleteCookedEventError {}

#[derive(Debug)]
pub enum ListCookedEventsError {
    Storage(anyhow::Error),
}

impl fmt::Display for ListCookedEventsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read cooked events: {reason}"),
        }
    }
}

impl std::error::Error for ListCookedEventsError {}

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

#[derive(Debug)]
pub enum ListRecipeRevisionsError {
    Storage(anyhow::Error),
}

impl fmt::Display for ListRecipeRevisionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read recipe revisions: {reason}"),
        }
    }
}

impl std::error::Error for ListRecipeRevisionsError {}

#[derive(Debug)]
pub enum AddRecipeSourceDocumentsError {
    RecipeNotFound,
    Forbidden,
    Storage(anyhow::Error),
}

impl fmt::Display for AddRecipeSourceDocumentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecipeNotFound => f.write_str("recipe not found"),
            Self::Forbidden => f.write_str("the recipe cannot be edited by this user"),
            Self::Storage(reason) => write!(f, "failed to store source documents: {reason}"),
        }
    }
}

impl std::error::Error for AddRecipeSourceDocumentsError {}

#[derive(Debug)]
pub enum GetRecipeSourceDocumentsError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for GetRecipeSourceDocumentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe not found"),
            Self::Storage(reason) => write!(f, "failed to read source documents: {reason}"),
        }
    }
}

impl std::error::Error for GetRecipeSourceDocumentsError {}

#[derive(Debug)]
pub enum GetRecipeSourceDocumentError {
    Storage(anyhow::Error),
}

impl fmt::Display for GetRecipeSourceDocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read source document: {reason}"),
        }
    }
}

impl std::error::Error for GetRecipeSourceDocumentError {}

#[derive(Debug)]
pub enum GetRecipeImagesError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for GetRecipeImagesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("recipe not found"),
            Self::Storage(reason) => write!(f, "failed to read recipe images: {reason}"),
        }
    }
}

impl std::error::Error for GetRecipeImagesError {}

#[derive(Debug)]
pub enum ListRecipeImagesError {
    Storage(anyhow::Error),
}

impl fmt::Display for ListRecipeImagesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read recipe images: {reason}"),
        }
    }
}

impl std::error::Error for ListRecipeImagesError {}

#[derive(Debug)]
pub enum GetRecipeImageError {
    Storage(anyhow::Error),
}

impl fmt::Display for GetRecipeImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read recipe image: {reason}"),
        }
    }
}

impl std::error::Error for GetRecipeImageError {}

#[derive(Debug)]
pub enum DeleteRecipeSourceDocumentError {
    NotFound,
    Forbidden,
    Storage(anyhow::Error),
}

impl fmt::Display for DeleteRecipeSourceDocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("source document not found"),
            Self::Forbidden => f.write_str("the recipe cannot be edited by this user"),
            Self::Storage(reason) => write!(f, "failed to delete source document: {reason}"),
        }
    }
}

impl std::error::Error for DeleteRecipeSourceDocumentError {}

#[derive(Debug)]
pub enum AddSourceDocumentsError {
    RecipeNotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for AddSourceDocumentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecipeNotFound => f.write_str("recipe not found"),
            Self::Storage(reason) => write!(f, "failed to store source documents: {reason}"),
        }
    }
}

impl std::error::Error for AddSourceDocumentsError {}

#[derive(Debug)]
pub enum AddRecipeImagesError {
    RecipeNotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for AddRecipeImagesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecipeNotFound => f.write_str("recipe not found"),
            Self::Storage(reason) => write!(f, "failed to store recipe images: {reason}"),
        }
    }
}

impl std::error::Error for AddRecipeImagesError {}

#[derive(Debug)]
pub enum ListSourceDocumentsError {
    Storage(anyhow::Error),
}

impl fmt::Display for ListSourceDocumentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read source documents: {reason}"),
        }
    }
}

impl std::error::Error for ListSourceDocumentsError {}

#[derive(Debug)]
pub enum GetSourceDocumentError {
    Storage(anyhow::Error),
}

impl fmt::Display for GetSourceDocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to read source document: {reason}"),
        }
    }
}

impl std::error::Error for GetSourceDocumentError {}

#[derive(Debug)]
pub enum DeleteSourceDocumentError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for DeleteSourceDocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("source document not found"),
            Self::Storage(reason) => write!(f, "failed to delete source document: {reason}"),
        }
    }
}

impl std::error::Error for DeleteSourceDocumentError {}

#[cfg(test)]
mockall::mock! {
    pub RecipeService {}

    impl Clone for RecipeService {
        fn clone(&self) -> Self;
    }

    impl RecipeService for RecipeService {
        async fn create_verified_recipe(
            &self,
            request: CreateRecipeRequest,
        ) -> Result<RecipeWithContent, CreateRecipeError>;

        async fn create_imported_recipe(
            &self,
            request: CreateRecipeRequest,
        ) -> Result<RecipeWithContent, CreateRecipeError>;

        async fn update_recipe(
            &self,
            request: UpdateRecipeRequest,
        ) -> Result<RecipeWithContent, UpdateRecipeError>;

        async fn convert_recipe_structure(
            &self,
            request: ConvertRecipeStructureRequest,
        ) -> Result<RecipeWithContent, UpdateRecipeError>;

        async fn restore_recipe_revision(
            &self,
            request: RestoreRecipeRevisionRequest,
        ) -> Result<RecipeWithContent, RestoreRecipeRevisionError>;

        async fn update_recipe_scope(
            &self,
            request: UpdateRecipeScopeRequest,
        ) -> Result<RecipeWithContent, UpdateRecipeScopeError>;

        async fn delete_recipe(
            &self,
            id: RecipeId,
            actor: UserId,
        ) -> Result<(), DeleteRecipeError>;

        async fn purge_orphaned_private_recipes(
            &self,
        ) -> Result<usize, PurgeOrphanedRecipesError>;

        async fn get_recipes(
            &self,
            viewer: UserId,
        ) -> Result<Vec<RecipeWithContent>, GetRecipesError>;

        async fn search_recipes(
            &self,
            query: SearchRecipesQuery,
            viewer: UserId,
        ) -> Result<SearchRecipesResult, SearchRecipesError>;

        async fn get_recipe(
            &self,
            id: RecipeId,
            viewer: UserId,
        ) -> Result<Option<RecipeWithContent>, GetRecipeError>;

        async fn get_recipe_for_viewer(
            &self,
            id: RecipeId,
            viewer: UserId,
        ) -> Result<Option<RecipeForViewer>, GetRecipeForViewerError>;

        async fn set_recipe_favorite(
            &self,
            id: RecipeId,
            viewer: UserId,
            favorite: bool,
        ) -> Result<(), SetRecipeFavoriteError>;

        async fn add_recipe_cooked_event(
            &self,
            request: AddRecipeCookedEventRequest,
        ) -> Result<RecipeCookedEvent, AddRecipeCookedEventError>;

        async fn update_recipe_cooked_event(
            &self,
            request: UpdateRecipeCookedEventRequest,
        ) -> Result<(), UpdateRecipeCookedEventError>;

        async fn delete_recipe_cooked_event(
            &self,
            id: RecipeCookedEventId,
            actor: UserId,
        ) -> Result<(), DeleteRecipeCookedEventError>;

        async fn list_recipe_cooked_events(
            &self,
            recipe_id: RecipeId,
            viewer: UserId,
        ) -> Result<Vec<RecipeCookedEvent>, ListRecipeCookedEventsError>;

        async fn list_user_cooked_events(
            &self,
            query: ListUserCookedEventsQuery,
        ) -> Result<ListUserCookedEventsResult, ListUserCookedEventsError>;

        async fn get_recipe_history(
            &self,
            id: RecipeId,
            viewer: UserId,
        ) -> Result<Vec<RecipeRevision>, GetRecipeHistoryError>;

        async fn add_source_documents(
            &self,
            request: AddRecipeSourceDocumentsRequest,
        ) -> Result<Vec<RecipeSourceDocumentSummary>, AddRecipeSourceDocumentsError>;

        async fn get_source_documents(
            &self,
            recipe_id: RecipeId,
            viewer: UserId,
        ) -> Result<Vec<RecipeSourceDocumentSummary>, GetRecipeSourceDocumentsError>;

        async fn get_source_document(
            &self,
            recipe_id: RecipeId,
            image_id: RecipeSourceDocumentId,
            viewer: UserId,
        ) -> Result<Option<RecipeSourceDocument>, GetRecipeSourceDocumentError>;

        async fn delete_source_document(
            &self,
            recipe_id: RecipeId,
            image_id: RecipeSourceDocumentId,
            actor: UserId,
        ) -> Result<(), DeleteRecipeSourceDocumentError>;

        async fn get_recipe_images(
            &self,
            recipe_id: RecipeId,
            viewer: UserId,
        ) -> Result<Vec<RecipeImageMetadata>, GetRecipeImagesError>;

        async fn get_recipe_image(
            &self,
            recipe_id: RecipeId,
            image_id: RecipeImageId,
            viewer: UserId,
        ) -> Result<Option<RecipeImage>, GetRecipeImageError>;
    }
}

#[cfg(test)]
mockall::mock! {
    pub RecipeRepository {}

    impl Clone for RecipeRepository {
        fn clone(&self) -> Self;
    }

    impl RecipeRepository for RecipeRepository {
        async fn save_recipe(
            &self,
            recipe: Recipe,
        ) -> Result<(), SaveRecipeError>;

        async fn save_recipe_revision(
            &self,
            recipe: Recipe,
            revision: RecipeRevision,
        ) -> Result<(), SaveRecipeError>;

        async fn get_recipe(
            &self,
            id: RecipeId,
        ) -> Result<Option<RecipeWithContent>, GetRecipeError>;

        async fn get_recipes(&self) -> Result<Vec<RecipeWithContent>, ListRecipesError>;

        async fn find_orphaned_private_recipe_ids(
            &self,
        ) -> Result<Vec<RecipeId>, ListRecipesError>;

        async fn search_recipes(
            &self,
            criteria: RecipeSearchCriteria,
            viewer: UserId,
        ) -> Result<Vec<RecipeForViewer>, SearchRecipesError>;

        async fn get_recipe_revisions(
            &self,
            id: RecipeId,
        ) -> Result<Vec<RecipeRevision>, ListRecipeRevisionsError>;

        async fn delete_recipe(
            &self,
            id: RecipeId,
        ) -> Result<(), DeleteRecipeRecordError>;

        async fn add_favorite(
            &self,
            recipe_id: RecipeId,
            user_id: UserId,
            created_at: SystemTime,
        ) -> Result<(), AddFavoriteError>;

        async fn remove_favorite(
            &self,
            recipe_id: RecipeId,
            user_id: UserId,
        ) -> Result<(), RemoveFavoriteError>;

        async fn is_favorite(
            &self,
            recipe_id: RecipeId,
            user_id: UserId,
        ) -> Result<bool, GetFavoriteError>;

        async fn add_cooked_event(
            &self,
            event: RecipeCookedEvent,
        ) -> Result<(), AddCookedEventError>;

        async fn update_cooked_event(
            &self,
            id: RecipeCookedEventId,
            user_id: UserId,
            cooked_on: NaiveDate,
            note: CookedRecipeNote,
        ) -> Result<(), UpdateCookedEventError>;

        async fn delete_cooked_event(
            &self,
            id: RecipeCookedEventId,
            user_id: UserId,
        ) -> Result<(), DeleteCookedEventError>;

        async fn list_recipe_cooked_events(
            &self,
            recipe_id: RecipeId,
            user_id: UserId,
        ) -> Result<Vec<RecipeCookedEvent>, ListCookedEventsError>;

        async fn list_user_cooked_events(
            &self,
            user_id: UserId,
            offset: usize,
            limit: usize,
        ) -> Result<Vec<RecipeCookedEventEntry>, ListCookedEventsError>;

        async fn count_user_cooked_events(
            &self,
            user_id: UserId,
        ) -> Result<usize, ListCookedEventsError>;


    }
}

#[cfg(test)]
mockall::mock! {

    pub RecipeDocumentRepository {}

    impl Clone for RecipeDocumentRepository {
        fn clone(&self) -> Self;
    }

    impl RecipeDocumentRepository for RecipeDocumentRepository {
        async fn add_source_documents(
                &self,
                images: Vec<RecipeSourceDocument>,
            ) -> Result<(), AddSourceDocumentsError>;

            async fn get_source_documents(
                &self,
                recipe_id: RecipeId,
            ) -> Result<Vec<RecipeSourceDocumentSummary>, ListSourceDocumentsError>;

            async fn get_source_document(
                &self,
                recipe_id: RecipeId,
                image_id: RecipeSourceDocumentId,
            ) -> Result<Option<RecipeSourceDocument>, GetSourceDocumentError>;

            async fn delete_source_document(
                &self,
                recipe_id: RecipeId,
                image_id: RecipeSourceDocumentId,
            ) -> Result<(), DeleteSourceDocumentError>;

            async fn add_recipe_images(
                &self,
                images: Vec<RecipeImage>,
            ) -> Result<(), AddRecipeImagesError>;

            async fn get_recipe_images(
                &self,
                recipe_id: RecipeId,
            ) -> Result<Vec<RecipeImageMetadata>, ListRecipeImagesError>;

            async fn get_recipe_image(
                &self,
                recipe_id: RecipeId,
                image_id: RecipeImageId,
            ) -> Result<Option<RecipeImage>, GetRecipeImageError>;

            async fn delete_recipe_assets(
                &self,
                recipe_id: RecipeId,
            ) -> Result<(), DeleteRecipeAssetsError>;
    }
}
