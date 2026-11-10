use std::collections::HashMap;
use std::fmt;
use std::time::SystemTime;

use derive_more::Constructor;

use crate::user::models::UserId;

/// A recipe's stable identity. Its content lives in revisions.
#[derive(Debug, Clone, PartialEq, Eq, Constructor)]
pub struct Recipe {
    id: RecipeId,
    author: UserId,
}

impl Recipe {
    pub fn id(&self) -> &RecipeId {
        &self.id
    }

    pub fn author(&self) -> &UserId {
        &self.author
    }
}

/// A recipe paired with one of its revisions. Repositories return this with
/// the latest revision, so recipe content always comes from stored history.
#[derive(Debug, Clone)]
pub struct RecipeWithContent {
    recipe: Recipe,
    revision: RecipeRevision,
}

impl RecipeWithContent {
    pub fn new(recipe: Recipe, revision: RecipeRevision) -> Result<Self, RecipeRevisionMismatch> {
        if revision.recipe_id() != recipe.id() {
            return Err(RecipeRevisionMismatch);
        }

        Ok(Self { recipe, revision })
    }

    pub fn recipe(&self) -> &Recipe {
        &self.recipe
    }

    pub fn revision(&self) -> &RecipeRevision {
        &self.revision
    }

    pub fn id(&self) -> &RecipeId {
        self.recipe.id()
    }

    pub fn author(&self) -> &UserId {
        self.recipe.author()
    }

    pub fn content(&self) -> &RecipeContent {
        self.revision.content()
    }

    pub fn title(&self) -> &RecipeTitle {
        self.content().title()
    }

    pub fn description(&self) -> Option<&RecipeDescription> {
        self.content().description()
    }

    pub fn metadata(&self) -> &RecipeMetadata {
        self.content().metadata()
    }

    pub fn ingredients(&self) -> &HashMap<IngredientName, Quantity> {
        self.content().ingredients()
    }

    pub fn steps(&self) -> &[RecipeStep] {
        self.content().steps()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecipeRevisionMismatch;

impl fmt::Display for RecipeRevisionMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("revision does not belong to recipe")
    }
}

impl std::error::Error for RecipeRevisionMismatch {}

/// The editable part of a recipe. Revisions store a full copy of it.
#[derive(Debug, Clone, Constructor)]
pub struct RecipeContent {
    title: RecipeTitle,
    description: Option<RecipeDescription>,
    metadata: RecipeMetadata,
    ingredients: HashMap<IngredientName, Quantity>,
    steps: Vec<RecipeStep>,
}

impl RecipeContent {
    pub fn title(&self) -> &RecipeTitle {
        &self.title
    }

    pub fn description(&self) -> Option<&RecipeDescription> {
        self.description.as_ref()
    }

    pub fn metadata(&self) -> &RecipeMetadata {
        &self.metadata
    }

    pub fn ingredients(&self) -> &HashMap<IngredientName, Quantity> {
        &self.ingredients
    }

    pub fn steps(&self) -> &[RecipeStep] {
        &self.steps
    }
}

/// An immutable snapshot of a recipe after a create or edit operation.
#[derive(Debug, Clone, Constructor)]
pub struct RecipeRevision {
    id: RecipeRevisionId,
    recipe_id: RecipeId,
    content: RecipeContent,
    editor: UserId,
    edited_at: SystemTime,
    message: Option<RevisionMessage>,
}

impl RecipeRevision {
    pub fn id(&self) -> &RecipeRevisionId {
        &self.id
    }

    pub fn recipe_id(&self) -> &RecipeId {
        &self.recipe_id
    }

    pub fn content(&self) -> &RecipeContent {
        &self.content
    }

    pub fn editor(&self) -> &UserId {
        &self.editor
    }

    pub fn edited_at(&self) -> SystemTime {
        self.edited_at
    }

    pub fn message(&self) -> Option<&RevisionMessage> {
        self.message.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Constructor)]
pub struct RecipeId(String);

impl RecipeId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Constructor)]
pub struct RecipeRevisionId(String);

impl RecipeRevisionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Constructor)]
pub struct RecipeTitle(String);

impl RecipeTitle {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Constructor)]
pub struct RecipeDescription(String);

impl RecipeDescription {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Constructor)]
pub struct RecipeMetadata {
    preparation: Option<DurationMinutes>,
    rest: Option<DurationMinutes>,
    cooking: Option<DurationMinutes>,
}

impl RecipeMetadata {
    pub fn preparation(&self) -> Option<&DurationMinutes> {
        self.preparation.as_ref()
    }

    pub fn rest(&self) -> Option<&DurationMinutes> {
        self.rest.as_ref()
    }

    pub fn cooking(&self) -> Option<&DurationMinutes> {
        self.cooking.as_ref()
    }

    pub fn total_duration(&self) -> DurationMinutes {
        DurationMinutes(
            self.preparation.as_ref().map_or_default(|d| d.0)
                + self.rest.as_ref().map_or_default(|d| d.0)
                + self.cooking.as_ref().map_or_default(|d| d.0),
        )
    }
}

#[derive(Debug, Clone, Constructor, Default)]
pub struct DurationMinutes(usize);

impl DurationMinutes {
    pub fn as_minutes(&self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Constructor)]
pub struct IngredientName(String);

impl IngredientName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Quantity {
    /// A numeric amount with a unit, such as `200 g` or `2 eggs`.
    Measured { value: f32, unit: Unit },
    /// A free-form amount, such as `a pinch` or `salt to taste`.
    Text(String),
}

impl Quantity {
    pub fn from_measure(value: f32, unit: Unit) -> Self {
        Self::Measured { value, unit }
    }

    pub fn from_text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    pub fn as_measured(&self) -> Option<(f32, &Unit)> {
        match self {
            Self::Measured { value, unit } => Some((*value, unit)),
            Self::Text(_) => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Measured { .. } => None,
            Self::Text(value) => Some(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Unit {
    Litre,
    MilliLitre,
    CentiLitre,
    Gram,
    Kilogram,
    Piece,
    Teaspoon,
    Tablespoon,
    /// A unit supplied by the user, such as `sachet` or `bunch`.
    Other(String),
}

/// A source image as supplied by the application before it is stored.
///
/// The domain treats the payload as opaque bytes. The application layer
/// determines and validates the content type.
#[derive(Debug, Clone, Constructor)]
pub struct NewRecipeSourceImage {
    content_type: String,
    original_name: Option<String>,
    bytes: Vec<u8>,
}

impl NewRecipeSourceImage {
    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    pub fn original_name(&self) -> Option<&str> {
        self.original_name.as_deref()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// A stored source image belonging to a recipe.
#[derive(Debug, Clone, Constructor)]
pub struct RecipeSourceImage {
    id: RecipeSourceImageId,
    recipe_id: RecipeId,
    content_type: String,
    original_name: Option<String>,
    bytes: Vec<u8>,
    added_by: UserId,
    added_at: SystemTime,
}

impl RecipeSourceImage {
    pub fn id(&self) -> &RecipeSourceImageId {
        &self.id
    }

    pub fn recipe_id(&self) -> &RecipeId {
        &self.recipe_id
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    pub fn original_name(&self) -> Option<&str> {
        self.original_name.as_deref()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn added_by(&self) -> &UserId {
        &self.added_by
    }

    pub fn added_at(&self) -> SystemTime {
        self.added_at
    }

    pub fn summary(&self) -> RecipeSourceImageSummary {
        RecipeSourceImageSummary::new(
            self.id.clone(),
            self.recipe_id.clone(),
            self.content_type.clone(),
            self.original_name.clone(),
            self.bytes.len(),
            self.added_by.clone(),
            self.added_at,
        )
    }
}

/// Source image metadata without the image bytes.
#[derive(Debug, Clone, Constructor)]
pub struct RecipeSourceImageSummary {
    id: RecipeSourceImageId,
    recipe_id: RecipeId,
    content_type: String,
    original_name: Option<String>,
    byte_len: usize,
    added_by: UserId,
    added_at: SystemTime,
}

impl RecipeSourceImageSummary {
    pub fn id(&self) -> &RecipeSourceImageId {
        &self.id
    }

    pub fn recipe_id(&self) -> &RecipeId {
        &self.recipe_id
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    pub fn original_name(&self) -> Option<&str> {
        self.original_name.as_deref()
    }

    pub fn byte_len(&self) -> usize {
        self.byte_len
    }

    pub fn added_by(&self) -> &UserId {
        &self.added_by
    }

    pub fn added_at(&self) -> SystemTime {
        self.added_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Constructor)]
pub struct RecipeSourceImageId(String);

impl RecipeSourceImageId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Constructor)]
pub struct RecipeStep {
    title: Option<String>,
    ingredients: Option<HashMap<String, f32>>,
    instruction: String,
}

impl RecipeStep {
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn ingredients(&self) -> Option<&HashMap<String, f32>> {
        self.ingredients.as_ref()
    }

    pub fn instruction(&self) -> &str {
        &self.instruction
    }
}

/// An optional edit note, similar to a commit message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionMessage(String);

impl RevisionMessage {
    pub fn new(message: impl Into<String>) -> Result<Self, RevisionMessageError> {
        let message = message.into().trim().to_string();

        if message.is_empty() {
            return Err(RevisionMessageError::Empty);
        }

        Ok(Self(message))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevisionMessageError {
    Empty,
}

impl fmt::Display for RevisionMessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("an edit note cannot be empty"),
        }
    }
}

impl std::error::Error for RevisionMessageError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(
        preparation: Option<usize>,
        rest: Option<usize>,
        cooking: Option<usize>,
    ) -> RecipeMetadata {
        RecipeMetadata::new(
            preparation.map(DurationMinutes::new),
            rest.map(DurationMinutes::new),
            cooking.map(DurationMinutes::new),
        )
    }

    #[test]
    fn total_duration_is_zero_when_no_durations_are_set() {
        let metadata = metadata(None, None, None);

        assert_eq!(metadata.total_duration().0, 0);
    }

    #[test]
    fn total_duration_sums_preparation_rest_and_cooking() {
        let metadata = metadata(Some(10), Some(20), Some(30));

        assert_eq!(metadata.total_duration().0, 60);
    }

    #[test]
    fn total_duration_treats_missing_durations_as_zero() {
        assert_eq!(metadata(Some(10), None, None).total_duration().0, 10);
        assert_eq!(metadata(None, Some(20), None).total_duration().0, 20);
        assert_eq!(metadata(None, None, Some(30)).total_duration().0, 30);
    }

    #[test]
    fn exposes_individual_metadata_durations() {
        let metadata = metadata(Some(10), Some(20), Some(30));

        assert_eq!(
            metadata.preparation().map(DurationMinutes::as_minutes),
            Some(10)
        );
        assert_eq!(metadata.rest().map(DurationMinutes::as_minutes), Some(20));
        assert_eq!(
            metadata.cooking().map(DurationMinutes::as_minutes),
            Some(30)
        );
    }

    #[test]
    fn measured_quantity_exposes_its_value_and_unit() {
        let quantity = Quantity::from_measure(200.0, Unit::Gram);

        let (value, unit) = quantity.as_measured().expect("measured quantity");

        assert_eq!(value, 200.0);
        assert!(matches!(unit, &Unit::Gram));
        assert_eq!(quantity.as_text(), None);
    }

    #[test]
    fn textual_quantity_exposes_its_text() {
        let quantity = Quantity::from_text("salt to taste");

        assert_eq!(quantity.as_text(), Some("salt to taste"));
        assert_eq!(quantity.as_measured(), None);
    }

    #[test]
    fn source_image_summary_contains_metadata_but_not_bytes() {
        let image = RecipeSourceImage::new(
            RecipeSourceImageId::new("image-1".to_string()),
            RecipeId::new("soup".to_string()),
            "image/jpeg".to_string(),
            Some("front.jpg".to_string()),
            vec![1, 2, 3, 4],
            UserId::new("user-1".to_string()),
            SystemTime::UNIX_EPOCH,
        );

        let summary = image.summary();

        assert_eq!(summary.id().as_str(), "image-1");
        assert_eq!(summary.recipe_id().as_str(), "soup");
        assert_eq!(summary.content_type(), "image/jpeg");
        assert_eq!(summary.original_name(), Some("front.jpg"));
        assert_eq!(summary.byte_len(), 4);
        assert_eq!(summary.added_by().as_str(), "user-1");
        assert_eq!(summary.added_at(), SystemTime::UNIX_EPOCH);
    }

    #[test]
    fn revision_message_is_trimmed() {
        let message = RevisionMessage::new("  reduced the sugar  ").unwrap();

        assert_eq!(message.as_str(), "reduced the sugar");
    }

    #[test]
    fn revision_message_rejects_blank_values() {
        assert_eq!(
            RevisionMessage::new("   "),
            Err(RevisionMessageError::Empty)
        );
    }
}
