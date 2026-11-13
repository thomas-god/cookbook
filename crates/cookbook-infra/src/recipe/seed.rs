use std::time::SystemTime;

use cookbook_domain::recipe::models::{
    DurationMinutes, Ingredient, IngredientName, Quantity, Recipe, RecipeBody, RecipeCategory,
    RecipeContent, RecipeDescription, RecipeDurations, RecipeId, RecipeRevision, RecipeRevisionId,
    RecipeStep, RecipeStepId, RecipeTitle, RecipeWithContent, RevisionOrigin, RevisionVerification,
    Servings, Unit,
};
use cookbook_domain::user::models::UserId;
use uuid::Uuid;

/// The user id used by seeded recipes.
pub const SEED_USER_ID: &str = "seed-author";
pub const SEED_USER_EMAIL: &str = "dev@example.com";
pub const SEED_USER_USERNAME: &str = "Dev";
pub const SEED_USER_PASSWORD: &str = "test";

/// Returns the sample recipes used by tests and the dev seeding tool.
///
/// Their author id matches [SEED_USER_ID], so the dev seeding tool can create
/// the matching user before inserting the recipes.
pub fn sample_recipes() -> Vec<RecipeWithContent> {
    vec![
        seeded_recipe(
            "seed-tomato-soup",
            "Tomato Soup",
            "A comforting weeknight soup made with ripe tomatoes.",
            Some(Servings::people(4)),
            Some(RecipeCategory::Main),
            RecipeDurations::new(
                Some(DurationMinutes::new(10)),
                None,
                Some(DurationMinutes::new(30)),
            ),
            vec![
                Ingredient::new(
                    IngredientName::new("tomatoes".to_string()),
                    Quantity::from_measure(800.0, Unit::Gram),
                ),
                Ingredient::new(
                    IngredientName::new("vegetable stock".to_string()),
                    Quantity::from_measure(0.5, Unit::Litre),
                ),
                Ingredient::new(
                    IngredientName::new("olive oil".to_string()),
                    Quantity::from_measure(30.0, Unit::MilliLitre),
                ),
            ],
            vec![
                RecipeStep::new(
                    RecipeStepId::generate(),
                    Some("Soften the aromatics".to_string()),
                    "Warm the olive oil in a pot and cook the tomatoes until soft.".to_string(),
                    Vec::new(),
                ),
                RecipeStep::new(
                    RecipeStepId::generate(),
                    None,
                    "Add the stock, simmer for 20 minutes, then blend until smooth.".to_string(),
                    Vec::new(),
                ),
            ],
        ),
        seeded_recipe(
            "seed-lemon-pasta",
            "Lemon Pasta",
            "A bright pasta dish that comes together in minutes.",
            Some(Servings::people(2)),
            Some(RecipeCategory::Main),
            RecipeDurations::new(
                Some(DurationMinutes::new(10)),
                None,
                Some(DurationMinutes::new(12)),
            ),
            vec![
                Ingredient::new(
                    IngredientName::new("spaghetti".to_string()),
                    Quantity::from_measure(400.0, Unit::Gram),
                ),
                Ingredient::new(
                    IngredientName::new("lemon".to_string()),
                    Quantity::from_measure(100.0, Unit::Gram),
                ),
                Ingredient::new(
                    IngredientName::new("olive oil".to_string()),
                    Quantity::from_measure(30.0, Unit::MilliLitre),
                ),
            ],
            vec![
                RecipeStep::new(
                    RecipeStepId::generate(),
                    None,
                    "Cook the spaghetti in salted water until al dente.".to_string(),
                    Vec::new(),
                ),
                RecipeStep::new(
                    RecipeStepId::generate(),
                    None,
                    "Toss with lemon zest, lemon juice and olive oil.".to_string(),
                    Vec::new(),
                ),
            ],
        ),
        seeded_recipe(
            "seed-banana-bread",
            "Banana Bread",
            "A moist loaf that is perfect for using up ripe bananas.",
            Some(Servings::portions_with_label(12, "slices")),
            Some(RecipeCategory::Dessert),
            RecipeDurations::new(
                Some(DurationMinutes::new(15)),
                Some(DurationMinutes::new(10)),
                Some(DurationMinutes::new(55)),
            ),
            vec![
                Ingredient::new(
                    IngredientName::new("bananas".to_string()),
                    Quantity::from_measure(3.0, Unit::Piece),
                ),
                Ingredient::new(
                    IngredientName::new("flour".to_string()),
                    Quantity::from_measure(250.0, Unit::Gram),
                ),
                Ingredient::new(
                    IngredientName::new("sugar".to_string()),
                    Quantity::from_measure(150.0, Unit::Gram),
                ),
                Ingredient::new(
                    IngredientName::new("butter".to_string()),
                    Quantity::from_measure(100.0, Unit::Gram),
                ),
            ],
            vec![
                RecipeStep::new(
                    RecipeStepId::generate(),
                    None,
                    "Mash the bananas and mix them with the melted butter and sugar.".to_string(),
                    Vec::new(),
                ),
                RecipeStep::new(
                    RecipeStepId::generate(),
                    None,
                    "Fold in the flour, pour into a loaf tin and bake until golden.".to_string(),
                    Vec::new(),
                ),
            ],
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn seeded_recipe(
    id: &str,
    title: &str,
    description: &str,
    servings: Option<Servings>,
    category: Option<RecipeCategory>,
    durations: RecipeDurations,
    ingredients: Vec<Ingredient>,
    steps: Vec<RecipeStep>,
) -> RecipeWithContent {
    let recipe = Recipe::new(RecipeId::new(id.to_string()), seed_author());
    let content = RecipeContent::new_simple(
        RecipeTitle::new(title.to_string()),
        Some(RecipeDescription::new(description.to_string())),
        servings,
        category,
        RecipeBody::new(durations, ingredients, Vec::new(), steps),
    )
    .expect("seed recipe content is valid");
    let revision = RecipeRevision::new(
        RecipeRevisionId::new(Uuid::new_v4().to_string()),
        recipe.id().clone(),
        content,
        recipe.author().clone(),
        SystemTime::now(),
        RevisionOrigin::Created,
        RevisionVerification::Verified,
    );

    RecipeWithContent::new(recipe, revision).expect("seed revision belongs to its recipe")
}

fn seed_author() -> UserId {
    UserId::new(SEED_USER_ID.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_recipes_are_complete() {
        let recipes = sample_recipes();

        assert_eq!(recipes.len(), 3);
        for recipe in recipes {
            assert_eq!(recipe.revision().recipe_id(), recipe.id());
            assert_eq!(recipe.author().as_str(), SEED_USER_ID);
            assert!(!recipe.title().as_str().is_empty());
        }
    }
}
