use anyhow::{Context, anyhow, bail};
use cookbook_domain::recipe::models::{
    Recipe, RecipeCategory, RecipeId, RecipeRevision, RecipeRevisionId, RecipeScope,
    RecipeWithContent, RevisionOrigin, RevisionVerification,
};
use cookbook_domain::recipe::ports::{
    GetRecipeError, ListRecipeRevisionsError, ListRecipesError, RecipeRepository,
    RecipeSearchCriteria, RecipeSort, SaveRecipeError, SearchRecipesError, SortDirection,
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
           rev.content_version, rev.content
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

    async fn search_recipes(
        &self,
        criteria: RecipeSearchCriteria,
    ) -> Result<Vec<RecipeWithContent>, SearchRecipesError> {
        let pattern = criteria
            .pattern
            .as_deref()
            .map(str::trim)
            .filter(|pattern| !pattern.is_empty())
            .map(str::to_lowercase);

        let mut builder = QueryBuilder::<Sqlite>::new(SELECT_CURRENT_SEARCH);

        if let Some(pattern) = &pattern {
            builder
                .push(" AND instr(rev.search_text, ")
                .push_bind(pattern.as_str())
                .push(") > 0");
        }

        if !criteria.categories.is_empty() {
            builder.push(" AND rev.category IN (");
            let mut separated = builder.separated(", ");
            for category in &criteria.categories {
                separated.push_bind(category.as_str());
            }
            separated.push_unseparated(")");
        }

        if criteria.exclude_needs_review {
            builder
                .push(" AND rev.verification <> ")
                .push_bind(RevisionVerification::NeedsReview.as_str());
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
            .map(decode_current)
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

fn search_storage(error: impl Into<anyhow::Error>) -> SearchRecipesError {
    SearchRecipesError::Storage(error.into())
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

    fn titles(recipes: &[RecipeWithContent]) -> Vec<&str> {
        recipes
            .iter()
            .map(|recipe| recipe.title().as_str())
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
            .search_recipes(RecipeSearchCriteria::default())
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
}
