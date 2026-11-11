use std::collections::{BTreeMap, HashMap, HashSet};

use cookbook_domain::recipe::{
    models::{
        DurationMinutes, IngredientName, NewRecipeImage, Quantity, RecipeContent,
        RecipeDescription, RecipeDurations, RecipeId as RecipeIdValue, RecipeImageId,
        RecipeImageMetadata, RecipeRevisionId, RecipeStep, RecipeStepId, RecipeTitle,
        RecipeWithContent, RevisionMessage, RevisionVerification, Servings, Unit,
    },
    ports::{CreateRecipeRequest, RecipeService, UpdateRecipeRequest},
};
use cookbook_domain::user::models::UserId;
use cookbook_vision::detect_image_content_type;
use topcoat::{
    Error, Result,
    context::{Cx, app_context},
    router::{
        content::multipart::Multipart,
        error::{RouterErrorExt, bad_request},
        href,
    },
    runtime::{Event, Signal, procedure, shard, signal},
    view::{View, attributes, component, element_id, view},
};

use crate::{
    app::{
        AppRecipeService, auth,
        recipes::{ImageId, RecipeId, recipe_detail, recipe_image},
    },
    components::base::{
        button::{ButtonSize, ButtonVariant, button},
        field::{field, field_description, field_label, field_legend, field_set},
        image_upload::image_upload,
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
const SERVINGS_TYPE_FIELD: &str = "servings_type";
const SERVINGS_COUNT_FIELD: &str = "servings_count";
const SERVINGS_LABEL_FIELD: &str = "servings_label";
const MESSAGE_FIELD: &str = "message";
const LATEST_REVISION_FIELD: &str = "latest_revision";
const IMAGES_FIELD: &str = "images";
const IMAGE_IDS_FIELD: &str = "image_ids";

/// Inserts a step slot at a display position, clamping past the end.
fn insert_step_key(mut order: Vec<usize>, index: usize, key: usize) -> Vec<usize> {
    order.insert(index.min(order.len()), key);
    order
}

/// Removes a step slot, if it is still present.
fn remove_step_key(mut order: Vec<usize>, key: usize) -> Vec<usize> {
    order.retain(|candidate| *candidate != key);
    order
}

/// Swaps a step slot with its neighbour. Boundary moves are no-ops.
fn move_step_key(mut order: Vec<usize>, key: usize, up: bool) -> Vec<usize> {
    let Some(index) = order.iter().position(|candidate| *candidate == key) else {
        return order;
    };

    let target = if up {
        index.checked_sub(1)
    } else {
        index.checked_add(1).filter(|target| *target < order.len())
    };

    if let Some(target) = target {
        order.swap(index, target);
    }

    order
}

/// Removes an image reference from a step's ordered image list.
fn remove_step_image_id(ids: Vec<String>, id: &str) -> Vec<String> {
    ids.into_iter()
        .filter(|candidate| candidate != id)
        .collect()
}

#[procedure]
async fn form_insert_step(order: Vec<usize>, index: usize, key: usize) -> Result<Vec<usize>> {
    Ok(insert_step_key(order, index, key))
}

#[procedure]
async fn form_remove_step(order: Vec<usize>, key: usize) -> Result<Vec<usize>> {
    Ok(remove_step_key(order, key))
}

#[procedure]
async fn form_move_step(order: Vec<usize>, key: usize, up: bool) -> Result<Vec<usize>> {
    Ok(move_step_key(order, key, up))
}

#[procedure]
async fn form_remove_step_image(ids: Vec<String>, id: String) -> Result<Vec<String>> {
    Ok(remove_step_image_id(ids, &id))
}

/// The fields of one ingredient row on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum IngredientField {
    Name,
    AmountType,
    Quantity,
    QuantityText,
    Unit,
    UnitOther,
}

impl IngredientField {
    const fn wire_name(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::AmountType => "amount_type",
            Self::Quantity => "quantity",
            Self::QuantityText => "quantity_text",
            Self::Unit => "unit",
            Self::UnitOther => "unit_other",
        }
    }

    fn key(self, row: usize) -> String {
        format!("ingredients[{row}][{}]", self.wire_name())
    }

    fn parse(wire_field: &str) -> Option<Self> {
        match wire_field {
            "name" => Some(Self::Name),
            "amount_type" => Some(Self::AmountType),
            "quantity" => Some(Self::Quantity),
            "quantity_text" => Some(Self::QuantityText),
            "unit" => Some(Self::Unit),
            "unit_other" => Some(Self::UnitOther),
            _ => None,
        }
    }
}

/// The fields of one step row on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum StepField {
    Id,
    Title,
    Instruction,
    Images,
    ImageIds,
}

impl StepField {
    const fn wire_name(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Title => "title",
            Self::Instruction => "instruction",
            Self::Images => "images",
            Self::ImageIds => "image_ids",
        }
    }

    fn key(self, row: usize) -> String {
        format!("steps[{row}][{}]", self.wire_name())
    }

    fn parse(wire_field: &str) -> Option<Self> {
        match wire_field {
            "id" => Some(Self::Id),
            "title" => Some(Self::Title),
            "instruction" => Some(Self::Instruction),
            // File and repeated image id fields are collected separately.
            "images" | "image_ids" => None,
            _ => None,
        }
    }
}

#[derive(Debug)]
struct RecipeFormData {
    title: String,
    description: Option<String>,
    servings: Option<Servings>,
    preparation: Option<usize>,
    rest: Option<usize>,
    cooking: Option<usize>,
    ingredients: Vec<IngredientFormData>,
    images: Vec<RecipeImageId>,
    steps: Vec<StepFormData>,
    message: Option<String>,
    latest_revision: Option<String>,
}

impl RecipeFormData {
    fn into_content(self) -> RecipeContent {
        RecipeContent::new(
            RecipeTitle::new(self.title),
            self.description.map(RecipeDescription::new),
            self.servings,
            RecipeDurations::new(
                self.preparation.map(DurationMinutes::new),
                self.rest.map(DurationMinutes::new),
                self.cooking.map(DurationMinutes::new),
            ),
            self.ingredients
                .into_iter()
                .map(|ingredient| (IngredientName::new(ingredient.name), ingredient.quantity))
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
struct IngredientFormData {
    name: String,
    quantity: Quantity,
}

#[derive(Debug)]
struct StepFormData {
    row: usize,
    id: RecipeStepId,
    title: Option<String>,
    instruction: String,
    images: Vec<RecipeImageId>,
}

#[derive(Clone, Default)]
struct IngredientInitial {
    name: String,
    amount_type: String,
    quantity: String,
    quantity_text: String,
    unit: String,
    unit_other: String,
}

#[derive(Clone, Default)]
struct FormImageInitial {
    id: String,
    url: String,
    alt: String,
}

#[derive(Clone, Default)]
struct StepInitial {
    id: String,
    title: String,
    instruction: String,
    images: Vec<String>,
}

/// Signals shared by every step row in the form's ordered step list.
#[derive(Clone)]
struct StepListState {
    order: Signal<Vec<usize>>,
    next_key: Signal<usize>,
    inserted_slot: Signal<Option<usize>>,
    insert_title: Signal<String>,
    insert_instruction: Signal<String>,
    insert_images: Signal<Vec<String>>,
}

#[derive(Default)]
struct FormInitial {
    title: String,
    description: String,
    servings_type: String,
    servings_count: String,
    servings_label: String,
    preparation: String,
    rest: String,
    cooking: String,
    ingredients: Vec<IngredientInitial>,
    steps: Vec<StepInitial>,
    images: Vec<FormImageInitial>,
}

fn form_image_initial(image: &RecipeImageMetadata, recipe_id: &str, cx: &Cx) -> FormImageInitial {
    FormImageInitial {
        id: image.id().as_str().to_string(),
        url: href!(
            recipe_image,
            RecipeId(recipe_id),
            ImageId(image.id().as_str())
        )
        .resolve(cx),
        alt: t!(recipe_image_alt).resolve(cx).into_owned(),
    }
}

impl FormInitial {
    fn from_recipe(
        recipe: &RecipeWithContent,
        images: &[RecipeImageMetadata],
        recipe_id: &str,
        cx: &Cx,
    ) -> Self {
        let mut ingredients: Vec<_> = recipe
            .ingredients()
            .iter()
            .map(|(name, quantity)| ingredient_initial(name.as_str(), quantity))
            .collect();
        ingredients.sort_by(|left, right| left.name.cmp(&right.name));

        let (servings_type, servings_count, servings_label) = match recipe.servings() {
            Some(Servings::People(count)) => {
                ("people".to_string(), count.to_string(), String::new())
            }
            Some(Servings::Portions { count, label }) => (
                "portions".to_string(),
                count.to_string(),
                label.clone().unwrap_or_default(),
            ),
            None => ("none".to_string(), String::new(), String::new()),
        };

        let image_by_id: HashMap<&str, &RecipeImageMetadata> = images
            .iter()
            .map(|image| (image.id().as_str(), image))
            .collect();

        let recipe_images = recipe
            .content()
            .images()
            .iter()
            .filter_map(|image_id| image_by_id.get(image_id.as_str()).copied())
            .map(|image| form_image_initial(image, recipe_id, cx))
            .collect();

        Self {
            title: recipe.title().as_str().to_string(),
            description: recipe
                .description()
                .map(|description| description.as_str().to_string())
                .unwrap_or_default(),
            servings_type,
            servings_count,
            servings_label,
            preparation: recipe
                .durations()
                .preparation()
                .map(|duration| duration.as_minutes().to_string())
                .unwrap_or_default(),
            rest: recipe
                .durations()
                .rest()
                .map(|duration| duration.as_minutes().to_string())
                .unwrap_or_default(),
            cooking: recipe
                .durations()
                .cooking()
                .map(|duration| duration.as_minutes().to_string())
                .unwrap_or_default(),
            ingredients,
            steps: recipe
                .steps()
                .iter()
                .map(|step| StepInitial {
                    id: step.id().as_str().to_string(),
                    title: step.title().unwrap_or_default().to_string(),
                    instruction: step.instruction().to_string(),
                    images: step
                        .images()
                        .iter()
                        .filter(|image_id| image_by_id.contains_key(image_id.as_str()))
                        .map(|image_id| image_id.as_str().to_string())
                        .collect(),
                })
                .collect(),
            images: recipe_images,
        }
    }
}

async fn load_current_recipe(cx: &Cx, recipe_id: &str) -> Result<Option<RecipeWithContent>> {
    if recipe_id.is_empty() {
        return Ok(None);
    }

    let user = auth::require_user(cx)?;
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id.to_string()), user.id().clone())
        .await?;

    Ok(Some(recipe.ok_or_not_found()?))
}

fn ingredient_initial(name: &str, quantity: &Quantity) -> IngredientInitial {
    match quantity {
        Quantity::Measured { value, unit } => {
            let (unit, unit_other) = match unit {
                Unit::Other(label) => ("other", label.clone()),
                unit => (unit_wire(unit), String::new()),
            };

            IngredientInitial {
                name: name.to_string(),
                amount_type: "measured".to_string(),
                quantity: value.to_string(),
                quantity_text: String::new(),
                unit: unit.to_string(),
                unit_other,
            }
        }
        Quantity::Text(text) => IngredientInitial {
            name: name.to_string(),
            amount_type: "text".to_string(),
            quantity: String::new(),
            quantity_text: text.clone(),
            unit: "gram".to_string(),
            unit_other: String::new(),
        },
    }
}

fn unit_wire(unit: &Unit) -> &'static str {
    match unit {
        Unit::Gram => "gram",
        Unit::Kilogram => "kilogram",
        Unit::MilliLitre => "millilitre",
        Unit::CentiLitre => "centilitre",
        Unit::Litre => "litre",
        Unit::Piece => "piece",
        Unit::Teaspoon => "teaspoon",
        Unit::Tablespoon => "tablespoon",
        Unit::Other(_) => "other",
    }
}

#[derive(Clone)]
pub enum RecipeFormMode {
    New,
    Edit {
        recipe_id: RecipeIdValue,
        latest_revision: RecipeRevisionId,
        /// Whether the current revision still needs review. Saving the form
        /// verifies it, so the wording changes accordingly.
        needs_review: bool,
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
    let (recipe_id, latest_revision, needs_review) = match &mode {
        RecipeFormMode::New => (String::new(), String::new(), false),
        RecipeFormMode::Edit {
            recipe_id,
            latest_revision,
            needs_review,
        } => (
            recipe_id.as_str().to_string(),
            latest_revision.as_str().to_string(),
            *needs_review,
        ),
    };

    Ok(view! {
        cx =>
        recipe_form_fields(
            action: action,
            recipe_id: recipe_id,
            latest_revision: latest_revision,
            needs_review: needs_review
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
    needs_review: bool,
) -> Result<impl View> {
    auth::require_user(cx)?;

    let current = load_current_recipe(cx, &recipe_id).await?;
    let images = if current.is_some() && !recipe_id.is_empty() {
        let user = auth::require_user(cx)?;
        let recipe_service: &AppRecipeService = app_context(cx);
        recipe_service
            .get_recipe_images(RecipeIdValue::new(recipe_id.clone()), user.id().clone())
            .await?
    } else {
        Vec::new()
    };
    let initial = current
        .as_ref()
        .map(|recipe| FormInitial::from_recipe(recipe, &images, &recipe_id, cx))
        .unwrap_or_default();
    let is_edit = !latest_revision.is_empty();
    let initial_ingredient_count = initial.ingredients.len().max(1);
    let initial_step_count = initial.steps.len().max(1);
    let initial_step_order: Vec<usize> = (0..initial_step_count).collect();
    let image_lookup: HashMap<String, FormImageInitial> = images
        .iter()
        .map(|image| {
            (
                image.id().as_str().to_string(),
                form_image_initial(image, &recipe_id, cx),
            )
        })
        .collect();

    let title_value = initial.title.clone();
    let title = signal(cx, move || title_value.clone());
    let description_value = initial.description.clone();
    let description = signal(cx, move || description_value.clone());
    let servings_type_value = if initial.servings_type.is_empty() {
        "none".to_string()
    } else {
        initial.servings_type.clone()
    };
    let servings_type = signal(cx, move || servings_type_value.clone());
    let servings_count_value = initial.servings_count.clone();
    let servings_count = signal(cx, move || servings_count_value.clone());
    let servings_label_value = initial.servings_label.clone();
    let servings_label = signal(cx, move || servings_label_value.clone());
    let preparation_value = initial.preparation.clone();
    let preparation = signal(cx, move || preparation_value.clone());
    let rest_value = initial.rest.clone();
    let rest = signal(cx, move || rest_value.clone());
    let cooking_value = initial.cooking.clone();
    let cooking = signal(cx, move || cooking_value.clone());
    let message = signal(cx, String::new);
    let ingredient_count = signal(cx, move || initial_ingredient_count);
    let step_order = signal(cx, move || initial_step_order.clone());
    let next_step_key = signal(cx, move || initial_step_count);
    let inserted_slot = signal(cx, || None::<usize>);
    let insert_title = signal(cx, String::new);
    let insert_instruction = signal(cx, String::new);
    let insert_images = signal(cx, Vec::<String>::new);
    let step_list = StepListState {
        order: step_order.clone(),
        next_key: next_step_key.clone(),
        inserted_slot: inserted_slot.clone(),
        insert_title: insert_title.clone(),
        insert_instruction: insert_instruction.clone(),
        insert_images: insert_images.clone(),
    };

    let ingredient_initials = initial.ingredients;
    let step_initials = initial.steps;
    let recipe_image_initials = initial.images;
    let latest_revision = (!latest_revision.is_empty()).then_some(latest_revision);

    let title_id = element_id(cx);
    let description_id = element_id(cx);
    let servings_type_id = element_id(cx);
    let servings_count_id = element_id(cx);
    let servings_label_id = element_id(cx);
    let preparation_id = element_id(cx);
    let rest_id = element_id(cx);
    let cooking_id = element_id(cx);
    let recipe_images_id = element_id(cx);
    let message_id = element_id(cx);
    let message_label = if needs_review {
        t!(form_verification_message).resolve(cx).into_owned()
    } else {
        t!(form_edit_message).resolve(cx).into_owned()
    };
    let message_placeholder = if needs_review {
        t!(form_verification_message.placeholder)
            .resolve(cx)
            .into_owned()
    } else {
        t!(form_edit_message.placeholder).resolve(cx).into_owned()
    };
    let submit_label = if needs_review {
        t!(form_verify_and_save).resolve(cx).into_owned()
    } else {
        t!(form_save_recipe).resolve(cx).into_owned()
    };

    Ok(view! {
        <form
            method="post"
            action=(action)
            enctype="multipart/form-data"
            class="flex flex-col gap-6"
        >
            if let Some(latest_revision) = &latest_revision {
                <input
                    type="hidden"
                    name=(LATEST_REVISION_FIELD)
                    value=(latest_revision)
                >
            }
            if needs_review {
                <p
                    role="status"
                    class="rounded-lg border border-warning/40 bg-warning/10 px-4 py-3 text-sm text-warning-foreground"
                >
                    (t!(recipe_needs_review_notice))
                </p>
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
                field_set(
                    field_legend((t!(form_servings)))
                    <div class="grid gap-4 sm:grid-cols-3">
                        field(
                            field_label(
                                attrs: attributes! { for=(&servings_type_id) },
                                (t!(form_servings_type))
                            )
                            <select
                                id=(&servings_type_id)
                                name=(SERVINGS_TYPE_FIELD)
                                class=(SELECT_CLASS)
                                :value=$(servings_type.get())
                                @change=$(|e: Event| servings_type.set(e.target.value))
                            >
                                <option value="none">(t!(form_servings_none))</option>
                                <option value="people">(t!(form_servings_people))</option>
                                <option value="portions">
                                    (t!(form_servings_portions))
                                </option>
                            </select>
                        )
                        field(
                            attrs: attributes! { :hidden=$(servings_type.get() == "none") },
                            field_label(
                                attrs: attributes! { for=(&servings_count_id) },
                                (t!(form_servings_count))
                            )
                            input(
                                attrs: attributes! {
                                    id=(&servings_count_id)
                                    name=(SERVINGS_COUNT_FIELD)
                                    type="number"
                                    min="1"
                                    step="1"
                                    placeholder="4"
                                    :disabled=$(servings_type.get() == "none")
                                    :value=$(servings_count.get())
                                    @input=$(|e: Event| servings_count.set(e.target.value))
                                }
                            )
                        )
                        field(
                            attrs: attributes! { :hidden=$(servings_type.get() != "portions") },
                            field_label(
                                attrs: attributes! { for=(&servings_label_id) },
                                (t!(form_servings_label))
                            )
                            input(
                                attrs: attributes! {
                                    id=(&servings_label_id)
                                    name=(SERVINGS_LABEL_FIELD)
                                    type="text"
                                    placeholder=(t!(form_servings_label.placeholder))
                                    :disabled=$(servings_type.get() != "portions")
                                    :value=$(servings_label.get())
                                    @input=$(|e: Event| servings_label.set(e.target.value))
                                }
                            )
                        )
                    </div>
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
                field_legend((t!(form_recipe_images)))
                if recipe_image_initials.is_empty() {
                    <p class="text-sm text-muted-foreground">
                        (t!(form_recipe_images_empty))
                    </p>
                } else {
                    <ul class="grid grid-cols-2 gap-3 sm:grid-cols-3">
                        for image in &recipe_image_initials {
                            form_image_row(
                                image: image.clone(),
                                image_ids_field: IMAGE_IDS_FIELD.to_string()
                            )
                        }
                    </ul>
                }
                field(
                    field_label(
                        attrs: attributes! { for=(&recipe_images_id) },
                        (t!(form_recipe_images_upload))
                    )
                    image_upload(
                        multiple: true,
                        attrs: attributes! { id=(&recipe_images_id) name=(IMAGES_FIELD) }
                    )
                    field_description((t!(form_recipe_images_upload_hint)))
                )
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
                    #[key(key)]
                    for (position, key) in step_order.get().into_iter().enumerate() {
                        step_row(
                            row: position,
                            key: key,
                            initial: if inserted_slot.get() == Some(key) {
                                Some(
                                    StepInitial {
                                        id: String::new(),
                                        title: insert_title.get(),
                                        instruction: insert_instruction.get(),
                                        images: insert_images.get(),
                                    },
                                )
                            } else {
                                step_initials.get(key).cloned()
                            },
                            list: step_list.clone(),
                            image_lookup: image_lookup.clone()
                        )
                    }
                </div>
                <div class="flex flex-wrap gap-2">
                    button(
                        variant: ButtonVariant::Outline,
                        size: ButtonSize::Sm,
                        attrs: attributes! {
                            type="button"
                            @click=$(async |_e: Event| {
                                let new_key = next_step_key.get();
                                next_step_key.increment();
                                let order = step_order.get();
                                let index = order.len();
                                let next = form_insert_step(order, index, new_key).await;
                                inserted_slot.set(None);
                                step_order.set(next);
                            })
                        },
                        (t!(form_add_step))
                    )
                </div>
            )

            if is_edit {
                field(
                    field_label(
                        attrs: attributes! { for=(&message_id) },
                        (message_label)
                    )
                    <textarea
                        id=(&message_id)
                        name=(MESSAGE_FIELD)
                        rows="2"
                        class=(TEXTAREA_CLASS)
                        placeholder=(message_placeholder)
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
                    button(attrs: attributes! { type="submit" }, (submit_label))
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
    let amount_type_value = if initial.amount_type.is_empty() {
        "measured".to_string()
    } else {
        initial.amount_type
    };
    let quantity_value = initial.quantity;
    let quantity_text_value = initial.quantity_text;
    let unit_value = if initial.unit.is_empty() {
        "gram".to_string()
    } else {
        initial.unit
    };
    let unit_other_value = initial.unit_other;
    let name = signal(cx, move || name_value.clone());
    let amount_type = signal(cx, move || amount_type_value.clone());
    let quantity = signal(cx, move || quantity_value.clone());
    let quantity_text = signal(cx, move || quantity_text_value.clone());
    let unit = signal(cx, move || unit_value.clone());
    let unit_other = signal(cx, move || unit_other_value.clone());
    let removed = signal(cx, || false);

    let name_id = element_id(cx);
    let amount_type_id = element_id(cx);
    let quantity_id = element_id(cx);
    let quantity_text_id = element_id(cx);
    let unit_id = element_id(cx);
    let unit_other_id = element_id(cx);

    let name_field = IngredientField::Name.key(row);
    let amount_type_field = IngredientField::AmountType.key(row);
    let quantity_field = IngredientField::Quantity.key(row);
    let quantity_text_field = IngredientField::QuantityText.key(row);
    let unit_field = IngredientField::Unit.key(row);
    let unit_other_field = IngredientField::UnitOther.key(row);

    Ok(view! {
        <fieldset
            :hidden=$(removed.get())
            :disabled=$(removed.get())
            class="m-0 flex min-w-0 flex-col gap-3 rounded-lg border border-border p-4"
        >
            <div
                class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_10rem_auto] sm:items-end"
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
                        attrs: attributes! { for=(&amount_type_id) },
                        (t!(form_amount_type))
                    )
                    <select
                        id=(&amount_type_id)
                        name=(amount_type_field)
                        class=(SELECT_CLASS)
                        :value=$(amount_type.get())
                        @change=$(|e: Event| amount_type.set(e.target.value))
                    >
                        <option value="measured">(t!(form_amount_measured))</option>
                        <option value="text">(t!(form_amount_text))</option>
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
            </div>

            <div
                :hidden=$(amount_type.get() != "measured")
                class="grid gap-3 sm:grid-cols-[8rem_10rem_minmax(0,1fr)] sm:items-end"
            >
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
                            :disabled=$(amount_type.get() != "measured")
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
                        :disabled=$(amount_type.get() != "measured")
                        :value=$(unit.get())
                        @change=$(|e: Event| unit.set(e.target.value))
                    >
                        <option value="gram">(t!(unit_gram))</option>
                        <option value="kilogram">(t!(unit_kilogram))</option>
                        <option value="millilitre">(t!(unit_millilitre))</option>
                        <option value="centilitre">(t!(unit_centilitre))</option>
                        <option value="litre">(t!(unit_litre))</option>
                        <option value="piece">(t!(unit_piece))</option>
                        <option value="teaspoon">(t!(unit_teaspoon))</option>
                        <option value="tablespoon">(t!(unit_tablespoon))</option>
                        <option value="other">(t!(unit_other))</option>
                    </select>
                )
                field(
                    attrs: attributes! { :hidden=$(unit.get() != "other") },
                    field_label(
                        attrs: attributes! { for=(&unit_other_id) },
                        (t!(form_unit_other))
                    )
                    input(
                        attrs: attributes! {
                            id=(&unit_other_id)
                            name=(unit_other_field)
                            type="text"
                            placeholder=(t!(form_unit_other.placeholder))
                            :disabled=$(amount_type.get() != "measured")
                            :value=$(unit_other.get())
                            @input=$(|e: Event| unit_other.set(e.target.value))
                        }
                    )
                )
            </div>

            field(
                attrs: attributes! { :hidden=$(amount_type.get() != "text") },
                field_label(
                    attrs: attributes! { for=(&quantity_text_id) },
                    (t!(form_amount_text_value))
                )
                input(
                    attrs: attributes! {
                        id=(&quantity_text_id)
                        name=(quantity_text_field)
                        type="text"
                        placeholder=(t!(form_amount_text_value.placeholder))
                        :disabled=$(amount_type.get() != "text")
                        :value=$(quantity_text.get())
                        @input=$(|e: Event| quantity_text.set(e.target.value))
                    }
                )
            )
        </fieldset>
    })
}

#[component]
async fn form_image_row(
    cx: &Cx,
    image: FormImageInitial,
    image_ids_field: String,
) -> Result<impl View> {
    let removed = signal(cx, || false);

    Ok(view! {
        <li class="flex flex-col gap-2" :hidden=$(removed.get())>
            <a href=(image.url.clone()) target="_blank" rel="noreferrer">
                <img
                    src=(image.url.clone())
                    alt=(image.alt.clone())
                    loading="lazy"
                    class="aspect-square w-full rounded-lg border border-border object-cover"
                >
            </a>
            <input
                type="hidden"
                name=(image_ids_field)
                value=(image.id)
                :disabled=$(removed.get())
            >
            <div class="flex justify-end">
                button(
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Sm,
                    attrs: attributes! { type="button" @click=$(|_e: Event| removed.set(true)) },
                    (t!(form_recipe_images_remove))
                )
            </div>
        </li>
    })
}

#[component]
async fn step_row(
    cx: &Cx,
    row: usize,
    key: usize,
    initial: Option<StepInitial>,
    list: StepListState,
    image_lookup: HashMap<String, FormImageInitial>,
) -> Result<impl View> {
    let StepListState {
        order,
        next_key,
        inserted_slot,
        insert_title,
        insert_instruction,
        insert_images,
    } = list;
    let initial = initial.unwrap_or_default();
    let id_value = initial.id;
    let title_value = initial.title;
    let instruction_value = initial.instruction;
    let image_ids_value = initial.images;
    let title = signal(cx, move || title_value.clone());
    let instruction = signal(cx, move || instruction_value.clone());
    let step_image_ids = signal(cx, move || image_ids_value.clone());

    let title_id = element_id(cx);
    let instruction_id = element_id(cx);
    let step_images_id = element_id(cx);

    let id_field = StepField::Id.key(row);
    let title_field = StepField::Title.key(row);
    let instruction_field = StepField::Instruction.key(row);
    let step_images_field = StepField::Images.key(row);
    let image_ids_field = StepField::ImageIds.key(row);

    Ok(view! {
        <fieldset
            class="m-0 flex min-w-0 flex-col gap-3 rounded-lg border border-border p-4"
        >
            <input type="hidden" name=(id_field) value=(id_value)>
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
                    attrs: attributes! { for=(&step_images_id) },
                    (t!(form_step_images))
                )
                if !step_image_ids.get().is_empty() {
                    <ul class="mb-2 grid grid-cols-2 gap-3 sm:grid-cols-3">
                        #[key(image_id.clone())]
                        for image_id in step_image_ids.get() {
                            if let Some(image) = image_lookup.get(&image_id) {
                                step_image_row(
                                    image: image.clone(),
                                    image_ids: step_image_ids.clone(),
                                    image_ids_field: image_ids_field.clone()
                                )
                            }
                        }
                    </ul>
                }
                image_upload(
                    multiple: true,
                    attrs: attributes! { id=(&step_images_id) name=(step_images_field) }
                )
                field_description((t!(form_step_images_upload_hint)))
            )
            <div class="flex flex-wrap items-center justify-end gap-2">
                button(
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Sm,
                    attrs: attributes! {
                        type="button"
                        aria-label=(t!(form_insert_step_below))
                        title=(t!(form_insert_step_below))
                        @click=$(async |_e: Event| {
                            let new_key = next_key.get();
                            next_key.increment();
                            let next = form_insert_step(order.get(), row + 1, new_key).await;
                            inserted_slot.set(None);
                            order.set(next);
                        })
                    },
                    ("+")
                )
                button(
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Sm,
                    attrs: attributes! {
                        type="button"
                        aria-label=(t!(form_move_step_up))
                        title=(t!(form_move_step_up))
                        :disabled=$(row == 0)
                        @click=$(async |_e: Event| {
                            let next = form_move_step(order.get(), key, true).await;
                            order.set(next);
                        })
                    },
                    ("↑")
                )
                button(
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Sm,
                    attrs: attributes! {
                        type="button"
                        aria-label=(t!(form_move_step_down))
                        title=(t!(form_move_step_down))
                        :disabled=$(row + 1 == order.get().len())
                        @click=$(async |_e: Event| {
                            let next = form_move_step(order.get(), key, false).await;
                            order.set(next);
                        })
                    },
                    ("↓")
                )
                button(
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Sm,
                    attrs: attributes! {
                        type="button"
                        @click=$(async |_e: Event| {
                            let new_key = next_key.get();
                            next_key.increment();
                            let next = form_insert_step(order.get(), row + 1, new_key).await;
                            insert_title.set(title.get());
                            insert_instruction.set(instruction.get());
                            insert_images.set(step_image_ids.get());
                            inserted_slot.set(Some(new_key));
                            order.set(next);
                        })
                    },
                    (t!(form_split_step))
                )
                button(
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Sm,
                    attrs: attributes! {
                        type="button"
                        @click=$(async |_e: Event| {
                            let next = form_remove_step(order.get(), key).await;
                            order.set(next);
                        })
                    },
                    (t!(form_remove))
                )
            </div>
        </fieldset>
    })
}

#[component]
async fn step_image_row(
    cx: &Cx,
    image: FormImageInitial,
    image_ids: Signal<Vec<String>>,
    image_ids_field: String,
) -> Result<impl View> {
    let _ = cx;
    let image_id = image.id.clone();

    Ok(view! {
        <li class="flex flex-col gap-2">
            <a href=(image.url.clone()) target="_blank" rel="noreferrer">
                <img
                    src=(image.url.clone())
                    alt=(image.alt.clone())
                    loading="lazy"
                    class="aspect-square w-full rounded-lg border border-border object-cover"
                >
            </a>
            <input type="hidden" name=(image_ids_field) value=(image.id)>
            <div class="flex justify-end">
                button(
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Sm,
                    attrs: attributes! {
                        type="button"
                        @click=$(async |_e: Event| {
                            let next = form_remove_step_image(image_ids.get(), image_id).await;
                            image_ids.set(next);
                        })
                    },
                    (t!(form_recipe_images_remove))
                )
            </div>
        </li>
    })
}

/// Parses and validates the URL-encoded form into its wire DTO.
fn parse_recipe_form(pairs: &[(String, String)]) -> Result<RecipeFormData> {
    let mut title = None;
    let mut description = None;
    let mut servings_type = None;
    let mut servings_count = None;
    let mut servings_label = None;
    let mut preparation = None;
    let mut rest = None;
    let mut cooking = None;
    let mut message = None;
    let mut latest_revision = None;
    let mut images = Vec::new();
    let mut step_images: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let mut ingredients: BTreeMap<usize, HashMap<IngredientField, String>> = BTreeMap::new();
    let mut steps: BTreeMap<usize, HashMap<StepField, String>> = BTreeMap::new();

    for (key, value) in pairs {
        match key.as_str() {
            IMAGE_IDS_FIELD => images.push(value.clone()),
            TITLE_FIELD => set_once(&mut title, value, key)?,
            DESCRIPTION_FIELD => set_once(&mut description, value, key)?,
            SERVINGS_TYPE_FIELD => set_once(&mut servings_type, value, key)?,
            SERVINGS_COUNT_FIELD => set_once(&mut servings_count, value, key)?,
            SERVINGS_LABEL_FIELD => set_once(&mut servings_label, value, key)?,
            PREPARATION_FIELD => set_once(&mut preparation, value, key)?,
            REST_FIELD => set_once(&mut rest, value, key)?,
            COOKING_FIELD => set_once(&mut cooking, value, key)?,
            MESSAGE_FIELD => set_once(&mut message, value, key)?,
            LATEST_REVISION_FIELD => set_once(&mut latest_revision, value, key)?,
            _ => {
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
        servings: parse_servings(
            servings_type.as_deref(),
            servings_count.as_deref(),
            servings_label.as_deref(),
        )?,
        preparation: parse_optional_minutes(preparation.as_deref(), PREPARATION_FIELD)?,
        rest: parse_optional_minutes(rest.as_deref(), REST_FIELD)?,
        cooking: parse_optional_minutes(cooking.as_deref(), COOKING_FIELD)?,
        ingredients: parse_ingredient_rows(ingredients)?,
        images: parse_image_ids(images)?,
        steps: parse_step_rows(steps, step_images)?,
        message,
        latest_revision,
    })
}

pub(crate) struct RecipeFormSubmission {
    pub fields: Vec<(String, String)>,
    pub uploads: Vec<RecipeFormUpload>,
}

pub(crate) struct RecipeFormUpload {
    field: String,
    bytes: Vec<u8>,
}

pub(crate) async fn read_recipe_form(multipart: &mut Multipart) -> Result<RecipeFormSubmission> {
    let mut fields = Vec::new();
    let mut uploads = Vec::new();

    while let Some(part) = multipart.next_field().await? {
        let Some(field_name) = part.name().map(str::to_owned) else {
            continue;
        };

        let file_name = part.file_name().map(str::to_owned);

        if let Some(file_name) = file_name {
            let bytes = part.bytes().await?;

            if file_name.is_empty() && bytes.is_empty() {
                continue;
            }

            uploads.push(RecipeFormUpload {
                field: field_name,
                bytes: bytes.to_vec(),
            });
        } else {
            fields.push((field_name, part.text().await?));
        }
    }

    Ok(RecipeFormSubmission { fields, uploads })
}

pub(crate) fn create_recipe_request(
    submission: RecipeFormSubmission,
    author: UserId,
) -> Result<CreateRecipeRequest> {
    let mut form = parse_recipe_form(&submission.fields)?;
    let images = parse_recipe_uploads(&mut form, submission.uploads)?;

    Ok(CreateRecipeRequest {
        content: form.into_content(),
        author,
        source_images: Vec::new(),
        images,
        verification: RevisionVerification::Verified,
    })
}

pub(crate) fn update_recipe_request(
    submission: RecipeFormSubmission,
    id: RecipeIdValue,
    editor: UserId,
) -> Result<UpdateRecipeRequest> {
    let mut form = parse_recipe_form(&submission.fields)?;
    let images = parse_recipe_uploads(&mut form, submission.uploads)?;

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
        new_images: images,
    })
}

fn parse_recipe_uploads(
    form: &mut RecipeFormData,
    uploads: Vec<RecipeFormUpload>,
) -> Result<Vec<NewRecipeImage>> {
    let mut images = Vec::with_capacity(uploads.len());

    for upload in uploads {
        let image_ids = if upload.field == IMAGES_FIELD {
            &mut form.images
        } else if let Some((row, ())) = parse_indexed_key(&upload.field, "steps", |wire_field| {
            (wire_field == StepField::Images.wire_name()).then_some(())
        })? {
            &mut form
                .steps
                .iter_mut()
                .find(|step| step.row == row)
                .ok_or_else(|| {
                    invalid(format!(
                        "image upload `{}` targets an unknown step row",
                        upload.field
                    ))
                })?
                .images
        } else {
            return Err(invalid(format!(
                "unknown image upload field `{}`",
                upload.field
            )));
        };

        let content_type = detect_image_content_type(&upload.bytes).ok_or_else(|| {
            invalid(format!(
                "image upload `{}` is not a supported JPEG or PNG image",
                upload.field
            ))
        })?;
        let id = RecipeImageId::generate();
        image_ids.push(id.clone());

        images.push(NewRecipeImage::new(
            id,
            content_type.to_string(),
            upload.bytes,
        ));
    }

    Ok(images)
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
) -> Result<Vec<StepFormData>> {
    let mut seen_ids = HashSet::new();
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
    use std::time::SystemTime;

    use cookbook_domain::recipe::models::{Recipe, RecipeRevision};

    use super::*;

    fn pairs(entries: &[(&str, &str)]) -> Vec<(String, String)> {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn submission(entries: &[(&str, &str)]) -> RecipeFormSubmission {
        RecipeFormSubmission {
            fields: pairs(entries),
            uploads: Vec::new(),
        }
    }

    #[test]
    fn inserts_step_keys_at_a_display_position() {
        assert_eq!(insert_step_key(vec![1, 2], 1, 3), vec![1, 3, 2]);
        assert_eq!(insert_step_key(vec![1, 2], 2, 3), vec![1, 2, 3]);
        assert_eq!(insert_step_key(vec![1, 2], 9, 3), vec![1, 2, 3]);
        assert_eq!(insert_step_key(Vec::new(), 0, 3), vec![3]);
    }

    #[test]
    fn removes_step_keys() {
        assert_eq!(remove_step_key(vec![1, 2, 3], 2), vec![1, 3]);
        assert_eq!(remove_step_key(vec![1, 2, 3], 9), vec![1, 2, 3]);
    }

    #[test]
    fn moves_step_keys_one_position_and_stops_at_the_edges() {
        assert_eq!(move_step_key(vec![1, 2, 3], 2, true), vec![2, 1, 3]);
        assert_eq!(move_step_key(vec![1, 2, 3], 2, false), vec![1, 3, 2]);
        assert_eq!(move_step_key(vec![1, 2, 3], 1, true), vec![1, 2, 3]);
        assert_eq!(move_step_key(vec![1, 2, 3], 3, false), vec![1, 2, 3]);
        assert_eq!(move_step_key(vec![1, 2, 3], 9, true), vec![1, 2, 3]);
    }

    #[test]
    fn removes_every_reference_to_a_step_image() {
        assert_eq!(
            remove_step_image_id(vec!["a".to_string(), "b".to_string(), "a".to_string()], "a"),
            vec!["b".to_string()]
        );
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
        assert_eq!(form.preparation, Some(10));
        assert_eq!(form.rest, Some(20));
        assert_eq!(form.cooking, Some(30));
        assert_eq!(form.message, None);
        assert_eq!(form.latest_revision, None);
        assert_eq!(form.ingredients.len(), 2);
        assert_eq!(form.ingredients[0].name, "Flour");
        assert_eq!(
            form.ingredients[0].quantity,
            Quantity::from_measure(200.0, Unit::Gram)
        );
        assert_eq!(form.ingredients[1].name, "Milk");
        assert_eq!(
            form.ingredients[1].quantity,
            Quantity::from_measure(0.5, Unit::Litre)
        );
        assert_eq!(form.steps.len(), 1);
        assert_eq!(form.steps[0].title.as_deref(), Some("Mix"));
        assert_eq!(form.steps[0].instruction, "Stir everything together.");
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
                form.ingredients[0].quantity,
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

        assert_eq!(form.ingredients[0].name, "Salt");
        assert_eq!(
            form.ingredients[0].quantity,
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
            form.ingredients[0].quantity,
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
            form.ingredients[0].quantity,
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
            IngredientField::AmountType,
            IngredientField::Quantity,
            IngredientField::QuantityText,
            IngredientField::Unit,
            IngredientField::UnitOther,
        ] {
            assert_eq!(
                parse_indexed_key(&wire_field.key(7), "ingredients", IngredientField::parse,)
                    .unwrap(),
                Some((7, wire_field))
            );
        }

        for wire_field in [StepField::Title, StepField::Instruction] {
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

        assert_eq!(form.images.len(), 2);
        assert_eq!(form.images[0].as_str(), "image-1");
        assert_eq!(form.images[1].as_str(), "image-2");
        assert_eq!(form.steps[0].images.len(), 2);
        assert_eq!(form.steps[0].images[0].as_str(), "step-image-1");
        assert_eq!(form.steps[0].images[1].as_str(), "step-image-2");
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

        assert_eq!(form.steps[0].images[0].as_str(), "shared-image");
        assert_eq!(form.steps[1].images[0].as_str(), "shared-image");
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
    fn converts_the_form_into_a_create_request() {
        let request = create_recipe_request(
            submission(&[
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
        assert_eq!(request.verification, RevisionVerification::Verified);
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
            needs_review: false,
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
            Some(Servings::portions_with_label(12, "cookies")),
            RecipeDurations::new(
                Some(DurationMinutes::new(10)),
                None,
                Some(DurationMinutes::new(20)),
            ),
            HashMap::from([
                (
                    IngredientName::new("milk".to_string()),
                    Quantity::from_measure(0.5, Unit::Litre),
                ),
                (
                    IngredientName::new("flour".to_string()),
                    Quantity::from_measure(200.0, Unit::Gram),
                ),
                (
                    IngredientName::new("salt".to_string()),
                    Quantity::from_text("to taste"),
                ),
                (
                    IngredientName::new("yeast".to_string()),
                    Quantity::from_measure(1.0, Unit::Other("sachet".to_string())),
                ),
            ]),
            Vec::new(),
            vec![RecipeStep::new(
                RecipeStepId::generate(),
                Some("Mix".to_string()),
                "Stir everything together.".to_string(),
                Vec::new(),
            )],
        );
        let revision = RecipeRevision::new(
            RecipeRevisionId::new("revision-1".to_string()),
            recipe.id().clone(),
            content,
            UserId::new("user-1".to_string()),
            SystemTime::UNIX_EPOCH,
            None,
            RevisionVerification::Verified,
        );
        let current = RecipeWithContent::new(recipe, revision).unwrap();

        let initial = FormInitial::from_recipe(&current, &[], "soup", &Cx::default());

        assert_eq!(initial.title, "Soup");
        assert_eq!(initial.description, "A warming soup.");
        assert_eq!(initial.servings_type, "portions");
        assert_eq!(initial.servings_count, "12");
        assert_eq!(initial.servings_label, "cookies");
        assert_eq!(initial.preparation, "10");
        assert_eq!(initial.rest, "");
        assert_eq!(initial.cooking, "20");
        assert_eq!(initial.ingredients[0].name, "flour");
        assert_eq!(initial.ingredients[0].amount_type, "measured");
        assert_eq!(initial.ingredients[0].quantity, "200");
        assert_eq!(initial.ingredients[0].quantity_text, "");
        assert_eq!(initial.ingredients[0].unit, "gram");
        assert_eq!(initial.ingredients[0].unit_other, "");
        assert_eq!(initial.ingredients[1].name, "milk");
        assert_eq!(initial.ingredients[1].amount_type, "measured");
        assert_eq!(initial.ingredients[1].quantity, "0.5");
        assert_eq!(initial.ingredients[1].unit, "litre");
        assert_eq!(initial.ingredients[2].name, "salt");
        assert_eq!(initial.ingredients[2].amount_type, "text");
        assert_eq!(initial.ingredients[2].quantity, "");
        assert_eq!(initial.ingredients[2].quantity_text, "to taste");
        assert_eq!(initial.ingredients[3].name, "yeast");
        assert_eq!(initial.ingredients[3].amount_type, "measured");
        assert_eq!(initial.ingredients[3].quantity, "1");
        assert_eq!(initial.ingredients[3].unit, "other");
        assert_eq!(initial.ingredients[3].unit_other, "sachet");
        assert_eq!(initial.steps[0].title, "Mix");
        assert_eq!(initial.steps[0].instruction, "Stir everything together.");
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
            submission(&[
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
                submission(&[("title", "Soup")]),
                RecipeIdValue::new("soup".to_string()),
                UserId::new("editor-1".to_string()),
            )
            .is_err()
        );
    }

    #[test]
    fn update_request_treats_a_blank_message_as_missing() {
        let request = update_recipe_request(
            submission(&[
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

    #[test]
    fn preserves_submitted_step_ids_and_generates_missing_ids() {
        let form = parse_recipe_form(&pairs(&[
            ("title", "Soup"),
            ("steps[0][id]", "step-existing"),
            ("steps[0][instruction]", "Stir."),
            ("steps[1][instruction]", "Bake."),
        ]))
        .unwrap();

        assert_eq!(form.steps[0].id.as_str(), "step-existing");
        assert_ne!(form.steps[1].id.as_str(), "");
        assert_ne!(form.steps[0].id, form.steps[1].id);
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
    fn parses_recipe_and_step_image_uploads() {
        let mut form = parse_recipe_form(&pairs(&[
            ("title", "Soup"),
            ("steps[0][id]", "step-1"),
            ("steps[0][instruction]", "Stir."),
        ]))
        .unwrap();
        let png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];

        let images = parse_recipe_uploads(
            &mut form,
            vec![
                RecipeFormUpload {
                    field: IMAGES_FIELD.to_string(),
                    bytes: png.clone(),
                },
                RecipeFormUpload {
                    field: "steps[0][images]".to_string(),
                    bytes: png,
                },
            ],
        )
        .unwrap();

        assert_eq!(images.len(), 2);
        assert_eq!(images[0].content_type(), "image/png");
        assert_eq!(form.images, vec![images[0].id().clone()]);
        assert_eq!(form.steps[0].images, vec![images[1].id().clone()]);
    }

    #[test]
    fn rejects_an_image_for_an_unknown_step_row() {
        let mut form = parse_recipe_form(&pairs(&[("title", "Soup")])).unwrap();

        let result = parse_recipe_uploads(
            &mut form,
            vec![RecipeFormUpload {
                field: "steps[4][images]".to_string(),
                bytes: vec![0xFF, 0xD8, 0xFF, 1],
            }],
        );

        assert!(result.is_err());
    }

    #[test]
    fn rejects_an_unsupported_image_upload() {
        let mut form = parse_recipe_form(&pairs(&[("title", "Soup")])).unwrap();

        let result = parse_recipe_uploads(
            &mut form,
            vec![RecipeFormUpload {
                field: IMAGES_FIELD.to_string(),
                bytes: b"not an image".to_vec(),
            }],
        );

        assert!(result.is_err());
    }
}
