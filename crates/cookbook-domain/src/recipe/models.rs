use std::collections::HashMap;

use derive_more::Constructor;

#[derive(Debug, Clone, Constructor)]
pub struct Recipe {
    id: RecipeId,
    title: RecipeTitle,
    metadata: RecipeMetadata,
    ingredients: HashMap<IngredientName, Quantity>,
    steps: Vec<RecipeStep>,
}

impl Recipe {
    pub fn id(&self) -> &RecipeId {
        &self.id
    }

    pub fn title(&self) -> &RecipeTitle {
        &self.title
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Constructor)]
pub struct RecipeId(String);

#[derive(Debug, Clone, Constructor)]
pub struct RecipeTitle(String);

impl RecipeTitle {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Constructor)]
pub struct IngredientName(String);

#[derive(Debug, Clone, Constructor)]
pub struct Quantity {
    value: f32,
    unit: Unit,
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
