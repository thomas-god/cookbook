use std::collections::HashSet;

use anyhow::{Context, anyhow};
use cookbook_domain::recipe::models::{
    RecipeId, RecipeImage, RecipeImageId, RecipeImageMetadata, RecipeSourceDocument,
    RecipeSourceDocumentId, RecipeSourceDocumentSummary,
};
use cookbook_domain::recipe::ports::{
    AddRecipeImagesError, AddSourceDocumentsError, DeleteRecipeAssetsError,
    DeleteSourceDocumentError, GetRecipeImageError, GetSourceDocumentError, ListRecipeImagesError,
    ListSourceDocumentsError, RecipeDocumentRepository,
};
use cookbook_domain::user::models::UserId;
use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::{Connection, Row};
use tracing::warn;

use crate::recipe::file_store::FileStore;
use crate::storage::time;

#[derive(Debug, Clone)]
pub struct FileRecipeDocumentRepository {
    pool: SqlitePool,
    files: FileStore,
}

impl FileRecipeDocumentRepository {
    pub fn new(pool: SqlitePool, files: FileStore) -> Self {
        Self { pool, files }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn files(&self) -> &FileStore {
        &self.files
    }

    async fn insert_source_documents(
        &self,
        documents: &[RecipeSourceDocument],
        storage_keys: &[String],
    ) -> anyhow::Result<()> {
        let mut connection = self.pool.acquire().await?;
        let mut transaction = connection.begin_with("BEGIN IMMEDIATE").await?;

        for (document, storage_key) in documents.iter().zip(storage_keys) {
            let byte_len = i64::try_from(document.summary().byte_len())
                .context("source document is too large for SQLite")?;
            let added_at_ms = time::to_millis(document.added_at())?;

            sqlx::query(
                "INSERT INTO recipe_source_documents (
                    id, recipe_id, content_type, original_name, byte_len,
                    added_by, added_at_ms, storage_key
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(document.id().as_str())
            .bind(document.recipe_id().as_str())
            .bind(document.content_type())
            .bind(document.original_name())
            .bind(byte_len)
            .bind(document.added_by().as_str())
            .bind(added_at_ms)
            .bind(storage_key)
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    async fn insert_recipe_images(
        &self,
        images: &[RecipeImage],
        storage_keys: &[String],
    ) -> anyhow::Result<()> {
        let mut connection = self.pool.acquire().await?;
        let mut transaction = connection.begin_with("BEGIN IMMEDIATE").await?;

        for (image, storage_key) in images.iter().zip(storage_keys) {
            let byte_len = i64::try_from(image.metadata().byte_len())
                .context("recipe image is too large for SQLite")?;
            let added_at_ms = time::to_millis(image.added_at())?;

            sqlx::query(
                "INSERT INTO recipe_images (
                    id, recipe_id, content_type, byte_len,
                    added_by, added_at_ms, storage_key
                 ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(image.id().as_str())
            .bind(image.recipe_id().as_str())
            .bind(image.content_type())
            .bind(byte_len)
            .bind(image.added_by().as_str())
            .bind(added_at_ms)
            .bind(storage_key)
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        Ok(())
    }
}

impl RecipeDocumentRepository for FileRecipeDocumentRepository {
    async fn add_source_documents(
        &self,
        documents: Vec<RecipeSourceDocument>,
    ) -> Result<(), AddSourceDocumentsError> {
        if documents.is_empty() {
            return Ok(());
        }

        let recipe_id = documents[0].recipe_id().clone();
        if let Some(document) = documents
            .iter()
            .find(|document| document.recipe_id() != &recipe_id)
        {
            return Err(AddSourceDocumentsError::Storage(anyhow!(
                "source documents belong to different recipes: `{}` and `{}`",
                recipe_id.as_str(),
                document.recipe_id().as_str()
            )));
        }

        if !recipe_exists(&self.pool, &recipe_id)
            .await
            .map_err(|error| AddSourceDocumentsError::Storage(error.into()))?
        {
            return Err(AddSourceDocumentsError::RecipeNotFound);
        }

        let mut storage_keys = Vec::with_capacity(documents.len());
        for document in &documents {
            match self.files.write_source_document(
                document.recipe_id(),
                document.id(),
                document.bytes(),
            ) {
                Ok(storage_key) => storage_keys.push(storage_key),
                Err(error) => {
                    remove_files(&self.files, &storage_keys);
                    return Err(AddSourceDocumentsError::Storage(error));
                }
            }
        }

        if let Err(error) = self
            .insert_source_documents(&documents, &storage_keys)
            .await
        {
            remove_files(&self.files, &storage_keys);
            return Err(AddSourceDocumentsError::Storage(error));
        }

        Ok(())
    }

    async fn get_source_documents(
        &self,
        recipe_id: RecipeId,
    ) -> Result<Vec<RecipeSourceDocumentSummary>, ListSourceDocumentsError> {
        let rows = sqlx::query(
            "SELECT id, recipe_id, content_type, original_name, byte_len,
                    added_by, added_at_ms
             FROM recipe_source_documents
             WHERE recipe_id = ?
             ORDER BY seq ASC",
        )
        .bind(recipe_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|error| ListSourceDocumentsError::Storage(error.into()))?;

        rows.iter()
            .map(decode_source_summary)
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(ListSourceDocumentsError::Storage)
    }

    async fn get_source_document(
        &self,
        recipe_id: RecipeId,
        document_id: RecipeSourceDocumentId,
    ) -> Result<Option<RecipeSourceDocument>, GetSourceDocumentError> {
        let row = sqlx::query(
            "SELECT id, recipe_id, content_type, original_name,
                    added_by, added_at_ms, storage_key
             FROM recipe_source_documents
             WHERE recipe_id = ? AND id = ?
             ORDER BY seq ASC
             LIMIT 1",
        )
        .bind(recipe_id.as_str())
        .bind(document_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| GetSourceDocumentError::Storage(error.into()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let storage_key: String = row
            .try_get("storage_key")
            .map_err(|error| GetSourceDocumentError::Storage(error.into()))?;
        let bytes = self
            .files
            .read(&storage_key)
            .map_err(GetSourceDocumentError::Storage)?;
        let document =
            decode_source_document(&row, bytes).map_err(GetSourceDocumentError::Storage)?;

        Ok(Some(document))
    }

    async fn delete_source_document(
        &self,
        recipe_id: RecipeId,
        document_id: RecipeSourceDocumentId,
    ) -> Result<(), DeleteSourceDocumentError> {
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|error| DeleteSourceDocumentError::Storage(error.into()))?;
        let mut transaction = connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|error| DeleteSourceDocumentError::Storage(error.into()))?;

        let recipe: Option<String> = sqlx::query_scalar("SELECT id FROM recipes WHERE id = ?")
            .bind(recipe_id.as_str())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|error| DeleteSourceDocumentError::Storage(error.into()))?;
        if recipe.is_none() {
            return Err(DeleteSourceDocumentError::NotFound);
        }

        let storage_key: Option<String> = sqlx::query_scalar(
            "SELECT storage_key FROM recipe_source_documents
             WHERE recipe_id = ? AND id = ?
             ORDER BY seq ASC
             LIMIT 1",
        )
        .bind(recipe_id.as_str())
        .bind(document_id.as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| DeleteSourceDocumentError::Storage(error.into()))?;
        let Some(storage_key) = storage_key else {
            return Err(DeleteSourceDocumentError::NotFound);
        };

        sqlx::query("DELETE FROM recipe_source_documents WHERE recipe_id = ? AND id = ?")
            .bind(recipe_id.as_str())
            .bind(document_id.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(|error| DeleteSourceDocumentError::Storage(error.into()))?;

        transaction
            .commit()
            .await
            .map_err(|error| DeleteSourceDocumentError::Storage(error.into()))?;

        if let Err(error) = self.files.delete(&storage_key) {
            warn!(
                recipe_id = recipe_id.as_str(),
                document_id = document_id.as_str(),
                %error,
                "deleted source document metadata but failed to remove its file"
            );
        }

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
        if let Some(image) = images.iter().find(|image| image.recipe_id() != &recipe_id) {
            return Err(AddRecipeImagesError::Storage(anyhow!(
                "images belong to different recipes: `{}` and `{}`",
                recipe_id.as_str(),
                image.recipe_id().as_str()
            )));
        }

        let mut seen = HashSet::new();
        for image in &images {
            if !seen.insert(image.id().as_str()) {
                return Err(AddRecipeImagesError::Storage(anyhow!(
                    "duplicate image id `{}`",
                    image.id().as_str()
                )));
            }
        }

        if !recipe_exists(&self.pool, &recipe_id)
            .await
            .map_err(|error| AddRecipeImagesError::Storage(error.into()))?
        {
            return Err(AddRecipeImagesError::RecipeNotFound);
        }

        let existing: Vec<String> =
            sqlx::query_scalar("SELECT id FROM recipe_images WHERE recipe_id = ?")
                .bind(recipe_id.as_str())
                .fetch_all(&self.pool)
                .await
                .map_err(|error| AddRecipeImagesError::Storage(error.into()))?;

        if let Some(image) = images
            .iter()
            .find(|image| existing.iter().any(|id| id.as_str() == image.id().as_str()))
        {
            return Err(AddRecipeImagesError::Storage(anyhow!(
                "a recipe image id already exists for recipe `{}`: `{}`",
                recipe_id.as_str(),
                image.id().as_str()
            )));
        }

        let mut storage_keys = Vec::with_capacity(images.len());
        for image in &images {
            match self
                .files
                .write_recipe_image(image.recipe_id(), image.id(), image.bytes())
            {
                Ok(storage_key) => storage_keys.push(storage_key),
                Err(error) => {
                    remove_files(&self.files, &storage_keys);
                    return Err(AddRecipeImagesError::Storage(error));
                }
            }
        }

        if let Err(error) = self.insert_recipe_images(&images, &storage_keys).await {
            remove_files(&self.files, &storage_keys);
            return Err(AddRecipeImagesError::Storage(error));
        }

        Ok(())
    }

    async fn get_recipe_images(
        &self,
        recipe_id: RecipeId,
    ) -> Result<Vec<RecipeImageMetadata>, ListRecipeImagesError> {
        let rows = sqlx::query(
            "SELECT id, recipe_id, content_type, byte_len, added_by, added_at_ms
             FROM recipe_images
             WHERE recipe_id = ?
             ORDER BY seq ASC",
        )
        .bind(recipe_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|error| ListRecipeImagesError::Storage(error.into()))?;

        rows.iter()
            .map(decode_image_metadata)
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(ListRecipeImagesError::Storage)
    }

    async fn get_recipe_image(
        &self,
        recipe_id: RecipeId,
        image_id: RecipeImageId,
    ) -> Result<Option<RecipeImage>, GetRecipeImageError> {
        let row = sqlx::query(
            "SELECT id, recipe_id, content_type, added_by, added_at_ms, storage_key
             FROM recipe_images
             WHERE recipe_id = ? AND id = ?
             ORDER BY seq ASC
             LIMIT 1",
        )
        .bind(recipe_id.as_str())
        .bind(image_id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| GetRecipeImageError::Storage(error.into()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let storage_key: String = row
            .try_get("storage_key")
            .map_err(|error| GetRecipeImageError::Storage(error.into()))?;
        let bytes = self
            .files
            .read(&storage_key)
            .map_err(GetRecipeImageError::Storage)?;

        Ok(Some(
            decode_image(&row, bytes).map_err(GetRecipeImageError::Storage)?,
        ))
    }

    async fn delete_recipe_assets(
        &self,
        recipe_id: RecipeId,
    ) -> Result<(), DeleteRecipeAssetsError> {
        self.files
            .delete_recipe(&recipe_id)
            .map_err(DeleteRecipeAssetsError::Storage)
    }
}

async fn recipe_exists(pool: &SqlitePool, recipe_id: &RecipeId) -> Result<bool, sqlx::Error> {
    let id: Option<String> = sqlx::query_scalar("SELECT id FROM recipes WHERE id = ?")
        .bind(recipe_id.as_str())
        .fetch_optional(pool)
        .await?;

    Ok(id.is_some())
}

fn remove_files(files: &FileStore, storage_keys: &[String]) {
    for storage_key in storage_keys {
        let _ = files.delete(storage_key);
    }
}

fn decode_source_summary(row: &SqliteRow) -> anyhow::Result<RecipeSourceDocumentSummary> {
    let byte_len = usize::try_from(row.try_get::<i64, _>("byte_len")?)
        .context("source document byte length is invalid")?;
    let added_at = time::from_millis(row.try_get("added_at_ms")?)?;

    Ok(RecipeSourceDocumentSummary::new(
        RecipeSourceDocumentId::new(row.try_get("id")?),
        RecipeId::new(row.try_get("recipe_id")?),
        row.try_get("content_type")?,
        row.try_get("original_name")?,
        byte_len,
        UserId::new(row.try_get("added_by")?),
        added_at,
    ))
}

fn decode_source_document(row: &SqliteRow, bytes: Vec<u8>) -> anyhow::Result<RecipeSourceDocument> {
    let added_at = time::from_millis(row.try_get("added_at_ms")?)?;

    Ok(RecipeSourceDocument::new(
        RecipeSourceDocumentId::new(row.try_get("id")?),
        RecipeId::new(row.try_get("recipe_id")?),
        row.try_get("content_type")?,
        row.try_get("original_name")?,
        bytes,
        UserId::new(row.try_get("added_by")?),
        added_at,
    ))
}

fn decode_image_metadata(row: &SqliteRow) -> anyhow::Result<RecipeImageMetadata> {
    let byte_len = usize::try_from(row.try_get::<i64, _>("byte_len")?)
        .context("recipe image byte length is invalid")?;
    let added_at = time::from_millis(row.try_get("added_at_ms")?)?;

    Ok(RecipeImageMetadata::new(
        RecipeImageId::new(row.try_get("id")?),
        RecipeId::new(row.try_get("recipe_id")?),
        row.try_get("content_type")?,
        byte_len,
        UserId::new(row.try_get("added_by")?),
        added_at,
    ))
}

fn decode_image(row: &SqliteRow, bytes: Vec<u8>) -> anyhow::Result<RecipeImage> {
    let added_at = time::from_millis(row.try_get("added_at_ms")?)?;

    Ok(RecipeImage::new(
        RecipeImageId::new(row.try_get("id")?),
        RecipeId::new(row.try_get("recipe_id")?),
        row.try_get("content_type")?,
        bytes,
        UserId::new(row.try_get("added_by")?),
        added_at,
    ))
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use cookbook_domain::recipe::models::{
        RecipeImage, RecipeImageId, RecipeSourceDocument, RecipeSourceDocumentId,
    };
    use tempfile::{TempDir, tempdir};

    use super::*;
    use crate::storage::SqliteDatabase;

    async fn repository() -> (FileRecipeDocumentRepository, TempDir) {
        let directory = tempdir().unwrap();
        let database = SqliteDatabase::open_in_memory().await.unwrap();
        let files = FileStore::new(directory.path());
        let repository = FileRecipeDocumentRepository::new(database.pool().clone(), files);

        (repository, directory)
    }

    async fn seed_recipe(repository: &FileRecipeDocumentRepository, recipe_id: &str) {
        sqlx::query(
            "INSERT INTO recipes (id, author_id, scope)
             VALUES (?, 'user-1', 'editable')",
        )
        .bind(recipe_id)
        .execute(repository.pool())
        .await
        .unwrap();
    }

    fn source_document(recipe_id: &str, id: &str, bytes: &[u8]) -> RecipeSourceDocument {
        RecipeSourceDocument::new(
            RecipeSourceDocumentId::new(id.to_string()),
            RecipeId::new(recipe_id.to_string()),
            "image/jpeg".to_string(),
            Some(format!("{id}.jpg")),
            bytes.to_vec(),
            UserId::new("user-1".to_string()),
            SystemTime::UNIX_EPOCH,
        )
    }

    fn recipe_image(recipe_id: &str, id: &str, bytes: &[u8]) -> RecipeImage {
        RecipeImage::new(
            RecipeImageId::new(id.to_string()),
            RecipeId::new(recipe_id.to_string()),
            "image/jpeg".to_string(),
            bytes.to_vec(),
            UserId::new("user-1".to_string()),
            SystemTime::UNIX_EPOCH,
        )
    }

    #[tokio::test]
    async fn adds_and_lists_source_documents() {
        let (repository, _directory) = repository().await;
        seed_recipe(&repository, "soup").await;

        repository
            .add_source_documents(vec![
                source_document("soup", "document-1", b"front"),
                source_document("soup", "document-2", b"back"),
            ])
            .await
            .unwrap();

        let documents = repository
            .get_source_documents(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert_eq!(documents.len(), 2);
        assert_eq!(documents[0].id().as_str(), "document-1");
        assert_eq!(documents[0].content_type(), "image/jpeg");
        assert_eq!(documents[0].original_name(), Some("document-1.jpg"));
        assert_eq!(documents[0].byte_len(), 5);
        assert_eq!(documents[0].added_by().as_str(), "user-1");
        assert_eq!(documents[0].added_at(), SystemTime::UNIX_EPOCH);
        assert_eq!(documents[1].id().as_str(), "document-2");
    }

    #[tokio::test]
    async fn gets_a_source_document_by_id() {
        let (repository, _directory) = repository().await;
        seed_recipe(&repository, "soup").await;
        repository
            .add_source_documents(vec![source_document("soup", "document-1", b"front")])
            .await
            .unwrap();

        let document = repository
            .get_source_document(
                RecipeId::new("soup".to_string()),
                RecipeSourceDocumentId::new("document-1".to_string()),
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(document.bytes(), b"front");
        assert_eq!(document.original_name(), Some("document-1.jpg"));
    }

    #[tokio::test]
    async fn returns_none_for_an_unknown_source_document() {
        let (repository, _directory) = repository().await;

        let document = repository
            .get_source_document(
                RecipeId::new("soup".to_string()),
                RecipeSourceDocumentId::new("missing".to_string()),
            )
            .await
            .unwrap();

        assert!(document.is_none());
    }

    #[tokio::test]
    async fn deletes_a_source_document_and_its_file() {
        let (repository, _directory) = repository().await;
        seed_recipe(&repository, "soup").await;
        repository
            .add_source_documents(vec![source_document("soup", "document-1", b"front")])
            .await
            .unwrap();

        repository
            .delete_source_document(
                RecipeId::new("soup".to_string()),
                RecipeSourceDocumentId::new("document-1".to_string()),
            )
            .await
            .unwrap();

        assert!(
            repository
                .get_source_documents(RecipeId::new("soup".to_string()))
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            repository
                .files()
                .read("recipes/soup/documents/document-1")
                .is_err()
        );
    }

    #[tokio::test]
    async fn rejects_source_documents_for_an_unknown_recipe() {
        let (repository, _directory) = repository().await;

        let result = repository
            .add_source_documents(vec![source_document("soup", "document-1", b"front")])
            .await;

        assert!(matches!(
            result,
            Err(AddSourceDocumentsError::RecipeNotFound)
        ));
    }

    #[tokio::test]
    async fn rejects_source_documents_from_different_recipes() {
        let (repository, _directory) = repository().await;
        seed_recipe(&repository, "soup").await;
        seed_recipe(&repository, "stew").await;

        let result = repository
            .add_source_documents(vec![
                source_document("soup", "document-1", b"front"),
                source_document("stew", "document-2", b"back"),
            ])
            .await;

        assert!(matches!(result, Err(AddSourceDocumentsError::Storage(_))));
    }

    #[tokio::test]
    async fn missing_file_is_a_storage_error() {
        let (repository, _directory) = repository().await;
        seed_recipe(&repository, "soup").await;
        repository
            .add_source_documents(vec![source_document("soup", "document-1", b"front")])
            .await
            .unwrap();
        repository
            .files()
            .delete("recipes/soup/documents/document-1")
            .unwrap();

        let result = repository
            .get_source_document(
                RecipeId::new("soup".to_string()),
                RecipeSourceDocumentId::new("document-1".to_string()),
            )
            .await;

        assert!(matches!(result, Err(GetSourceDocumentError::Storage(_))));
    }

    #[tokio::test]
    async fn adds_and_lists_recipe_images() {
        let (repository, _directory) = repository().await;
        seed_recipe(&repository, "soup").await;

        repository
            .add_recipe_images(vec![
                recipe_image("soup", "image-1", b"front"),
                recipe_image("soup", "image-2", b"back"),
            ])
            .await
            .unwrap();

        let images = repository
            .get_recipe_images(RecipeId::new("soup".to_string()))
            .await
            .unwrap();
        let image = repository
            .get_recipe_image(
                RecipeId::new("soup".to_string()),
                RecipeImageId::new("image-2".to_string()),
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(images.len(), 2);
        assert_eq!(images[0].id().as_str(), "image-1");
        assert_eq!(images[0].byte_len(), 5);
        assert_eq!(image.bytes(), b"back");
    }

    #[tokio::test]
    async fn rejects_duplicate_image_ids_in_one_batch() {
        let (repository, _directory) = repository().await;
        seed_recipe(&repository, "soup").await;

        let result = repository
            .add_recipe_images(vec![
                recipe_image("soup", "image-1", b"front"),
                recipe_image("soup", "image-1", b"back"),
            ])
            .await;

        assert!(matches!(result, Err(AddRecipeImagesError::Storage(_))));
    }

    #[tokio::test]
    async fn rejects_an_image_id_that_already_exists_for_the_recipe() {
        let (repository, _directory) = repository().await;
        seed_recipe(&repository, "soup").await;
        repository
            .add_recipe_images(vec![recipe_image("soup", "image-1", b"front")])
            .await
            .unwrap();

        let result = repository
            .add_recipe_images(vec![recipe_image("soup", "image-1", b"back")])
            .await;

        assert!(matches!(result, Err(AddRecipeImagesError::Storage(_))));
    }

    #[tokio::test]
    async fn rejects_recipe_images_for_an_unknown_recipe() {
        let (repository, _directory) = repository().await;

        let result = repository
            .add_recipe_images(vec![recipe_image("soup", "image-1", b"front")])
            .await;

        assert!(matches!(result, Err(AddRecipeImagesError::RecipeNotFound)));
    }

    #[tokio::test]
    async fn empty_batches_are_no_ops() {
        let (repository, _directory) = repository().await;

        repository.add_source_documents(Vec::new()).await.unwrap();
        repository.add_recipe_images(Vec::new()).await.unwrap();
    }

    #[tokio::test]
    async fn deleting_recipe_assets_removes_the_recipe_directory() {
        let (repository, directory) = repository().await;
        seed_recipe(&repository, "soup").await;
        repository
            .add_source_documents(vec![source_document("soup", "document-1", b"front")])
            .await
            .unwrap();
        repository
            .add_recipe_images(vec![recipe_image("soup", "image-1", b"front")])
            .await
            .unwrap();

        repository
            .delete_recipe_assets(RecipeId::new("soup".to_string()))
            .await
            .unwrap();

        assert!(!directory.path().join("recipes/soup").exists());
    }

    #[tokio::test]
    async fn deleting_assets_for_a_recipe_without_files_is_a_no_op() {
        let (repository, _directory) = repository().await;

        repository
            .delete_recipe_assets(RecipeId::new("missing".to_string()))
            .await
            .unwrap();
    }
}
