use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, bail};
use cookbook_domain::recipe::models::{RecipeId, RecipeImageId, RecipeSourceDocumentId};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
enum AssetKind {
    Image,
    Document,
}

impl AssetKind {
    const fn directory(self) -> &'static str {
        match self {
            Self::Image => "images",
            Self::Document => "documents",
        }
    }
}

/// Stores image and source-document bytes under one directory per recipe.
///
/// The file layout is:
///
/// ```text
/// <root>/recipes/<recipe_id>/images/<image_id>
/// <root>/recipes/<recipe_id>/documents/<document_id>
/// ```
///
/// Metadata lives in SQLite; this type only owns the bytes.
#[derive(Debug, Clone)]
pub struct FileStore {
    root: PathBuf,
}

impl FileStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Writes an image and returns its storage key relative to the root.
    pub fn write_recipe_image(
        &self,
        recipe_id: &RecipeId,
        image_id: &RecipeImageId,
        bytes: &[u8],
    ) -> anyhow::Result<String> {
        self.write_asset(recipe_id, AssetKind::Image, image_id.as_str(), bytes)
    }

    /// Writes a source document and returns its storage key relative to the root.
    pub fn write_source_document(
        &self,
        recipe_id: &RecipeId,
        document_id: &RecipeSourceDocumentId,
        bytes: &[u8],
    ) -> anyhow::Result<String> {
        self.write_asset(recipe_id, AssetKind::Document, document_id.as_str(), bytes)
    }

    pub fn read(&self, storage_key: &str) -> anyhow::Result<Vec<u8>> {
        let path = self.resolve(storage_key)?;

        fs::read(&path).with_context(|| format!("failed to read stored asset {}", path.display()))
    }

    /// Deletes an asset. A missing file is treated as already deleted.
    pub fn delete(&self, storage_key: &str) -> anyhow::Result<()> {
        let path = self.resolve(storage_key)?;

        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error)
                .with_context(|| format!("failed to delete stored asset {}", path.display())),
        }
    }

    /// Removes every asset belonging to a recipe. A missing directory is fine.
    pub fn delete_recipe(&self, recipe_id: &RecipeId) -> anyhow::Result<()> {
        let id = recipe_id.as_str();
        validate_component(id)?;

        let path = self.root.join("recipes").join(id);

        match fs::remove_dir_all(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error)
                .with_context(|| format!("failed to delete recipe assets {}", path.display())),
        }
    }

    fn write_asset(
        &self,
        recipe_id: &RecipeId,
        kind: AssetKind,
        asset_id: &str,
        bytes: &[u8],
    ) -> anyhow::Result<String> {
        validate_component(recipe_id.as_str())?;
        validate_component(asset_id)?;

        let storage_key = format!(
            "recipes/{}/{}/{}",
            recipe_id.as_str(),
            kind.directory(),
            asset_id
        );
        let path = self.resolve(&storage_key)?;

        write_atomic(&path, bytes)?;

        Ok(storage_key)
    }

    fn resolve(&self, storage_key: &str) -> anyhow::Result<PathBuf> {
        let relative = Path::new(storage_key);

        if relative.is_absolute() {
            bail!("asset storage key must be relative: `{storage_key}`");
        }

        let mut components = relative.components();

        if components.next().is_none() {
            bail!("asset storage key is empty");
        }

        for component in relative.components() {
            match component {
                Component::Normal(value) => {
                    let value = value
                        .to_str()
                        .with_context(|| format!("asset path is not valid UTF-8: {value:?}"))?;
                    validate_component(value)?;
                }
                _ => bail!("asset storage key contains an unsafe path component: `{storage_key}`"),
            }
        }

        Ok(self.root.join(relative))
    }
}

fn validate_component(value: &str) -> anyhow::Result<()> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        bail!("invalid asset path component `{value}`");
    }

    Ok(())
}

/// Writes through a unique temporary file in the destination directory, syncs
/// it, and renames it into place so readers never observe a partial file. The
/// temporary file is removed when any step fails.
fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("asset path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create asset directory {}", parent.display()))?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("asset path has no file name: {}", path.display()))?;
    let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_path = path.with_file_name(format!(".{file_name}.{sequence}.tmp"));

    let result = (|| -> anyhow::Result<()> {
        let mut file = fs::File::create(&temp_path)
            .with_context(|| format!("failed to create {}", temp_path.display()))?;
        file.write_all(bytes)
            .with_context(|| format!("failed to write {}", temp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to sync {}", temp_path.display()))?;
        drop(file);

        fs::rename(&temp_path, path)
            .with_context(|| format!("failed to move asset into place at {}", path.display()))?;

        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    result
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    fn recipe_id() -> RecipeId {
        RecipeId::new("recipe-1".to_string())
    }

    #[test]
    fn writes_reads_and_deletes_recipe_images() {
        let directory = tempdir().unwrap();
        let store = FileStore::new(directory.path());
        let image_id = RecipeImageId::new("image-1".to_string());

        let key = store
            .write_recipe_image(&recipe_id(), &image_id, b"front")
            .unwrap();

        assert_eq!(key, "recipes/recipe-1/images/image-1");
        assert_eq!(store.read(&key).unwrap(), b"front");

        store.delete(&key).unwrap();
        assert!(store.read(&key).is_err());
    }

    #[test]
    fn writes_reads_and_deletes_source_documents() {
        let directory = tempdir().unwrap();
        let store = FileStore::new(directory.path());
        let document_id = RecipeSourceDocumentId::new("document-1".to_string());

        let key = store
            .write_source_document(&recipe_id(), &document_id, b"original")
            .unwrap();

        assert_eq!(key, "recipes/recipe-1/documents/document-1");
        assert_eq!(store.read(&key).unwrap(), b"original");

        store.delete(&key).unwrap();
        assert!(store.read(&key).is_err());
    }

    #[test]
    fn deleting_a_missing_asset_is_a_no_op() {
        let directory = tempdir().unwrap();
        let store = FileStore::new(directory.path());

        store.delete("recipes/recipe-1/images/missing").unwrap();
    }

    #[test]
    fn deleting_a_recipe_removes_all_of_its_assets() {
        let directory = tempdir().unwrap();
        let store = FileStore::new(directory.path());
        let image_id = RecipeImageId::new("image-1".to_string());
        let document_id = RecipeSourceDocumentId::new("document-1".to_string());
        let image_key = store
            .write_recipe_image(&recipe_id(), &image_id, b"front")
            .unwrap();
        let document_key = store
            .write_source_document(&recipe_id(), &document_id, b"original")
            .unwrap();

        store.delete_recipe(&recipe_id()).unwrap();

        assert!(store.read(&image_key).is_err());
        assert!(store.read(&document_key).is_err());
    }

    #[test]
    fn rejects_unsafe_recipe_and_asset_ids() {
        let directory = tempdir().unwrap();
        let store = FileStore::new(directory.path());

        assert!(
            store
                .write_recipe_image(
                    &RecipeId::new("../recipe".to_string()),
                    &RecipeImageId::new("image-1".to_string()),
                    b"front",
                )
                .is_err()
        );
        assert!(
            store
                .write_recipe_image(
                    &recipe_id(),
                    &RecipeImageId::new("../image".to_string()),
                    b"front",
                )
                .is_err()
        );
    }

    #[test]
    fn rejects_unsafe_storage_keys() {
        let directory = tempdir().unwrap();
        let store = FileStore::new(directory.path());

        assert!(store.read("../etc/passwd").is_err());
        assert!(store.read("/etc/passwd").is_err());
        assert!(store.read("recipes/../etc/passwd").is_err());
    }
}
