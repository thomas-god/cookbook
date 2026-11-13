use anyhow::{Context, bail};
use cookbook_domain::recipe::models::RecipeContent;

/// The current JSON representation of [RecipeContent].
///
/// Bump this only for a breaking change to the stored content shape. Old
/// revisions keep their original version and are decoded through the matching
/// branch in [decode_content].
pub const CURRENT_CONTENT_VERSION: i32 = 1;

pub fn encode_content(content: &RecipeContent) -> anyhow::Result<(i32, String)> {
    let json = serde_json::to_string(content).context("failed to encode recipe content")?;

    Ok((CURRENT_CONTENT_VERSION, json))
}

pub fn decode_content(version: i32, json: &str) -> anyhow::Result<RecipeContent> {
    match version {
        1 => decode_v1(json),
        version if version > CURRENT_CONTENT_VERSION => bail!(
            "recipe content version {version} is newer than supported version \
             {CURRENT_CONTENT_VERSION}"
        ),
        version => bail!("unsupported recipe content version {version}"),
    }
}

fn decode_v1(json: &str) -> anyhow::Result<RecipeContent> {
    serde_json::from_str(json).context("failed to decode v1 recipe content")
}

/// Builds the lowercased text used by substring search.
///
/// Fields are separated by a unit separator so a pattern cannot match across
/// two unrelated fields.
pub fn search_text(content: &RecipeContent) -> String {
    let mut text = String::new();
    let mut push = |value: &str| {
        if !text.is_empty() {
            text.push('\u{1f}');
        }
        text.push_str(&value.to_lowercase());
    };

    push(content.title().as_str());

    if let Some(description) = content.description() {
        push(description.as_str());
    }

    for body in content.bodies() {
        for ingredient in body.ingredients() {
            push(ingredient.name().as_str());
        }

        for step in body.steps() {
            if let Some(title) = step.title() {
                push(title);
            }
            push(step.instruction());
        }
    }

    for part in content.parts() {
        push(part.title().as_str());
        if let Some(description) = part.description() {
            push(description.as_str());
        }
    }

    text
}

#[cfg(test)]
mod tests {
    use cookbook_domain::recipe::models::{
        Ingredient, IngredientName, Quantity, RecipeBody, RecipeCategory, RecipeContent,
        RecipeDescription, RecipeDurations, RecipeStep, RecipeStepId, RecipeTitle,
    };

    use super::*;

    const SIMPLE_V1: &str =
        include_str!("../../../cookbook-domain/tests/fixtures/recipe_content_v1_simple.json");

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

    #[test]
    fn encodes_with_the_current_version() {
        let (version, json) = encode_content(&content("Soup")).unwrap();

        assert_eq!(version, CURRENT_CONTENT_VERSION);
        assert!(json.contains("\"Soup\""));
    }

    #[test]
    fn round_trips_current_content() {
        let original = content("Soup");
        let (version, json) = encode_content(&original).unwrap();
        let restored = decode_content(version, &json).unwrap();

        assert_eq!(restored.title().as_str(), original.title().as_str());
    }

    #[test]
    fn decodes_the_v1_fixture() {
        let restored = decode_content(1, SIMPLE_V1).unwrap();

        assert_eq!(restored.title().as_str(), "Tomato Soup");
        assert_eq!(restored.parts().len(), 0);
        assert_eq!(restored.simple_body().unwrap().ingredients().len(), 3);
    }

    #[test]
    fn rejects_unsupported_and_newer_versions() {
        let (_, json) = encode_content(&content("Soup")).unwrap();

        let unsupported = decode_content(0, &json).unwrap_err();
        let newer = decode_content(CURRENT_CONTENT_VERSION + 1, &json).unwrap_err();

        assert!(unsupported.to_string().contains("unsupported"));
        assert!(newer.to_string().contains("newer than supported"));
    }

    #[test]
    fn rejects_invalid_json() {
        let error = decode_content(1, "{not json").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("failed to decode v1 recipe content")
        );
    }

    #[test]
    fn search_text_contains_all_searchable_fields_lowercased() {
        let content = RecipeContent::new_simple(
            RecipeTitle::new("Tomato Soup".to_string()),
            Some(RecipeDescription::new("A Comforting Soup.".to_string())),
            None,
            Some(RecipeCategory::Main),
            RecipeBody::new(
                RecipeDurations::new(None, None, None),
                vec![Ingredient::new(
                    IngredientName::new("Basil".to_string()),
                    Quantity::from_text("a handful"),
                )],
                Vec::new(),
                vec![RecipeStep::new(
                    RecipeStepId::new("step-1".to_string()),
                    Some("Simmer".to_string()),
                    "Cook gently.".to_string(),
                    Vec::new(),
                )],
            ),
        )
        .unwrap();

        let text = search_text(&content);

        assert!(text.contains("tomato soup"));
        assert!(text.contains("a comforting soup."));
        assert!(text.contains("basil"));
        assert!(text.contains("simmer"));
        assert!(text.contains("cook gently."));
    }
}
