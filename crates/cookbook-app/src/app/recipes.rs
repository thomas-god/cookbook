use std::time::SystemTime;

use cookbook_domain::recipe::models::{
    NewRecipeSourceImage, RecipeId as RecipeIdValue, RecipeRevision, RecipeRevisionId,
    RecipeSourceImageId as RecipeSourceImageIdValue, RevisionVerification,
};
use cookbook_domain::recipe::ports::{
    AddRecipeSourceImagesError, AddRecipeSourceImagesRequest, DeleteRecipeSourceImageError,
    RecipeService, UpdateRecipeError,
};
use cookbook_domain::user::models::UserId;
use cookbook_infra::auth::ports::UserRepository;
use cookbook_vision::detect_image_content_type;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        Body,
        content::{Form, multipart::Multipart},
        error::{RouterErrorExt, SeeOther, bad_request, not_found, see_other},
        href, page, path_param,
        response::Response,
        route,
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
            source_images::{source_image_editor, source_image_gallery},
        },
    },
    t,
};

pub(crate) const MAX_UPLOAD_BYTES: usize = 100 * 1024 * 1024;

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
                        if recipe.needs_review() {
                            <span
                                class="ml-2 inline-flex items-center rounded-full border border-warning/40 bg-warning/10 px-2 py-0.5 text-xs font-medium text-warning-foreground"
                            >
                                (t!(recipe_needs_review))
                            </span>
                        }
                    </li>
                }
            </ul>
        }
    })
}

path_param!(pub recipe_id);
path_param!(revision_id);
path_param!(pub source_image_id);

#[page("./{recipe_id}")]
pub async fn recipe_detail(cx: &Cx) -> Result<impl View> {
    auth::require_user(cx)?;

    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id))
        .await?
        .ok_or_not_found()?;
    let source_images = recipe_service
        .get_source_images(recipe.id().clone())
        .await?;

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
                    <div class="flex flex-wrap items-center gap-3">
                        <h2 class="text-3xl font-semibold">(recipe.title().as_str())</h2>
                        if recipe.needs_review() {
                            <span
                                class="inline-flex items-center rounded-full border border-warning/40 bg-warning/10 px-2.5 py-0.5 text-sm font-medium text-warning-foreground"
                            >
                                (t!(recipe_needs_review))
                            </span>
                        }
                    </div>
                    <div class="flex flex-col items-end gap-1 text-sm">
                        <a
                            class="underline underline-offset-4"
                            href=(href!(edit_recipe, RecipeId(recipe.id().as_str())))
                        >
                            if recipe.needs_review() {
                                (t!(recipe_verify))
                            } else {
                                (t!(recipe_edit))
                            }
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
                if recipe.needs_review() {
                    <p
                        role="status"
                        class="mt-4 rounded-lg border border-warning/40 bg-warning/10 px-4 py-3 text-sm text-warning-foreground"
                    >
                        (t!(recipe_needs_review_notice))
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
            source_image_gallery(
                recipe_id: recipe.id().as_str(),
                images: &source_images
            )
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
    let source_images = recipe_service
        .get_source_images(recipe.id().clone())
        .await?;
    let needs_review = recipe.needs_review();
    let mode = RecipeFormMode::Edit {
        recipe_id: RecipeIdValue::new(recipe_id.clone()),
        latest_revision: RecipeRevisionId::new(recipe.revision().id().as_str().to_string()),
        needs_review,
    };
    let form_title = if needs_review {
        t!(form_verify_recipe).resolve(cx).into_owned()
    } else {
        t!(form_edit_recipe).resolve(cx).into_owned()
    };

    Ok(view! {
        <div class="flex flex-col gap-6">
            card(
                card_header(card_title((form_title)))
                card_content(recipe_form(mode: mode))
            )
            card(
                card_header(card_title((t!(source_images_heading))))
                card_content(
                    source_image_editor(
                        recipe_id: recipe_id.as_str(),
                        images: &source_images
                    )
                )
            )
        </div>
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

#[route(GET "./{recipe_id}/source-images/{source_image_id}")]
pub async fn source_image(cx: &Cx) -> Result<Response> {
    auth::require_user(cx)?;

    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let image_id = path_param::<SourceImageId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);
    let image = recipe_service
        .get_source_image(
            RecipeIdValue::new(recipe_id),
            RecipeSourceImageIdValue::new(image_id),
        )
        .await?
        .ok_or_not_found()?;

    Ok(Response::builder()
        .header("Content-Type", image.content_type())
        .header("Cache-Control", "private, max-age=3600")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(image.bytes().to_vec()))?)
}

#[route(POST "./{recipe_id}/source-images")]
pub async fn upload_source_images(cx: &Cx, mut multipart: Multipart) -> Result<SeeOther> {
    let user = auth::require_user(cx)?;
    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let mut images = Vec::new();

    while let Some(part) = multipart.next_field().await? {
        if part.name() != Some("images") {
            continue;
        }

        let original_name = part.file_name().map(str::to_owned);
        let bytes = part.bytes().await?;
        let Some(content_type) = detect_image_content_type(&bytes) else {
            return Err(bad_request(t!(source_images_unsupported).resolve(cx)).into());
        };

        images.push(NewRecipeSourceImage::new(
            content_type.to_string(),
            original_name,
            bytes.to_vec(),
        ));
    }

    if images.is_empty() {
        return Err(bad_request(t!(source_images_upload_empty).resolve(cx)).into());
    }

    let recipe_service: &AppRecipeService = app_context(cx);

    match recipe_service
        .add_source_images(AddRecipeSourceImagesRequest {
            recipe_id: RecipeIdValue::new(recipe_id.clone()),
            added_by: user.id().clone(),
            images,
        })
        .await
    {
        Ok(_) => Ok(see_other(
            href!(edit_recipe, RecipeId(recipe_id.as_str())).resolve(cx),
        )),
        Err(AddRecipeSourceImagesError::RecipeNotFound) => Err(not_found().into()),
        Err(error) => Err(error.into()),
    }
}

// Not DELETE since its driven by an HTTP form that can only use POST
#[route(POST "./{recipe_id}/source-images/{source_image_id}/delete")]
pub async fn delete_source_image(cx: &Cx) -> Result<SeeOther> {
    auth::require_user(cx)?;

    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let image_id = path_param::<SourceImageId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);

    match recipe_service
        .delete_source_image(
            RecipeIdValue::new(recipe_id.clone()),
            RecipeSourceImageIdValue::new(image_id),
        )
        .await
    {
        Ok(_) => Ok(see_other(
            href!(edit_recipe, RecipeId(recipe_id.as_str())).resolve(cx),
        )),
        Err(DeleteRecipeSourceImageError::NotFound) => Err(not_found().into()),
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
                                        if entry.revision.verification() == RevisionVerification::NeedsReview {
                                            <p class="font-medium">(t!(recipe_history_imported))</p>
                                        } else {
                                            <p class="font-medium">(t!(recipe_history_created))</p>
                                        }
                                    } else {
                                        <p class="font-medium">(t!(recipe_history_edited))</p>
                                    }
                                    <p class="mt-1">
                                        if entry.revision.verification() == RevisionVerification::NeedsReview {
                                            <span
                                                class="inline-flex items-center rounded-full border border-warning/40 bg-warning/10 px-2 py-0.5 text-xs font-medium text-warning-foreground"
                                            >
                                                (t!(recipe_needs_review))
                                            </span>
                                        } else {
                                            <span
                                                class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-xs font-medium text-muted-foreground"
                                            >
                                                (t!(recipe_verified))
                                            </span>
                                        }
                                    </p>
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
                <div class="mt-3 flex flex-wrap items-center gap-3">
                    <h2 class="text-3xl font-semibold">
                        (revision.content().title().as_str())
                    </h2>
                    if revision.verification() == RevisionVerification::NeedsReview {
                        <span
                            class="inline-flex items-center rounded-full border border-warning/40 bg-warning/10 px-2.5 py-0.5 text-sm font-medium text-warning-foreground"
                        >
                            (t!(recipe_needs_review))
                        </span>
                    } else {
                        <span
                            class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-sm font-medium text-muted-foreground"
                        >
                            (t!(recipe_verified))
                        </span>
                    }
                </div>
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
