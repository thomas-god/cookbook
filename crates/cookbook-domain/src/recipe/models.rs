use std::collections::HashSet;
use std::fmt;
use std::time::SystemTime;

use derive_more::Constructor;
use uuid::Uuid;

use crate::user::models::UserId;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RecipeScope {
    Private,
    Viewable,
    #[default]
    Editable,
}

impl RecipeScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Viewable => "viewable",
            Self::Editable => "editable",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "private" => Some(Self::Private),
            "viewable" => Some(Self::Viewable),
            "editable" => Some(Self::Editable),
            _ => None,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecipeCategory {
    Aperitif,
    Starter,
    Main,
    Side,
    Dessert,
    Breakfast,
    Snack,
    Drink,
    Other,
}

impl RecipeCategory {
    pub const ALL: [Self; 9] = [
        Self::Aperitif,
        Self::Starter,
        Self::Main,
        Self::Side,
        Self::Dessert,
        Self::Breakfast,
        Self::Snack,
        Self::Drink,
        Self::Other,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Aperitif => "aperitif",
            Self::Starter => "starter",
            Self::Main => "main",
            Self::Side => "side",
            Self::Dessert => "dessert",
            Self::Breakfast => "breakfast",
            Self::Snack => "snack",
            Self::Drink => "drink",
            Self::Other => "other",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "aperitif" => Some(Self::Aperitif),
            "starter" => Some(Self::Starter),
            "main" => Some(Self::Main),
            "side" => Some(Self::Side),
            "dessert" => Some(Self::Dessert),
            "breakfast" => Some(Self::Breakfast),
            "snack" => Some(Self::Snack),
            "drink" => Some(Self::Drink),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}

/// A recipe's stable identity. Its content lives in revisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    id: RecipeId,
    author: UserId,
    scope: RecipeScope,
}

impl Recipe {
    pub fn new(id: RecipeId, author: UserId) -> Self {
        Self::with_scope(id, author, RecipeScope::default())
    }

    pub fn with_scope(id: RecipeId, author: UserId, scope: RecipeScope) -> Self {
        Self { id, author, scope }
    }

    pub fn id(&self) -> &RecipeId {
        &self.id
    }

    pub fn author(&self) -> &UserId {
        &self.author
    }

    pub fn scope(&self) -> RecipeScope {
        self.scope
    }

    pub fn can_view(&self, user: &UserId) -> bool {
        self.scope != RecipeScope::Private || &self.author == user
    }

    pub fn can_edit(&self, user: &UserId) -> bool {
        self.scope == RecipeScope::Editable || &self.author == user
    }

    pub fn update_scope(self, scope: RecipeScope) -> Self {
        Self { scope, ..self }
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

    pub fn scope(&self) -> RecipeScope {
        self.recipe.scope()
    }

    pub fn can_view(&self, user: &UserId) -> bool {
        self.recipe.can_view(user)
    }

    pub fn can_edit(&self, user: &UserId) -> bool {
        self.recipe.can_edit(user)
    }

    pub fn content(&self) -> &RecipeContent {
        self.revision.content()
    }

    pub fn verification(&self) -> RevisionVerification {
        self.revision.verification()
    }

    pub fn needs_review(&self) -> bool {
        self.verification() == RevisionVerification::NeedsReview
    }

    pub fn title(&self) -> &RecipeTitle {
        self.content().title()
    }

    pub fn description(&self) -> Option<&RecipeDescription> {
        self.content().description()
    }

    pub fn servings(&self) -> Option<&Servings> {
        self.content().servings()
    }

    pub fn category(&self) -> Option<RecipeCategory> {
        self.content().category()
    }

    pub fn durations(&self) -> RecipeDurations {
        self.content().durations()
    }

    pub fn structure(&self) -> &RecipeStructure {
        self.content().structure()
    }

    pub fn simple_body(&self) -> Option<&RecipeBody> {
        self.content().simple_body()
    }

    pub fn parts(&self) -> &[RecipePart] {
        self.content().parts()
    }

    pub fn bodies(&self) -> impl Iterator<Item = &RecipeBody> {
        self.content().bodies()
    }

    pub fn all_image_ids(&self) -> Vec<RecipeImageId> {
        self.content().all_image_ids()
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

/// The edible content shared by a standard recipe and a multipart sub-recipe.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct RecipeBody {
    durations: RecipeDurations,
    ingredients: Vec<Ingredient>,
    images: Vec<RecipeImageId>,
    steps: Vec<RecipeStep>,
}

impl RecipeBody {
    pub fn new(
        durations: RecipeDurations,
        ingredients: Vec<Ingredient>,
        images: Vec<RecipeImageId>,
        steps: Vec<RecipeStep>,
    ) -> Self {
        Self {
            durations,
            ingredients: normalize_ingredients(ingredients),
            images,
            steps,
        }
    }

    pub fn durations(&self) -> &RecipeDurations {
        &self.durations
    }

    pub fn ingredients(&self) -> &[Ingredient] {
        &self.ingredients
    }

    pub fn images(&self) -> &[RecipeImageId] {
        &self.images
    }

    pub fn steps(&self) -> &[RecipeStep] {
        &self.steps
    }

    /// Returns the names of ingredients that still have more than one quantity.
    pub fn ingredient_conflicts(&self) -> Vec<&IngredientName> {
        let mut conflicts = Vec::new();

        for (index, ingredient) in self.ingredients.iter().enumerate() {
            let already_listed = conflicts
                .iter()
                .any(|name: &&IngredientName| **name == ingredient.name);

            if !already_listed
                && self.ingredients[index + 1..]
                    .iter()
                    .any(|other| other.name == ingredient.name)
            {
                conflicts.push(&ingredient.name);
            }
        }

        conflicts
    }

    pub fn has_ingredient_conflicts(&self) -> bool {
        !self.ingredient_conflicts().is_empty()
    }
}

/// Ingredients with the same name and measured unit are summed. Identical
/// free-text quantities are de-duplicated. Any other duplicate names stay
/// as separate ingredients, which makes them visible as conflicts.
fn normalize_ingredients(ingredients: Vec<Ingredient>) -> Vec<Ingredient> {
    let mut normalized: Vec<Ingredient> = Vec::with_capacity(ingredients.len());

    for ingredient in ingredients {
        let mut merged = false;

        for existing in &mut normalized {
            if existing.name != ingredient.name {
                continue;
            }

            match (&mut existing.quantity, &ingredient.quantity) {
                (
                    Quantity::Measured {
                        value: existing_value,
                        unit: existing_unit,
                    },
                    Quantity::Measured { value, unit },
                ) if existing_unit == unit => {
                    *existing_value += *value;
                    merged = true;
                    break;
                }
                (Quantity::Text(existing_text), Quantity::Text(text)) if existing_text == text => {
                    merged = true;
                    break;
                }
                _ => {}
            }
        }

        if !merged {
            normalized.push(ingredient);
        }
    }

    normalized
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Constructor, PartialEq)]
pub struct Ingredient {
    name: IngredientName,
    quantity: Quantity,
}

impl Ingredient {
    pub fn name(&self) -> &IngredientName {
        &self.name
    }

    pub fn quantity(&self) -> &Quantity {
        &self.quantity
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipeStructureKind {
    Simple,
    Multipart,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Debug, Clone)]
pub enum RecipeStructure {
    Simple(RecipeBody),
    Multipart(Vec<RecipePart>),
}

/// Titled [RecipeBody] for multipart recipes.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Constructor)]
pub struct RecipePart {
    id: RecipePartId,
    title: RecipePartTitle,
    description: Option<RecipeDescription>,
    body: RecipeBody,
}

impl RecipePart {
    pub fn id(&self) -> &RecipePartId {
        &self.id
    }

    pub fn title(&self) -> &RecipePartTitle {
        &self.title
    }

    pub fn description(&self) -> Option<&RecipeDescription> {
        self.description.as_ref()
    }

    pub fn body(&self) -> &RecipeBody {
        &self.body
    }

    pub fn durations(&self) -> &RecipeDurations {
        self.body.durations()
    }
}

/// The editable part of a recipe. Revisions store a full copy of it.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct RecipeContent {
    title: RecipeTitle,
    description: Option<RecipeDescription>,
    servings: Option<Servings>,
    category: Option<RecipeCategory>,
    structure: RecipeStructure,
}

impl RecipeContent {
    pub fn new_simple(
        title: RecipeTitle,
        description: Option<RecipeDescription>,
        servings: Option<Servings>,
        category: Option<RecipeCategory>,
        body: RecipeBody,
    ) -> Result<Self, RecipeContentError> {
        validate_recipe_title(&title)?;

        Ok(Self {
            title,
            description,
            servings,
            category,
            structure: RecipeStructure::Simple(body),
        })
    }

    pub fn new_multipart(
        title: RecipeTitle,
        description: Option<RecipeDescription>,
        servings: Option<Servings>,
        category: Option<RecipeCategory>,
        parts: Vec<RecipePart>,
    ) -> Result<Self, RecipeContentError> {
        validate_recipe_title(&title)?;

        if parts.is_empty() {
            return Err(RecipeContentError::NoParts);
        }

        for part in &parts {
            if part.title.as_str().trim().is_empty() {
                return Err(RecipeContentError::EmptyPartTitle);
            }
        }

        let mut seen_ids = HashSet::new();
        for part in &parts {
            if !seen_ids.insert(part.id.clone()) {
                return Err(RecipeContentError::DuplicatePartId(part.id.clone()));
            }
        }

        Ok(Self {
            title,
            description,
            servings,
            category,
            structure: RecipeStructure::Multipart(parts),
        })
    }

    pub fn title(&self) -> &RecipeTitle {
        &self.title
    }

    pub fn description(&self) -> Option<&RecipeDescription> {
        self.description.as_ref()
    }

    pub fn servings(&self) -> Option<&Servings> {
        self.servings.as_ref()
    }

    pub fn category(&self) -> Option<RecipeCategory> {
        self.category
    }

    pub fn structure(&self) -> &RecipeStructure {
        &self.structure
    }

    pub fn is_multipart(&self) -> bool {
        matches!(self.structure, RecipeStructure::Multipart(_))
    }

    pub fn structure_kind(&self) -> RecipeStructureKind {
        match self.structure {
            RecipeStructure::Simple(_) => RecipeStructureKind::Simple,
            RecipeStructure::Multipart(_) => RecipeStructureKind::Multipart,
        }
    }

    pub fn simple_body(&self) -> Option<&RecipeBody> {
        match &self.structure {
            RecipeStructure::Simple(body) => Some(body),
            RecipeStructure::Multipart(_) => None,
        }
    }

    pub fn parts(&self) -> &[RecipePart] {
        match &self.structure {
            RecipeStructure::Simple(_) => &[],
            RecipeStructure::Multipart(parts) => parts,
        }
    }

    pub fn bodies(&self) -> impl Iterator<Item = &RecipeBody> {
        self.simple_body()
            .into_iter()
            .chain(self.parts().iter().map(RecipePart::body))
    }

    pub fn durations(&self) -> RecipeDurations {
        match &self.structure {
            RecipeStructure::Simple(body) => body.durations().clone(),
            RecipeStructure::Multipart(parts) => {
                RecipeDurations::sum(parts.iter().map(RecipePart::durations))
            }
        }
    }

    /// All body image ids in display order, without duplicates.
    pub fn all_image_ids(&self) -> Vec<RecipeImageId> {
        let mut seen = HashSet::new();

        self.bodies()
            .flat_map(|body| body.images().iter())
            .filter(|image_id| seen.insert((*image_id).clone()))
            .cloned()
            .collect()
    }

    /// Converts a [RecipeStructure::Simple] recipe into a [RecipeStructure::Multipart] recipe.
    ///
    /// Wraps its body into a single part.
    pub fn into_multipart(self) -> Self {
        let body = match self.structure {
            RecipeStructure::Simple(body) => body,
            RecipeStructure::Multipart(_) => return self,
        };

        let part_title = RecipePartTitle::new(self.title.as_str().to_string());

        let structure = RecipeStructure::Multipart(vec![RecipePart::new(
            RecipePartId::generate(),
            part_title,
            None,
            body,
        )]);

        Self {
            title: self.title,
            description: self.description,
            servings: self.servings,
            category: self.category,
            structure,
        }
    }

    /// Merges every part into a single simple body.
    ///
    /// Ingredients are concatenated and normalized. Incompatible quantities
    /// remain as separate ingredients so the resulting content can expose them
    /// as conflicts. Part descriptions are appended to the recipe description.
    pub fn into_simple(self) -> Self {
        let parts = match self.structure {
            RecipeStructure::Simple(_) => return self,
            RecipeStructure::Multipart(parts) => parts,
        };
        let RecipeContent {
            title,
            description,
            servings,
            category,
            ..
        } = self;

        let mut durations = Vec::with_capacity(parts.len());
        let mut ingredients = Vec::new();
        let mut images = Vec::new();
        let mut steps = Vec::new();
        let mut seen_images = HashSet::new();
        let mut part_descriptions = Vec::new();

        for part in parts {
            let RecipePart {
                title,
                description,
                body:
                    RecipeBody {
                        durations: part_durations,
                        ingredients: part_ingredients,
                        images: part_images,
                        steps: part_steps,
                    },
                ..
            } = part;

            durations.push(part_durations);
            ingredients.extend(part_ingredients);
            steps.extend(part_steps);

            for image_id in part_images {
                if seen_images.insert(image_id.clone()) {
                    images.push(image_id);
                }
            }

            if let Some(description) = description {
                part_descriptions.push(format!("{}: {}", title.as_str(), description.as_str()));
            }
        }

        let description = merge_descriptions(description, part_descriptions);

        Self {
            title,
            description,
            servings,
            category,
            structure: RecipeStructure::Simple(RecipeBody::new(
                RecipeDurations::sum(durations.iter()),
                ingredients,
                images,
                steps,
            )),
        }
    }
}

fn validate_recipe_title(title: &RecipeTitle) -> Result<(), RecipeContentError> {
    if title.as_str().trim().is_empty() {
        return Err(RecipeContentError::EmptyTitle);
    }

    Ok(())
}

fn merge_descriptions(
    description: Option<RecipeDescription>,
    part_descriptions: Vec<String>,
) -> Option<RecipeDescription> {
    let mut blocks = Vec::new();

    if let Some(description) = description {
        blocks.push(description.as_str().to_string());
    }

    blocks.extend(part_descriptions);

    (!blocks.is_empty()).then(|| RecipeDescription::new(blocks.join("\n\n")))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipeContentError {
    EmptyTitle,
    NoParts,
    EmptyPartTitle,
    DuplicatePartId(RecipePartId),
}

impl fmt::Display for RecipeContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTitle => f.write_str("a recipe title cannot be empty"),
            Self::NoParts => f.write_str("a multipart recipe needs at least one part"),
            Self::EmptyPartTitle => f.write_str("a recipe part needs a title"),
            Self::DuplicatePartId(id) => {
                write!(f, "recipe part id `{}` is used more than once", id.as_str())
            }
        }
    }
}

impl std::error::Error for RecipeContentError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionVerification {
    Verified,
    NeedsReview,
}

impl RevisionVerification {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::NeedsReview => "needs_review",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "verified" => Some(Self::Verified),
            "needs_review" => Some(Self::NeedsReview),
            _ => None,
        }
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
    origin: RevisionOrigin,
    verification: RevisionVerification,
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

    pub fn verification(&self) -> RevisionVerification {
        self.verification
    }

    pub fn editor(&self) -> &UserId {
        &self.editor
    }

    pub fn edited_at(&self) -> SystemTime {
        self.edited_at
    }

    pub fn origin(&self) -> &RevisionOrigin {
        &self.origin
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Constructor)]
pub struct RecipeId(String);

impl RecipeId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Constructor)]
pub struct RecipeRevisionId(String);

impl RecipeRevisionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Constructor)]
pub struct RecipeTitle(String);

impl RecipeTitle {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Constructor)]
pub struct RecipePartTitle(String);

impl RecipePartTitle {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Constructor)]
pub struct RecipePartId(String);

impl RecipePartId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Constructor)]
pub struct RecipeDescription(String);

impl RecipeDescription {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// How much a recipe yields.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Servings {
    /// A number of people, such as `4 people`.
    People(usize),
    /// A number of portions with an optional free-form label, such as `12 cookies`.
    Portions { count: usize, label: Option<String> },
}

impl Servings {
    pub fn people(count: usize) -> Self {
        Self::People(count)
    }

    pub fn portions(count: usize) -> Self {
        Self::Portions { count, label: None }
    }

    pub fn portions_with_label(count: usize, label: impl Into<String>) -> Self {
        Self::Portions {
            count,
            label: Some(label.into()),
        }
    }

    pub fn as_people(&self) -> Option<usize> {
        match self {
            Self::People(count) => Some(*count),
            Self::Portions { .. } => None,
        }
    }

    pub fn as_portions(&self) -> Option<(usize, Option<&str>)> {
        match self {
            Self::People(_) => None,
            Self::Portions { count, label } => Some((*count, label.as_deref())),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Constructor)]
pub struct RecipeDurations {
    preparation: Option<DurationMinutes>,
    rest: Option<DurationMinutes>,
    cooking: Option<DurationMinutes>,
}

impl RecipeDurations {
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

    pub fn sum<'a>(durations: impl IntoIterator<Item = &'a RecipeDurations>) -> Self {
        let mut preparation: Option<usize> = None;
        let mut rest: Option<usize> = None;
        let mut cooking: Option<usize> = None;

        for durations in durations {
            if let Some(value) = durations.preparation() {
                preparation = Some(preparation.unwrap_or_default() + value.as_minutes());
            }
            if let Some(value) = durations.rest() {
                rest = Some(rest.unwrap_or_default() + value.as_minutes());
            }
            if let Some(value) = durations.cooking() {
                cooking = Some(cooking.unwrap_or_default() + value.as_minutes());
            }
        }

        Self {
            preparation: preparation.map(DurationMinutes::new),
            rest: rest.map(DurationMinutes::new),
            cooking: cooking.map(DurationMinutes::new),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Constructor, Default)]
pub struct DurationMinutes(usize);

impl DurationMinutes {
    pub fn as_minutes(&self) -> usize {
        self.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Constructor)]
pub struct IngredientName(String);

impl IngredientName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
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

/// A source document as supplied by the application before it is stored.
///
/// The domain treats the payload as opaque bytes. The application layer
/// determines and validates the content type.
#[derive(Debug, Clone, Constructor)]
pub struct NewRecipeSourceDocument {
    content_type: String,
    original_name: Option<String>,
    bytes: Vec<u8>,
}

impl NewRecipeSourceDocument {
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

/// A stored source document belonging to a recipe.
#[derive(Debug, Clone, Constructor)]
pub struct RecipeSourceDocument {
    id: RecipeSourceDocumentId,
    recipe_id: RecipeId,
    content_type: String,
    original_name: Option<String>,
    bytes: Vec<u8>,
    added_by: UserId,
    added_at: SystemTime,
}

impl RecipeSourceDocument {
    pub fn id(&self) -> &RecipeSourceDocumentId {
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

    pub fn summary(&self) -> RecipeSourceDocumentSummary {
        RecipeSourceDocumentSummary::new(
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

/// Source document metadata without the document bytes.
#[derive(Debug, Clone, Constructor)]
pub struct RecipeSourceDocumentSummary {
    id: RecipeSourceDocumentId,
    recipe_id: RecipeId,
    content_type: String,
    original_name: Option<String>,
    byte_len: usize,
    added_by: UserId,
    added_at: SystemTime,
}

impl RecipeSourceDocumentSummary {
    pub fn id(&self) -> &RecipeSourceDocumentId {
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
pub struct RecipeSourceDocumentId(String);

impl RecipeSourceDocumentId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A recipe presentation image. Opaque from the domain point of view.
#[derive(Debug, Clone, Constructor)]
pub struct NewRecipeImage {
    id: RecipeImageId,
    content_type: String,
    bytes: Vec<u8>,
}

impl NewRecipeImage {
    pub fn id(&self) -> &RecipeImageId {
        &self.id
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// A stored presentation image belonging to a recipe. Presentation images are
/// handled separately from recipe revisions. Revisions reference their ids.
#[derive(Debug, Clone)]
pub struct RecipeImage {
    metadata: RecipeImageMetadata,
    bytes: Vec<u8>,
}

impl RecipeImage {
    pub fn new(
        id: RecipeImageId,
        recipe_id: RecipeId,
        content_type: String,
        bytes: Vec<u8>,
        added_by: UserId,
        added_at: SystemTime,
    ) -> Self {
        let metadata =
            RecipeImageMetadata::new(id, recipe_id, content_type, bytes.len(), added_by, added_at);

        Self { metadata, bytes }
    }

    pub fn metadata(&self) -> &RecipeImageMetadata {
        &self.metadata
    }

    pub fn id(&self) -> &RecipeImageId {
        self.metadata.id()
    }

    pub fn recipe_id(&self) -> &RecipeId {
        self.metadata.recipe_id()
    }

    pub fn content_type(&self) -> &str {
        self.metadata.content_type()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn added_by(&self) -> &UserId {
        self.metadata.added_by()
    }

    pub fn added_at(&self) -> SystemTime {
        self.metadata.added_at()
    }
}

#[derive(Debug, Clone, Constructor)]
pub struct RecipeImageMetadata {
    id: RecipeImageId,
    recipe_id: RecipeId,
    content_type: String,
    byte_len: usize,
    added_by: UserId,
    added_at: SystemTime,
}

impl RecipeImageMetadata {
    pub fn id(&self) -> &RecipeImageId {
        &self.id
    }

    pub fn recipe_id(&self) -> &RecipeId {
        &self.recipe_id
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Constructor)]
pub struct RecipeImageId(String);

impl RecipeImageId {
    pub fn generate() -> Self {
        Self(Uuid::now_v7().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Constructor)]
pub struct RecipeStepId(String);

impl RecipeStepId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Constructor)]
pub struct RecipeStep {
    id: RecipeStepId,
    title: Option<String>,
    instruction: String,
    images: Vec<RecipeImageId>,
}

impl RecipeStep {
    pub fn id(&self) -> &RecipeStepId {
        &self.id
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn instruction(&self) -> &str {
        &self.instruction
    }

    pub fn images(&self) -> &[RecipeImageId] {
        &self.images
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevisionOrigin {
    Created,
    Imported,
    UserEdit { message: Option<RevisionMessage> },
    StructureConverted { to: RecipeStructureKind },
    Restored { revision_id: RecipeRevisionId },
}

/// An optional edit note, similar to a commit message.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

    fn durations(
        preparation: Option<usize>,
        rest: Option<usize>,
        cooking: Option<usize>,
    ) -> RecipeDurations {
        RecipeDurations::new(
            preparation.map(DurationMinutes::new),
            rest.map(DurationMinutes::new),
            cooking.map(DurationMinutes::new),
        )
    }

    #[test]
    fn total_duration_is_zero_when_no_durations_are_set() {
        let durations = durations(None, None, None);

        assert_eq!(durations.total_duration().0, 0);
    }

    #[test]
    fn total_duration_sums_preparation_rest_and_cooking() {
        let durations = durations(Some(10), Some(20), Some(30));

        assert_eq!(durations.total_duration().0, 60);
    }

    #[test]
    fn total_duration_treats_missing_durations_as_zero() {
        assert_eq!(durations(Some(10), None, None).total_duration().0, 10);
        assert_eq!(durations(None, Some(20), None).total_duration().0, 20);
        assert_eq!(durations(None, None, Some(30)).total_duration().0, 30);
    }

    #[test]
    fn exposes_individual_durations() {
        let durations = durations(Some(10), Some(20), Some(30));

        assert_eq!(
            durations.preparation().map(DurationMinutes::as_minutes),
            Some(10)
        );
        assert_eq!(durations.rest().map(DurationMinutes::as_minutes), Some(20));
        assert_eq!(
            durations.cooking().map(DurationMinutes::as_minutes),
            Some(30)
        );
    }

    #[test]
    fn people_servings_expose_their_count() {
        let servings = Servings::people(4);

        assert_eq!(servings.as_people(), Some(4));
        assert_eq!(servings.as_portions(), None);
    }

    #[test]
    fn portion_servings_expose_their_count_and_label() {
        let servings = Servings::portions_with_label(12, "cookies");

        assert_eq!(servings.as_people(), None);
        assert_eq!(servings.as_portions(), Some((12, Some("cookies"))));
    }

    #[test]
    fn portion_servings_can_have_no_label() {
        let servings = Servings::portions(12);

        assert_eq!(servings.as_portions(), Some((12, None)));
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
    fn source_document_summary_contains_metadata_but_not_bytes() {
        let image = RecipeSourceDocument::new(
            RecipeSourceDocumentId::new("image-1".to_string()),
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

    #[test]
    fn new_recipes_are_editable_by_default() {
        let recipe = Recipe::new(
            RecipeId::new("soup".to_string()),
            UserId::new("author".to_string()),
        );

        assert_eq!(recipe.scope(), RecipeScope::Editable);
    }

    #[test]
    fn private_recipes_are_visible_and_editable_only_by_their_author() {
        let recipe = Recipe::with_scope(
            RecipeId::new("soup".to_string()),
            UserId::new("author".to_string()),
            RecipeScope::Private,
        );
        let author = UserId::new("author".to_string());
        let other = UserId::new("other".to_string());

        assert!(recipe.can_view(&author));
        assert!(recipe.can_edit(&author));
        assert!(!recipe.can_view(&other));
        assert!(!recipe.can_edit(&other));
    }

    #[test]
    fn viewable_recipes_are_editable_only_by_their_author() {
        let recipe = Recipe::with_scope(
            RecipeId::new("soup".to_string()),
            UserId::new("author".to_string()),
            RecipeScope::Viewable,
        );
        let author = UserId::new("author".to_string());
        let other = UserId::new("other".to_string());

        assert!(recipe.can_view(&other));
        assert!(!recipe.can_edit(&other));
        assert!(recipe.can_edit(&author));
    }

    #[test]
    fn editable_recipes_are_visible_and_editable_by_everyone() {
        let recipe = Recipe::with_scope(
            RecipeId::new("soup".to_string()),
            UserId::new("author".to_string()),
            RecipeScope::Editable,
        );
        let other = UserId::new("other".to_string());

        assert!(recipe.can_view(&other));
        assert!(recipe.can_edit(&other));
    }

    #[test]
    fn recipe_category_round_trips_every_variant() {
        for category in RecipeCategory::ALL {
            assert_eq!(RecipeCategory::parse(category.as_str()), Some(category));
        }
    }

    #[test]
    fn recipe_category_parses_only_known_wire_values() {
        assert_eq!(
            RecipeCategory::parse("aperitif"),
            Some(RecipeCategory::Aperitif)
        );
        assert_eq!(
            RecipeCategory::parse("dessert"),
            Some(RecipeCategory::Dessert)
        );
        assert_eq!(RecipeCategory::parse("Dessert"), None);
        assert_eq!(RecipeCategory::parse(""), None);
    }

    #[test]
    fn recipe_scope_parses_only_known_wire_values() {
        assert_eq!(RecipeScope::parse("private"), Some(RecipeScope::Private));
        assert_eq!(RecipeScope::parse("viewable"), Some(RecipeScope::Viewable));
        assert_eq!(RecipeScope::parse("editable"), Some(RecipeScope::Editable));
        assert_eq!(RecipeScope::parse("public"), None);
    }

    fn ingredient(name: &str, quantity: Quantity) -> Ingredient {
        Ingredient::new(IngredientName::new(name.to_string()), quantity)
    }

    fn simple_content(title: &str, body: RecipeBody) -> RecipeContent {
        RecipeContent::new_simple(RecipeTitle::new(title.to_string()), None, None, None, body)
            .unwrap()
    }

    fn part(title: &str, body: RecipeBody) -> RecipePart {
        RecipePart::new(
            RecipePartId::generate(),
            RecipePartTitle::new(title.to_string()),
            None,
            body,
        )
    }

    #[test]
    fn body_normalization_sums_same_unit_quantities_and_keeps_conflicts() {
        let body = RecipeBody::new(
            RecipeDurations::new(None, None, None),
            vec![
                ingredient("flour", Quantity::from_measure(100.0, Unit::Gram)),
                ingredient("flour", Quantity::from_measure(50.0, Unit::Gram)),
                ingredient("flour", Quantity::from_measure(2.0, Unit::Tablespoon)),
                ingredient("salt", Quantity::from_text("to taste")),
                ingredient("salt", Quantity::from_text("to taste")),
            ],
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(body.ingredients().len(), 3);
        assert_eq!(
            body.ingredients()[0].quantity(),
            &Quantity::from_measure(150.0, Unit::Gram)
        );
        assert_eq!(
            body.ingredients()[1].quantity(),
            &Quantity::from_measure(2.0, Unit::Tablespoon)
        );
        assert_eq!(
            body.ingredients()[2].quantity(),
            &Quantity::from_text("to taste")
        );
        assert_eq!(body.ingredient_conflicts().len(), 1);
        assert!(body.has_ingredient_conflicts());
    }

    #[test]
    fn multipart_durations_are_summed_across_parts() {
        let content = RecipeContent::new_multipart(
            RecipeTitle::new("Cake".to_string()),
            None,
            None,
            None,
            vec![
                part(
                    "Sponge",
                    RecipeBody::new(
                        RecipeDurations::new(
                            Some(DurationMinutes::new(10)),
                            None,
                            Some(DurationMinutes::new(20)),
                        ),
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    ),
                ),
                part(
                    "Filling",
                    RecipeBody::new(
                        RecipeDurations::new(
                            Some(DurationMinutes::new(5)),
                            Some(DurationMinutes::new(30)),
                            None,
                        ),
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    ),
                ),
            ],
        )
        .unwrap();
        let durations = content.durations();

        assert_eq!(
            durations.preparation().map(DurationMinutes::as_minutes),
            Some(15)
        );
        assert_eq!(durations.rest().map(DurationMinutes::as_minutes), Some(30));
        assert_eq!(
            durations.cooking().map(DurationMinutes::as_minutes),
            Some(20)
        );
        assert_eq!(durations.total_duration().as_minutes(), 65);
    }

    #[test]
    fn simple_and_multipart_conversions_preserve_body_content() {
        let step = RecipeStep::new(
            RecipeStepId::generate(),
            Some("Mix".to_string()),
            "Stir.".to_string(),
            vec![RecipeImageId::new("step-image".to_string())],
        );
        let simple = simple_content(
            "Pastry",
            RecipeBody::new(
                RecipeDurations::new(Some(DurationMinutes::new(10)), None, None),
                vec![ingredient(
                    "flour",
                    Quantity::from_measure(200.0, Unit::Gram),
                )],
                vec![RecipeImageId::new("hero".to_string())],
                vec![step],
            ),
        );

        let multipart = simple.clone().into_multipart();

        assert!(multipart.is_multipart());
        assert_eq!(multipart.parts()[0].title().as_str(), "Pastry");

        let restored = multipart.into_simple();

        assert_eq!(restored.simple_body().unwrap().ingredients().len(), 1);
        assert_eq!(restored.simple_body().unwrap().images()[0].as_str(), "hero");
        assert_eq!(
            restored.simple_body().unwrap().steps()[0].title(),
            Some("Mix")
        );
        assert_eq!(
            restored
                .durations()
                .preparation()
                .map(DurationMinutes::as_minutes),
            Some(10)
        );
    }

    #[test]
    fn multipart_to_simple_merges_images_and_folds_part_descriptions() {
        let content = RecipeContent::new_multipart(
            RecipeTitle::new("Cake".to_string()),
            Some(RecipeDescription::new("Overall description.".to_string())),
            None,
            None,
            vec![
                RecipePart::new(
                    RecipePartId::generate(),
                    RecipePartTitle::new("Sponge".to_string()),
                    Some(RecipeDescription::new("Light and airy.".to_string())),
                    RecipeBody::new(
                        RecipeDurations::new(None, None, None),
                        Vec::new(),
                        vec![RecipeImageId::new("shared".to_string())],
                        Vec::new(),
                    ),
                ),
                RecipePart::new(
                    RecipePartId::generate(),
                    RecipePartTitle::new("Filling".to_string()),
                    Some(RecipeDescription::new("Whipped cream.".to_string())),
                    RecipeBody::new(
                        RecipeDurations::new(None, None, None),
                        Vec::new(),
                        vec![
                            RecipeImageId::new("shared".to_string()),
                            RecipeImageId::new("filling".to_string()),
                        ],
                        Vec::new(),
                    ),
                ),
            ],
        )
        .unwrap();

        let simple = content.into_simple();
        let body = simple.simple_body().unwrap();

        assert_eq!(
            simple.description().map(RecipeDescription::as_str),
            Some("Overall description.\n\nSponge: Light and airy.\n\nFilling: Whipped cream.")
        );
        assert_eq!(body.images().len(), 2);
        assert_eq!(body.images()[0].as_str(), "shared");
        assert_eq!(body.images()[1].as_str(), "filling");
    }

    #[test]
    fn all_image_ids_are_unique_and_ordered() {
        let content = RecipeContent::new_multipart(
            RecipeTitle::new("Cake".to_string()),
            None,
            None,
            None,
            vec![
                part(
                    "Sponge",
                    RecipeBody::new(
                        RecipeDurations::new(None, None, None),
                        Vec::new(),
                        vec![
                            RecipeImageId::new("a".to_string()),
                            RecipeImageId::new("b".to_string()),
                        ],
                        Vec::new(),
                    ),
                ),
                part(
                    "Filling",
                    RecipeBody::new(
                        RecipeDurations::new(None, None, None),
                        Vec::new(),
                        vec![
                            RecipeImageId::new("b".to_string()),
                            RecipeImageId::new("c".to_string()),
                        ],
                        Vec::new(),
                    ),
                ),
            ],
        )
        .unwrap();

        let ids: Vec<_> = content
            .all_image_ids()
            .into_iter()
            .map(|id| id.as_str().to_string())
            .collect();

        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn content_construction_rejects_an_empty_title() {
        let content = RecipeContent::new_simple(
            RecipeTitle::new("   ".to_string()),
            None,
            None,
            None,
            RecipeBody::new(
                RecipeDurations::new(None, None, None),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
        );

        assert_eq!(content.unwrap_err(), RecipeContentError::EmptyTitle);
    }

    #[test]
    fn multipart_construction_rejects_invalid_structures() {
        let empty = RecipeContent::new_multipart(
            RecipeTitle::new("Cake".to_string()),
            None,
            None,
            None,
            Vec::new(),
        );
        assert_eq!(empty.unwrap_err(), RecipeContentError::NoParts);

        let empty_title = RecipeContent::new_multipart(
            RecipeTitle::new("Cake".to_string()),
            None,
            None,
            None,
            vec![part(
                "   ",
                RecipeBody::new(
                    RecipeDurations::new(None, None, None),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                ),
            )],
        );
        assert_eq!(empty_title.unwrap_err(), RecipeContentError::EmptyPartTitle);

        let duplicate_id = RecipePartId::generate();
        let duplicate = RecipeContent::new_multipart(
            RecipeTitle::new("Cake".to_string()),
            None,
            None,
            None,
            vec![
                RecipePart::new(
                    duplicate_id.clone(),
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
                    duplicate_id,
                    RecipePartTitle::new("Filling".to_string()),
                    None,
                    RecipeBody::new(
                        RecipeDurations::new(None, None, None),
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    ),
                ),
            ],
        );
        assert!(matches!(
            duplicate,
            Err(RecipeContentError::DuplicatePartId(_))
        ));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trips_multipart_content_exactly() {
        let content = RecipeContent::new_multipart(
            RecipeTitle::new("Cake".to_string()),
            Some(RecipeDescription::new("Overall.".to_string())),
            Some(Servings::portions_with_label(12, "slices")),
            Some(RecipeCategory::Dessert),
            vec![RecipePart::new(
                RecipePartId::new("part-1".to_string()),
                RecipePartTitle::new("Sponge".to_string()),
                Some(RecipeDescription::new("Light.".to_string())),
                RecipeBody::new(
                    RecipeDurations::new(
                        Some(DurationMinutes::new(10)),
                        None,
                        Some(DurationMinutes::new(20)),
                    ),
                    vec![
                        ingredient("flour", Quantity::from_measure(100.0, Unit::Gram)),
                        // A conflicting quantity must survive the round-trip unchanged.
                        ingredient("flour", Quantity::from_measure(2.0, Unit::Tablespoon)),
                    ],
                    vec![RecipeImageId::new("image-1".to_string())],
                    vec![RecipeStep::new(
                        RecipeStepId::new("step-1".to_string()),
                        Some("Mix".to_string()),
                        "Stir.".to_string(),
                        vec![RecipeImageId::new("image-2".to_string())],
                    )],
                ),
            )],
        )
        .unwrap();

        let json = serde_json::to_string(&content).unwrap();
        let restored: RecipeContent = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.title().as_str(), "Cake");
        assert_eq!(
            restored.description().map(RecipeDescription::as_str),
            Some("Overall.")
        );
        assert_eq!(
            restored
                .servings()
                .and_then(|servings| servings.as_portions()),
            Some((12, Some("slices")))
        );
        assert_eq!(restored.category(), Some(RecipeCategory::Dessert));

        let part = &restored.parts()[0];
        assert_eq!(part.title().as_str(), "Sponge");
        assert_eq!(
            part.description().map(RecipeDescription::as_str),
            Some("Light.")
        );
        assert_eq!(part.body().ingredients().len(), 2);
        assert_eq!(
            part.body().ingredients()[0].quantity(),
            &Quantity::from_measure(100.0, Unit::Gram)
        );
        assert_eq!(
            part.body().ingredients()[1].quantity(),
            &Quantity::from_measure(2.0, Unit::Tablespoon)
        );
        assert_eq!(part.body().images()[0].as_str(), "image-1");
        assert_eq!(part.body().steps()[0].images()[0].as_str(), "image-2");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trips_revision_origin() {
        let origin = RevisionOrigin::Restored {
            revision_id: RecipeRevisionId::new("revision-1".to_string()),
        };

        let json = serde_json::to_string(&origin).unwrap();
        let restored: RevisionOrigin = serde_json::from_str(&json).unwrap();

        assert_eq!(restored, origin);
    }
}
