use std::time::SystemTime;

use cookbook_domain::recipe::models::{
    DurationMinutes, Ingredient, IngredientName, Quantity, Recipe, RecipeBody, RecipeCategory,
    RecipeContent, RecipeDescription, RecipeDurations, RecipeId, RecipePart, RecipePartId,
    RecipePartTitle, RecipeRevision, RecipeRevisionId, RecipeStep, RecipeStepId, RecipeTitle,
    RecipeWithContent, RevisionOrigin, RevisionVerification, Servings, Unit,
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
            RevisionVerification::Verified,
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
            RevisionVerification::Verified,
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
            RevisionVerification::Verified,
        ),
        seeded_recipe(
            "seed-pickled-onions",
            "Quick Pickled Onions",
            "A sharp, crunchy topping that is ready in under an hour.",
            Some(Servings::people(6)),
            Some(RecipeCategory::Side),
            RecipeDurations::new(
                Some(DurationMinutes::new(10)),
                Some(DurationMinutes::new(40)),
                None,
            ),
            vec![
                Ingredient::new(
                    IngredientName::new("red onion".to_string()),
                    Quantity::from_measure(2.0, Unit::Piece),
                ),
                Ingredient::new(
                    IngredientName::new("vinegar".to_string()),
                    Quantity::from_measure(200.0, Unit::MilliLitre),
                ),
                Ingredient::new(
                    IngredientName::new("sugar".to_string()),
                    Quantity::from_measure(2.0, Unit::Tablespoon),
                ),
            ],
            vec![
                RecipeStep::new(
                    RecipeStepId::generate(),
                    None,
                    "Slice the onions as thinly as possible.".to_string(),
                    Vec::new(),
                ),
                RecipeStep::new(
                    RecipeStepId::generate(),
                    None,
                    "Warm the vinegar and sugar, pour over the onions and leave to pickle.".to_string(),
                    Vec::new(),
                ),
            ],
            RevisionVerification::NeedsReview,
        ),
        seeded_multipart_recipe(
            "seed-roast-chicken",
            "Sunday Roast Chicken",
            "A roast dinner in two parts: the bird and the gravy.",
            Some(Servings::people(4)),
            Some(RecipeCategory::Main),
            vec![
                seeded_part(
                    "Roast chicken",
                    Some("Crisp skin and juicy meat."),
                    RecipeDurations::new(
                        Some(DurationMinutes::new(15)),
                        None,
                        Some(DurationMinutes::new(80)),
                    ),
                    vec![
                        Ingredient::new(
                            IngredientName::new("whole chicken".to_string()),
                            Quantity::from_measure(1.5, Unit::Kilogram),
                        ),
                        Ingredient::new(
                            IngredientName::new("butter".to_string()),
                            Quantity::from_measure(50.0, Unit::Gram),
                        ),
                    ],
                    vec![RecipeStep::new(
                        RecipeStepId::generate(),
                        None,
                        "Rub the chicken with butter, season and roast until the juices run clear."
                            .to_string(),
                        Vec::new(),
                    )],
                ),
                seeded_part(
                    "Pan gravy",
                    None,
                    RecipeDurations::new(None, None, Some(DurationMinutes::new(15))),
                    vec![
                        Ingredient::new(
                            IngredientName::new("flour".to_string()),
                            Quantity::from_measure(2.0, Unit::Tablespoon),
                        ),
                        Ingredient::new(
                            IngredientName::new("chicken stock".to_string()),
                            Quantity::from_measure(0.5, Unit::Litre),
                        ),
                    ],
                    vec![RecipeStep::new(
                        RecipeStepId::generate(),
                        None,
                        "Stir the flour into the roasting juices, then whisk in the stock and simmer."
                            .to_string(),
                        Vec::new(),
                    )],
                ),
            ],
            RevisionVerification::Verified,
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
    verification: RevisionVerification,
) -> RecipeWithContent {
    let content = RecipeContent::new_simple(
        RecipeTitle::new(title.to_string()),
        Some(RecipeDescription::new(description.to_string())),
        servings,
        category,
        RecipeBody::new(durations, ingredients, Vec::new(), steps),
    )
    .expect("seed recipe content is valid");

    seeded_revision(id, content, verification)
}

#[allow(clippy::too_many_arguments)]
fn seeded_multipart_recipe(
    id: &str,
    title: &str,
    description: &str,
    servings: Option<Servings>,
    category: Option<RecipeCategory>,
    parts: Vec<RecipePart>,
    verification: RevisionVerification,
) -> RecipeWithContent {
    let content = RecipeContent::new_multipart(
        RecipeTitle::new(title.to_string()),
        Some(RecipeDescription::new(description.to_string())),
        servings,
        category,
        parts,
    )
    .expect("seed multipart recipe content is valid");

    seeded_revision(id, content, verification)
}

fn seeded_part(
    title: &str,
    description: Option<&str>,
    durations: RecipeDurations,
    ingredients: Vec<Ingredient>,
    steps: Vec<RecipeStep>,
) -> RecipePart {
    RecipePart::new(
        RecipePartId::generate(),
        RecipePartTitle::new(title.to_string()),
        description.map(|description| RecipeDescription::new(description.to_string())),
        RecipeBody::new(durations, ingredients, Vec::new(), steps),
    )
}

fn seeded_revision(
    id: &str,
    content: RecipeContent,
    verification: RevisionVerification,
) -> RecipeWithContent {
    let recipe = Recipe::new(RecipeId::new(id.to_string()), seed_author());
    let revision = RecipeRevision::new(
        RecipeRevisionId::new(Uuid::new_v4().to_string()),
        recipe.id().clone(),
        content,
        recipe.author().clone(),
        SystemTime::now(),
        RevisionOrigin::Created,
        verification,
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

        assert_eq!(recipes.len(), 5);
        for recipe in &recipes {
            assert_eq!(recipe.revision().recipe_id(), recipe.id());
            assert_eq!(recipe.author().as_str(), SEED_USER_ID);
            assert!(!recipe.title().as_str().is_empty());
        }

        assert!(
            recipes
                .iter()
                .any(|recipe| recipe.verification() == RevisionVerification::NeedsReview)
        );
        assert!(recipes.iter().any(|recipe| recipe.content().is_multipart()));
    }
}
