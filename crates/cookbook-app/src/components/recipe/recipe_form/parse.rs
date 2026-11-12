use std::collections::{BTreeMap, HashMap, HashSet};

use cookbook_domain::recipe::models::{
    DurationMinutes, Ingredient, IngredientName, Quantity, RecipeBody, RecipeCategory,
    RecipeContent, RecipeDescription, RecipeDurations, RecipeImageId, RecipePart, RecipePartId,
    RecipePartTitle, RecipeStep, RecipeStepId, RecipeTitle, Servings, Unit,
};
use topcoat::{Error, Result, router::error::bad_request};

use super::fields::{
    CATEGORY_FIELD, COOKING_FIELD, DESCRIPTION_FIELD, IMAGE_IDS_FIELD, IngredientField,
    LATEST_REVISION_FIELD, MESSAGE_FIELD, PREPARATION_FIELD, PartField, RECIPE_TYPE_FIELD,
    REST_FIELD, SERVINGS_COUNT_FIELD, SERVINGS_LABEL_FIELD, SERVINGS_TYPE_FIELD, StepField,
    TITLE_FIELD,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecipeFormType {
    Simple,
    Multipart,
}

impl RecipeFormType {
    fn parse(value: &str) -> Result<Self> {
        match value.trim() {
            "" | "simple" => Ok(Self::Simple),
            "multipart" => Ok(Self::Multipart),
            other => Err(invalid(format!("unknown recipe type `{other}`"))),
        }
    }
}

#[derive(Debug)]
pub(super) struct BodyFormData {
    preparation: Option<usize>,
    rest: Option<usize>,
    cooking: Option<usize>,
    ingredients: Vec<IngredientFormData>,
    pub(super) images: Vec<RecipeImageId>,
    pub(super) steps: Vec<StepFormData>,
}

impl BodyFormData {
    fn into_body(self) -> RecipeBody {
        RecipeBody::new(
            RecipeDurations::new(
                self.preparation.map(DurationMinutes::new),
                self.rest.map(DurationMinutes::new),
                self.cooking.map(DurationMinutes::new),
            ),
            self.ingredients
                .into_iter()
                .map(|ingredient| {
                    Ingredient::new(IngredientName::new(ingredient.name), ingredient.quantity)
                })
                .collect(),
            self.images,
            self.steps
                .into_iter()
                .map(|step| RecipeStep::new(step.id, step.title, step.instruction, step.images))
                .collect(),
        )
    }
}

#[derive(Debug)]
pub(super) struct PartFormData {
    pub(super) row: usize,
    pub(super) id: RecipePartId,
    pub(super) title: String,
    pub(super) description: Option<String>,
    pub(super) body: BodyFormData,
}

#[derive(Debug)]
pub(super) enum StructureFormData {
    Simple(BodyFormData),
    Multipart(Vec<PartFormData>),
}

#[derive(Debug)]
pub(super) struct RecipeFormData {
    recipe_type: RecipeFormType,
    pub(super) title: String,
    pub(super) description: Option<String>,
    pub(super) servings: Option<Servings>,
    pub(super) category: Option<RecipeCategory>,
    pub(super) structure: StructureFormData,
    pub(super) message: Option<String>,
    pub(super) latest_revision: Option<String>,
}

impl RecipeFormData {
    #[cfg(test)]
    pub(super) fn simple_body(&self) -> &BodyFormData {
        match &self.structure {
            StructureFormData::Simple(body) => body,
            StructureFormData::Multipart(_) => panic!("expected a simple recipe form"),
        }
    }

    pub(super) fn into_content(self) -> Result<RecipeContent> {
        let description = self.description.map(RecipeDescription::new);
        let type_matches = matches!(
            (self.recipe_type, &self.structure),
            (RecipeFormType::Simple, StructureFormData::Simple(_))
                | (RecipeFormType::Multipart, StructureFormData::Multipart(_))
        );

        if !type_matches {
            return Err(invalid("the recipe type does not match its contents"));
        }

        let content = match self.structure {
            StructureFormData::Simple(body) => RecipeContent::new_simple(
                RecipeTitle::new(self.title),
                description,
                self.servings,
                self.category,
                body.into_body(),
            ),
            StructureFormData::Multipart(parts) => RecipeContent::new_multipart(
                RecipeTitle::new(self.title),
                description,
                self.servings,
                self.category,
                parts
                    .into_iter()
                    .map(|part| {
                        RecipePart::new(
                            part.id,
                            RecipePartTitle::new(part.title),
                            part.description.map(RecipeDescription::new),
                            part.body.into_body(),
                        )
                    })
                    .collect(),
            ),
        };

        content.map_err(|error| invalid(error.to_string()))
    }
}

#[derive(Debug)]
struct IngredientFormData {
    name: String,
    quantity: Quantity,
}

#[derive(Debug)]
pub(super) struct StepFormData {
    pub(super) row: usize,
    pub(super) id: RecipeStepId,
    pub(super) title: Option<String>,
    pub(super) instruction: String,
    pub(super) images: Vec<RecipeImageId>,
}

#[derive(Default)]
struct RawPart {
    fields: HashMap<PartField, String>,
    images: Vec<String>,
    ingredients: BTreeMap<usize, HashMap<IngredientField, String>>,
    steps: BTreeMap<usize, HashMap<StepField, String>>,
    step_images: BTreeMap<usize, Vec<String>>,
}

/// Parses and validates the URL-encoded form into its wire DTO.
pub(super) fn parse_recipe_form(pairs: &[(String, String)]) -> Result<RecipeFormData> {
    let mut title = None;
    let mut description = None;
    let mut category = None;
    let mut servings_type = None;
    let mut servings_count = None;
    let mut servings_label = None;
    let mut recipe_type = None;
    let mut preparation = None;
    let mut rest = None;
    let mut cooking = None;
    let mut message = None;
    let mut latest_revision = None;
    let mut images = Vec::new();
    let mut step_images: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let mut ingredients: BTreeMap<usize, HashMap<IngredientField, String>> = BTreeMap::new();
    let mut steps: BTreeMap<usize, HashMap<StepField, String>> = BTreeMap::new();
    let mut parts: BTreeMap<usize, RawPart> = BTreeMap::new();

    for (key, value) in pairs {
        match key.as_str() {
            IMAGE_IDS_FIELD => images.push(value.clone()),
            TITLE_FIELD => set_once(&mut title, value, key)?,
            DESCRIPTION_FIELD => set_once(&mut description, value, key)?,
            CATEGORY_FIELD => set_once(&mut category, value, key)?,
            RECIPE_TYPE_FIELD => set_once(&mut recipe_type, value, key)?,
            SERVINGS_TYPE_FIELD => set_once(&mut servings_type, value, key)?,
            SERVINGS_COUNT_FIELD => set_once(&mut servings_count, value, key)?,
            SERVINGS_LABEL_FIELD => set_once(&mut servings_label, value, key)?,
            PREPARATION_FIELD => set_once(&mut preparation, value, key)?,
            REST_FIELD => set_once(&mut rest, value, key)?,
            COOKING_FIELD => set_once(&mut cooking, value, key)?,
            MESSAGE_FIELD => set_once(&mut message, value, key)?,
            LATEST_REVISION_FIELD => set_once(&mut latest_revision, value, key)?,
            _ => {
                if let Some((part_index, rest)) = parse_part_key(key)? {
                    parse_part_field(parts.entry(part_index).or_default(), &rest, value, key)?;
                } else {
                    let step_image_ids_suffix = format!("[{IMAGE_IDS_FIELD}]");
                    if key.starts_with("steps[") && key.ends_with(&step_image_ids_suffix) {
                        if let Some((row, ())) = parse_indexed_key(key, "steps", |_| Some(()))? {
                            step_images.entry(row).or_default().push(value.clone());
                        }
                    } else if let Some((row, wire_field)) =
                        parse_indexed_key(key, "ingredients", IngredientField::parse)?
                    {
                        insert_row_value(&mut ingredients, row, wire_field, value, key)?;
                    } else if let Some((row, wire_field)) =
                        parse_indexed_key(key, "steps", StepField::parse)?
                    {
                        insert_row_value(&mut steps, row, wire_field, value, key)?;
                    } else {
                        return Err(invalid(format!("unknown form field `{key}`")));
                    }
                }
            }
        }
    }

    let title = title
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
        .ok_or_else(|| invalid("title is required"))?;

    let description = description
        .map(|description| description.trim().to_string())
        .filter(|description| !description.is_empty());
    let category = category
        .as_deref()
        .map(str::trim)
        .filter(|category| !category.is_empty())
        .map(|category| {
            RecipeCategory::parse(category)
                .ok_or_else(|| invalid(format!("unknown recipe category `{category}`")))
        })
        .transpose()?;
    let recipe_type = RecipeFormType::parse(recipe_type.as_deref().unwrap_or("simple"))?;

    let structure = match recipe_type {
        RecipeFormType::Simple => {
            if !parts.is_empty() {
                return Err(invalid("a simple recipe cannot contain parts"));
            }

            let mut seen_step_ids = HashSet::new();
            StructureFormData::Simple(parse_body_form_data(
                preparation,
                rest,
                cooking,
                ingredients,
                images,
                steps,
                step_images,
                &mut seen_step_ids,
            )?)
        }
        RecipeFormType::Multipart => {
            if preparation.is_some()
                || rest.is_some()
                || cooking.is_some()
                || !ingredients.is_empty()
                || !steps.is_empty()
                || !images.is_empty()
            {
                return Err(invalid(
                    "a multipart recipe cannot contain top-level body fields",
                ));
            }

            StructureFormData::Multipart(parse_part_rows(parts)?)
        }
    };

    Ok(RecipeFormData {
        recipe_type,
        title,
        description,
        servings: parse_servings(
            servings_type.as_deref(),
            servings_count.as_deref(),
            servings_label.as_deref(),
        )?,
        category,
        structure,
        message,
        latest_revision,
    })
}

#[allow(clippy::too_many_arguments)]
fn parse_body_form_data(
    preparation: Option<String>,
    rest: Option<String>,
    cooking: Option<String>,
    ingredients: BTreeMap<usize, HashMap<IngredientField, String>>,
    images: Vec<String>,
    steps: BTreeMap<usize, HashMap<StepField, String>>,
    step_images: BTreeMap<usize, Vec<String>>,
    seen_step_ids: &mut HashSet<RecipeStepId>,
) -> Result<BodyFormData> {
    Ok(BodyFormData {
        preparation: parse_optional_minutes(preparation.as_deref(), PREPARATION_FIELD)?,
        rest: parse_optional_minutes(rest.as_deref(), REST_FIELD)?,
        cooking: parse_optional_minutes(cooking.as_deref(), COOKING_FIELD)?,
        ingredients: parse_ingredient_rows(ingredients)?,
        images: parse_image_ids(images)?,
        steps: parse_step_rows(steps, step_images, seen_step_ids)?,
    })
}

fn parse_part_rows(rows: BTreeMap<usize, RawPart>) -> Result<Vec<PartFormData>> {
    let mut parts = Vec::new();
    let mut seen_part_ids = HashSet::new();
    let mut seen_step_ids = HashSet::new();

    for (row, mut raw) in rows {
        let id = raw.fields.remove(&PartField::Id).unwrap_or_default();
        let title = raw.fields.remove(&PartField::Title).unwrap_or_default();
        let description = raw
            .fields
            .remove(&PartField::Description)
            .unwrap_or_default();
        let preparation = raw.fields.remove(&PartField::Preparation);
        let rest = raw.fields.remove(&PartField::Rest);
        let cooking = raw.fields.remove(&PartField::Cooking);

        let title = title.trim().to_string();
        let description = description.trim().to_string();
        let id = id.trim().to_string();

        let is_empty = title.is_empty()
            && description.is_empty()
            && preparation.is_none()
            && rest.is_none()
            && cooking.is_none()
            && raw.ingredients.is_empty()
            && raw.steps.is_empty()
            && raw.images.is_empty();

        if is_empty {
            continue;
        }

        if title.is_empty() {
            return Err(invalid(format!("part row {} needs a title", row + 1)));
        }

        let id = if id.is_empty() {
            RecipePartId::generate()
        } else {
            RecipePartId::new(id)
        };

        if !seen_part_ids.insert(id.clone()) {
            return Err(invalid(format!(
                "part row {} reuses another part's identity",
                row + 1
            )));
        }

        parts.push(PartFormData {
            row,
            id,
            title,
            description: (!description.is_empty()).then_some(description),
            body: parse_body_form_data(
                preparation,
                rest,
                cooking,
                raw.ingredients,
                raw.images,
                raw.steps,
                raw.step_images,
                &mut seen_step_ids,
            )?,
        });
    }

    if parts.is_empty() {
        return Err(invalid("a multipart recipe needs at least one part"));
    }

    Ok(parts)
}

/// Parses `parts[<row>][...]` into a part row and the remaining nested key.
///
/// For example, `parts[0][ingredients][1][name]` becomes
/// `(0, "ingredients[1][name]")`.
pub(super) fn parse_part_key(key: &str) -> Result<Option<(usize, String)>> {
    let Some(rest) = key.strip_prefix("parts[") else {
        return Ok(None);
    };

    let Some((row, rest)) = rest.split_once("][") else {
        return Err(invalid(format!(
            "malformed form field `{key}`; expected `parts[row][field]`"
        )));
    };

    let row = row
        .parse::<usize>()
        .map_err(|_| invalid(format!("invalid row in form field `{key}`")))?;

    let rest = if rest.contains("][") {
        rest.replacen("][", "[", 1)
    } else {
        rest.strip_suffix(']')
            .ok_or_else(|| invalid(format!("malformed form field `{key}`")))?
            .to_string()
    };

    Ok(Some((row, rest)))
}

fn parse_part_field(part: &mut RawPart, part_field: &str, value: &str, key: &str) -> Result<()> {
    if part_field == IMAGE_IDS_FIELD {
        part.images.push(value.to_string());
        return Ok(());
    }

    let step_image_ids_suffix = format!("[{IMAGE_IDS_FIELD}]");
    if part_field.starts_with("steps[")
        && part_field.ends_with(&step_image_ids_suffix)
        && let Some((row, ())) = parse_indexed_key(part_field, "steps", |_| Some(()))?
    {
        part.step_images
            .entry(row)
            .or_default()
            .push(value.to_string());
        return Ok(());
    }

    if let Some((row, wire_field)) =
        parse_indexed_key(part_field, "ingredients", IngredientField::parse)?
    {
        return insert_row_value(&mut part.ingredients, row, wire_field, value, key);
    }

    if let Some((row, wire_field)) = parse_indexed_key(part_field, "steps", StepField::parse)? {
        return insert_row_value(&mut part.steps, row, wire_field, value, key);
    }

    if let Some(wire_field) = PartField::parse(part_field) {
        if part.fields.insert(wire_field, value.to_string()).is_some() {
            return Err(invalid(format!("duplicate form field `{key}`")));
        }

        return Ok(());
    }

    Err(invalid(format!("unknown form field `{key}`")))
}

fn set_once(target: &mut Option<String>, value: &str, key: &str) -> Result<()> {
    if target.replace(value.to_string()).is_some() {
        return Err(invalid(format!("duplicate form field `{key}`")));
    }

    Ok(())
}

/// Parses `collection[row][field]` form keys.
pub(super) fn parse_indexed_key<T>(
    key: &str,
    collection: &str,
    parse_field: fn(&str) -> Option<T>,
) -> Result<Option<(usize, T)>> {
    let Some(rest) = key
        .strip_prefix(collection)
        .and_then(|rest| rest.strip_prefix('['))
    else {
        return Ok(None);
    };

    let Some((row, wire_field)) = rest.split_once("][") else {
        return Err(invalid(format!(
            "malformed form field `{key}`; expected `{collection}[row][field]`"
        )));
    };

    let Some(wire_field) = wire_field.strip_suffix(']') else {
        return Err(invalid(format!(
            "malformed form field `{key}`; expected `{collection}[row][field]`"
        )));
    };

    let row = row
        .parse::<usize>()
        .map_err(|_| invalid(format!("invalid row in form field `{key}`")))?;
    let parsed_field =
        parse_field(wire_field).ok_or_else(|| invalid(format!("unknown form field `{key}`")))?;

    Ok(Some((row, parsed_field)))
}

fn insert_row_value<K>(
    rows: &mut BTreeMap<usize, HashMap<K, String>>,
    row: usize,
    wire_field: K,
    value: &str,
    key: &str,
) -> Result<()>
where
    K: Eq + std::hash::Hash,
{
    if rows
        .entry(row)
        .or_default()
        .insert(wire_field, value.to_string())
        .is_some()
    {
        return Err(invalid(format!("duplicate form field `{key}`")));
    }

    Ok(())
}

pub(super) fn invalid(message: impl Into<String>) -> Error {
    bad_request(message).into()
}

const MAX_SERVINGS_LABEL_LENGTH: usize = 40;

fn parse_servings(
    servings_type: Option<&str>,
    count: Option<&str>,
    label: Option<&str>,
) -> Result<Option<Servings>> {
    let servings_type = servings_type.map(str::trim).unwrap_or_default();
    let count = count.map(str::trim).unwrap_or_default();
    let label = label.map(str::trim).unwrap_or_default();

    match servings_type {
        "" | "none" => {
            if !count.is_empty() || !label.is_empty() {
                return Err(invalid(
                    "servings cannot have a count or label when they are not specified",
                ));
            }

            Ok(None)
        }
        "people" => {
            if !label.is_empty() {
                return Err(invalid("people servings cannot have a free-form label"));
            }

            Ok(Some(Servings::people(parse_servings_count(count)?)))
        }
        "portions" => {
            let count = parse_servings_count(count)?;

            if label.is_empty() {
                return Ok(Some(Servings::portions(count)));
            }

            if label.chars().count() > MAX_SERVINGS_LABEL_LENGTH {
                return Err(invalid(format!(
                    "a portions label cannot exceed {MAX_SERVINGS_LABEL_LENGTH} characters"
                )));
            }

            Ok(Some(Servings::portions_with_label(count, label)))
        }
        other => Err(invalid(format!("unknown servings type `{other}`"))),
    }
}

fn parse_servings_count(value: &str) -> Result<usize> {
    if value.is_empty() {
        return Err(invalid("a serving count is required"));
    }

    let count = value
        .parse::<usize>()
        .map_err(|_| invalid("a serving count must be a whole number"))?;

    if count == 0 {
        return Err(invalid("a serving count must be greater than zero"));
    }

    Ok(count)
}

fn parse_optional_minutes(value: Option<&str>, wire_field: &str) -> Result<Option<usize>> {
    match value.map(str::trim) {
        None | Some("") => Ok(None),
        Some(value) => value
            .parse::<usize>()
            .map(Some)
            .map_err(|_| invalid(format!("{wire_field} must be a whole number of minutes"))),
    }
}

fn parse_ingredient_rows(
    rows: BTreeMap<usize, HashMap<IngredientField, String>>,
) -> Result<Vec<IngredientFormData>> {
    let mut ingredients = Vec::new();

    for (row, mut values) in rows {
        let name = values.remove(&IngredientField::Name).unwrap_or_default();
        let amount_type = values
            .remove(&IngredientField::AmountType)
            .unwrap_or_default();
        let quantity = values
            .remove(&IngredientField::Quantity)
            .unwrap_or_default();
        let quantity_text = values
            .remove(&IngredientField::QuantityText)
            .unwrap_or_default();
        let unit = values.remove(&IngredientField::Unit).unwrap_or_default();
        let unit_other = values
            .remove(&IngredientField::UnitOther)
            .unwrap_or_default();

        let name = name.trim();
        let amount_type = amount_type.trim();
        let quantity = quantity.trim();
        let quantity_text = quantity_text.trim();

        if name.is_empty() && quantity.is_empty() && quantity_text.is_empty() {
            continue;
        }

        if name.is_empty() {
            return Err(invalid(format!("ingredient row {} needs a name", row + 1)));
        }

        let quantity = match amount_type {
            "" | "measured" => {
                if quantity.is_empty() {
                    return Err(invalid(format!("quantity is required for `{name}`")));
                }

                let quantity = quantity
                    .parse::<f32>()
                    .map_err(|_| invalid(format!("quantity for `{name}` must be a number")))?;

                if !quantity.is_finite() || quantity < 0.0 {
                    return Err(invalid(format!(
                        "quantity for `{name}` must be zero or greater"
                    )));
                }

                let unit = parse_unit(&unit, &unit_other)?;

                Quantity::from_measure(quantity, unit)
            }
            "text" => {
                if quantity_text.is_empty() {
                    return Err(invalid(format!(
                        "a free-form amount is required for `{name}`"
                    )));
                }

                Quantity::from_text(quantity_text)
            }
            other => {
                return Err(invalid(format!("unknown ingredient amount type `{other}`")));
            }
        };

        ingredients.push(IngredientFormData {
            name: name.to_string(),
            quantity,
        });
    }

    Ok(ingredients)
}

const MAX_CUSTOM_UNIT_LENGTH: usize = 40;

fn parse_unit(value: &str, unit_other: &str) -> Result<Unit> {
    match value.trim() {
        "gram" => Ok(Unit::Gram),
        "kilogram" => Ok(Unit::Kilogram),
        "millilitre" => Ok(Unit::MilliLitre),
        "centilitre" => Ok(Unit::CentiLitre),
        "litre" => Ok(Unit::Litre),
        "piece" => Ok(Unit::Piece),
        "teaspoon" => Ok(Unit::Teaspoon),
        "tablespoon" => Ok(Unit::Tablespoon),
        "other" => {
            let label = unit_other.trim();

            if label.is_empty() {
                return Err(invalid("a custom ingredient unit needs a label"));
            }

            if label.chars().count() > MAX_CUSTOM_UNIT_LENGTH {
                return Err(invalid(format!(
                    "a custom ingredient unit cannot exceed {MAX_CUSTOM_UNIT_LENGTH} characters"
                )));
            }

            Ok(Unit::Other(label.to_string()))
        }
        "" => Err(invalid("an ingredient unit is required")),
        other => Err(invalid(format!("unknown ingredient unit `{other}`"))),
    }
}

fn parse_image_ids(values: Vec<String>) -> Result<Vec<RecipeImageId>> {
    let mut seen = HashSet::new();
    let mut ids = Vec::with_capacity(values.len());

    for value in values {
        let value = value.trim();

        if value.is_empty() {
            return Err(invalid("an image id cannot be empty"));
        }

        let id = RecipeImageId::new(value.to_string());

        if !seen.insert(id.clone()) {
            return Err(invalid(format!("duplicate image id `{value}`")));
        }

        ids.push(id);
    }

    Ok(ids)
}

fn parse_step_rows(
    rows: BTreeMap<usize, HashMap<StepField, String>>,
    mut image_ids: BTreeMap<usize, Vec<String>>,
    seen_ids: &mut HashSet<RecipeStepId>,
) -> Result<Vec<StepFormData>> {
    let mut steps = Vec::new();

    for (row, mut fields) in rows {
        let id = fields.remove(&StepField::Id).unwrap_or_default();
        let id = id.trim().to_string();
        let title = fields.remove(&StepField::Title).unwrap_or_default();
        let instruction = fields.remove(&StepField::Instruction).unwrap_or_default();

        let title = title.trim();
        let instruction = instruction.trim();

        if title.is_empty() && instruction.is_empty() {
            continue;
        }

        if instruction.is_empty() {
            return Err(invalid(format!(
                "step row {} needs an instruction",
                row + 1
            )));
        }

        let id = if id.is_empty() {
            RecipeStepId::generate()
        } else {
            RecipeStepId::new(id)
        };

        if !seen_ids.insert(id.clone()) {
            return Err(invalid(format!(
                "step row {} reuses another step's identity",
                row + 1
            )));
        }

        steps.push(StepFormData {
            row,
            id,
            title: (!title.is_empty()).then(|| title.to_string()),
            instruction: instruction.to_string(),
            images: parse_image_ids(image_ids.remove(&row).unwrap_or_default())?,
        });
    }

    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(entries: &[(&str, &str)]) -> Vec<(String, String)> {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn parses_every_recipe_field() {
        let form = parse_recipe_form(&pairs(&[
            ("title", "Soup"),
            ("description", "A warming soup."),
            ("preparation", "10"),
            ("rest", "20"),
            ("cooking", "30"),
            ("ingredients[0][name]", "Flour"),
            ("ingredients[0][quantity]", "200"),
            ("ingredients[0][unit]", "gram"),
            ("ingredients[1][name]", "Milk"),
            ("ingredients[1][quantity]", "0.5"),
            ("ingredients[1][unit]", "litre"),
            ("steps[0][title]", "Mix"),
            ("steps[0][instruction]", "Stir everything together."),
        ]))
        .unwrap();

        assert_eq!(form.title, "Soup");
        assert_eq!(form.description.as_deref(), Some("A warming soup."));
        assert_eq!(form.simple_body().preparation, Some(10));
        assert_eq!(form.simple_body().rest, Some(20));
        assert_eq!(form.simple_body().cooking, Some(30));
        assert_eq!(form.message, None);
        assert_eq!(form.latest_revision, None);
        assert_eq!(form.simple_body().ingredients.len(), 2);
        assert_eq!(form.simple_body().ingredients[0].name, "Flour");
        assert_eq!(
            form.simple_body().ingredients[0].quantity,
            Quantity::from_measure(200.0, Unit::Gram)
        );
        assert_eq!(form.simple_body().ingredients[1].name, "Milk");
        assert_eq!(
            form.simple_body().ingredients[1].quantity,
            Quantity::from_measure(0.5, Unit::Litre)
        );
        assert_eq!(form.simple_body().steps.len(), 1);
        assert_eq!(form.simple_body().steps[0].title.as_deref(), Some("Mix"));
        assert_eq!(
            form.simple_body().steps[0].instruction,
            "Stir everything together."
        );
    }

    #[test]
    fn parses_people_servings() {
        let form = parse_recipe_form(&pairs(&[
            ("title", "Soup"),
            ("servings_type", "people"),
            ("servings_count", "4"),
        ]))
        .unwrap();

        assert_eq!(form.servings, Some(Servings::people(4)));
    }

    #[test]
    fn parses_portion_servings() {
        let form = parse_recipe_form(&pairs(&[
            ("title", "Cookies"),
            ("servings_type", "portions"),
            ("servings_count", "12"),
            ("servings_label", "  cookies  "),
        ]))
        .unwrap();

        assert_eq!(
            form.servings,
            Some(Servings::portions_with_label(12, "cookies"))
        );
    }

    #[test]
    fn parses_portion_servings_without_a_label() {
        let form = parse_recipe_form(&pairs(&[
            ("title", "Cookies"),
            ("servings_type", "portions"),
            ("servings_count", "12"),
            ("servings_label", "   "),
        ]))
        .unwrap();

        assert_eq!(form.servings, Some(Servings::portions(12)));
    }

    #[test]
    fn defaults_to_no_servings() {
        let form = parse_recipe_form(&pairs(&[("title", "Soup")])).unwrap();

        assert_eq!(form.servings, None);
    }

    #[test]
    fn parses_an_optional_category() {
        let with_category =
            parse_recipe_form(&pairs(&[("title", "Soup"), ("category", "main")])).unwrap();
        let without_category =
            parse_recipe_form(&pairs(&[("title", "Soup"), ("category", "   ")])).unwrap();

        assert_eq!(with_category.category, Some(RecipeCategory::Main));
        assert_eq!(without_category.category, None);
    }

    #[test]
    fn rejects_an_unknown_category() {
        assert!(parse_recipe_form(&pairs(&[("title", "Soup"), ("category", "brunch")])).is_err());
    }

    #[test]
    fn rejects_servings_with_a_missing_or_zero_count() {
        for entries in [
            vec![("title", "Soup"), ("servings_type", "people")],
            vec![
                ("title", "Soup"),
                ("servings_type", "people"),
                ("servings_count", "0"),
            ],
            vec![("title", "Soup"), ("servings_type", "unknown")],
        ] {
            assert!(
                parse_recipe_form(&pairs(&entries)).is_err(),
                "expected an error for {entries:?}"
            );
        }
    }

    #[test]
    fn rejects_servings_values_when_servings_are_not_specified() {
        assert!(
            parse_recipe_form(&pairs(&[
                ("title", "Soup"),
                ("servings_type", "none"),
                ("servings_count", "4"),
            ]))
            .is_err()
        );
    }

    #[test]
    fn parses_measured_amounts_with_every_built_in_unit() {
        let units = [
            ("gram", Unit::Gram),
            ("kilogram", Unit::Kilogram),
            ("millilitre", Unit::MilliLitre),
            ("centilitre", Unit::CentiLitre),
            ("litre", Unit::Litre),
            ("piece", Unit::Piece),
            ("teaspoon", Unit::Teaspoon),
            ("tablespoon", Unit::Tablespoon),
        ];

        for (index, (wire, expected)) in units.into_iter().enumerate() {
            let form = parse_recipe_form(&pairs(&[
                ("title", "Soup"),
                ("ingredients[0][amount_type]", "measured"),
                ("ingredients[0][name]", "Ingredient"),
                ("ingredients[0][quantity]", "2"),
                ("ingredients[0][unit]", wire),
            ]))
            .unwrap();

            assert_eq!(
                form.simple_body().ingredients[0].quantity,
                Quantity::from_measure(2.0, expected),
                "unit row {index}"
            );
        }
    }

    #[test]
    fn parses_free_text_amounts() {
        let form = parse_recipe_form(&pairs(&[
            ("title", "Soup"),
            ("ingredients[0][amount_type]", "text"),
            ("ingredients[0][name]", "Salt"),
            ("ingredients[0][quantity_text]", "  to taste  "),
        ]))
        .unwrap();

        assert_eq!(form.simple_body().ingredients[0].name, "Salt");
        assert_eq!(
            form.simple_body().ingredients[0].quantity,
            Quantity::from_text("to taste")
        );
    }

    #[test]
    fn parses_custom_units() {
        let form = parse_recipe_form(&pairs(&[
            ("title", "Soup"),
            ("ingredients[0][amount_type]", "measured"),
            ("ingredients[0][name]", "Yeast"),
            ("ingredients[0][quantity]", "1"),
            ("ingredients[0][unit]", "other"),
            ("ingredients[0][unit_other]", "  sachet  "),
        ]))
        .unwrap();

        assert_eq!(
            form.simple_body().ingredients[0].quantity,
            Quantity::from_measure(1.0, Unit::Other("sachet".to_string()))
        );
    }

    #[test]
    fn defaults_a_missing_amount_type_to_measured() {
        let form = parse_recipe_form(&pairs(&[
            ("title", "Soup"),
            ("ingredients[0][name]", "Flour"),
            ("ingredients[0][quantity]", "200"),
            ("ingredients[0][unit]", "gram"),
        ]))
        .unwrap();

        assert_eq!(
            form.simple_body().ingredients[0].quantity,
            Quantity::from_measure(200.0, Unit::Gram)
        );
    }

    #[test]
    fn rejects_a_missing_free_text_amount() {
        assert!(
            parse_recipe_form(&pairs(&[
                ("title", "Soup"),
                ("ingredients[0][amount_type]", "text"),
                ("ingredients[0][name]", "Salt"),
                ("ingredients[0][quantity_text]", "   "),
            ]))
            .is_err()
        );
    }

    #[test]
    fn rejects_a_missing_custom_unit_label() {
        assert!(
            parse_recipe_form(&pairs(&[
                ("title", "Soup"),
                ("ingredients[0][amount_type]", "measured"),
                ("ingredients[0][name]", "Yeast"),
                ("ingredients[0][quantity]", "1"),
                ("ingredients[0][unit]", "other"),
                ("ingredients[0][unit_other]", "   "),
            ]))
            .is_err()
        );
    }

    #[test]
    fn parses_out_of_order_keys_and_sparse_rows_in_order() {
        let form = parse_recipe_form(&pairs(&[
            ("title", "Soup"),
            ("steps[5][instruction]", "Second step"),
            ("steps[2][instruction]", "First step"),
            ("ingredients[2][name]", "Milk"),
            ("ingredients[2][quantity]", "0.5"),
            ("ingredients[2][unit]", "litre"),
            ("ingredients[0][unit]", "gram"),
            ("ingredients[0][quantity]", "200"),
            ("ingredients[0][name]", "Flour"),
        ]))
        .unwrap();

        assert_eq!(form.simple_body().ingredients.len(), 2);
        assert_eq!(form.simple_body().ingredients[0].name, "Flour");
        assert_eq!(form.simple_body().ingredients[1].name, "Milk");
        assert_eq!(form.simple_body().steps.len(), 2);
        assert_eq!(form.simple_body().steps[0].instruction, "First step");
        assert_eq!(form.simple_body().steps[1].instruction, "Second step");
    }

    #[test]
    fn generated_keys_round_trip_through_the_parser() {
        for wire_field in [
            IngredientField::Name,
            IngredientField::AmountType,
            IngredientField::Quantity,
            IngredientField::QuantityText,
            IngredientField::Unit,
            IngredientField::UnitOther,
        ] {
            assert_eq!(
                parse_indexed_key(
                    &wire_field.key(7, ""),
                    "ingredients",
                    IngredientField::parse,
                )
                .unwrap(),
                Some((7, wire_field))
            );
        }

        for wire_field in [StepField::Title, StepField::Instruction] {
            assert_eq!(
                parse_indexed_key(&wire_field.key(7, ""), "steps", StepField::parse).unwrap(),
                Some((7, wire_field))
            );
        }
    }

    #[test]
    fn skips_blank_ingredient_and_step_rows() {
        let form = parse_recipe_form(&pairs(&[
            ("title", "Soup"),
            ("ingredients[0][name]", ""),
            ("ingredients[0][quantity]", ""),
            ("ingredients[0][unit]", "gram"),
            ("steps[0][title]", ""),
            ("steps[0][instruction]", ""),
        ]))
        .unwrap();

        assert!(form.simple_body().ingredients.is_empty());
        assert!(form.simple_body().steps.is_empty());
    }

    #[test]
    fn requires_a_title() {
        assert!(parse_recipe_form(&pairs(&[("title", "  ")])).is_err());
    }

    #[test]
    fn treats_a_blank_description_as_missing() {
        let form = parse_recipe_form(&pairs(&[("title", "Soup"), ("description", "   ")])).unwrap();

        assert_eq!(form.description, None);
    }

    #[test]
    fn rejects_an_ingredient_without_a_quantity() {
        assert!(
            parse_recipe_form(&pairs(&[
                ("title", "Soup"),
                ("ingredients[0][name]", "Flour"),
                ("ingredients[0][quantity]", ""),
                ("ingredients[0][unit]", "gram"),
            ]))
            .is_err()
        );
    }

    #[test]
    fn rejects_a_step_without_an_instruction() {
        assert!(
            parse_recipe_form(&pairs(&[
                ("title", "Soup"),
                ("steps[0][title]", "Mix"),
                ("steps[0][instruction]", ""),
            ]))
            .is_err()
        );
    }

    #[test]
    fn rejects_unknown_and_malformed_fields() {
        assert!(parse_recipe_form(&pairs(&[("title", "Soup"), ("mystery", "x")])).is_err());
        assert!(
            parse_recipe_form(&pairs(&[
                ("title", "Soup"),
                ("ingredients[0][colour]", "red"),
            ]))
            .is_err()
        );
        assert!(
            parse_recipe_form(&pairs(&[
                ("title", "Soup"),
                ("ingredients[zero][name]", "Flour"),
            ]))
            .is_err()
        );
    }

    #[test]
    fn rejects_duplicate_fields() {
        assert!(parse_recipe_form(&pairs(&[("title", "Soup"), ("title", "Stew")])).is_err());
        assert!(
            parse_recipe_form(&pairs(&[
                ("title", "Soup"),
                ("ingredients[0][name]", "Flour"),
                ("ingredients[0][name]", "Sugar"),
            ]))
            .is_err()
        );
    }

    #[test]
    fn parses_recipe_and_step_image_ids() {
        let form = parse_recipe_form(&pairs(&[
            ("title", "Soup"),
            ("image_ids", "image-1"),
            ("image_ids", "image-2"),
            ("steps[0][id]", "step-1"),
            ("steps[0][instruction]", "Stir."),
            ("steps[0][image_ids]", "step-image-1"),
            ("steps[0][image_ids]", "step-image-2"),
        ]))
        .unwrap();

        assert_eq!(form.simple_body().images.len(), 2);
        assert_eq!(form.simple_body().images[0].as_str(), "image-1");
        assert_eq!(form.simple_body().images[1].as_str(), "image-2");
        assert_eq!(form.simple_body().steps[0].images.len(), 2);
        assert_eq!(
            form.simple_body().steps[0].images[0].as_str(),
            "step-image-1"
        );
        assert_eq!(
            form.simple_body().steps[0].images[1].as_str(),
            "step-image-2"
        );
    }

    #[test]
    fn allows_the_same_image_reference_in_multiple_steps() {
        let form = parse_recipe_form(&pairs(&[
            ("title", "Soup"),
            ("steps[0][id]", "step-1"),
            ("steps[0][instruction]", "Stir."),
            ("steps[0][image_ids]", "shared-image"),
            ("steps[1][id]", "step-2"),
            ("steps[1][instruction]", "Bake."),
            ("steps[1][image_ids]", "shared-image"),
        ]))
        .unwrap();

        assert_eq!(
            form.simple_body().steps[0].images[0].as_str(),
            "shared-image"
        );
        assert_eq!(
            form.simple_body().steps[1].images[0].as_str(),
            "shared-image"
        );
    }

    #[test]
    fn rejects_duplicate_image_ids() {
        assert!(
            parse_recipe_form(&pairs(&[
                ("title", "Soup"),
                ("image_ids", "image-1"),
                ("image_ids", "image-1"),
            ]))
            .is_err()
        );
    }

    #[test]
    fn parses_edit_message_and_latest_revision() {
        let form = parse_recipe_form(&pairs(&[
            ("title", "Soup"),
            ("message", "  reduced the salt  "),
            ("latest_revision", "revision-1"),
        ]))
        .unwrap();

        assert_eq!(form.message.as_deref(), Some("  reduced the salt  "));
        assert_eq!(form.latest_revision.as_deref(), Some("revision-1"));
    }

    #[test]
    fn preserves_submitted_step_ids_and_generates_missing_ids() {
        let form = parse_recipe_form(&pairs(&[
            ("title", "Soup"),
            ("steps[0][id]", "step-existing"),
            ("steps[0][instruction]", "Stir."),
            ("steps[1][instruction]", "Bake."),
        ]))
        .unwrap();

        assert_eq!(form.simple_body().steps[0].id.as_str(), "step-existing");
        assert_ne!(form.simple_body().steps[1].id.as_str(), "");
        assert_ne!(
            form.simple_body().steps[0].id,
            form.simple_body().steps[1].id
        );
    }

    #[test]
    fn rejects_duplicate_step_ids() {
        assert!(
            parse_recipe_form(&pairs(&[
                ("title", "Soup"),
                ("steps[0][id]", "step-1"),
                ("steps[0][instruction]", "Stir."),
                ("steps[1][id]", "step-1"),
                ("steps[1][instruction]", "Bake."),
            ]))
            .is_err()
        );
    }

    #[test]
    fn parses_multipart_recipe_parts() {
        let form = parse_recipe_form(&pairs(&[
            ("recipe_type", "multipart"),
            ("title", "Cake"),
            ("parts[0][title]", "Sponge"),
            ("parts[0][description]", "Light and airy."),
            ("parts[0][preparation]", "10"),
            ("parts[0][ingredients][0][name]", "Flour"),
            ("parts[0][ingredients][0][quantity]", "200"),
            ("parts[0][ingredients][0][unit]", "gram"),
            ("parts[0][steps][0][instruction]", "Mix."),
            ("parts[0][steps][0][image_ids]", "step-image"),
            ("parts[1][title]", "Filling"),
            ("parts[1][cooking]", "5"),
            ("parts[1][steps][0][instruction]", "Whip."),
        ]))
        .unwrap();

        assert_eq!(form.recipe_type, RecipeFormType::Multipart);

        let StructureFormData::Multipart(parts) = &form.structure else {
            panic!("expected a multipart form");
        };

        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].title, "Sponge");
        assert_eq!(parts[0].description.as_deref(), Some("Light and airy."));
        assert_eq!(parts[0].body.preparation, Some(10));
        assert_eq!(parts[0].body.ingredients.len(), 1);
        assert_eq!(parts[0].body.steps[0].instruction, "Mix.");
        assert_eq!(parts[0].body.steps[0].images[0].as_str(), "step-image");
        assert_eq!(parts[1].title, "Filling");
        assert_eq!(parts[1].body.cooking, Some(5));
        assert_ne!(parts[0].id, parts[1].id);
    }

    #[test]
    fn multipart_parts_need_titles() {
        let result = parse_recipe_form(&pairs(&[
            ("recipe_type", "multipart"),
            ("title", "Cake"),
            ("parts[0][ingredients][0][name]", "Flour"),
            ("parts[0][ingredients][0][quantity]", "200"),
            ("parts[0][ingredients][0][unit]", "gram"),
        ]));

        assert!(result.is_err());
    }

    #[test]
    fn rejects_parts_on_a_simple_recipe() {
        let result = parse_recipe_form(&pairs(&[
            ("recipe_type", "simple"),
            ("title", "Soup"),
            ("parts[0][title]", "Broth"),
        ]));

        assert!(result.is_err());
    }
}
