mod fields;
mod initial;
mod parse;
mod submission;
mod view;

use std::collections::HashMap;

use cookbook_domain::recipe::{
    models::{RecipeId as RecipeIdValue, RecipeRevisionId},
    ports::RecipeService,
};
use topcoat::{
    Result,
    context::{Cx, app_context},
    runtime::shard,
    view::{View, component, view},
};

use crate::{
    app::{AppRecipeService, auth},
    t,
};

use self::{
    fields::RECIPE_TYPE_FIELD,
    initial::{
        FormImageInitial, FormInitial, StructureInitial, form_image_initial, load_current_recipe,
    },
    view::{body_fields, form_actions, metadata_fields, parts_editor, revision_message_field},
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

    let user = auth::require_user(cx)?;
    let current = load_current_recipe(cx, &recipe_id, user.id()).await?;
    let images = if current.is_some() {
        let recipe_service: &AppRecipeService = app_context(cx);
        recipe_service
            .get_recipe_images(RecipeIdValue::new(recipe_id.clone()), user.id().clone())
            .await?
    } else {
        Vec::new()
    };
    let image_lookup: HashMap<String, FormImageInitial> = images
        .iter()
        .map(|image| {
            (
                image.id().as_str().to_string(),
                form_image_initial(image, &recipe_id, cx),
            )
        })
        .collect();
    let initial = current
        .as_ref()
        .map(|recipe| FormInitial::from_recipe(recipe, &image_lookup))
        .unwrap_or_else(|| FormInitial::new(multipart));
    let is_edit = !latest_revision.is_empty();
    let is_multipart = matches!(initial.structure, StructureInitial::Multipart(_));

    let FormInitial {
        title,
        description,
        category,
        servings_type,
        servings_count,
        servings_label,
        structure,
    } = initial;
    let (simple_initial, part_initials) = match structure {
        StructureInitial::Simple(body) => (Some(body), Vec::new()),
        StructureInitial::Multipart(parts) => (None, parts),
    };

    let latest_revision = (!latest_revision.is_empty()).then_some(latest_revision);

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
            metadata_fields(
                title: title,
                description: description,
                category: category,
                servings_type: servings_type,
                servings_count: servings_count,
                servings_label: servings_label
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
                revision_message_field(needs_review: needs_review)
            }

            form_actions(recipe_id: recipe_id, needs_review: needs_review)
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
