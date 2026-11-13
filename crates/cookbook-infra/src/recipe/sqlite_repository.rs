use anyhow::{Context, anyhow, bail};
use cookbook_domain::recipe::models::{
    Recipe, RecipeCategory, RecipeId, RecipeRevision, RecipeRevisionId, RecipeScope,
    RecipeWithContent, RevisionOrigin, RevisionVerification,
};
use cookbook_domain::recipe::ports::{
    GetRecipeError, ListRecipeRevisionsError, ListRecipesError, RecipeRepository,
    RecipeSearchCriteria, SaveRecipeError, SearchRecipesError,
};
use cookbook_domain::user::models::UserId;
use sqlx::Connection;
use sqlx::Row;
use sqlx::sqlite::{SqlitePool, SqliteRow};

use crate::storage::content::{decode_content, encode_content};
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
        let search_text = "";

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

    async fn search_recipes(
        &self,
        _criteria: RecipeSearchCriteria,
    ) -> Result<Vec<RecipeWithContent>, SearchRecipesError> {
        Err(SearchRecipesError::Storage(anyhow!(
            "recipe search is not implemented yet"
        )))
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use cookbook_domain::recipe::models::{
        RecipeBody, RecipeContent, RecipeDurations, RecipeTitle, RevisionOrigin,
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
}
