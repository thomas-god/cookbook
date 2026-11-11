use std::collections::HashMap;

use cookbook_domain::recipe::models::{
    DurationMinutes, IngredientName, Quantity, RecipeContent, RecipeDescription, RecipeDurations,
    RecipeStep, RecipeStepId, RecipeTitle, Servings, Unit,
};
use schemars::{JsonSchema, generate::SchemaSettings};
use serde::Deserialize;

pub(crate) const SYSTEM_PROMPT: &str = r#"You extract recipes from photos of handwritten, printed, or book pages.

Preserve the recipe's original language and wording for the title, description, ingredient
names, step titles, and instructions. Do not invent missing information.

Rules for ingredient amounts:
- Use "amount" plus "unit" for measurable quantities.
- Prefer these unit names when they fit: gram, kilogram, millilitre, centilitre, litre,
  piece, teaspoon, tablespoon.
- Use "piece" for countable items such as eggs, onions, or carrots.
- Use "amount_text" instead of amount/unit for free-form amounts such as "a pinch" or
  "to taste".
- Every ingredient must have either a numeric amount or an amount_text.
- Step-level ingredients contain only names and numeric amounts from the ingredient list.

Rules for servings:
- Use "servings_people" for a number of people, such as 4.
- Use "servings_amount" with an optional "servings_label" for a count of items, such as
  12 cookies or 8 slices. Keep the label as written in the source when it is present.
- Leave all three servings fields null when the source does not state a yield.
- Do not fill both a people count and an item count.

Durations are whole minutes. Use null when a duration is not written in the source."#;

pub(crate) fn user_prompt(image_count: usize) -> String {
    format!("Extract the recipe from these {image_count} photo(s).")
}

/// The JSON schema passed to the endpoint as structured output.
///
/// The schema is derived from the same types used to deserialize the
/// response, so it cannot drift from the parser.
pub(crate) fn response_format() -> serde_json::Value {
    let mut schema = SchemaSettings::default()
        .with(|settings| {
            settings.meta_schema = None;
            settings.inline_subschemas = true;
        })
        .into_generator()
        .into_root_schema_for::<RawRecipe>()
        .to_value();

    if let Some(object) = schema.as_object_mut() {
        object.remove("$schema");
        object.remove("title");
        object.remove("$defs");
    }

    // OpenAI structured output requires every declared property to be listed
    // as required, even when its schema also permits null.
    require_all_properties(&mut schema);

    serde_json::json!({
        "type": "json_schema",
        "json_schema": {
            "name": "recipe",
            "strict": true,
            "schema": schema,
        }
    })
}

fn require_all_properties(schema: &mut serde_json::Value) {
    match schema {
        serde_json::Value::Object(object) => {
            if let Some(serde_json::Value::Object(properties)) = object.get("properties") {
                let required = properties
                    .keys()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect();

                object.insert("required".to_string(), serde_json::Value::Array(required));
            }

            for value in object.values_mut() {
                require_all_properties(value);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                require_all_properties(item);
            }
        }
        _ => {}
    }
}

pub(crate) fn parse_recipe_json(response: &str) -> Result<RecipeContent, String> {
    let json = extract_json(response);
    let raw: RawRecipe =
        serde_json::from_str(json).map_err(|error| format!("invalid recipe JSON: {error}"))?;

    raw.into_content()
}

fn extract_json(response: &str) -> &str {
    let trimmed = response.trim();

    if let Some(rest) = trimmed.strip_prefix("```") {
        let rest = rest.strip_prefix("json").unwrap_or(rest);
        let rest = rest.trim_start_matches(['\r', '\n']);

        if let Some(end) = rest.rfind("```") {
            return rest[..end].trim();
        }

        return rest.trim();
    }

    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}'))
        && start <= end
    {
        return &trimmed[start..=end];
    }

    trimmed
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RawRecipe {
    #[serde(default)]
    title: String,
    description: Option<String>,
    preparation_minutes: Option<f64>,
    rest_minutes: Option<f64>,
    cooking_minutes: Option<f64>,
    servings_people: Option<f64>,
    servings_amount: Option<f64>,
    servings_label: Option<String>,
    #[serde(default)]
    ingredients: Vec<RawIngredient>,
    #[serde(default)]
    steps: Vec<RawStep>,
}

impl RawRecipe {
    fn into_content(self) -> Result<RecipeContent, String> {
        let title = self.title.trim().to_string();

        if title.is_empty() {
            return Err("the model did not return a recipe title".to_string());
        }

        let description = self
            .description
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(RecipeDescription::new);
        let servings = parse_servings(
            self.servings_people,
            self.servings_amount,
            self.servings_label.as_deref(),
        );

        let ingredients = self
            .ingredients
            .into_iter()
            .filter_map(RawIngredient::into_pair)
            .collect::<Result<HashMap<_, _>, _>>()?;

        let steps = self
            .steps
            .into_iter()
            .filter_map(RawStep::into_step)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(RecipeContent::new(
            RecipeTitle::new(title),
            description,
            servings,
            RecipeDurations::new(
                duration_minutes(self.preparation_minutes),
                duration_minutes(self.rest_minutes),
                duration_minutes(self.cooking_minutes),
            ),
            ingredients,
            Vec::new(),
            steps,
        ))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RawIngredient {
    #[serde(default)]
    name: String,
    amount: Option<f64>,
    unit: Option<String>,
    amount_text: Option<String>,
}

impl RawIngredient {
    fn into_pair(self) -> Option<Result<(IngredientName, Quantity), String>> {
        let name = self.name.trim().to_string();

        if name.is_empty() {
            return None;
        }

        Some(
            parse_quantity(
                self.amount,
                self.unit.as_deref(),
                self.amount_text.as_deref(),
            )
            .ok_or_else(|| format!("ingredient `{name}` has no valid amount"))
            .map(|quantity| (IngredientName::new(name), quantity)),
        )
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RawStep {
    title: Option<String>,
    #[serde(default)]
    instruction: String,
    #[serde(default)]
    ingredients: Vec<RawStepIngredient>,
}

impl RawStep {
    fn into_step(self) -> Option<Result<RecipeStep, String>> {
        let instruction = self.instruction.trim().to_string();

        if instruction.is_empty() {
            return None;
        }

        let title = self
            .title
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let ingredients = self
            .ingredients
            .into_iter()
            .filter_map(RawStepIngredient::into_pair)
            .collect::<HashMap<_, _>>();

        Some(Ok(RecipeStep::new(
            RecipeStepId::generate(),
            title,
            (!ingredients.is_empty()).then_some(ingredients),
            instruction,
            Vec::new(),
        )))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RawStepIngredient {
    #[serde(default)]
    name: String,
    amount: Option<f64>,
}

impl RawStepIngredient {
    fn into_pair(self) -> Option<(String, f32)> {
        let name = self.name.trim().to_string();
        let amount = self.amount?;

        if name.is_empty() || !amount.is_finite() || amount < 0.0 {
            return None;
        }

        Some((name, amount as f32))
    }
}

fn duration_minutes(value: Option<f64>) -> Option<DurationMinutes> {
    value
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| DurationMinutes::new(value.round() as usize))
}

fn parse_servings(
    people: Option<f64>,
    amount: Option<f64>,
    label: Option<&str>,
) -> Option<Servings> {
    if let Some(people) = people.filter(|value| value.is_finite() && *value >= 1.0) {
        return Some(Servings::people(people.round() as usize));
    }

    let amount = amount.filter(|value| value.is_finite() && *value >= 1.0)?;
    let count = amount.round() as usize;

    Some(
        match label.map(str::trim).filter(|label| !label.is_empty()) {
            Some(label) => Servings::portions_with_label(count, label),
            None => Servings::portions(count),
        },
    )
}

fn parse_quantity(
    amount: Option<f64>,
    unit: Option<&str>,
    amount_text: Option<&str>,
) -> Option<Quantity> {
    if let Some(text) = amount_text.map(str::trim).filter(|text| !text.is_empty()) {
        return Some(Quantity::from_text(text));
    }

    let amount = amount?;

    if !amount.is_finite() || amount < 0.0 {
        return None;
    }

    Some(Quantity::from_measure(amount as f32, parse_unit(unit)))
}

fn parse_unit(unit: Option<&str>) -> Unit {
    let Some(unit) = unit.map(str::trim).filter(|unit| !unit.is_empty()) else {
        return Unit::Piece;
    };
    let normalized = unit.to_lowercase();

    match normalized.as_str() {
        "g" | "gram" | "grams" | "gramme" | "grammes" => Unit::Gram,
        "kg" | "kilo" | "kilos" | "kilogram" | "kilograms" | "kilogramme" | "kilogrammes" => {
            Unit::Kilogram
        }
        "ml" | "millilitre" | "millilitres" | "milliliter" | "milliliters" => Unit::MilliLitre,
        "cl" | "centilitre" | "centilitres" | "centiliter" | "centiliters" => Unit::CentiLitre,
        "l" | "litre" | "litres" | "liter" | "liters" => Unit::Litre,
        "piece" | "pieces" | "pc" | "pcs" | "unit" | "units" | "unité" | "unités" | "item"
        | "items" | "whole" => Unit::Piece,
        "tsp"
        | "teaspoon"
        | "teaspoons"
        | "cac"
        | "cc"
        | "c. à café"
        | "cuillère à café"
        | "cuillères à café" => Unit::Teaspoon,
        "tbsp"
        | "tablespoon"
        | "tablespoons"
        | "cas"
        | "cs"
        | "c. à soupe"
        | "cuillère à soupe"
        | "cuillères à soupe" => Unit::Tablespoon,
        _ => Unit::Other(unit.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_complete_recipe() {
        let content = parse_recipe_json(
            r#"{
                "title": "Tomato Soup",
                "description": "A simple soup.",
                "preparation_minutes": 10,
                "rest_minutes": null,
                "cooking_minutes": 30,
                "servings_people": 4,
                "servings_amount": null,
                "servings_label": null,
                "ingredients": [
                    {"name": "tomatoes", "amount": 800, "unit": "gram", "amount_text": null},
                    {"name": "salt", "amount": null, "unit": null, "amount_text": "to taste"}
                ],
                "steps": [
                    {
                        "title": "Cook",
                        "instruction": "Simmer everything.",
                        "ingredients": [{"name": "tomatoes", "amount": 800}]
                    }
                ]
            }"#,
        )
        .unwrap();

        assert_eq!(content.title().as_str(), "Tomato Soup");
        assert_eq!(
            content.description().map(RecipeDescription::as_str),
            Some("A simple soup.")
        );
        assert_eq!(content.durations().total_duration().as_minutes(), 40);
        assert_eq!(content.servings(), Some(&Servings::people(4)));
        assert_eq!(
            content.ingredients()[&IngredientName::new("tomatoes".to_string())],
            Quantity::from_measure(800.0, Unit::Gram)
        );
        assert_eq!(
            content.ingredients()[&IngredientName::new("salt".to_string())],
            Quantity::from_text("to taste")
        );
        assert_eq!(content.steps()[0].instruction(), "Simmer everything.");
    }

    #[test]
    fn parses_portion_servings_and_ignores_absent_servings() {
        let content = parse_recipe_json(
            r#"{
                "title": "Cookies",
                "servings_people": null,
                "servings_amount": 12,
                "servings_label": "  cookies  ",
                "ingredients": [],
                "steps": []
            }"#,
        )
        .unwrap();
        let without_servings = parse_recipe_json(
            r#"{
                "title": "Soup",
                "servings_people": null,
                "servings_amount": null,
                "servings_label": null,
                "ingredients": [],
                "steps": []
            }"#,
        )
        .unwrap();
        let without_label = parse_recipe_json(
            r#"{
                "title": "Cookies",
                "servings_people": null,
                "servings_amount": 12,
                "servings_label": null,
                "ingredients": [],
                "steps": []
            }"#,
        )
        .unwrap();

        assert_eq!(
            content.servings(),
            Some(&Servings::portions_with_label(12, "cookies"))
        );
        assert_eq!(without_label.servings(), Some(&Servings::portions(12)));
        assert_eq!(without_servings.servings(), None);
    }

    #[test]
    fn extracts_json_from_markdown_fences() {
        let content =
            parse_recipe_json("```json\n{\"title\":\"Soup\",\"ingredients\":[],\"steps\":[]}\n```")
                .unwrap();

        assert_eq!(content.title().as_str(), "Soup");
    }

    #[test]
    fn extracts_json_from_surrounding_text() {
        let content = parse_recipe_json(
            "Here is the recipe:\n{\"title\":\"Soup\",\"ingredients\":[],\"steps\":[]}\nDone.",
        )
        .unwrap();

        assert_eq!(content.title().as_str(), "Soup");
    }

    #[test]
    fn maps_unit_aliases_and_custom_units() {
        let content = parse_recipe_json(
            r#"{
                "title": "Soup",
                "ingredients": [
                    {"name": "a", "amount": 1, "unit": "tbsp"},
                    {"name": "b", "amount": 2, "unit": "sachet"},
                    {"name": "c", "amount": 3, "unit": null}
                ],
                "steps": []
            }"#,
        )
        .unwrap();

        assert_eq!(
            content.ingredients()[&IngredientName::new("a".to_string())],
            Quantity::from_measure(1.0, Unit::Tablespoon)
        );
        assert_eq!(
            content.ingredients()[&IngredientName::new("b".to_string())],
            Quantity::from_measure(2.0, Unit::Other("sachet".to_string()))
        );
        assert_eq!(
            content.ingredients()[&IngredientName::new("c".to_string())],
            Quantity::from_measure(3.0, Unit::Piece)
        );
    }

    #[test]
    fn structured_output_schema_matches_the_recipe_shape() {
        let format = response_format();
        let schema = &format["json_schema"]["schema"];

        assert_eq!(format["type"], "json_schema");
        assert_eq!(format["json_schema"]["strict"], true);
        assert_eq!(schema["additionalProperties"], false);
        let required = schema["required"].as_array().unwrap();
        for field in [
            "title",
            "description",
            "preparation_minutes",
            "rest_minutes",
            "cooking_minutes",
            "servings_people",
            "servings_amount",
            "servings_label",
            "ingredients",
            "steps",
        ] {
            assert!(
                required.iter().any(|value| value == field),
                "missing required field: {field}"
            );
        }
        assert_eq!(
            schema["properties"]["ingredients"]["items"]["additionalProperties"],
            false
        );
        assert_eq!(
            schema["properties"]["steps"]["items"]["additionalProperties"],
            false
        );
    }

    #[test]
    fn rejects_a_recipe_without_a_title() {
        assert!(parse_recipe_json(r#"{"ingredients":[],"steps":[]}"#).is_err());
    }

    #[test]
    fn rejects_an_ingredient_without_an_amount() {
        let result = parse_recipe_json(
            r#"{
                "title": "Soup",
                "ingredients": [{"name": "salt", "amount": null, "unit": null, "amount_text": null}],
                "steps": []
            }"#,
        );

        assert!(result.is_err());
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_recipe_json("not json").is_err());
    }
}
