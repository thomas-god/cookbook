use std::time::SystemTime;

use anyhow::{Context, anyhow, bail};
use chrono::NaiveDate;
use cookbook_domain::recipe::models::{
    CookedRecipeLink, CookedRecipeNote, CookedRecipeSnapshot, Recipe, RecipeCategory,
    RecipeCookedEvent, RecipeCookedEventEntry, RecipeCookedEventId, RecipeForViewer, RecipeId,
    RecipeRevision, RecipeRevisionId, RecipeScope, RecipeTitle, RecipeWithContent, RevisionOrigin,
    RevisionVerification,
};
use cookbook_domain::recipe::ports::{
    AddCookedEventError, AddFavoriteError, CategorySelection, DeleteCookedEventError,
    DeleteRecipeRecordError, GetFavoriteError, GetRecipeError, ListCookedEventsError,
    ListRecipeRevisionsError, ListRecipesError, RecipeRepository, RecipeSearchCriteria, RecipeSort,
    RemoveFavoriteError, SaveRecipeError, SearchRecipesError, SortDirection,
    UpdateCookedEventError,
};
use cookbook_domain::user::models::UserId;
use sqlx::sqlite::{Sqlite, SqlitePool, SqliteRow};
use sqlx::{Connection, QueryBuilder, Row};

use crate::storage::content::{decode_content, encode_content, search_text as build_search_text};
use crate::storage::time;

const SELECT_CURRENT_BY_ID: &str = "
    SELECT r.id AS recipe_id, r.author_id, r.scope,
           rev.id AS revision_id, rev.recipe_id AS revision_recipe_id,
           rev.editor_id, rev.edited_at_ms, rev.origin, rev.verification,
           rev.content_version, rev.content
    FROM recipes r
    LEFT JOIN recipe_revisions rev ON rev.seq = (
        SELECT MAX(seq) FROM recipe_revisions WHERE recipe_id = r.id
    )
    WHERE r.id = ?";

const SELECT_ALL_CURRENT: &str = "
    SELECT r.id AS recipe_id, r.author_id, r.scope,
           rev.id AS revision_id, rev.recipe_id AS revision_recipe_id,
           rev.editor_id, rev.edited_at_ms, rev.origin, rev.verification,
           rev.content_version, rev.content
    FROM recipes r
    LEFT JOIN recipe_revisions rev ON rev.seq = (
        SELECT MAX(seq) FROM recipe_revisions WHERE recipe_id = r.id
    )
    ORDER BY r.id";

const SELECT_CURRENT_SEARCH: &str = "
    SELECT r.id AS recipe_id, r.author_id, r.scope,
           rev.id AS revision_id, rev.recipe_id AS revision_recipe_id,
           rev.editor_id, rev.edited_at_ms, rev.origin, rev.verification,
           rev.content_version, rev.content,
           EXISTS (
               SELECT 1 FROM recipe_favorites f
               WHERE f.recipe_id = r.id AND f.user_id = ";

const SEARCH_FROM_CURRENT: &str = "
           ) AS is_favorite
    FROM recipes r
    JOIN recipe_revisions rev ON rev.seq = (
        SELECT MAX(seq) FROM recipe_revisions WHERE recipe_id = r.id
    )
    WHERE 1 = 1";

const SELECT_REVISIONS: &str = "
    SELECT id AS revision_id, recipe_id AS revision_recipe_id,
           editor_id, edited_at_ms, origin, verification,
           content_version, content
    FROM recipe_revisions
    WHERE recipe_id = ?
    ORDER BY seq ASC";

#[derive(Debug, Clone)]
pub struct SqliteRecipeRepository {
    pool: SqlitePool,
}

impl SqliteRecipeRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

impl RecipeRepository for SqliteRecipeRepository {
    async fn save_recipe(&self, recipe: Recipe) -> Result<(), SaveRecipeError> {
        let mut connection = self.pool.acquire().await.map_err(save_storage)?;
        let mut transaction = connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(save_storage)?;

        let author: Option<String> =
            sqlx::query_scalar("SELECT author_id FROM recipes WHERE id = ?")
                .bind(recipe.id().as_str())
                .fetch_optional(&mut *transaction)
                .await
                .map_err(save_storage)?;

        match author {
            None => return Err(SaveRecipeError::RecipeNotFound),
            Some(author) if author != recipe.author().as_str() => {
                return Err(SaveRecipeError::RecipeAuthorMismatch);
            }
            Some(_) => {}
        }

        sqlx::query("UPDATE recipes SET scope = ? WHERE id = ?")
            .bind(recipe.scope().as_str())
            .bind(recipe.id().as_str())
            .execute(&mut *transaction)
            .await
            .map_err(save_storage)?;

        transaction.commit().await.map_err(save_storage)?;
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

        let (content_version, content) =
            encode_content(revision.content()).map_err(SaveRecipeError::Storage)?;
        let origin = serde_json::to_string(revision.origin())
            .context("failed to encode revision origin")
            .map_err(SaveRecipeError::Storage)?;
        let edited_at_ms =
            time::to_millis(revision.edited_at()).map_err(SaveRecipeError::Storage)?;
        let title = revision.content().title().as_str();
        let title_key = title.to_lowercase();
        let category = revision.content().category().map(RecipeCategory::as_str);
        let search_text = build_search_text(revision.content());

        let mut connection = self.pool.acquire().await.map_err(save_storage)?;
        let mut transaction = connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(save_storage)?;

        let existing = sqlx::query("SELECT author_id, scope FROM recipes WHERE id = ?")
            .bind(recipe.id().as_str())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(save_storage)?;

        if let Some(row) = existing {
            let author: String = row.try_get("author_id").map_err(save_storage)?;
            let scope: String = row.try_get("scope").map_err(save_storage)?;

            if author != recipe.author().as_str() || scope != recipe.scope().as_str() {
                return Err(SaveRecipeError::InvalidRecipeReference);
            }
        } else {
            sqlx::query("INSERT INTO recipes (id, author_id, scope) VALUES (?, ?, ?)")
                .bind(recipe.id().as_str())
                .bind(recipe.author().as_str())
                .bind(recipe.scope().as_str())
                .execute(&mut *transaction)
                .await
                .map_err(save_storage)?;
        }

        sqlx::query(
            "INSERT INTO recipe_revisions (
                id, recipe_id, editor_id, edited_at_ms, origin, verification,
                content_version, content, title, title_key, category, search_text
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(revision.id().as_str())
        .bind(revision.recipe_id().as_str())
        .bind(revision.editor().as_str())
        .bind(edited_at_ms)
        .bind(origin)
        .bind(revision.verification().as_str())
        .bind(content_version)
        .bind(content)
        .bind(title)
        .bind(title_key)
        .bind(category)
        .bind(search_text)
        .execute(&mut *transaction)
        .await
        .map_err(save_storage)?;

        transaction.commit().await.map_err(save_storage)?;
        Ok(())
    }

    async fn get_recipe(&self, id: RecipeId) -> Result<Option<RecipeWithContent>, GetRecipeError> {
        let row = sqlx::query(SELECT_CURRENT_BY_ID)
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(get_storage)?;

        row.map(|row| decode_current(&row))
            .transpose()
            .map_err(get_storage)
    }

    async fn get_recipes(&self) -> Result<Vec<RecipeWithContent>, ListRecipesError> {
        let rows = sqlx::query(SELECT_ALL_CURRENT)
            .fetch_all(&self.pool)
            .await
            .map_err(list_storage)?;

        rows.iter()
            .map(decode_current)
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(list_storage)
    }

    async fn find_orphaned_private_recipe_ids(&self) -> Result<Vec<RecipeId>, ListRecipesError> {
        let rows = sqlx::query(
            "
            SELECT r.id
            FROM recipes r
            LEFT JOIN users u ON u.id = r.author_id
            WHERE u.id IS NULL AND r.scope = ?
            ORDER BY r.id",
        )
        .bind(RecipeScope::Private.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(list_storage)?;

        rows.iter()
            .map(|row| row.try_get::<String, _>("id").map(RecipeId::new))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| ListRecipesError::Storage(error.into()))
    }

    async fn search_recipes(
        &self,
        criteria: RecipeSearchCriteria,
        viewer: UserId,
    ) -> Result<Vec<RecipeForViewer>, SearchRecipesError> {
        let pattern = criteria
            .pattern
            .as_deref()
            .map(str::trim)
            .filter(|pattern| !pattern.is_empty())
            .map(str::to_lowercase);

        let mut builder = QueryBuilder::<Sqlite>::new(SELECT_CURRENT_SEARCH);
        builder.push_bind(viewer.as_str()).push(SEARCH_FROM_CURRENT);

        if let Some(pattern) = &pattern {
            builder
                .push(" AND instr(rev.search_text, ")
                .push_bind(pattern.as_str())
                .push(") > 0");
        }

        match &criteria.categories {
            CategorySelection::All => {}
            CategorySelection::Only(categories) if categories.is_empty() => {
                builder.push(" AND 0");
            }
            CategorySelection::Only(categories) => {
                builder.push(" AND rev.category IN (");
                let mut separated = builder.separated(", ");
                for category in categories {
                    separated.push_bind(category.as_str());
                }
                separated.push_unseparated(")");
            }
        }

        if criteria.exclude_needs_review {
            builder
                .push(" AND rev.verification <> ")
                .push_bind(RevisionVerification::NeedsReview.as_str());
        }

        if criteria.favorites_only {
            builder
                .push(
                    " AND EXISTS (
                        SELECT 1 FROM recipe_favorites f
                        WHERE f.recipe_id = r.id AND f.user_id = ",
                )
                .push_bind(viewer.as_str())
                .push(")");
        }

        match criteria.sort {
            RecipeSort::LastEdited(direction) => {
                builder
                    .push(" ORDER BY rev.edited_at_ms ")
                    .push(sort_direction(direction))
                    .push(", rev.title_key ASC, r.id ASC");
            }
            RecipeSort::Title(direction) => {
                builder
                    .push(" ORDER BY rev.title_key ")
                    .push(sort_direction(direction))
                    .push(", r.id ASC");
            }
        }

        let rows = builder
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(search_storage)?;

        rows.iter()
            .map(decode_for_viewer)
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(search_storage)
    }

    async fn get_recipe_revisions(
        &self,
        id: RecipeId,
    ) -> Result<Vec<RecipeRevision>, ListRecipeRevisionsError> {
        let rows = sqlx::query(SELECT_REVISIONS)
            .bind(id.as_str())
            .fetch_all(&self.pool)
            .await
            .map_err(revisions_storage)?;

        rows.iter()
            .map(decode_revision)
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(revisions_storage)
    }

    async fn delete_recipe(&self, id: RecipeId) -> Result<(), DeleteRecipeRecordError> {
        let result = sqlx::query("DELETE FROM recipes WHERE id = ?")
            .bind(id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|error| DeleteRecipeRecordError::Storage(error.into()))?;

        if result.rows_affected() == 0 {
            return Err(DeleteRecipeRecordError::NotFound);
        }

        Ok(())
    }

    async fn add_favorite(
        &self,
        recipe_id: RecipeId,
        user_id: UserId,
        created_at: SystemTime,
    ) -> Result<(), AddFavoriteError> {
        let created_at_ms = time::to_millis(created_at).map_err(AddFavoriteError::Storage)?;

        sqlx::query(
            "INSERT OR IGNORE INTO recipe_favorites (user_id, recipe_id, created_at_ms)
             VALUES (?, ?, ?)",
        )
        .bind(user_id.as_str())
        .bind(recipe_id.as_str())
        .bind(created_at_ms)
        .execute(&self.pool)
        .await
        .map_err(|error| AddFavoriteError::Storage(error.into()))?;

        Ok(())
    }

    async fn remove_favorite(
        &self,
        recipe_id: RecipeId,
        user_id: UserId,
    ) -> Result<(), RemoveFavoriteError> {
        sqlx::query("DELETE FROM recipe_favorites WHERE user_id = ? AND recipe_id = ?")
            .bind(user_id.as_str())
            .bind(recipe_id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|error| RemoveFavoriteError::Storage(error.into()))?;

        Ok(())
    }

    async fn is_favorite(
        &self,
        recipe_id: RecipeId,
        user_id: UserId,
    ) -> Result<bool, GetFavoriteError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM recipe_favorites WHERE user_id = ? AND recipe_id = ?
             )",
        )
        .bind(user_id.as_str())
        .bind(recipe_id.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(|error| GetFavoriteError::Storage(error.into()))?;

        Ok(exists)
    }

    async fn add_cooked_event(&self, event: RecipeCookedEvent) -> Result<(), AddCookedEventError> {
        let recipe_id = event
            .recipe_id()
            .ok_or_else(|| AddCookedEventError::Storage(anyhow!("cooked event has no recipe")))?;
        let created_at_ms =
            time::to_millis(event.created_at()).map_err(AddCookedEventError::Storage)?;

        sqlx::query(
            "INSERT INTO recipe_cooked_events (
                id, recipe_id, user_id, recipe_revision_id, recipe_title,
                cooked_on, note, created_at_ms
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(event.id().as_str())
        .bind(recipe_id.as_str())
        .bind(event.user_id().as_str())
        .bind(event.snapshot().revision_id().as_str())
        .bind(event.snapshot().title().as_str())
        .bind(format_cooked_on(event.cooked_on()))
        .bind(event.note().as_str())
        .bind(created_at_ms)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            let is_foreign_key = error
                .as_database_error()
                .is_some_and(|error| error.is_foreign_key_violation());

            if is_foreign_key {
                AddCookedEventError::RecipeNotFound
            } else {
                AddCookedEventError::Storage(error.into())
            }
        })?;

        Ok(())
    }

    async fn update_cooked_event(
        &self,
        id: RecipeCookedEventId,
        user_id: UserId,
        cooked_on: NaiveDate,
        note: CookedRecipeNote,
    ) -> Result<(), UpdateCookedEventError> {
        let result = sqlx::query(
            "UPDATE recipe_cooked_events
             SET cooked_on = ?, note = ?
             WHERE id = ? AND user_id = ?",
        )
        .bind(format_cooked_on(cooked_on))
        .bind(note.as_str())
        .bind(id.as_str())
        .bind(user_id.as_str())
        .execute(&self.pool)
        .await
        .map_err(|error| UpdateCookedEventError::Storage(error.into()))?;

        if result.rows_affected() == 0 {
            return Err(UpdateCookedEventError::NotFound);
        }

        Ok(())
    }

    async fn delete_cooked_event(
        &self,
        id: RecipeCookedEventId,
        user_id: UserId,
    ) -> Result<(), DeleteCookedEventError> {
        let result = sqlx::query("DELETE FROM recipe_cooked_events WHERE id = ? AND user_id = ?")
            .bind(id.as_str())
            .bind(user_id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|error| DeleteCookedEventError::Storage(error.into()))?;

        if result.rows_affected() == 0 {
            return Err(DeleteCookedEventError::NotFound);
        }

        Ok(())
    }

    async fn list_recipe_cooked_events(
        &self,
        recipe_id: RecipeId,
        user_id: UserId,
    ) -> Result<Vec<RecipeCookedEvent>, ListCookedEventsError> {
        let rows = sqlx::query(
            "SELECT id, recipe_id, user_id, recipe_revision_id, recipe_title,
                    cooked_on, note, created_at_ms
             FROM recipe_cooked_events
             WHERE recipe_id = ? AND user_id = ?
             ORDER BY cooked_on DESC, created_at_ms DESC, id DESC",
        )
        .bind(recipe_id.as_str())
        .bind(user_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(list_cooked_storage)?;

        rows.iter()
            .map(decode_cooked_event)
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(list_cooked_storage)
    }

    async fn list_user_cooked_events(
        &self,
        user_id: UserId,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<RecipeCookedEventEntry>, ListCookedEventsError> {
        let rows = sqlx::query(
            "SELECT e.id, e.recipe_id, e.user_id, e.recipe_revision_id, e.recipe_title,
                    e.cooked_on, e.note, e.created_at_ms,
                    CASE
                        WHEN r.id IS NOT NULL
                             AND (r.scope <> 'private' OR r.author_id = e.user_id)
                        THEN rev.title
                        ELSE e.recipe_title
                    END AS display_title,
                    CASE
                        WHEN r.id IS NOT NULL
                             AND (r.scope <> 'private' OR r.author_id = e.user_id)
                        THEN r.id
                        ELSE NULL
                    END AS visible_recipe_id,
                    CASE
                        WHEN r.id IS NOT NULL
                             AND (r.scope <> 'private' OR r.author_id = e.user_id)
                             AND EXISTS (
                                 SELECT 1 FROM recipe_revisions visible_rev
                                 WHERE visible_rev.recipe_id = r.id
                                   AND visible_rev.id = e.recipe_revision_id
                             )
                        THEN e.recipe_revision_id
                        ELSE NULL
                    END AS visible_revision_id
             FROM recipe_cooked_events e
             LEFT JOIN recipes r ON r.id = e.recipe_id
             LEFT JOIN recipe_revisions rev ON rev.seq = (
                 SELECT MAX(seq) FROM recipe_revisions WHERE recipe_id = r.id
             )
             WHERE e.user_id = ?
             ORDER BY e.cooked_on DESC, e.created_at_ms DESC, e.id DESC
             LIMIT ? OFFSET ?",
        )
        .bind(user_id.as_str())
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .bind(i64::try_from(offset).unwrap_or(i64::MAX))
        .fetch_all(&self.pool)
        .await
        .map_err(list_cooked_storage)?;

        rows.iter()
            .map(decode_cooked_event_entry)
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(list_cooked_storage)
    }

    async fn count_user_cooked_events(
        &self,
        user_id: UserId,
    ) -> Result<usize, ListCookedEventsError> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM recipe_cooked_events WHERE user_id = ?")
                .bind(user_id.as_str())
                .fetch_one(&self.pool)
                .await
                .map_err(list_cooked_storage)?;

        usize::try_from(count).map_err(|error| ListCookedEventsError::Storage(error.into()))
    }
}

fn decode_current(row: &SqliteRow) -> anyhow::Result<RecipeWithContent> {
    let recipe_id: String = row.try_get("recipe_id")?;
    let author_id: String = row.try_get("author_id")?;
    let scope: String = row.try_get("scope")?;
    let scope =
        RecipeScope::parse(&scope).ok_or_else(|| anyhow!("unknown recipe scope `{scope}`"))?;

    let recipe = Recipe::with_scope(RecipeId::new(recipe_id), UserId::new(author_id), scope);
    let revision = decode_revision(row)?;

    RecipeWithContent::new(recipe, revision).map_err(Into::into)
}

fn decode_for_viewer(row: &SqliteRow) -> anyhow::Result<RecipeForViewer> {
    let recipe = decode_current(row)?;
    let is_favorite: bool = row.try_get("is_favorite")?;

    Ok(RecipeForViewer::new(recipe, is_favorite))
}

fn decode_cooked_event(row: &SqliteRow) -> anyhow::Result<RecipeCookedEvent> {
    let id: String = row.try_get("id")?;
    let recipe_id: Option<String> = row.try_get("recipe_id")?;
    let user_id: String = row.try_get("user_id")?;
    let recipe_revision_id: String = row.try_get("recipe_revision_id")?;
    let recipe_title: String = row.try_get("recipe_title")?;
    let cooked_on: String = row.try_get("cooked_on")?;
    let note: String = row.try_get("note")?;
    let created_at_ms: i64 = row.try_get("created_at_ms")?;

    let cooked_on = parse_cooked_on(&cooked_on)?;
    let created_at = time::from_millis(created_at_ms)?;
    let note = CookedRecipeNote::try_new(note).map_err(|error| anyhow!(error))?;

    Ok(RecipeCookedEvent::new(
        RecipeCookedEventId::new(id),
        recipe_id.map(RecipeId::new),
        UserId::new(user_id),
        CookedRecipeSnapshot::new(
            RecipeRevisionId::new(recipe_revision_id),
            RecipeTitle::new(recipe_title),
        ),
        cooked_on,
        note,
        created_at,
    ))
}

fn decode_cooked_event_entry(row: &SqliteRow) -> anyhow::Result<RecipeCookedEventEntry> {
    let event = decode_cooked_event(row)?;
    let display_title: String = row.try_get("display_title")?;
    let visible_recipe_id: Option<String> = row.try_get("visible_recipe_id")?;
    let visible_revision_id: Option<String> = row.try_get("visible_revision_id")?;

    let recipe_link = match (visible_recipe_id, visible_revision_id) {
        (Some(recipe_id), Some(revision_id)) => Some(CookedRecipeLink::new(
            RecipeId::new(recipe_id),
            RecipeRevisionId::new(revision_id),
        )),
        _ => None,
    };

    Ok(RecipeCookedEventEntry::new(
        event,
        RecipeTitle::new(display_title),
        recipe_link,
    ))
}

fn format_cooked_on(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn parse_cooked_on(value: &str) -> anyhow::Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .with_context(|| format!("failed to parse cooked event date `{value}`"))
}

fn decode_revision(row: &SqliteRow) -> anyhow::Result<RecipeRevision> {
    let revision_id: Option<String> = row.try_get("revision_id")?;
    let Some(revision_id) = revision_id else {
        bail!("recipe has no revisions");
    };

    let recipe_id: String = row.try_get("revision_recipe_id")?;
    let editor_id: String = row.try_get("editor_id")?;
    let edited_at_ms: i64 = row.try_get("edited_at_ms")?;
    let origin: String = row.try_get("origin")?;
    let verification: String = row.try_get("verification")?;
    let content_version: i32 = row.try_get("content_version")?;
    let content: String = row.try_get("content")?;

    let origin: RevisionOrigin =
        serde_json::from_str(&origin).context("failed to decode revision origin")?;
    let verification = RevisionVerification::parse(&verification)
        .ok_or_else(|| anyhow!("unknown revision verification `{verification}`"))?;
    let content = decode_content(content_version, &content)?;

    Ok(RecipeRevision::new(
        RecipeRevisionId::new(revision_id),
        RecipeId::new(recipe_id),
        content,
        UserId::new(editor_id),
        time::from_millis(edited_at_ms)?,
        origin,
        verification,
    ))
}

fn save_storage(error: impl Into<anyhow::Error>) -> SaveRecipeError {
    SaveRecipeError::Storage(error.into())
}

fn get_storage(error: impl Into<anyhow::Error>) -> GetRecipeError {
    GetRecipeError::Storage(error.into())
}

fn list_storage(error: impl Into<anyhow::Error>) -> ListRecipesError {
    ListRecipesError::Storage(error.into())
}

fn revisions_storage(error: impl Into<anyhow::Error>) -> ListRecipeRevisionsError {
    ListRecipeRevisionsError::Storage(error.into())
}

fn search_storage(error: impl Into<anyhow::Error>) -> SearchRecipesError {
    SearchRecipesError::Storage(error.into())
}

fn list_cooked_storage(error: impl Into<anyhow::Error>) -> ListCookedEventsError {
    ListCookedEventsError::Storage(error.into())
}

fn sort_direction(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Ascending => "ASC",
        SortDirection::Descending => "DESC",
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use cookbook_domain::recipe::models::{
        Ingredient, IngredientName, Quantity, RecipeBody, RecipeCategory, RecipeContent,
        RecipeDurations, RecipeStep, RecipeStepId, RecipeTitle, RevisionOrigin,
        RevisionVerification,
    };

    use super::*;
    use crate::storage::SqliteDatabase;

    async fn repository() -> SqliteRecipeRepository {
        let database = SqliteDatabase::open_in_memory().await.unwrap();

        SqliteRecipeRepository::new(database.pool().clone())
    }

    fn recipe(id: &str) -> Recipe {
        Recipe::new(
            RecipeId::new(id.to_string()),
            UserId::new("user-1".to_string()),
        )
    }

    fn viewer() -> UserId {
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

    fn revision(id: &str, recipe: &Recipe, title: &str, edited_at: SystemTime) -> RecipeRevision {
        RecipeRevision::new(
            RecipeRevisionId::new(id.to_string()),
            recipe.id().clone(),
            content(title),
            recipe.author().clone(),
            edited_at,
            RevisionOrigin::Created,
            RevisionVerification::Verified,
        )
    }

    async fn save(repository: &SqliteRecipeRepository, recipe: Recipe, revision: RecipeRevision) {
        repository
            .save_recipe_revision(recipe, revision)
            .await
            .unwrap();
    }

    #[allow(clippy::too_many_arguments)]
    fn searchable_revision(
        id: &str,
        recipe: &Recipe,
        title: &str,
        category: Option<RecipeCategory>,
        verification: RevisionVerification,
        edited_at: SystemTime,
        ingredients: Vec<Ingredient>,
        steps: Vec<RecipeStep>,
    ) -> RecipeRevision {
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

        RecipeRevision::new(
            RecipeRevisionId::new(id.to_string()),
            recipe.id().clone(),
            content,
            recipe.author().clone(),
            edited_at,
            RevisionOrigin::Created,
            verification,
        )
    }

    #[allow(clippy::too_many_arguments)]
    async fn save_searchable(
        repository: &SqliteRecipeRepository,
        recipe_id: &str,
        title: &str,
        category: Option<RecipeCategory>,
        verification: RevisionVerification,
        edited_at: SystemTime,
        ingredients: Vec<Ingredient>,
        steps: Vec<RecipeStep>,
    ) {
        let recipe = recipe(recipe_id);
        let revision = searchable_revision(
            &format!("{recipe_id}-revision"),
            &recipe,
            title,
            category,
            verification,
            edited_at,
            ingredients,
            steps,
        );

        save(repository, recipe, revision).await;
    }

    fn titles(recipes: &[RecipeForViewer]) -> Vec<&str> {
        recipes
            .iter()
            .map(|recipe| recipe.recipe().title().as_str())
            .collect()
    }

    #[tokio::test]
    async fn saves_and_gets_a_recipe_by_id() {
        let repository = repository().await;
        let recipe = recipe("soup");

        save(
            &repository,
            recipe.clone(),
            revision("revision-1", &recipe, "Soup", SystemTime::UNIX_EPOCH),
        )
        .await;

        let stored = repository
            .get_recipe(RecipeId::new("soup".to_string()))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(stored.id().as_str(), "soup");
        assert_eq!(stored.title().as_str(), "Soup");
    }

    #[tokio::test]
    async fn returns_none_for_an_unknown_recipe() {
        let repository = repository().await;

        let stored = repository
            .get_recipe(RecipeId::new("unknown".to_string()))
            .await
            .unwrap();

        assert!(stored.is_none());
    }

    #[tokio::test]
    async fn returns_every_saved_recipe() {
        let repository = repository().await;

        for (id, title) in [("soup", "Soup"), ("stew", "Stew")] {
            let recipe = recipe(id);
            save(
                &repository,
                recipe.clone(),
                revision(
                    &format!("{id}-revision"),
                    &recipe,
                    title,
                    SystemTime::UNIX_EPOCH,
                ),
            )
            .await;
        }

        let stored = repository.get_recipes().await.unwrap();

        assert_eq!(stored.len(), 2);
    }

    #[tokio::test]
    async fn latest_revision_wins_and_revisions_stay_ordered() {
        let repository = repository().await;
        let recipe = recipe("soup");

        save(
            &repository,
            recipe.clone(),
            revision("revision-1", &recipe, "Soup", SystemTime::UNIX_EPOCH),
        )
        .await;
        save(
            &repository,
            recipe.clone(),
            revision(
                "revision-2",
                &recipe,
                "Stew",
                SystemTime::UNIX_EPOCH + Duration::from_secs(60),
            ),
        )
        .await;

        let stored = repository
            .get_recipe(RecipeId::new("soup".to_string()))
            .await
            .unwrap()
            .unwrap();
        let revisions = repository
            .get_recipe_revisions(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert_eq!(stored.title().as_str(), "Stew");
        assert_eq!(revisions.len(), 2);
        assert_eq!(revisions[0].id().as_str(), "revision-1");
        assert_eq!(revisions[1].id().as_str(), "revision-2");
    }

    #[tokio::test]
    async fn rejects_a_revision_for_another_recipe() {
        let repository = repository().await;
        let soup = recipe("soup");
        let revision = revision(
            "salad-revision",
            &recipe("salad"),
            "Salad",
            SystemTime::UNIX_EPOCH,
        );

        let result = repository.save_recipe_revision(soup, revision).await;

        assert!(matches!(result, Err(SaveRecipeError::RecipeIdMismatch)));
    }

    #[tokio::test]
    async fn rejects_a_revision_when_the_stored_recipe_differs() {
        let repository = repository().await;
        let soup = recipe("soup");
        save(
            &repository,
            soup.clone(),
            revision("revision-1", &soup, "Soup", SystemTime::UNIX_EPOCH),
        )
        .await;

        let other = Recipe::new(
            RecipeId::new("soup".to_string()),
            UserId::new("user-2".to_string()),
        );
        let result = repository
            .save_recipe_revision(
                other.clone(),
                revision("revision-2", &other, "Other", SystemTime::UNIX_EPOCH),
            )
            .await;

        assert!(matches!(
            result,
            Err(SaveRecipeError::InvalidRecipeReference)
        ));
    }

    #[tokio::test]
    async fn saves_scope_changes_without_adding_a_revision() {
        let repository = repository().await;
        let recipe = recipe("soup");
        save(
            &repository,
            recipe.clone(),
            revision("revision-1", &recipe, "Soup", SystemTime::UNIX_EPOCH),
        )
        .await;

        repository
            .save_recipe(recipe.update_scope(RecipeScope::Private))
            .await
            .unwrap();

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
    async fn rejects_invalid_scope_changes() {
        let repository = repository().await;

        let missing = recipe("soup").update_scope(RecipeScope::Private);
        assert!(matches!(
            repository.save_recipe(missing).await,
            Err(SaveRecipeError::RecipeNotFound)
        ));

        let stored = recipe("soup");
        save(
            &repository,
            stored.clone(),
            revision("revision-1", &stored, "Soup", SystemTime::UNIX_EPOCH),
        )
        .await;

        let other = Recipe::new(
            RecipeId::new("soup".to_string()),
            UserId::new("user-2".to_string()),
        )
        .update_scope(RecipeScope::Private);

        assert!(matches!(
            repository.save_recipe(other).await,
            Err(SaveRecipeError::RecipeAuthorMismatch)
        ));
    }

    #[tokio::test]
    async fn a_recipe_without_revisions_is_a_storage_error() {
        let repository = repository().await;

        sqlx::query(
            "INSERT INTO recipes (id, author_id, scope)
             VALUES ('soup', 'user-1', 'editable')",
        )
        .execute(repository.pool())
        .await
        .unwrap();

        let result = repository
            .get_recipe(RecipeId::new("soup".to_string()))
            .await;

        assert!(matches!(result, Err(GetRecipeError::Storage(_))));
    }

    #[tokio::test]
    async fn search_returns_every_recipe_by_default() {
        let repository = repository().await;

        for (id, title) in [("soup", "Tomato Soup"), ("stew", "Beef Stew")] {
            save_searchable(
                &repository,
                id,
                title,
                None,
                RevisionVerification::Verified,
                SystemTime::UNIX_EPOCH,
                Vec::new(),
                Vec::new(),
            )
            .await;
        }

        let results = repository
            .search_recipes(RecipeSearchCriteria::default(), viewer())
            .await
            .unwrap();

        assert_eq!(titles(&results), ["Beef Stew", "Tomato Soup"]);
    }

    #[tokio::test]
    async fn search_matches_title_ingredients_and_steps() {
        let repository = repository().await;

        save_searchable(
            &repository,
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
        )
        .await;

        for pattern in ["tomato", "BASIL", "simmer"] {
            let results = repository
                .search_recipes(
                    RecipeSearchCriteria {
                        pattern: Some(pattern.to_string()),
                        ..Default::default()
                    },
                    viewer(),
                )
                .await
                .unwrap();

            assert_eq!(titles(&results), ["Tomato Soup"], "pattern `{pattern}`");
        }

        let results = repository
            .search_recipes(
                RecipeSearchCriteria {
                    pattern: Some("missing".to_string()),
                    ..Default::default()
                },
                viewer(),
            )
            .await
            .unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn search_treats_a_blank_pattern_as_no_filter() {
        let repository = repository().await;

        save_searchable(
            &repository,
            "soup",
            "Soup",
            None,
            RevisionVerification::Verified,
            SystemTime::UNIX_EPOCH,
            Vec::new(),
            Vec::new(),
        )
        .await;

        let results = repository
            .search_recipes(
                RecipeSearchCriteria {
                    pattern: Some("   ".to_string()),
                    ..Default::default()
                },
                viewer(),
            )
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn search_filters_by_category_and_excludes_uncategorized_recipes() {
        let repository = repository().await;

        for (id, title, category) in [
            ("soup", "Soup", Some(RecipeCategory::Main)),
            ("cake", "Cake", Some(RecipeCategory::Dessert)),
            ("mystery", "Mystery", None),
        ] {
            save_searchable(
                &repository,
                id,
                title,
                category,
                RevisionVerification::Verified,
                SystemTime::UNIX_EPOCH,
                Vec::new(),
                Vec::new(),
            )
            .await;
        }

        let results = repository
            .search_recipes(
                RecipeSearchCriteria {
                    categories: CategorySelection::Only(vec![RecipeCategory::Main]),
                    ..Default::default()
                },
                viewer(),
            )
            .await
            .unwrap();

        assert_eq!(titles(&results), ["Soup"]);
    }

    #[tokio::test]
    async fn search_with_an_empty_category_selection_matches_nothing() {
        let repository = repository().await;

        save_searchable(
            &repository,
            "soup",
            "Soup",
            Some(RecipeCategory::Main),
            RevisionVerification::Verified,
            SystemTime::UNIX_EPOCH,
            Vec::new(),
            Vec::new(),
        )
        .await;

        let results = repository
            .search_recipes(
                RecipeSearchCriteria {
                    categories: CategorySelection::Only(Vec::new()),
                    ..Default::default()
                },
                viewer(),
            )
            .await
            .unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn search_reports_and_filters_favorites_for_the_viewer() {
        let repository = repository().await;
        insert_user(&repository, "user-1").await;
        insert_user(&repository, "user-2").await;

        for (id, title) in [("soup", "Soup"), ("cake", "Cake")] {
            save_searchable(
                &repository,
                id,
                title,
                None,
                RevisionVerification::Verified,
                SystemTime::UNIX_EPOCH,
                Vec::new(),
                Vec::new(),
            )
            .await;
        }

        repository
            .add_favorite(
                RecipeId::new("soup".to_string()),
                UserId::new("user-1".to_string()),
                SystemTime::UNIX_EPOCH,
            )
            .await
            .unwrap();

        let results = repository
            .search_recipes(RecipeSearchCriteria::default(), viewer())
            .await
            .unwrap();
        let states: Vec<_> = results
            .iter()
            .map(|recipe| (recipe.recipe().title().as_str(), recipe.is_favorite()))
            .collect();
        assert_eq!(states, [("Cake", false), ("Soup", true)]);

        let favorites = repository
            .search_recipes(
                RecipeSearchCriteria {
                    favorites_only: true,
                    ..Default::default()
                },
                viewer(),
            )
            .await
            .unwrap();
        assert_eq!(titles(&favorites), ["Soup"]);

        let other_viewer = repository
            .search_recipes(
                RecipeSearchCriteria {
                    favorites_only: true,
                    ..Default::default()
                },
                UserId::new("user-2".to_string()),
            )
            .await
            .unwrap();
        assert!(other_viewer.is_empty());
    }

    #[tokio::test]
    async fn search_can_exclude_recipes_that_need_review() {
        let repository = repository().await;

        save_searchable(
            &repository,
            "verified",
            "Verified",
            None,
            RevisionVerification::Verified,
            SystemTime::UNIX_EPOCH,
            Vec::new(),
            Vec::new(),
        )
        .await;
        save_searchable(
            &repository,
            "review",
            "Needs Review",
            None,
            RevisionVerification::NeedsReview,
            SystemTime::UNIX_EPOCH,
            Vec::new(),
            Vec::new(),
        )
        .await;

        let all = repository
            .search_recipes(RecipeSearchCriteria::default(), viewer())
            .await
            .unwrap();
        let verified = repository
            .search_recipes(
                RecipeSearchCriteria {
                    exclude_needs_review: true,
                    ..Default::default()
                },
                viewer(),
            )
            .await
            .unwrap();

        assert_eq!(all.len(), 2);
        assert_eq!(titles(&verified), ["Verified"]);
    }

    #[tokio::test]
    async fn search_sorts_by_latest_revision_date_in_both_directions() {
        let repository = repository().await;
        let older = SystemTime::UNIX_EPOCH;
        let newer = SystemTime::UNIX_EPOCH + Duration::from_secs(60);

        for (id, title, edited_at) in [("old", "Older", older), ("new", "Newer", newer)] {
            save_searchable(
                &repository,
                id,
                title,
                None,
                RevisionVerification::Verified,
                edited_at,
                Vec::new(),
                Vec::new(),
            )
            .await;
        }

        let newest_first = repository
            .search_recipes(
                RecipeSearchCriteria {
                    sort: RecipeSort::LastEdited(SortDirection::Descending),
                    ..Default::default()
                },
                viewer(),
            )
            .await
            .unwrap();
        let oldest_first = repository
            .search_recipes(
                RecipeSearchCriteria {
                    sort: RecipeSort::LastEdited(SortDirection::Ascending),
                    ..Default::default()
                },
                viewer(),
            )
            .await
            .unwrap();

        assert_eq!(titles(&newest_first), ["Newer", "Older"]);
        assert_eq!(titles(&oldest_first), ["Older", "Newer"]);
    }

    #[tokio::test]
    async fn search_sorts_titles_case_insensitively_in_both_directions() {
        let repository = repository().await;

        for (id, title) in [("pie", "apple pie"), ("bread", "Banana Bread")] {
            save_searchable(
                &repository,
                id,
                title,
                None,
                RevisionVerification::Verified,
                SystemTime::UNIX_EPOCH,
                Vec::new(),
                Vec::new(),
            )
            .await;
        }

        let ascending = repository
            .search_recipes(
                RecipeSearchCriteria {
                    sort: RecipeSort::Title(SortDirection::Ascending),
                    ..Default::default()
                },
                viewer(),
            )
            .await
            .unwrap();
        let descending = repository
            .search_recipes(
                RecipeSearchCriteria {
                    sort: RecipeSort::Title(SortDirection::Descending),
                    ..Default::default()
                },
                viewer(),
            )
            .await
            .unwrap();

        assert_eq!(titles(&ascending), ["apple pie", "Banana Bread"]);
        assert_eq!(titles(&descending), ["Banana Bread", "apple pie"]);
    }

    #[tokio::test]
    async fn deletes_a_recipe_and_cascades_its_rows() {
        let repository = repository().await;
        let recipe = recipe("soup");
        save(
            &repository,
            recipe.clone(),
            revision("revision-1", &recipe, "Soup", SystemTime::UNIX_EPOCH),
        )
        .await;

        sqlx::query(
            "INSERT INTO recipe_images (
                id, recipe_id, content_type, byte_len, added_by, added_at_ms, storage_key
             ) VALUES (
                'image-1', 'soup', 'image/jpeg', 4, 'user-1', 0,
                'recipes/soup/images/image-1'
             )",
        )
        .execute(repository.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO recipe_source_documents (
                id, recipe_id, content_type, original_name, byte_len, added_by,
                added_at_ms, storage_key
             ) VALUES (
                'document-1', 'soup', 'image/jpeg', 'front.jpg', 4, 'user-1',
                0, 'recipes/soup/documents/document-1'
             )",
        )
        .execute(repository.pool())
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

        let images: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM recipe_images")
            .fetch_one(repository.pool())
            .await
            .unwrap();
        let documents: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM recipe_source_documents")
            .fetch_one(repository.pool())
            .await
            .unwrap();

        assert_eq!((images, documents), (0, 0));
    }

    #[tokio::test]
    async fn finds_only_private_recipes_whose_author_no_longer_exists() {
        let repository = repository().await;

        for id in ["user-1", "user-2"] {
            sqlx::query(
                "INSERT INTO users (id, email, username, password_hash, role)
                 VALUES (?, ?, NULL, 'hash', 'user')",
            )
            .bind(id)
            .bind(format!("{id}@example.com"))
            .execute(repository.pool())
            .await
            .unwrap();
        }

        for (id, author, scope) in [
            ("orphan-private", "user-3", RecipeScope::Private),
            ("orphan-viewable", "user-3", RecipeScope::Viewable),
            ("orphan-editable", "user-3", RecipeScope::Editable),
            ("owned-private", "user-1", RecipeScope::Private),
            ("widowed-private", "user-2", RecipeScope::Private),
        ] {
            let recipe = Recipe::with_scope(
                RecipeId::new(id.to_string()),
                UserId::new(author.to_string()),
                scope,
            );
            save(
                &repository,
                recipe.clone(),
                revision(
                    &format!("{id}-revision"),
                    &recipe,
                    id,
                    SystemTime::UNIX_EPOCH,
                ),
            )
            .await;
        }

        sqlx::query("DELETE FROM users WHERE id = 'user-2'")
            .execute(repository.pool())
            .await
            .unwrap();

        let ids = repository.find_orphaned_private_recipe_ids().await.unwrap();
        let ids: Vec<_> = ids.iter().map(|id| id.as_str()).collect();

        assert_eq!(ids, ["orphan-private", "widowed-private"]);
    }

    #[tokio::test]
    async fn deleting_an_unknown_recipe_is_not_found() {
        let repository = repository().await;

        let result = repository
            .delete_recipe(RecipeId::new("missing".to_string()))
            .await;

        assert!(matches!(result, Err(DeleteRecipeRecordError::NotFound)));
    }

    async fn insert_user(repository: &SqliteRecipeRepository, id: &str) {
        sqlx::query(
            "INSERT INTO users (id, email, username, password_hash, role)
             VALUES (?, ?, NULL, 'hash', 'user')",
        )
        .bind(id)
        .bind(format!("{id}@example.com"))
        .execute(repository.pool())
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn adds_lists_and_removes_favorites() {
        let repository = repository().await;
        insert_user(&repository, "user-1").await;
        let recipe = recipe("soup");
        save(
            &repository,
            recipe.clone(),
            revision("revision-1", &recipe, "Soup", SystemTime::UNIX_EPOCH),
        )
        .await;

        let created_at = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        repository
            .add_favorite(
                recipe.id().clone(),
                UserId::new("user-1".to_string()),
                created_at,
            )
            .await
            .unwrap();

        assert!(
            repository
                .is_favorite(recipe.id().clone(), UserId::new("user-1".to_string()))
                .await
                .unwrap()
        );

        repository
            .remove_favorite(recipe.id().clone(), UserId::new("user-1".to_string()))
            .await
            .unwrap();

        assert!(
            !repository
                .is_favorite(recipe.id().clone(), UserId::new("user-1".to_string()))
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn favoriting_the_same_recipe_twice_keeps_one_row() {
        let repository = repository().await;
        insert_user(&repository, "user-1").await;
        let recipe = recipe("soup");
        save(
            &repository,
            recipe.clone(),
            revision("revision-1", &recipe, "Soup", SystemTime::UNIX_EPOCH),
        )
        .await;

        for seconds in [1, 2] {
            repository
                .add_favorite(
                    recipe.id().clone(),
                    UserId::new("user-1".to_string()),
                    SystemTime::UNIX_EPOCH + Duration::from_secs(seconds),
                )
                .await
                .unwrap();
        }

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM recipe_favorites")
            .fetch_one(repository.pool())
            .await
            .unwrap();

        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn favorites_are_isolated_per_user() {
        let repository = repository().await;
        for id in ["user-1", "user-2"] {
            insert_user(&repository, id).await;
        }
        let recipe = recipe("soup");
        save(
            &repository,
            recipe.clone(),
            revision("revision-1", &recipe, "Soup", SystemTime::UNIX_EPOCH),
        )
        .await;

        repository
            .add_favorite(
                recipe.id().clone(),
                UserId::new("user-1".to_string()),
                SystemTime::UNIX_EPOCH,
            )
            .await
            .unwrap();

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM recipe_favorites WHERE user_id = ?;")
                .bind(UserId::new("user-2".to_string()).as_str())
                .fetch_one(repository.pool())
                .await
                .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn deleting_a_recipe_or_user_cascades_its_favorites() {
        let repository = repository().await;
        for id in ["user-1", "user-2"] {
            insert_user(&repository, id).await;
        }
        for id in ["soup", "pie"] {
            let recipe = recipe(id);
            save(
                &repository,
                recipe.clone(),
                revision(
                    &format!("{id}-revision"),
                    &recipe,
                    id,
                    SystemTime::UNIX_EPOCH,
                ),
            )
            .await;
        }

        for (recipe_id, user_id) in [("soup", "user-1"), ("pie", "user-2")] {
            repository
                .add_favorite(
                    RecipeId::new(recipe_id.to_string()),
                    UserId::new(user_id.to_string()),
                    SystemTime::UNIX_EPOCH,
                )
                .await
                .unwrap();
        }

        repository
            .delete_recipe(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM recipe_favorites WHERE user_id = ?;")
                .bind(UserId::new("user-1".to_string()).as_str())
                .fetch_one(repository.pool())
                .await
                .unwrap();
        assert_eq!(count, 0);

        sqlx::query("DELETE FROM users WHERE id = 'user-2'")
            .execute(repository.pool())
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM recipe_favorites")
            .fetch_one(repository.pool())
            .await
            .unwrap();

        assert_eq!(count, 0);
    }
    fn cooked_event(
        id: &str,
        recipe: &Recipe,
        user: &UserId,
        title: &str,
        cooked_on: NaiveDate,
        note: &str,
    ) -> RecipeCookedEvent {
        RecipeCookedEvent::new(
            RecipeCookedEventId::new(id.to_string()),
            Some(recipe.id().clone()),
            user.clone(),
            CookedRecipeSnapshot::new(
                RecipeRevisionId::new("revision-1".to_string()),
                RecipeTitle::new(title.to_string()),
            ),
            cooked_on,
            CookedRecipeNote::try_new(note).unwrap(),
            SystemTime::UNIX_EPOCH,
        )
    }

    async fn save_recipe(repository: &SqliteRecipeRepository, recipe: &Recipe, title: &str) {
        save(
            repository,
            recipe.clone(),
            revision("revision-1", recipe, title, SystemTime::UNIX_EPOCH),
        )
        .await;
    }

    async fn cooked_rows(repository: &SqliteRecipeRepository) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM recipe_cooked_events")
            .fetch_one(repository.pool())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn saves_lists_updates_and_deletes_cooked_events() {
        let repository = repository().await;
        insert_user(&repository, "user-1").await;
        let recipe = recipe("soup");
        save_recipe(&repository, &recipe, "Soup").await;
        let user = UserId::new("user-1".to_string());
        let first = cooked_event(
            "event-1",
            &recipe,
            &user,
            "Soup",
            NaiveDate::from_ymd_opt(2026, 11, 18).unwrap(),
            "Good",
        );
        let second = cooked_event(
            "event-2",
            &recipe,
            &user,
            "Soup",
            NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
            "",
        );

        repository.add_cooked_event(first.clone()).await.unwrap();
        repository.add_cooked_event(second).await.unwrap();

        let events = repository
            .list_recipe_cooked_events(recipe.id().clone(), user.clone())
            .await
            .unwrap();
        let ids: Vec<_> = events.iter().map(|event| event.id().as_str()).collect();
        assert_eq!(ids, ["event-2", "event-1"]);
        assert_eq!(events[1].note().as_str(), "Good");
        assert_eq!(events[1].snapshot().revision_id().as_str(), "revision-1");
        assert_eq!(events[1].snapshot().title().as_str(), "Soup");

        repository
            .update_cooked_event(
                first.id().clone(),
                user.clone(),
                NaiveDate::from_ymd_opt(2026, 11, 19).unwrap(),
                CookedRecipeNote::try_new("Better").unwrap(),
            )
            .await
            .unwrap();

        let events = repository
            .list_recipe_cooked_events(recipe.id().clone(), user.clone())
            .await
            .unwrap();
        assert_eq!(
            events[1].cooked_on(),
            NaiveDate::from_ymd_opt(2026, 11, 19).unwrap()
        );
        assert_eq!(events[1].note().as_str(), "Better");

        repository
            .delete_cooked_event(first.id().clone(), user.clone())
            .await
            .unwrap();
        assert_eq!(
            repository
                .list_recipe_cooked_events(recipe.id().clone(), user.clone())
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(matches!(
            repository
                .delete_cooked_event(first.id().clone(), user)
                .await,
            Err(DeleteCookedEventError::NotFound)
        ));
    }

    #[tokio::test]
    async fn cooked_events_are_isolated_per_user() {
        let repository = repository().await;
        insert_user(&repository, "user-1").await;
        insert_user(&repository, "user-2").await;
        let recipe = recipe("soup");
        save_recipe(&repository, &recipe, "Soup").await;

        repository
            .add_cooked_event(cooked_event(
                "event-1",
                &recipe,
                &UserId::new("user-1".to_string()),
                "Soup",
                NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
                "",
            ))
            .await
            .unwrap();

        let other = repository
            .list_recipe_cooked_events(recipe.id().clone(), UserId::new("user-2".to_string()))
            .await
            .unwrap();
        assert!(other.is_empty());
        assert!(matches!(
            repository
                .update_cooked_event(
                    RecipeCookedEventId::new("event-1".to_string()),
                    UserId::new("user-2".to_string()),
                    NaiveDate::from_ymd_opt(2026, 11, 21).unwrap(),
                    CookedRecipeNote::try_new("").unwrap(),
                )
                .await,
            Err(UpdateCookedEventError::NotFound)
        ));
    }

    #[tokio::test]
    async fn deleting_a_recipe_keeps_cooked_events_with_their_snapshot() {
        let repository = repository().await;
        insert_user(&repository, "user-1").await;
        let recipe = recipe("soup");
        save_recipe(&repository, &recipe, "Soup").await;
        let user = UserId::new("user-1".to_string());

        repository
            .add_cooked_event(cooked_event(
                "event-1",
                &recipe,
                &user,
                "Soup",
                NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
                "Cozy",
            ))
            .await
            .unwrap();

        repository.delete_recipe(recipe.id().clone()).await.unwrap();

        assert_eq!(cooked_rows(&repository).await, 1);
        let recipe_id: Option<String> =
            sqlx::query_scalar("SELECT recipe_id FROM recipe_cooked_events")
                .fetch_one(repository.pool())
                .await
                .unwrap();
        assert!(recipe_id.is_none());

        let entries = repository
            .list_user_cooked_events(user.clone(), 0, 20)
            .await
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].display_title().as_str(), "Soup");
        assert!(entries[0].recipe_link().is_none());
        assert_eq!(entries[0].event().note().as_str(), "Cozy");

        repository
            .update_cooked_event(
                RecipeCookedEventId::new("event-1".to_string()),
                user.clone(),
                NaiveDate::from_ymd_opt(2026, 11, 21).unwrap(),
                CookedRecipeNote::try_new("Still good").unwrap(),
            )
            .await
            .unwrap();
        repository
            .delete_cooked_event(RecipeCookedEventId::new("event-1".to_string()), user)
            .await
            .unwrap();
        assert_eq!(cooked_rows(&repository).await, 0);
    }

    #[tokio::test]
    async fn deleting_a_user_removes_their_cooked_events() {
        let repository = repository().await;
        insert_user(&repository, "user-1").await;
        let recipe = recipe("soup");
        save_recipe(&repository, &recipe, "Soup").await;

        repository
            .add_cooked_event(cooked_event(
                "event-1",
                &recipe,
                &UserId::new("user-1".to_string()),
                "Soup",
                NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
                "",
            ))
            .await
            .unwrap();

        sqlx::query("DELETE FROM users WHERE id = 'user-1'")
            .execute(repository.pool())
            .await
            .unwrap();

        assert_eq!(cooked_rows(&repository).await, 0);
    }

    #[tokio::test]
    async fn global_log_prefers_the_current_visible_recipe_title() {
        let repository = repository().await;
        insert_user(&repository, "user-1").await;
        let recipe = recipe("soup");
        save_recipe(&repository, &recipe, "Soup").await;
        let user = UserId::new("user-1".to_string());

        repository
            .add_cooked_event(cooked_event(
                "event-1",
                &recipe,
                &user,
                "Old Name",
                NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
                "",
            ))
            .await
            .unwrap();

        save(
            &repository,
            recipe.clone(),
            revision("revision-2", &recipe, "New Name", SystemTime::UNIX_EPOCH),
        )
        .await;

        let entries = repository
            .list_user_cooked_events(user.clone(), 0, 20)
            .await
            .unwrap();
        assert_eq!(entries[0].display_title().as_str(), "New Name");
        assert_eq!(
            entries[0].recipe_link().map(|link| link.recipe_id()),
            Some(recipe.id())
        );
        assert_eq!(
            entries[0]
                .recipe_link()
                .map(|link| link.revision_id().as_str()),
            Some("revision-1")
        );

        let count = repository.count_user_cooked_events(user).await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn global_log_omits_the_link_when_the_cooked_revision_is_missing() {
        let repository = repository().await;
        insert_user(&repository, "user-1").await;
        let recipe = recipe("soup");
        save_recipe(&repository, &recipe, "Soup").await;
        let user = UserId::new("user-1".to_string());

        repository
            .add_cooked_event(cooked_event(
                "event-1",
                &recipe,
                &user,
                "Soup",
                NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
                "",
            ))
            .await
            .unwrap();

        sqlx::query(
            "UPDATE recipe_cooked_events SET recipe_revision_id = 'missing' WHERE id = 'event-1'",
        )
        .execute(repository.pool())
        .await
        .unwrap();

        let entries = repository
            .list_user_cooked_events(user, 0, 20)
            .await
            .unwrap();
        assert_eq!(entries[0].display_title().as_str(), "Soup");
        assert!(entries[0].recipe_link().is_none());
    }

    #[tokio::test]
    async fn global_log_falls_back_to_the_snapshot_for_a_hidden_recipe() {
        let repository = repository().await;
        insert_user(&repository, "user-1").await;
        insert_user(&repository, "user-2").await;
        let recipe = Recipe::with_scope(
            RecipeId::new("soup".to_string()),
            UserId::new("user-2".to_string()),
            RecipeScope::Private,
        );
        save_recipe(&repository, &recipe, "Secret Soup").await;
        let user = UserId::new("user-1".to_string());

        repository
            .add_cooked_event(cooked_event(
                "event-1",
                &recipe,
                &user,
                "Secret Soup",
                NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
                "",
            ))
            .await
            .unwrap();

        let entries = repository
            .list_user_cooked_events(user, 0, 20)
            .await
            .unwrap();
        assert_eq!(entries[0].display_title().as_str(), "Secret Soup");
        assert!(entries[0].recipe_link().is_none());
    }
}
