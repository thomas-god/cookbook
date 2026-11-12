use std::collections::HashMap;

use cookbook_domain::recipe::models::RecipeCategory;
use topcoat::{
    Result,
    context::Cx,
    router::href,
    runtime::{Event, Signal, procedure, signal},
    view::{View, attributes, component, element_id, view},
};

use crate::{
    app::recipes::{RecipeId, recipe_detail},
    components::{
        base::{
            button::{ButtonSize, ButtonVariant, button},
            field::{field, field_description, field_label, field_legend, field_set},
            image_upload::image_upload,
            input::input,
        },
        recipe::category::category_label,
    },
    t,
};

use super::{
    fields::*,
    initial::{BodyInitial, FormImageInitial, IngredientInitial, PartInitial, StepInitial},
};

pub(super) const TEXTAREA_CLASS: &str = "min-h-24 w-full rounded-lg border border-border bg-transparent px-3 py-2 text-sm transition-colors outline-none placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background";

pub(super) const SELECT_CLASS: &str = "h-9 w-full rounded-lg border border-border bg-transparent px-3 text-sm transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background";

/// Inserts a step slot at a display position, clamping past the end.
fn insert_order_key(mut order: Vec<usize>, index: usize, key: usize) -> Vec<usize> {
    order.insert(index.min(order.len()), key);
    order
}

/// Removes a step slot, if it is still present.
fn remove_order_key(mut order: Vec<usize>, key: usize) -> Vec<usize> {
    order.retain(|candidate| *candidate != key);
    order
}

/// Swaps a step slot with its neighbour. Boundary moves are no-ops.
fn move_order_key(mut order: Vec<usize>, key: usize, up: bool) -> Vec<usize> {
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
async fn form_insert_item(order: Vec<usize>, index: usize, key: usize) -> Result<Vec<usize>> {
    Ok(insert_order_key(order, index, key))
}

#[procedure]
async fn form_remove_item(order: Vec<usize>, key: usize) -> Result<Vec<usize>> {
    Ok(remove_order_key(order, key))
}

#[procedure]
async fn form_move_item(order: Vec<usize>, key: usize, up: bool) -> Result<Vec<usize>> {
    Ok(move_order_key(order, key, up))
}

#[procedure]
async fn form_remove_step_image(ids: Vec<String>, id: String) -> Result<Vec<String>> {
    Ok(remove_step_image_id(ids, &id))
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

/// Signals shared by every part row in a multipart form.
#[derive(Clone)]
struct PartListState {
    order: Signal<Vec<usize>>,
    next_key: Signal<usize>,
}

#[component]
pub(super) async fn metadata_fields(
    cx: &Cx,
    title: String,
    description: String,
    category: String,
    servings_type: String,
    servings_count: String,
    servings_label: String,
) -> Result<impl View> {
    let title = signal(cx, move || title.clone());
    let description = signal(cx, move || description.clone());
    let category = signal(cx, move || category.clone());

    let title_id = element_id(cx);
    let description_id = element_id(cx);
    let category_id = element_id(cx);

    Ok(view! {
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
            field(
                field_label(
                    attrs: attributes! { for=(&category_id) },
                    (t!(category_label))
                )
                <select
                    id=(&category_id)
                    name=(CATEGORY_FIELD)
                    class=(SELECT_CLASS)
                    :value=$(category.get())
                    @change=$(|e: Event| category.set(e.target.value))
                >
                    <option value="">(t!(category_none))</option>
                    for option_category in RecipeCategory::ALL {
                        <option value=(option_category.as_str())>
                            (category_label(cx, option_category))
                        </option>
                    }
                </select>
            )
            servings_fields(
                servings_type: servings_type,
                servings_count: servings_count,
                servings_label: servings_label
            )
        )
    })
}

#[component]
async fn servings_fields(
    cx: &Cx,
    servings_type: String,
    servings_count: String,
    servings_label: String,
) -> Result<impl View> {
    let servings_type_value = if servings_type.is_empty() {
        "none".to_string()
    } else {
        servings_type
    };
    let servings_type = signal(cx, move || servings_type_value.clone());
    let servings_count = signal(cx, move || servings_count.clone());
    let servings_label = signal(cx, move || servings_label.clone());

    let servings_type_id = element_id(cx);
    let servings_count_id = element_id(cx);
    let servings_label_id = element_id(cx);

    Ok(view! {
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
    })
}

#[component]
pub(super) async fn revision_message_field(cx: &Cx, needs_review: bool) -> Result<impl View> {
    let message = signal(cx, String::new);
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

    Ok(view! {
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
    })
}

#[component]
pub(super) async fn form_actions(
    cx: &Cx,
    recipe_id: String,
    needs_review: bool,
) -> Result<impl View> {
    let is_edit = !recipe_id.is_empty();
    let submit_label = if needs_review {
        t!(form_verify_and_save).resolve(cx).into_owned()
    } else {
        t!(form_save_recipe).resolve(cx).into_owned()
    };

    Ok(view! {
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
    })
}

#[component]
pub(super) async fn body_fields(
    prefix: String,
    initial: BodyInitial,
    image_lookup: HashMap<String, FormImageInitial>,
) -> Result<impl View> {
    let BodyInitial {
        preparation,
        rest,
        cooking,
        ingredients,
        images,
        steps,
    } = initial;

    Ok(view! {
        body_durations(
            prefix: prefix.clone(),
            preparation: preparation,
            rest: rest,
            cooking: cooking
        )
        body_images(prefix: prefix.clone(), initials: images)
        body_ingredients(
            list_id: "ingredients".to_string(),
            prefix: prefix.clone(),
            initials: ingredients
        )
        body_steps(
            list_id: "steps".to_string(),
            prefix: prefix,
            initials: steps,
            image_lookup: image_lookup
        )
    })
}

#[component]
async fn body_durations(
    cx: &Cx,
    prefix: String,
    preparation: String,
    rest: String,
    cooking: String,
) -> Result<impl View> {
    let preparation_field = body_field(&prefix, PREPARATION_FIELD);
    let rest_field = body_field(&prefix, REST_FIELD);
    let cooking_field = body_field(&prefix, COOKING_FIELD);

    let preparation = signal(cx, move || preparation.clone());
    let rest = signal(cx, move || rest.clone());
    let cooking = signal(cx, move || cooking.clone());

    let preparation_id = element_id(cx);
    let rest_id = element_id(cx);
    let cooking_id = element_id(cx);

    Ok(view! {
        <div class="grid gap-4 sm:grid-cols-3">
            field(
                field_label(
                    attrs: attributes! { for=(&preparation_id) },
                    (t!(form_preparation))
                )
                input(
                    attrs: attributes! {
                        id=(&preparation_id)
                        name=(preparation_field)
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
                        name=(rest_field)
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
                        name=(cooking_field)
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
    })
}

#[component]
async fn body_images(
    cx: &Cx,
    prefix: String,
    initials: Vec<FormImageInitial>,
) -> Result<impl View> {
    let image_ids_field = body_field(&prefix, IMAGE_IDS_FIELD);
    let images_field = body_field(&prefix, IMAGES_FIELD);
    let recipe_images_id = element_id(cx);

    Ok(view! {
        field_set(
            field_legend((t!(form_recipe_images)))
            if initials.is_empty() {
                <p class="text-sm text-muted-foreground">
                    (t!(form_recipe_images_empty))
                </p>
            } else {
                <ul class="grid grid-cols-2 gap-3 sm:grid-cols-3">
                    for image in &initials {
                        form_image_row(
                            image: image.clone(),
                            image_ids_field: image_ids_field.clone()
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
                    attrs: attributes! { id=(&recipe_images_id) name=(images_field) }
                )
                field_description((t!(form_recipe_images_upload_hint)))
            )
        )
    })
}

#[component]
async fn body_ingredients(
    cx: &Cx,
    list_id: String,
    prefix: String,
    initials: Vec<IngredientInitial>,
) -> Result<impl View> {
    let initial_count = initials.len().max(1);
    let count = signal(cx, move || initial_count);

    Ok(view! {
        field_set(
            field_legend((t!(form_ingredients)))
            <div id=(list_id) class="flex flex-col gap-3">
                #[key(index)]
                for index in 0..count.get() {
                    ingredient_row(
                        row: index,
                        prefix: prefix.clone(),
                        initial: initials.get(index).cloned()
                    )
                }
            </div>
            <div class="flex flex-wrap gap-2">
                button(
                    variant: ButtonVariant::Outline,
                    size: ButtonSize::Sm,
                    attrs: attributes! {
                        type="button"
                        @click=$(|_e: Event| count.increment())
                    },
                    (t!(form_add_ingredient))
                )
            </div>
        )
    })
}

#[component]
async fn body_steps(
    cx: &Cx,
    list_id: String,
    prefix: String,
    initials: Vec<StepInitial>,
    image_lookup: HashMap<String, FormImageInitial>,
) -> Result<impl View> {
    let initial_step_count = initials.len().max(1);
    let initial_step_order: Vec<usize> = (0..initial_step_count).collect();
    let step_order = signal(cx, move || initial_step_order.clone());
    let next_step_key = signal(cx, move || initial_step_count);
    let inserted_slot = signal(cx, || None::<usize>);
    let insert_title = signal(cx, String::new);
    let insert_instruction = signal(cx, String::new);
    let insert_images = signal(cx, Vec::<String>::new);
    let list = StepListState {
        order: step_order.clone(),
        next_key: next_step_key.clone(),
        inserted_slot: inserted_slot.clone(),
        insert_title: insert_title.clone(),
        insert_instruction: insert_instruction.clone(),
        insert_images: insert_images.clone(),
    };

    Ok(view! {
        field_set(
            field_legend((t!(form_steps)))
            <div id=(list_id) class="flex flex-col gap-4">
                #[key(step_key)]
                for (position, step_key) in step_order.get().into_iter().enumerate() {
                    step_row(
                        row: position,
                        key: step_key,
                        prefix: prefix.clone(),
                        initial: if inserted_slot.get() == Some(step_key) {
                            Some(
                                StepInitial {
                                    id: String::new(),
                                    title: insert_title.get(),
                                    instruction: insert_instruction.get(),
                                    images: insert_images.get(),
                                },
                            )
                        } else {
                            initials.get(step_key).cloned()
                        },
                        list: list.clone(),
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
                            let next = form_insert_item(order, index, new_key).await;
                            inserted_slot.set(None);
                            step_order.set(next);
                        })
                    },
                    (t!(form_add_step))
                )
            </div>
        )
    })
}

#[component]
pub(super) async fn parts_editor(
    cx: &Cx,
    initial: Vec<PartInitial>,
    image_lookup: HashMap<String, FormImageInitial>,
) -> Result<impl View> {
    let initial_part_count = initial.len().max(1);
    let initial_order: Vec<usize> = (0..initial_part_count).collect();
    let order = signal(cx, move || initial_order.clone());
    let next_key = signal(cx, move || initial_part_count);
    let list = PartListState {
        order: order.clone(),
        next_key: next_key.clone(),
    };

    Ok(view! {
        field_set(
            field_legend((t!(form_parts)))
            <div id="parts" class="flex flex-col gap-4">
                #[key(format!("part-{key}"))]
                for (position, key) in order.get().into_iter().enumerate() {
                    part_row(
                        row: position,
                        key: key,
                        initial: initial.get(key).cloned(),
                        list: list.clone(),
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
                            let new_key = next_key.get();
                            next_key.increment();
                            let current_order = order.get();
                            let index = current_order.len();
                            let next = form_insert_item(current_order, index, new_key).await;
                            order.set(next);
                        })
                    },
                    (t!(form_add_part))
                )
            </div>
        )
    })
}

#[component]
async fn part_row(
    cx: &Cx,
    row: usize,
    key: usize,
    initial: Option<PartInitial>,
    list: PartListState,
    image_lookup: HashMap<String, FormImageInitial>,
) -> Result<impl View> {
    let PartListState { order, next_key } = list;
    let initial = initial.unwrap_or_default();
    let id_value = initial.id;
    let title_value = initial.title;
    let description_value = initial.description;
    let BodyInitial {
        preparation,
        rest,
        cooking,
        ingredients,
        images,
        steps,
    } = initial.body;
    let title = signal(cx, move || title_value.clone());
    let description = signal(cx, move || description_value.clone());
    let prefix = format!("parts[{row}]");

    let title_id = element_id(cx);
    let description_id = element_id(cx);
    let id_field = PartField::Id.key(row);
    let title_field = PartField::Title.key(row);
    let description_field = PartField::Description.key(row);

    Ok(view! {
        <fieldset class="m-0 flex min-w-0 flex-col gap-4 rounded-lg border border-border p-4">
            <input type="hidden" name=(id_field) value=(id_value)>
            field(
                field_label(
                    attrs: attributes! { for=(&title_id) },
                    (t!(form_part_title))
                )
                input(
                    attrs: attributes! {
                        id=(&title_id)
                        name=(title_field)
                        type="text"
                        required=""
                        :value=$(title.get())
                        @input=$(|e: Event| title.set(e.target.value))
                    }
                )
            )
            field(
                field_label(
                    attrs: attributes! { for=(&description_id) },
                    (t!(form_part_description))
                )
                <textarea
                    id=(&description_id)
                    name=(description_field)
                    rows="2"
                    class=(TEXTAREA_CLASS)
                    :value=$(description.get())
                    @input=$(|e: Event| description.set(e.target.value))
                ></textarea>
            )

            body_durations(
                prefix: prefix.clone(),
                preparation: preparation,
                rest: rest,
                cooking: cooking
            )
            body_images(prefix: prefix.clone(), initials: images)
            body_ingredients(
                list_id: format!("part-{key}-ingredients"),
                prefix: prefix.clone(),
                initials: ingredients
            )
            body_steps(
                list_id: format!("part-{key}-steps"),
                prefix: prefix,
                initials: steps,
                image_lookup: image_lookup
            )

            <div class="flex flex-wrap items-center justify-end gap-2">
                button(
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Sm,
                    attrs: attributes! {
                        type="button"
                        aria-label=(t!(form_insert_part_below))
                        title=(t!(form_insert_part_below))
                        @click=$(async |_e: Event| {
                            let new_key = next_key.get();
                            next_key.increment();
                            let next = form_insert_item(order.get(), row + 1, new_key).await;
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
                        aria-label=(t!(form_move_part_up))
                        title=(t!(form_move_part_up))
                        :disabled=$(row == 0)
                        @click=$(async |_e: Event| {
                            let next = form_move_item(order.get(), key, true).await;
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
                        aria-label=(t!(form_move_part_down))
                        title=(t!(form_move_part_down))
                        :disabled=$(row + 1 == order.get().len())
                        @click=$(async |_e: Event| {
                            let next = form_move_item(order.get(), key, false).await;
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
                            let next = form_remove_item(order.get(), key).await;
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
async fn ingredient_row(
    cx: &Cx,
    row: usize,
    prefix: String,
    initial: Option<IngredientInitial>,
) -> Result<impl View> {
    let IngredientInitial {
        name,
        amount_type,
        quantity,
        quantity_text,
        unit,
        unit_other,
    } = initial.unwrap_or_default();

    let name = signal(cx, move || name.clone());
    let amount_type_value = if amount_type.is_empty() {
        "measured".to_string()
    } else {
        amount_type
    };
    let amount_type = signal(cx, move || amount_type_value.clone());
    let removed = signal(cx, || false);

    let name_id = element_id(cx);
    let amount_type_id = element_id(cx);

    let name_field = IngredientField::Name.key(row, &prefix);
    let amount_type_field = IngredientField::AmountType.key(row, &prefix);

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

            ingredient_amount_fields(
                row: row,
                prefix: prefix,
                amount_type: amount_type.clone(),
                quantity: quantity,
                quantity_text: quantity_text,
                unit: unit,
                unit_other: unit_other
            )
        </fieldset>
    })
}

#[component]
async fn ingredient_amount_fields(
    cx: &Cx,
    row: usize,
    prefix: String,
    amount_type: Signal<String>,
    quantity: String,
    quantity_text: String,
    unit: String,
    unit_other: String,
) -> Result<impl View> {
    let quantity = signal(cx, move || quantity.clone());
    let quantity_text = signal(cx, move || quantity_text.clone());
    let unit_value = if unit.is_empty() {
        "gram".to_string()
    } else {
        unit
    };
    let unit = signal(cx, move || unit_value.clone());
    let unit_other = signal(cx, move || unit_other.clone());

    let quantity_id = element_id(cx);
    let quantity_text_id = element_id(cx);
    let unit_id = element_id(cx);
    let unit_other_id = element_id(cx);

    let quantity_field = IngredientField::Quantity.key(row, &prefix);
    let quantity_text_field = IngredientField::QuantityText.key(row, &prefix);
    let unit_field = IngredientField::Unit.key(row, &prefix);
    let unit_other_field = IngredientField::UnitOther.key(row, &prefix);

    Ok(view! {
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
    prefix: String,
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

    let id_field = StepField::Id.key(row, &prefix);
    let title_field = StepField::Title.key(row, &prefix);
    let instruction_field = StepField::Instruction.key(row, &prefix);

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
            step_images_field(
                row: row,
                prefix: prefix.clone(),
                image_ids: step_image_ids.clone(),
                image_lookup: image_lookup
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
                            let next = form_insert_item(order.get(), row + 1, new_key).await;
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
                            let next = form_move_item(order.get(), key, true).await;
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
                            let next = form_move_item(order.get(), key, false).await;
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
                            let next = form_insert_item(order.get(), row + 1, new_key).await;
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
                            let next = form_remove_item(order.get(), key).await;
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
async fn step_images_field(
    cx: &Cx,
    row: usize,
    prefix: String,
    image_ids: Signal<Vec<String>>,
    image_lookup: HashMap<String, FormImageInitial>,
) -> Result<impl View> {
    let step_images_id = element_id(cx);
    let step_images_field = StepField::Images.key(row, &prefix);
    let image_ids_field = StepField::ImageIds.key(row, &prefix);

    Ok(view! {
        field(
            field_label(
                attrs: attributes! { for=(&step_images_id) },
                (t!(form_step_images))
            )
            if !image_ids.get().is_empty() {
                <ul class="mb-2 grid grid-cols-2 gap-3 sm:grid-cols-3">
                    #[key(image_id.clone())]
                    for image_id in image_ids.get() {
                        if let Some(image) = image_lookup.get(&image_id) {
                            step_image_row(
                                image: image.clone(),
                                image_ids: image_ids.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_order_keys_at_a_display_position() {
        assert_eq!(insert_order_key(vec![1, 2], 1, 3), vec![1, 3, 2]);
        assert_eq!(insert_order_key(vec![1, 2], 2, 3), vec![1, 2, 3]);
        assert_eq!(insert_order_key(vec![1, 2], 9, 3), vec![1, 2, 3]);
        assert_eq!(insert_order_key(Vec::new(), 0, 3), vec![3]);
    }

    #[test]
    fn removes_order_keys() {
        assert_eq!(remove_order_key(vec![1, 2, 3], 2), vec![1, 3]);
        assert_eq!(remove_order_key(vec![1, 2, 3], 9), vec![1, 2, 3]);
    }

    #[test]
    fn moves_order_keys_one_position_and_stops_at_the_edges() {
        assert_eq!(move_order_key(vec![1, 2, 3], 2, true), vec![2, 1, 3]);
        assert_eq!(move_order_key(vec![1, 2, 3], 2, false), vec![1, 3, 2]);
        assert_eq!(move_order_key(vec![1, 2, 3], 1, true), vec![1, 2, 3]);
        assert_eq!(move_order_key(vec![1, 2, 3], 3, false), vec![1, 2, 3]);
        assert_eq!(move_order_key(vec![1, 2, 3], 9, true), vec![1, 2, 3]);
    }

    #[test]
    fn removes_every_reference_to_a_step_image() {
        assert_eq!(
            remove_step_image_id(vec!["a".to_string(), "b".to_string(), "a".to_string()], "a"),
            vec!["b".to_string()]
        );
    }
}
