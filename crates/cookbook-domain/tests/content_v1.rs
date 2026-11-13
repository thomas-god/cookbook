//! Golden tests for the persisted v1 JSON representation.
//!
//! These fixtures are intentionally raw and are not generated from the domain
//! types. If a change to `RecipeContent` or `RevisionOrigin` breaks one of
//! these tests, the stored format changed. That is only allowed together with a
//! `content_version` bump and a decode path for the old version.

#![cfg(feature = "serde")]

use cookbook_domain::recipe::models::{
    DurationMinutes, Ingredient, IngredientName, Quantity, RecipeBody, RecipeCategory,
    RecipeContent, RecipeDescription, RecipeDurations, RecipeImageId, RecipePart, RecipePartId,
    RecipePartTitle, RecipeRevisionId, RecipeStep, RecipeStepId, RecipeStructureKind, RecipeTitle,
    RevisionMessage, RevisionOrigin, Servings, Unit,
};

const SIMPLE_V1: &str = include_str!("fixtures/recipe_content_v1_simple.json");
const MULTIPART_V1: &str = include_str!("fixtures/recipe_content_v1_multipart.json");
const ORIGIN_V1: &str = include_str!("fixtures/revision_origin_v1.json");

fn assert_content_fixture(fixture: &str, expected: &RecipeContent) {
    let parsed: RecipeContent =
        serde_json::from_str(fixture).expect("v1 recipe content fixture must deserialize");
    let expected_json: serde_json::Value =
        serde_json::from_str(fixture).expect("v1 recipe content fixture must be valid JSON");

    assert_eq!(
        serde_json::to_value(&parsed).unwrap(),
        expected_json,
        "the v1 fixture must survive a deserialize/serialize round-trip"
    );
    assert_eq!(
        serde_json::to_value(expected).unwrap(),
        expected_json,
        "the current serialization must still match the v1 fixture"
    );
}

fn simple_content() -> RecipeContent {
    RecipeContent::new_simple(
        RecipeTitle::new("Tomato Soup".to_string()),
        Some(RecipeDescription::new("A comforting soup.".to_string())),
        Some(Servings::people(4)),
        Some(RecipeCategory::Main),
        RecipeBody::new(
            RecipeDurations::new(
                Some(DurationMinutes::new(10)),
                None,
                Some(DurationMinutes::new(30)),
            ),
            vec![
                ingredient("tomatoes", Quantity::from_measure(800.0, Unit::Gram)),
                ingredient("vegetable stock", Quantity::from_measure(0.5, Unit::Litre)),
                ingredient("salt", Quantity::from_text("to taste")),
            ],
            vec![RecipeImageId::new("image-hero".to_string())],
            vec![
                RecipeStep::new(
                    RecipeStepId::new("step-1".to_string()),
                    Some("Soften".to_string()),
                    "Warm the oil.".to_string(),
                    Vec::new(),
                ),
                RecipeStep::new(
                    RecipeStepId::new("step-2".to_string()),
                    None,
                    "Simmer.".to_string(),
                    vec![RecipeImageId::new("image-step".to_string())],
                ),
            ],
        ),
    )
    .unwrap()
}

fn multipart_content() -> RecipeContent {
    RecipeContent::new_multipart(
        RecipeTitle::new("Layer Cake".to_string()),
        Some(RecipeDescription::new("A two-part cake.".to_string())),
        Some(Servings::portions_with_label(12, "slices")),
        Some(RecipeCategory::Dessert),
        vec![
            RecipePart::new(
                RecipePartId::new("part-sponge".to_string()),
                RecipePartTitle::new("Sponge".to_string()),
                Some(RecipeDescription::new("Light and airy.".to_string())),
                RecipeBody::new(
                    RecipeDurations::new(
                        Some(DurationMinutes::new(10)),
                        None,
                        Some(DurationMinutes::new(25)),
                    ),
                    vec![
                        ingredient("flour", Quantity::from_measure(200.0, Unit::Gram)),
                        // This conflict is intentional and must not be normalized
                        // away when the fixture is decoded.
                        ingredient("flour", Quantity::from_measure(2.0, Unit::Tablespoon)),
                        ingredient("salt", Quantity::from_text("a pinch")),
                    ],
                    vec![RecipeImageId::new("image-sponge".to_string())],
                    vec![RecipeStep::new(
                        RecipeStepId::new("step-mix".to_string()),
                        Some("Mix".to_string()),
                        "Whisk everything together.".to_string(),
                        vec![RecipeImageId::new("image-mix".to_string())],
                    )],
                ),
            ),
            RecipePart::new(
                RecipePartId::new("part-filling".to_string()),
                RecipePartTitle::new("Filling".to_string()),
                None,
                RecipeBody::new(
                    RecipeDurations::new(None, Some(DurationMinutes::new(30)), None),
                    vec![ingredient(
                        "cream",
                        Quantity::from_measure(0.5, Unit::Litre),
                    )],
                    Vec::new(),
                    Vec::new(),
                ),
            ),
        ],
    )
    .unwrap()
}

fn ingredient(name: &str, quantity: Quantity) -> Ingredient {
    Ingredient::new(IngredientName::new(name.to_string()), quantity)
}

#[test]
fn recipe_content_v1_simple_fixture_is_stable() {
    assert_content_fixture(SIMPLE_V1, &simple_content());
}

#[test]
fn recipe_content_v1_multipart_fixture_is_stable() {
    assert_content_fixture(MULTIPART_V1, &multipart_content());
}

#[test]
fn revision_origin_v1_fixture_is_stable() {
    let expected = vec![
        RevisionOrigin::Created,
        RevisionOrigin::Imported,
        RevisionOrigin::UserEdit { message: None },
        RevisionOrigin::UserEdit {
            message: Some(RevisionMessage::new("Reduced sugar").unwrap()),
        },
        RevisionOrigin::StructureConverted {
            to: RecipeStructureKind::Multipart,
        },
        RevisionOrigin::StructureConverted {
            to: RecipeStructureKind::Simple,
        },
        RevisionOrigin::Restored {
            revision_id: RecipeRevisionId::new("revision-1".to_string()),
        },
    ];

    let parsed: Vec<RevisionOrigin> =
        serde_json::from_str(ORIGIN_V1).expect("v1 revision origin fixture must deserialize");
    let expected_json: serde_json::Value =
        serde_json::from_str(ORIGIN_V1).expect("v1 revision origin fixture must be valid JSON");

    assert_eq!(serde_json::to_value(&parsed).unwrap(), expected_json);
    assert_eq!(serde_json::to_value(&expected).unwrap(), expected_json);
}
