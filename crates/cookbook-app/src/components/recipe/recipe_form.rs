use std::collections::{BTreeMap, HashMap};

use cookbook_domain::recipe::{
    models::{
        DurationMinutes, IngredientName, Quantity, RecipeContent, RecipeDescription,
        RecipeId as RecipeIdValue, RecipeMetadata, RecipeRevisionId, RecipeStep, RecipeTitle,
        RecipeWithContent, RevisionMessage, Unit,
    },
    ports::{CreateRecipeRequest, RecipeService, UpdateRecipeRequest},
};
use cookbook_domain::user::models::UserId;
use topcoat::{
    Error, Result,
    context::{Cx, app_context},
    router::{
        error::{RouterErrorExt, bad_request},
        href,
    },
    runtime::{Event, shard, signal},
    view::{View, attributes, component, element_id, view},
};

use crate::{
    app::{
        AppRecipeService, auth,
        recipes::{RecipeId, recipe_detail},
    },
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
const MESSAGE_FIELD: &str = "message";
const LATEST_REVISION_FIELD: &str = "latest_revision";

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
    message: Option<String>,
    latest_revision: Option<String>,
}

impl RecipeFormData {
    fn into_content(self) -> RecipeContent {
        RecipeContent::new(
            RecipeTitle::new(self.title),
            self.description.map(RecipeDescription::new),
            RecipeMetadata::new(
                self.preparation.map(DurationMinutes::new),
                self.rest.map(DurationMinutes::new),
                self.cooking.map(DurationMinutes::new),
            ),
            self.ingredients
                .into_iter()
                .map(|ingredient| {
                    (
                        IngredientName::new(ingredient.name),
                        Quantity::new(ingredient.quantity, ingredient.unit),
                    )
                })
                .collect(),
            self.steps
                .into_iter()
                .map(|step| RecipeStep::new(step.title, step.ingredients, step.instruction))
                .collect(),
        )
    }
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

#[derive(Clone, Default)]
struct IngredientInitial {
    name: String,
    quantity: String,
    unit: String,
}

#[derive(Clone, Default)]
struct StepInitial {
    title: String,
    instruction: String,
    ingredients: String,
}

#[derive(Default)]
struct FormInitial {
    title: String,
    description: String,
    preparation: String,
    rest: String,
    cooking: String,
    ingredients: Vec<IngredientInitial>,
    steps: Vec<StepInitial>,
}

impl FormInitial {
    fn from_recipe(recipe: &RecipeWithContent) -> Self {
        let mut ingredients: Vec<_> = recipe
            .ingredients()
            .iter()
            .map(|(name, quantity)| IngredientInitial {
                name: name.as_str().to_string(),
                quantity: quantity.value().to_string(),
                unit: unit_wire(quantity.unit()).to_string(),
            })
            .collect();
        ingredients.sort_by(|left, right| left.name.cmp(&right.name));

        Self {
            title: recipe.title().as_str().to_string(),
            description: recipe
                .description()
                .map(|description| description.as_str().to_string())
                .unwrap_or_default(),
            preparation: recipe
                .metadata()
                .preparation()
                .map(|duration| duration.as_minutes().to_string())
                .unwrap_or_default(),
            rest: recipe
                .metadata()
                .rest()
                .map(|duration| duration.as_minutes().to_string())
                .unwrap_or_default(),
            cooking: recipe
                .metadata()
                .cooking()
                .map(|duration| duration.as_minutes().to_string())
                .unwrap_or_default(),
            ingredients,
            steps: recipe
                .steps()
                .iter()
                .map(|step| StepInitial {
                    title: step.title().unwrap_or_default().to_string(),
                    instruction: step.instruction().to_string(),
                    ingredients: step_ingredients_text(step.ingredients()),
                })
                .collect(),
        }
    }
}

async fn load_current_recipe(cx: &Cx, recipe_id: &str) -> Result<Option<RecipeWithContent>> {
    if recipe_id.is_empty() {
        return Ok(None);
    }

    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id.to_string()))
        .await?;

    Ok(Some(recipe.ok_or_not_found()?))
}

fn unit_wire(unit: &Unit) -> &'static str {
    match unit {
        Unit::Gram => "gram",
        Unit::MilliLitre => "millilitre",
        Unit::Litre => "litre",
    }
}

fn step_ingredients_text(ingredients: Option<&HashMap<String, f32>>) -> String {
    let Some(ingredients) = ingredients else {
        return String::new();
    };

    let mut entries: Vec<_> = ingredients.iter().collect();
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));

    entries
        .into_iter()
        .map(|(name, quantity)| format!("{name}: {quantity}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Clone)]
pub enum RecipeFormMode {
    New,
    Edit {
        recipe_id: RecipeIdValue,
        latest_revision: RecipeRevisionId,
    },
}

impl RecipeFormMode {
    fn action(&self) -> String {
        match self {
            Self::New => "/recipe/new".to_string(),
            Self::Edit { recipe_id, .. } => {
                format!("/recipes/{}/edit", recipe_id.as_str())
            }
        }
    }
}

/// A recipe form in either creation or edit mode.
///
/// The public API uses [`RecipeFormMode`]. The shard body accepts primitive
/// arguments because shard arguments must belong to the runtime value
/// vocabulary.
#[component]
pub async fn recipe_form(cx: &Cx, mode: RecipeFormMode) -> Result<impl View> {
    let action = mode.action();
    let (recipe_id, latest_revision) = match &mode {
        RecipeFormMode::New => (String::new(), String::new()),
        RecipeFormMode::Edit {
            recipe_id,
            latest_revision,
        } => (
            recipe_id.as_str().to_string(),
            latest_revision.as_str().to_string(),
        ),
    };

    Ok(view! {
        cx =>
        recipe_form_fields(
            action: action,
            recipe_id: recipe_id,
            latest_revision: latest_revision
        )
    })
}

/// A form that creates or edits a recipe with every field from the recipe domain model.
///
/// The form is a shard, so adding or removing an ingredient or step re-renders
/// only the form. Its signals keep the current values while the form changes.
#[shard]
async fn recipe_form_fields(
    cx: &Cx,
    action: String,
    recipe_id: String,
    latest_revision: String,
) -> Result<impl View> {
    auth::require_user(cx)?;

    let current = load_current_recipe(cx, &recipe_id).await?;
    let initial = current
        .as_ref()
        .map(FormInitial::from_recipe)
        .unwrap_or_default();
    let is_edit = !latest_revision.is_empty();
    let initial_ingredient_count = initial.ingredients.len().max(1);
    let initial_step_count = initial.steps.len().max(1);

    let title_value = initial.title.clone();
    let title = signal(cx, move || title_value.clone());
    let description_value = initial.description.clone();
    let description = signal(cx, move || description_value.clone());
    let preparation_value = initial.preparation.clone();
    let preparation = signal(cx, move || preparation_value.clone());
    let rest_value = initial.rest.clone();
    let rest = signal(cx, move || rest_value.clone());
    let cooking_value = initial.cooking.clone();
    let cooking = signal(cx, move || cooking_value.clone());
    let message = signal(cx, String::new);
    let ingredient_count = signal(cx, move || initial_ingredient_count);
    let step_count = signal(cx, move || initial_step_count);

    let ingredient_initials = initial.ingredients;
    let step_initials = initial.steps;
    let latest_revision = (!latest_revision.is_empty()).then_some(latest_revision);

    let title_id = element_id(cx);
    let description_id = element_id(cx);
    let preparation_id = element_id(cx);
    let rest_id = element_id(cx);
    let cooking_id = element_id(cx);
    let message_id = element_id(cx);

    Ok(view! {
        <form method="post" action=(action) class="flex flex-col gap-6">
            if let Some(latest_revision) = &latest_revision {
                <input
                    type="hidden"
                    name=(LATEST_REVISION_FIELD)
                    value=(latest_revision)
                >
            }
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
                        ingredient_row(
                            row: index,
                            initial: ingredient_initials.get(index).cloned()
                        )
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
                        step_row(row: index, initial: step_initials.get(index).cloned())
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

            if is_edit {
                field(
                    field_label(
                        attrs: attributes! { for=(&message_id) },
                        (t!(form_edit_message))
                    )
                    <textarea
                        id=(&message_id)
                        name=(MESSAGE_FIELD)
                        rows="2"
                        class=(TEXTAREA_CLASS)
                        placeholder=(t!(form_edit_message.placeholder))
                        :value=$(message.get())
                        @input=$(|e: Event| message.set(e.target.value))
                    ></textarea>
                )
            }

            <div class="flex justify-end gap-2">
                if is_edit {
                    <a href=(href!(recipe_detail, RecipeId(recipe_id)))>
                        button(variant: ButtonVariant::Secondary, (t!(form_cancel)))
                    </a>
                    button(attrs: attributes! { type="submit" }, (t!(form_save_recipe)))
                } else {
                    button(attrs: attributes! { type="submit" }, (t!(form_add_recipe)))
                }
            </div>
        </form>
    })
}

#[component]
async fn ingredient_row(
    cx: &Cx,
    row: usize,
    initial: Option<IngredientInitial>,
) -> Result<impl View> {
    let initial = initial.unwrap_or_default();
    let name_value = initial.name;
    let quantity_value = initial.quantity;
    let unit_value = if initial.unit.is_empty() {
        "gram".to_string()
    } else {
        initial.unit
    };
    let name = signal(cx, move || name_value.clone());
    let quantity = signal(cx, move || quantity_value.clone());
    let unit = signal(cx, move || unit_value.clone());
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
async fn step_row(cx: &Cx, row: usize, initial: Option<StepInitial>) -> Result<impl View> {
    let initial = initial.unwrap_or_default();
    let title_value = initial.title;
    let instruction_value = initial.instruction;
    let ingredients_value = initial.ingredients;
    let title = signal(cx, move || title_value.clone());
    let instruction = signal(cx, move || instruction_value.clone());
    let ingredients = signal(cx, move || ingredients_value.clone());
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
    let mut message = None;
    let mut latest_revision = None;
    let mut ingredients: BTreeMap<usize, HashMap<IngredientField, String>> = BTreeMap::new();
    let mut steps: BTreeMap<usize, HashMap<StepField, String>> = BTreeMap::new();

    for (key, value) in pairs {
        match key.as_str() {
            TITLE_FIELD => set_once(&mut title, value, key)?,
            DESCRIPTION_FIELD => set_once(&mut description, value, key)?,
            PREPARATION_FIELD => set_once(&mut preparation, value, key)?,
            REST_FIELD => set_once(&mut rest, value, key)?,
            COOKING_FIELD => set_once(&mut cooking, value, key)?,
            MESSAGE_FIELD => set_once(&mut message, value, key)?,
            LATEST_REVISION_FIELD => set_once(&mut latest_revision, value, key)?,
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
        message,
        latest_revision,
    })
}

pub(crate) fn create_recipe_request(
    pairs: Vec<(String, String)>,
    author: UserId,
) -> Result<CreateRecipeRequest> {
    let form = parse_recipe_form(&pairs)?;

    Ok(CreateRecipeRequest {
        content: form.into_content(),
        author,
    })
}

pub(crate) fn update_recipe_request(
    pairs: Vec<(String, String)>,
    id: RecipeIdValue,
    editor: UserId,
) -> Result<UpdateRecipeRequest> {
    let form = parse_recipe_form(&pairs)?;

    let latest_revision = form
        .latest_revision
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid("latest revision is required"))?
        .to_string();
    let message = form
        .message
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|message| RevisionMessage::new(message).map_err(|error| invalid(error.to_string())))
        .transpose()?;

    Ok(UpdateRecipeRequest {
        id,
        content: form.into_content(),
        editor,
        latest_revision: RecipeRevisionId::new(latest_revision),
        message,
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
    use std::time::SystemTime;

    use cookbook_domain::recipe::models::{Recipe, RecipeRevision};

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
        assert_eq!(form.message, None);
        assert_eq!(form.latest_revision, None);
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

        assert_eq!(request.content.title().as_str(), "Soup");
        assert_eq!(request.author.as_str(), "user-1");
        assert_eq!(
            request.content.description().map(RecipeDescription::as_str),
            Some("A warming soup.")
        );
        assert_eq!(request.content.ingredients().len(), 1);
        assert_eq!(request.content.steps().len(), 1);
    }

    #[test]
    fn form_mode_derives_the_form_action() {
        assert_eq!(RecipeFormMode::New.action(), "/recipe/new");

        let edit = RecipeFormMode::Edit {
            recipe_id: RecipeIdValue::new("soup".to_string()),
            latest_revision: RecipeRevisionId::new("revision-1".to_string()),
        };

        assert_eq!(edit.action(), "/recipes/soup/edit");
    }

    #[test]
    fn prefills_form_values_from_a_recipe() {
        let recipe = Recipe::new(
            RecipeIdValue::new("soup".to_string()),
            UserId::new("user-1".to_string()),
        );
        let content = RecipeContent::new(
            RecipeTitle::new("Soup".to_string()),
            Some(RecipeDescription::new("A warming soup.".to_string())),
            RecipeMetadata::new(
                Some(DurationMinutes::new(10)),
                None,
                Some(DurationMinutes::new(20)),
            ),
            HashMap::from([
                (
                    IngredientName::new("milk".to_string()),
                    Quantity::new(0.5, Unit::Litre),
                ),
                (
                    IngredientName::new("flour".to_string()),
                    Quantity::new(200.0, Unit::Gram),
                ),
            ]),
            vec![RecipeStep::new(
                Some("Mix".to_string()),
                Some(HashMap::from([
                    ("milk".to_string(), 0.5),
                    ("flour".to_string(), 200.0),
                ])),
                "Stir everything together.".to_string(),
            )],
        );
        let revision = RecipeRevision::new(
            RecipeRevisionId::new("revision-1".to_string()),
            recipe.id().clone(),
            content,
            UserId::new("user-1".to_string()),
            SystemTime::UNIX_EPOCH,
            None,
        );
        let current = RecipeWithContent::new(recipe, revision).unwrap();

        let initial = FormInitial::from_recipe(&current);

        assert_eq!(initial.title, "Soup");
        assert_eq!(initial.description, "A warming soup.");
        assert_eq!(initial.preparation, "10");
        assert_eq!(initial.rest, "");
        assert_eq!(initial.cooking, "20");
        assert_eq!(initial.ingredients[0].name, "flour");
        assert_eq!(initial.ingredients[0].quantity, "200");
        assert_eq!(initial.ingredients[0].unit, "gram");
        assert_eq!(initial.ingredients[1].name, "milk");
        assert_eq!(initial.ingredients[1].quantity, "0.5");
        assert_eq!(initial.ingredients[1].unit, "litre");
        assert_eq!(initial.steps[0].title, "Mix");
        assert_eq!(initial.steps[0].instruction, "Stir everything together.");
        assert_eq!(initial.steps[0].ingredients, "flour: 200\nmilk: 0.5");
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
    fn converts_the_form_into_an_update_request() {
        let request = update_recipe_request(
            pairs(&[
                ("title", "Soup"),
                ("message", "reduced the salt"),
                ("latest_revision", "revision-1"),
            ]),
            RecipeIdValue::new("soup".to_string()),
            UserId::new("editor-1".to_string()),
        )
        .unwrap();

        assert_eq!(request.id.as_str(), "soup");
        assert_eq!(request.editor.as_str(), "editor-1");
        assert_eq!(request.latest_revision.as_str(), "revision-1");
        assert_eq!(
            request.message.as_ref().map(RevisionMessage::as_str),
            Some("reduced the salt")
        );
        assert_eq!(request.content.title().as_str(), "Soup");
    }

    #[test]
    fn update_request_requires_a_latest_revision() {
        assert!(
            update_recipe_request(
                pairs(&[("title", "Soup")]),
                RecipeIdValue::new("soup".to_string()),
                UserId::new("editor-1".to_string()),
            )
            .is_err()
        );
    }

    #[test]
    fn update_request_treats_a_blank_message_as_missing() {
        let request = update_recipe_request(
            pairs(&[
                ("title", "Soup"),
                ("message", "   "),
                ("latest_revision", "revision-1"),
            ]),
            RecipeIdValue::new("soup".to_string()),
            UserId::new("editor-1".to_string()),
        )
        .unwrap();

        assert!(request.message.is_none());
    }
}
