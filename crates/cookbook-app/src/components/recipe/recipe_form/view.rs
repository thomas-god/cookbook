use std::collections::HashMap;

use topcoat::{
    Result,
    context::Cx,
    runtime::{Event, Signal, procedure, signal},
    view::{View, attributes, component, element_id, view},
};

use crate::{
    components::base::{
        button::{ButtonSize, ButtonVariant, button},
        field::{field, field_description, field_label, field_legend, field_set},
        image_upload::image_upload,
        input::input,
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

#[procedure]
async fn form_insert_part(order: Vec<usize>, index: usize, key: usize) -> Result<Vec<usize>> {
    Ok(insert_step_key(order, index, key))
}

#[procedure]
async fn form_remove_part(order: Vec<usize>, key: usize) -> Result<Vec<usize>> {
    Ok(remove_step_key(order, key))
}

#[procedure]
async fn form_move_part(order: Vec<usize>, key: usize, up: bool) -> Result<Vec<usize>> {
    Ok(move_step_key(order, key, up))
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
                            let next = form_insert_step(order, index, new_key).await;
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
                            let next = form_insert_part(current_order, index, new_key).await;
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
                            let next = form_insert_part(order.get(), row + 1, new_key).await;
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
                            let next = form_move_part(order.get(), key, true).await;
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
                            let next = form_move_part(order.get(), key, false).await;
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
                            let next = form_remove_part(order.get(), key).await;
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

    let name_field = IngredientField::Name.key(row, &prefix);
    let amount_type_field = IngredientField::AmountType.key(row, &prefix);
    let quantity_field = IngredientField::Quantity.key(row, &prefix);
    let quantity_text_field = IngredientField::QuantityText.key(row, &prefix);
    let unit_field = IngredientField::Unit.key(row, &prefix);
    let unit_other_field = IngredientField::UnitOther.key(row, &prefix);

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
    let step_images_id = element_id(cx);

    let id_field = StepField::Id.key(row, &prefix);
    let title_field = StepField::Title.key(row, &prefix);
    let instruction_field = StepField::Instruction.key(row, &prefix);
    let step_images_field = StepField::Images.key(row, &prefix);
    let image_ids_field = StepField::ImageIds.key(row, &prefix);

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
