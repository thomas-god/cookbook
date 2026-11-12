mod fields;
mod initial;
mod parse;
mod submission;
mod view;

use std::collections::HashMap;

use cookbook_domain::recipe::{
    models::{RecipeCategory, RecipeId as RecipeIdValue, RecipeRevisionId},
    ports::RecipeService,
};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::href,
    runtime::{Event, shard, signal},
    view::{View, attributes, component, element_id, view},
};

use crate::{
    app::{
        AppRecipeService, auth,
        recipes::{RecipeId, recipe_detail},
    },
    components::base::{
        button::{ButtonVariant, button},
        field::{field, field_label, field_legend, field_set},
        input::input,
    },
    components::recipe::category::category_label,
    t,
};

use self::{
    fields::{
        CATEGORY_FIELD, DESCRIPTION_FIELD, MESSAGE_FIELD, RECIPE_TYPE_FIELD, SERVINGS_COUNT_FIELD,
        SERVINGS_LABEL_FIELD, SERVINGS_TYPE_FIELD, TITLE_FIELD,
    },
    initial::{
        FormImageInitial, FormInitial, StructureInitial, form_image_initial, load_current_recipe,
    },
    view::{SELECT_CLASS, TEXTAREA_CLASS, body_fields, parts_editor},
};

pub(crate) use self::fields::LATEST_REVISION_FIELD;
pub(crate) use self::submission::{create_recipe_request, read_recipe_form, update_recipe_request};

#[derive(Clone)]
pub enum RecipeFormMode {
    New {
        multipart: bool,
    },
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
            Self::New { multipart: false } => "/recipe/new".to_string(),
            Self::New { multipart: true } => "/recipe/new/multipart".to_string(),
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
    let (recipe_id, latest_revision, needs_review, multipart) = match &mode {
        RecipeFormMode::New { multipart } => (String::new(), String::new(), false, *multipart),
        RecipeFormMode::Edit {
            recipe_id,
            latest_revision,
            needs_review,
        } => (
            recipe_id.as_str().to_string(),
            latest_revision.as_str().to_string(),
            *needs_review,
            false,
        ),
    };

    Ok(view! {
        cx =>
        recipe_form_fields(
            action: action,
            recipe_id: recipe_id,
            latest_revision: latest_revision,
            needs_review: needs_review,
            multipart: multipart
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
    multipart: bool,
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
        .unwrap_or_else(|| FormInitial::new(multipart));
    let is_edit = !latest_revision.is_empty();
    let is_multipart = matches!(initial.structure, StructureInitial::Multipart(_));
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
    let category_value = initial.category.clone();
    let category = signal(cx, move || category_value.clone());
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
    let message = signal(cx, String::new);

    let (simple_initial, part_initials) = match initial.structure {
        StructureInitial::Simple(body) => (Some(body), Vec::new()),
        StructureInitial::Multipart(parts) => (None, parts),
    };

    let latest_revision = (!latest_revision.is_empty()).then_some(latest_revision);

    let title_id = element_id(cx);
    let description_id = element_id(cx);
    let category_id = element_id(cx);
    let servings_type_id = element_id(cx);
    let servings_count_id = element_id(cx);
    let servings_label_id = element_id(cx);
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
            <input
                type="hidden"
                name=(RECIPE_TYPE_FIELD)
                value=(if is_multipart { "multipart" } else { "simple" })
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
            )

            if let Some(body_initial) = simple_initial {
                body_fields(
                    prefix: String::new(),
                    initial: body_initial,
                    image_lookup: image_lookup.clone()
                )
            } else {
                parts_editor(
                    initial: part_initials,
                    image_lookup: image_lookup.clone()
                )
            }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_mode_derives_the_form_action() {
        assert_eq!(
            RecipeFormMode::New { multipart: false }.action(),
            "/recipe/new"
        );
        assert_eq!(
            RecipeFormMode::New { multipart: true }.action(),
            "/recipe/new/multipart"
        );

        let edit = RecipeFormMode::Edit {
            recipe_id: RecipeIdValue::new("soup".to_string()),
            latest_revision: RecipeRevisionId::new("revision-1".to_string()),
            needs_review: false,
        };

        assert_eq!(edit.action(), "/recipes/soup/edit");
    }
}
