use std::collections::HashMap;

use derive_more::Constructor;

use crate::user::models::UserId;

#[derive(Debug, Clone, Constructor)]
pub struct Recipe {
    id: RecipeId,
    title: RecipeTitle,
    description: Option<RecipeDescription>,
    metadata: RecipeMetadata,
    ingredients: HashMap<IngredientName, Quantity>,
    steps: Vec<RecipeStep>,
    author: UserId,
}

impl Recipe {
    pub fn id(&self) -> &RecipeId {
        &self.id
    }

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

    pub fn author(&self) -> &UserId {
        &self.author
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Constructor)]
pub struct RecipeId(String);

impl RecipeId {
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

#[derive(Debug, Clone, Constructor)]
pub struct Quantity {
    value: f32,
    unit: Unit,
}

impl Quantity {
    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn unit(&self) -> &Unit {
        &self.unit
    }
}

#[derive(Debug, Clone)]
pub enum Unit {
    Litre,
    MilliLitre,
    Gram,
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
}
