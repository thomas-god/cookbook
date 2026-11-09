use std::time::SystemTime;

use cookbook_domain::recipe::models::{
    RecipeId as RecipeIdValue, RecipeRevision, RecipeRevisionId,
};
use cookbook_domain::recipe::ports::{RecipeService, UpdateRecipeError};
use cookbook_domain::user::models::UserId;
use cookbook_infra::auth::ports::UserRepository;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Form,
        error::{RouterErrorExt, SeeOther, bad_request, not_found, see_other},
        href, page, path_param, route,
    },
    view::{View, view},
};

use crate::{
    app::{AppRecipeService, AppUserRepository, auth},
    components::{
        base::card::{card, card_content, card_header, card_title},
        recipe::{
            recipe_form::{RecipeFormMode, recipe_form},
            recipe_view::recipe_view,
        },
    },
    t,
};

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    auth::require_user(cx)?;

    let recipe_service: &AppRecipeService = app_context(cx);
    let recipes = recipe_service.get_recipes().await?;

    Ok(view! {
        <h2 class="mb-6 text-2xl font-semibold">(t!(recipes_list_heading))</h2>

        if recipes.is_empty() {
            <p class="mt-8 text-sm text-muted-foreground">(t!(recipes_empty))</p>
        } else {
            <ul class="mt-8 flex flex-col gap-2">
                for recipe in &recipes {
                    <li
                        class="rounded-lg border border-border bg-card px-4 py-3 text-sm shadow-sm"
                    >
                        <a
                            class="font-medium underline-offset-4 hover:underline"
                            href=(href!(recipe_detail, RecipeId(recipe.id().as_str())))
                        >
                            (recipe.title().as_str())
                        </a>
                    </li>
                }
            </ul>
        }
    })
}

path_param!(pub recipe_id);
path_param!(revision_id);

#[page("./{recipe_id}")]
pub async fn recipe_detail(cx: &Cx) -> Result<impl View> {
    auth::require_user(cx)?;

    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id))
        .await?
        .ok_or_not_found()?;

    let author_name = display_name(cx, recipe.author()).await?;
    let total_minutes = recipe.metadata().total_duration().as_minutes();

    Ok(view! {
        <div class="flex flex-col gap-8">
            <div>
                <a
                    class="text-sm text-muted-foreground underline-offset-4 hover:underline"
                    href=(href!(page))
                >
                    (t!(recipe_back_to_list))
                </a>
                <div class="mt-3 flex items-start justify-between gap-4">
                    <h2 class="text-3xl font-semibold">(recipe.title().as_str())</h2>
                    <div class="flex flex-col items-end gap-1 text-sm">
                        <a
                            class="underline underline-offset-4"
                            href=(href!(edit_recipe, RecipeId(recipe.id().as_str())))
                        >
                            (t!(recipe_edit))
                        </a>
                        <a
                            class="underline underline-offset-4"
                            href=(href!(recipe_history, RecipeId(recipe.id().as_str())))
                        >
                            (t!(recipe_history))
                        </a>
                    </div>
                </div>
                if let Some(description) = recipe.description() {
                    <p class="mt-2 text-sm text-muted-foreground">
                        (description.as_str())
                    </p>
                }
                <p class="mt-2 text-sm text-muted-foreground">
                    (t!(recipe_author))
                    ": "
                    (author_name)
                </p>
                if total_minutes > 0 {
                    <p class="mt-2 text-sm text-muted-foreground">
                        (t!(recipe_total_duration))
                        ": "
                        (total_minutes)
                        " "
                        (t!(recipe_minutes))
                    </p>
                }
            </div>

            recipe_view(content: recipe.content())
        </div>
    })
}

#[page("./{recipe_id}/edit")]
pub async fn edit_recipe(cx: &Cx) -> Result<impl View> {
    auth::require_user(cx)?;

    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id.clone()))
        .await?
        .ok_or_not_found()?;
    let mode = RecipeFormMode::Edit {
        recipe_id: RecipeIdValue::new(recipe_id),
        latest_revision: RecipeRevisionId::new(recipe.revision().id().as_str().to_string()),
    };

    Ok(view! {
        card(
            card_header(card_title((t!(form_edit_recipe))))
            card_content(recipe_form(mode: mode))
        )
    })
}

#[route(POST "./{recipe_id}/edit")]
pub async fn update_recipe(cx: &Cx, Form(form): Form<Vec<(String, String)>>) -> Result<SeeOther> {
    let user = auth::require_user(cx)?;
    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);
    let request = crate::components::recipe::recipe_form::update_recipe_request(
        form,
        RecipeIdValue::new(recipe_id.clone()),
        user.id().clone(),
    )?;

    match recipe_service.update_recipe(request).await {
        Ok(_) => Ok(see_other(
            href!(recipe_detail, RecipeId(recipe_id.as_str())).resolve(cx),
        )),
        Err(UpdateRecipeError::Conflict) => {
            Err(bad_request(t!(recipe_conflict).resolve(cx)).into())
        }
        Err(UpdateRecipeError::NotFound) => Err(not_found().into()),
        Err(error) => Err(error.into()),
    }
}

struct HistoryEntry {
    revision: RecipeRevision,
    editor: String,
    is_first: bool,
}

#[page("./{recipe_id}/history")]
pub async fn recipe_history(cx: &Cx) -> Result<impl View> {
    auth::require_user(cx)?;

    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id.clone()))
        .await?
        .ok_or_not_found()?;
    let revisions = recipe_service
        .get_recipe_history(RecipeIdValue::new(recipe_id.clone()))
        .await?;

    let total = revisions.len();
    let mut entries = Vec::with_capacity(total);

    for (index, revision) in revisions.iter().rev().enumerate() {
        entries.push(HistoryEntry {
            revision: revision.clone(),
            editor: display_name(cx, revision.editor()).await?,
            is_first: index + 1 == total,
        });
    }

    Ok(view! {
        <div class="flex flex-col gap-6">
            <a
                class="text-sm text-muted-foreground underline-offset-4 hover:underline"
                href=(href!(recipe_detail, RecipeId(recipe_id.as_str())))
            >
                (t!(recipe_back_to_recipe))
            </a>
            <h2 class="text-2xl font-semibold">
                (t!(recipe_history_heading))
                ": "
                (recipe.title().as_str())
            </h2>

            if entries.is_empty() {
                <p class="text-sm text-muted-foreground">(t!(recipe_history_empty))</p>
            } else {
                <ol class="flex flex-col gap-3">
                    for entry in entries {
                        <li
                            class="rounded-lg border border-border bg-card px-4 py-3 text-sm"
                        >
                            <div class="flex items-start justify-between gap-4">
                                <div>
                                    if let Some(message) = entry.revision.message() {
                                        <p class="font-medium">(message.as_str())</p>
                                    } else if entry.is_first {
                                        <p class="font-medium">(t!(recipe_history_created))</p>
                                    } else {
                                        <p class="font-medium">(t!(recipe_history_edited))</p>
                                    }
                                    <p class="mt-1 text-muted-foreground">
                                        (t!(recipe_history_edited_by))
                                        " "
                                        (entry.editor)
                                        " · "
                                        (format_timestamp(entry.revision.edited_at()))
                                    </p>
                                </div>
                                <a
                                    class="underline underline-offset-4"
                                    href=(href!(
                                        recipe_revision,
                                        RecipeId(recipe_id.as_str()),
                                        RevisionId(entry.revision.id().as_str()),
                                    ))
                                >
                                    (t!(recipe_history_view))
                                </a>
                            </div>
                        </li>
                    }
                </ol>
            }
        </div>
    })
}

#[page("./{recipe_id}/history/{revision_id}")]
pub async fn recipe_revision(cx: &Cx) -> Result<impl View> {
    auth::require_user(cx)?;

    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let revision_id = path_param::<RevisionId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id.clone()))
        .await?
        .ok_or_not_found()?;
    let revision = recipe_service
        .get_recipe_history(RecipeIdValue::new(recipe_id.clone()))
        .await?
        .into_iter()
        .find(|revision| revision.id().as_str() == revision_id.as_str())
        .ok_or_not_found()?;

    let editor = display_name(cx, revision.editor()).await?;
    let author_name = display_name(cx, recipe.author()).await?;
    let total_minutes = revision.content().metadata().total_duration().as_minutes();

    Ok(view! {
        <div class="flex flex-col gap-8">
            <div>
                <a
                    class="text-sm text-muted-foreground underline-offset-4 hover:underline"
                    href=(href!(recipe_history, RecipeId(recipe_id.as_str())))
                >
                    (t!(recipe_back_to_history))
                </a>
                <h2 class="mt-3 text-3xl font-semibold">
                    (revision.content().title().as_str())
                </h2>
                if let Some(description) = revision.content().description() {
                    <p class="mt-2 text-sm text-muted-foreground">
                        (description.as_str())
                    </p>
                }
                <p class="mt-2 text-sm text-muted-foreground">
                    (t!(recipe_history_edited_by))
                    " "
                    (editor)
                    " · "
                    (format_timestamp(revision.edited_at()))
                </p>
                if let Some(message) = revision.message() {
                    <p class="mt-2 text-sm">(message.as_str())</p>
                }
                if total_minutes > 0 {
                    <p class="mt-2 text-sm text-muted-foreground">
                        (t!(recipe_total_duration))
                        ": "
                        (total_minutes)
                        " "
                        (t!(recipe_minutes))
                    </p>
                }
                <p class="mt-2 text-sm text-muted-foreground">
                    (t!(recipe_author))
                    ": "
                    (author_name)
                </p>
                <p class="mt-4 text-sm">
                    <a
                        class="underline underline-offset-4"
                        href=(href!(recipe_detail, RecipeId(recipe_id.as_str())))
                    >
                        (t!(recipe_view_current))
                    </a>
                </p>
            </div>

            recipe_view(content: revision.content())
        </div>
    })
}

async fn display_name(cx: &Cx, user_id: &UserId) -> Result<String> {
    let user_repository: &AppUserRepository = app_context(cx);

    Ok(match user_repository.find_by_id(user_id).await? {
        Some(user) => user
            .username()
            .map(|username| username.as_str().to_string())
            .unwrap_or_else(|| user.email().as_str().to_string()),
        None => user_id.as_str().to_string(),
    })
}

fn format_timestamp(edited_at: SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Local> = edited_at.into();

    datetime.format("%Y-%m-%d %H:%M").to_string()
}
