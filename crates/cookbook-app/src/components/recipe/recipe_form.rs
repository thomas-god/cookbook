use std::collections::{BTreeMap, HashMap};

use cookbook_domain::recipe::{
    models::{
        DurationMinutes, IngredientName, Quantity, RecipeDescription, RecipeMetadata, RecipeStep,
        RecipeTitle, Unit,
    },
    ports::CreateRecipeRequest,
};
use cookbook_domain::user::models::UserId;
use topcoat::{
    Error, Result,
    context::Cx,
    router::error::bad_request,
    runtime::{Event, shard, signal},
    view::{View, attributes, component, element_id, view},
};

use crate::{
    components::base::{
        button::{ButtonSize, ButtonVariant, button},
        field::{field, field_label, field_legend, field_set},
        input::input,
    },
    t,
};

const TEXTAREA_CLASS: &str = "min-h-24 w-full rounded-lg border border-border bg-transparent px-3 py-2 text-sm transition-colors outline-none placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background";

const SELECT_CLASS: &str = "h-9 w-full rounded-lg border border-border bg-transparent px-3 text-sm transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background";

const TITLE_FIELD: &str = "title";
const DESCRIPTION_FIELD: &str = "description";
const PREPARATION_FIELD: &str = "preparation";
const REST_FIELD: &str = "rest";
const COOKING_FIELD: &str = "cooking";

/// The fields of one ingredient row on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum IngredientField {
    Name,
    Quantity,
    Unit,
}

impl IngredientField {
    const fn wire_name(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Quantity => "quantity",
            Self::Unit => "unit",
        }
    }

    fn key(self, row: usize) -> String {
        format!("ingredients[{row}][{}]", self.wire_name())
    }

    fn parse(wire_field: &str) -> Option<Self> {
        match wire_field {
            "name" => Some(Self::Name),
            "quantity" => Some(Self::Quantity),
            "unit" => Some(Self::Unit),
            _ => None,
        }
    }
}

/// The fields of one step row on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum StepField {
    Title,
    Instruction,
    Ingredients,
}

impl StepField {
    const fn wire_name(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Instruction => "instruction",
            Self::Ingredients => "ingredients",
        }
    }

    fn key(self, row: usize) -> String {
        format!("steps[{row}][{}]", self.wire_name())
    }

    fn parse(wire_field: &str) -> Option<Self> {
        match wire_field {
            "title" => Some(Self::Title),
            "instruction" => Some(Self::Instruction),
            "ingredients" => Some(Self::Ingredients),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct RecipeFormData {
    title: String,
    description: Option<String>,
    preparation: Option<usize>,
    rest: Option<usize>,
    cooking: Option<usize>,
    ingredients: Vec<IngredientFormData>,
    steps: Vec<StepFormData>,
}

#[derive(Debug)]
struct IngredientFormData {
    name: String,
    quantity: f32,
    unit: Unit,
}

#[derive(Debug)]
struct StepFormData {
    title: Option<String>,
    instruction: String,
    ingredients: Option<HashMap<String, f32>>,
}

/// A form that creates a recipe with every field from the recipe domain model.
///
/// The form is a shard, so adding or removing an ingredient or step re-renders
/// only the form. Its signals keep the current values while the form changes.
#[shard]
pub async fn recipe_form(cx: &Cx) -> Result<impl View> {
    let title = signal(cx, String::new);
    let description = signal(cx, String::new);
    let preparation = signal(cx, String::new);
    let rest = signal(cx, String::new);
    let cooking = signal(cx, String::new);
    let ingredient_count = signal(cx, || 1usize);
    let step_count = signal(cx, || 1usize);

    let title_id = element_id(cx);
    let description_id = element_id(cx);
    let preparation_id = element_id(cx);
    let rest_id = element_id(cx);
    let cooking_id = element_id(cx);

    Ok(view! {
        <form method="post" action="/recipe/new" class="flex flex-col gap-6">
            field_set(
                field(
                    input(
                        attrs: attributes! {
                            id=(&title_id)
                            name=(TITLE_FIELD)
                            type="text"
                            placeholder=(t!(form_title.placeholder))
                            required=""
                            :value=$(title.get())
                            @input=$(|e: Event| title.set(e.target.value))
                        }
                    )
                )
                field(
                    field_label(
                        attrs: attributes! { for=(&description_id) },
                        (t!(form_description))
                    )
                    <textarea
                        id=(&description_id)
                        name=(DESCRIPTION_FIELD)
                        rows="2"
                        class=(TEXTAREA_CLASS)
                        placeholder=(t!(form_description.placeholder))
                        :value=$(description.get())
                        @input=$(|e: Event| description.set(e.target.value))
                    ></textarea>
                )
                <div class="grid gap-4 sm:grid-cols-3">
                    field(
                        field_label(
                            attrs: attributes! { for=(&preparation_id) },
                            (t!(form_preparation))
                        )
                        input(
                            attrs: attributes! {
                                id=(&preparation_id)
                                name=(PREPARATION_FIELD)
                                type="number"
                                min="0"
                                step="1"
                                placeholder="15"
                                :value=$(preparation.get())
                                @input=$(|e: Event| preparation.set(e.target.value))
                            }
                        )
                    )
                    field(
                        field_label(
                            attrs: attributes! { for=(&rest_id) },
                            (t!(form_rest))
                        )
                        input(
                            attrs: attributes! {
                                id=(&rest_id)
                                name=(REST_FIELD)
                                type="number"
                                min="0"
                                step="1"
                                placeholder="30"
                                :value=$(rest.get())
                                @input=$(|e: Event| rest.set(e.target.value))
                            }
                        )
                    )
                    field(
                        field_label(
                            attrs: attributes! { for=(&cooking_id) },
                            (t!(form_cooking))
                        )
                        input(
                            attrs: attributes! {
                                id=(&cooking_id)
                                name=(COOKING_FIELD)
                                type="number"
                                min="0"
                                step="1"
                                placeholder="45"
                                :value=$(cooking.get())
                                @input=$(|e: Event| cooking.set(e.target.value))
                            }
                        )
                    )
                </div>
            )

            field_set(
                field_legend((t!(form_ingredients)))
                <div id="ingredients" class="flex flex-col gap-3">
                    #[key(index)]
                    for index in 0..ingredient_count.get() {
                        ingredient_row(row: index)
                    }
                </div>
                <div class="flex flex-wrap gap-2">
                    button(
                        variant: ButtonVariant::Outline,
                        size: ButtonSize::Sm,
                        attrs: attributes! {
                            type="button"
                            @click=$(|_e: Event| ingredient_count.increment())
                        },
                        (t!(form_add_ingredient))
                    )
                </div>
            )

            field_set(
                field_legend((t!(form_steps)))
                <div id="steps" class="flex flex-col gap-4">
                    #[key(index)]
                    for index in 0..step_count.get() {
                        step_row(row: index)
                    }
                </div>
                <div class="flex flex-wrap gap-2">
                    button(
                        variant: ButtonVariant::Outline,
                        size: ButtonSize::Sm,
                        attrs: attributes! {
                            type="button"
                            @click=$(|_e: Event| step_count.increment())
                        },
                        (t!(form_add_step))
                    )
                </div>
            )

            <div class="flex justify-end">
                button(attrs: attributes! { type="submit" }, (t!(form_add_recipe)))
            </div>
        </form>
    })
}

#[component]
async fn ingredient_row(cx: &Cx, row: usize) -> Result<impl View> {
    let name = signal(cx, String::new);
    let quantity = signal(cx, String::new);
    let unit = signal(cx, || "gram".to_owned());
    let removed = signal(cx, || false);

    let name_id = element_id(cx);
    let quantity_id = element_id(cx);
    let unit_id = element_id(cx);

    let name_field = IngredientField::Name.key(row);
    let quantity_field = IngredientField::Quantity.key(row);
    let unit_field = IngredientField::Unit.key(row);

    Ok(view! {
        <fieldset
            :hidden=$(removed.get())
            :disabled=$(removed.get())
            class="m-0 grid min-w-0 gap-3 rounded-lg border border-border p-4 sm:grid-cols-[minmax(0,1fr)_8rem_10rem_auto] sm:items-end"
        >
            field(
                field_label(
                    attrs: attributes! { for=(&name_id) },
                    (t!(form_ingredient))
                )
                input(
                    attrs: attributes! {
                        id=(&name_id)
                        name=(name_field)
                        type="text"
                        placeholder=(t!(form_ingredient.placeholder))
                        :value=$(name.get())
                        @input=$(|e: Event| name.set(e.target.value))
                    }
                )
            )
            field(
                field_label(
                    attrs: attributes! { for=(&quantity_id) },
                    (t!(form_quantity))
                )
                input(
                    attrs: attributes! {
                        id=(&quantity_id)
                        name=(quantity_field)
                        type="number"
                        min="0"
                        step="any"
                        placeholder=(t!(form_quantity.placeholder))
                        :value=$(quantity.get())
                        @input=$(|e: Event| quantity.set(e.target.value))
                    }
                )
            )
            field(
                field_label(attrs: attributes! { for=(&unit_id) }, (t!(form_unit)))
                <select
                    id=(&unit_id)
                    name=(unit_field)
                    class=(SELECT_CLASS)
                    :value=$(unit.get())
                    @change=$(|e: Event| unit.set(e.target.value))
                >
                    <option value="gram">(t!(unit_gram))</option>
                    <option value="millilitre">(t!(unit_millilitre))</option>
                    <option value="litre">(t!(unit_litre))</option>
                </select>
            )
            <div class="flex items-end">
                button(
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Sm,
                    attrs: attributes! { type="button" @click=$(|_e: Event| removed.set(true)) },
                    (t!(form_remove))
                )
            </div>
        </fieldset>
    })
}

#[component]
async fn step_row(cx: &Cx, row: usize) -> Result<impl View> {
    let title = signal(cx, String::new);
    let instruction = signal(cx, String::new);
    let ingredients = signal(cx, String::new);
    let removed = signal(cx, || false);

    let title_id = element_id(cx);
    let instruction_id = element_id(cx);
    let ingredients_id = element_id(cx);

    let title_field = StepField::Title.key(row);
    let instruction_field = StepField::Instruction.key(row);
    let ingredients_field = StepField::Ingredients.key(row);

    Ok(view! {
        <fieldset
            :hidden=$(removed.get())
            :disabled=$(removed.get())
            class="m-0 flex min-w-0 flex-col gap-3 rounded-lg border border-border p-4"
        >
            field(
                field_label(
                    attrs: attributes! { for=(&title_id) },
                    (t!(form_step_title))
                )
                input(
                    attrs: attributes! {
                        id=(&title_id)
                        name=(title_field)
                        type="text"
                        placeholder=(t!(form_step_title.placeholder))
                        :value=$(title.get())
                        @input=$(|e: Event| title.set(e.target.value))
                    }
                )
            )
            field(
                field_label(
                    attrs: attributes! { for=(&instruction_id) },
                    (t!(form_instruction))
                )
                <textarea
                    id=(&instruction_id)
                    name=(instruction_field)
                    rows="3"
                    class=(TEXTAREA_CLASS)
                    placeholder=(t!(form_instruction.placeholder))
                    required=""
                    :value=$(instruction.get())
                    @input=$(|e: Event| instruction.set(e.target.value))
                ></textarea>
            )
            field(
                field_label(
                    attrs: attributes! { for=(&ingredients_id) },
                    (t!(form_step_ingredients))
                )
                <textarea
                    id=(&ingredients_id)
                    name=(ingredients_field)
                    rows="2"
                    class=(TEXTAREA_CLASS)
                    placeholder=(t!(form_step_ingredients.placeholder))
                    :value=$(ingredients.get())
                    @input=$(|e: Event| ingredients.set(e.target.value))
                ></textarea>
            )
            <div class="flex justify-end">
                button(
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Sm,
                    attrs: attributes! { type="button" @click=$(|_e: Event| removed.set(true)) },
                    (t!(form_remove))
                )
            </div>
        </fieldset>
    })
}

/// Parses and validates the URL-encoded form into its wire DTO.
fn parse_recipe_form(pairs: &[(String, String)]) -> Result<RecipeFormData> {
    let mut title = None;
    let mut description = None;
    let mut preparation = None;
    let mut rest = None;
    let mut cooking = None;
    let mut ingredients: BTreeMap<usize, HashMap<IngredientField, String>> = BTreeMap::new();
    let mut steps: BTreeMap<usize, HashMap<StepField, String>> = BTreeMap::new();

    for (key, value) in pairs {
        match key.as_str() {
            TITLE_FIELD => set_once(&mut title, value, key)?,
            DESCRIPTION_FIELD => set_once(&mut description, value, key)?,
            PREPARATION_FIELD => set_once(&mut preparation, value, key)?,
            REST_FIELD => set_once(&mut rest, value, key)?,
            COOKING_FIELD => set_once(&mut cooking, value, key)?,
            _ => {
                if let Some((row, wire_field)) =
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

    let title = title
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
        .ok_or_else(|| invalid("title is required"))?;

    let description = description
        .map(|description| description.trim().to_string())
        .filter(|description| !description.is_empty());

    Ok(RecipeFormData {
        title,
        description,
        preparation: parse_optional_minutes(preparation.as_deref(), PREPARATION_FIELD)?,
        rest: parse_optional_minutes(rest.as_deref(), REST_FIELD)?,
        cooking: parse_optional_minutes(cooking.as_deref(), COOKING_FIELD)?,
        ingredients: parse_ingredient_rows(ingredients)?,
        steps: parse_step_rows(steps)?,
    })
}

pub(crate) fn create_recipe_request(
    pairs: Vec<(String, String)>,
    author: UserId,
) -> Result<CreateRecipeRequest> {
    let form = parse_recipe_form(&pairs)?;

    Ok(CreateRecipeRequest {
        title: RecipeTitle::new(form.title),
        description: form.description.map(RecipeDescription::new),
        metadata: RecipeMetadata::new(
            form.preparation.map(DurationMinutes::new),
            form.rest.map(DurationMinutes::new),
            form.cooking.map(DurationMinutes::new),
        ),
        ingredients: form
            .ingredients
            .into_iter()
            .map(|ingredient| {
                (
                    IngredientName::new(ingredient.name),
                    Quantity::new(ingredient.quantity, ingredient.unit),
                )
            })
            .collect(),
        steps: form
            .steps
            .into_iter()
            .map(|step| RecipeStep::new(step.title, step.ingredients, step.instruction))
            .collect(),
        author,
    })
}

fn set_once(target: &mut Option<String>, value: &str, key: &str) -> Result<()> {
    if target.replace(value.to_string()).is_some() {
        return Err(invalid(format!("duplicate form field `{key}`")));
    }

    Ok(())
}

/// Parses `collection[row][field]` form keys.
fn parse_indexed_key<T>(
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

fn invalid(message: impl Into<String>) -> Error {
    bad_request(message).into()
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
        let quantity = values
            .remove(&IngredientField::Quantity)
            .unwrap_or_default();
        let unit = values.remove(&IngredientField::Unit).unwrap_or_default();

        let name = name.trim();
        let quantity = quantity.trim();

        if name.is_empty() && quantity.is_empty() {
            continue;
        }

        if name.is_empty() {
            return Err(invalid(format!("ingredient row {} needs a name", row + 1)));
        }

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

        let unit = parse_unit(&unit)?;

        ingredients.push(IngredientFormData {
            name: name.to_string(),
            quantity,
            unit,
        });
    }

    Ok(ingredients)
}

fn parse_unit(value: &str) -> Result<Unit> {
    match value.trim() {
        "gram" => Ok(Unit::Gram),
        "millilitre" => Ok(Unit::MilliLitre),
        "litre" => Ok(Unit::Litre),
        other => Err(invalid(format!("unknown ingredient unit `{other}`"))),
    }
}

fn parse_step_rows(rows: BTreeMap<usize, HashMap<StepField, String>>) -> Result<Vec<StepFormData>> {
    let mut steps = Vec::new();

    for (row, mut values) in rows {
        let title = values.remove(&StepField::Title).unwrap_or_default();
        let instruction = values.remove(&StepField::Instruction).unwrap_or_default();
        let ingredients = values.remove(&StepField::Ingredients);

        let title = title.trim();
        let instruction = instruction.trim();
        let ingredients_text = ingredients.as_deref().map(str::trim).unwrap_or_default();

        if title.is_empty() && instruction.is_empty() && ingredients_text.is_empty() {
            continue;
        }

        if instruction.is_empty() {
            return Err(invalid(format!(
                "step row {} needs an instruction",
                row + 1
            )));
        }

        let ingredients = if ingredients_text.is_empty() {
            None
        } else {
            parse_step_ingredients(ingredients_text)?
        };

        steps.push(StepFormData {
            title: (!title.is_empty()).then(|| title.to_string()),
            instruction: instruction.to_string(),
            ingredients,
        });
    }

    Ok(steps)
}

fn parse_step_ingredients(value: &str) -> Result<Option<HashMap<String, f32>>> {
    let mut ingredients = HashMap::new();

    for line in value.lines() {
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        let Some((name, quantity)) = line.split_once(':') else {
            return Err(invalid(format!(
                "step ingredient `{line}` must be written as `name: quantity`"
            )));
        };

        let name = name.trim();

        if name.is_empty() {
            return Err(invalid("a step ingredient needs a name"));
        }

        let quantity = quantity
            .trim()
            .parse::<f32>()
            .map_err(|_| invalid(format!("step ingredient `{name}` needs a number")))?;

        if !quantity.is_finite() || quantity < 0.0 {
            return Err(invalid(format!(
                "step ingredient `{name}` must be zero or greater"
            )));
        }

        ingredients.insert(name.to_string(), quantity);
    }

    if ingredients.is_empty() {
        Ok(None)
    } else {
        Ok(Some(ingredients))
    }
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
            ("steps[0][ingredients]", "flour: 200"),
        ]))
        .unwrap();

        assert_eq!(form.title, "Soup");
        assert_eq!(form.description.as_deref(), Some("A warming soup."));
        assert_eq!(form.preparation, Some(10));
        assert_eq!(form.rest, Some(20));
        assert_eq!(form.cooking, Some(30));
        assert_eq!(form.ingredients.len(), 2);
        assert_eq!(form.ingredients[0].name, "Flour");
        assert_eq!(form.ingredients[0].quantity, 200.0);
        assert!(matches!(form.ingredients[0].unit, Unit::Gram));
        assert_eq!(form.ingredients[1].name, "Milk");
        assert_eq!(form.ingredients[1].quantity, 0.5);
        assert!(matches!(form.ingredients[1].unit, Unit::Litre));
        assert_eq!(form.steps.len(), 1);
        assert_eq!(form.steps[0].title.as_deref(), Some("Mix"));
        assert_eq!(form.steps[0].instruction, "Stir everything together.");
        assert_eq!(
            form.steps[0]
                .ingredients
                .as_ref()
                .expect("step ingredients")["flour"],
            200.0
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

        assert_eq!(form.ingredients.len(), 2);
        assert_eq!(form.ingredients[0].name, "Flour");
        assert_eq!(form.ingredients[1].name, "Milk");
        assert_eq!(form.steps.len(), 2);
        assert_eq!(form.steps[0].instruction, "First step");
        assert_eq!(form.steps[1].instruction, "Second step");
    }

    #[test]
    fn generated_keys_round_trip_through_the_parser() {
        for wire_field in [
            IngredientField::Name,
            IngredientField::Quantity,
            IngredientField::Unit,
        ] {
            assert_eq!(
                parse_indexed_key(&wire_field.key(7), "ingredients", IngredientField::parse,)
                    .unwrap(),
                Some((7, wire_field))
            );
        }

        for wire_field in [
            StepField::Title,
            StepField::Instruction,
            StepField::Ingredients,
        ] {
            assert_eq!(
                parse_indexed_key(&wire_field.key(7), "steps", StepField::parse).unwrap(),
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
            ("steps[0][ingredients]", ""),
        ]))
        .unwrap();

        assert!(form.ingredients.is_empty());
        assert!(form.steps.is_empty());
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
                ("steps[0][ingredients]", ""),
            ]))
            .is_err()
        );
    }

    #[test]
    fn rejects_a_malformed_step_ingredient() {
        assert!(
            parse_recipe_form(&pairs(&[
                ("title", "Soup"),
                ("steps[0][title]", "Mix"),
                ("steps[0][instruction]", "Stir."),
                ("steps[0][ingredients]", "flour without a quantity"),
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
    fn converts_the_form_into_a_create_request() {
        let request = create_recipe_request(
            pairs(&[
                ("title", "Soup"),
                ("description", "A warming soup."),
                ("ingredients[0][name]", "Flour"),
                ("ingredients[0][quantity]", "200"),
                ("ingredients[0][unit]", "gram"),
                ("steps[0][instruction]", "Stir."),
            ]),
            UserId::new("user-1".to_string()),
        )
        .unwrap();

        assert_eq!(request.title.as_str(), "Soup");
        assert_eq!(request.author.as_str(), "user-1");
        assert_eq!(
            request.description.as_ref().map(RecipeDescription::as_str),
            Some("A warming soup.")
        );
        assert_eq!(request.ingredients.len(), 1);
        assert_eq!(request.steps.len(), 1);
    }
}
