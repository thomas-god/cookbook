use cookbook_domain::recipe::models::{RecipeId as RecipeIdValue, RecipeWithContent};
use cookbook_domain::recipe::ports::RecipeService;
use cookbook_infra::auth::models::AuthenticatedUser;
use topcoat::icon::icon;
use topcoat::router::{href, page, path_param};
use topcoat::view::component;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::error::RouterErrorExt,
    view::{View, attributes, class, view},
};

use crate::app::recipe::cook::cook_recipe;
use crate::app::recipe::export::export_recipe;
use crate::app::recipes::{
    RecipeId, delete_recipe, display_name, edit_recipe, recipe_history, servings_label,
};
use crate::components::base::button::{ButtonSize, ButtonVariant, button_variants};
use crate::components::base::icons::{BACK_ARROW, MORE_VERT};
use crate::components::recipe::recipe_scope_form::recipe_scope_form;
use crate::components::recipe::source_documents::source_document_gallery;
use crate::{
    app::{AppRecipeService, auth},
    components::{
        base::button::button,
        recipe::{
            category::category_label, recipe_images::recipe_image_gallery, recipe_view::recipe_view,
        },
    },
    t,
};

#[page("/recipe/{recipe_id}")]
pub async fn recipe_detail(cx: &Cx) -> Result<impl View> {
    let user = auth::require_user(cx)?;

    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id), user.id().clone())
        .await?
        .ok_or_not_found()?;
    let source_documents = recipe_service
        .get_source_documents(recipe.id().clone(), user.id().clone())
        .await?;
    let images = recipe_service
        .get_recipe_images(recipe.id().clone(), user.id().clone())
        .await?;

    let is_author = recipe.author() == user.id();
    let scope = recipe.scope();
    let author_name = display_name(cx, recipe.author()).await?;
    let total_minutes = recipe.durations().total_duration().as_minutes();

    Ok(view! {
        <div class="flex flex-col gap-8">
            <div>
                <a
                    class="text-sm text-muted-foreground underline-offset-4 hover:underline italic opacity-80 flex items-center gap-1.5"
                    href=(href!(crate::app::recipes::page))
                >
                    icon(data: BACK_ARROW)
                    (t!(recipe_back_to_list))
                </a>
                <div class="mt-3 flex items-start justify-between gap-4">
                    <div class="flex flex-wrap items-center gap-3">
                        <h2 class="text-3xl font-semibold">
                            (recipe.title().as_str())
                        </h2>
                        if let Some(category) = recipe.category() {
                            <span
                                class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-sm font-medium text-muted-foreground"
                            >
                                (category_label(cx, category))
                            </span>
                        }
                    </div>
                    <div class="flex flex-row items-center gap-3 text-sm">
                        <a
                            class=(button_variants(
                                ButtonVariant::Primary,
                                ButtonSize::Sm,
                            ))
                            href=(href!(cook_recipe, RecipeId(recipe.id().as_str())))
                        >
                            (t!(recipe_cook))
                        </a>
                        recipe_menu(user: &user, recipe: &recipe)
                    </div>
                </div>

                if recipe.needs_review() {
                    <p
                        role="status"
                        class="mt-4 rounded-lg border border-warning/40 bg-warning/10 px-4 py-3 text-sm text-warning-foreground"
                    >
                        (t!(recipe_needs_review_notice))
                    </p>
                }
                <div
                    class="flex flex-row gap-2 items-center mt-2 text-sm text-muted-foreground"
                >
                    <span>
                        (t!(recipe_author))
                        ": "
                        (author_name)
                    </span>
                    if total_minutes > 0 {
                        <span>" · "</span>
                        <p>
                            (t!(recipe_total_duration))
                            ": "
                            (total_minutes)
                            " "
                            (t!(recipe_minutes))
                        </p>
                    }
                    if let Some(servings) = recipe.content().servings() {
                        <span>" · "</span>
                        <p>
                            (t!(recipe_servings))
                            ": "
                            (servings_label(cx, servings))
                        </p>
                    }
                </div>
                if let Some(description) = recipe.description() {
                    <p class="mt-3 pl-3 text-md text-foreground">
                        (description.as_str())
                    </p>
                }
            </div>

            if let Some(body) = recipe.content().simple_body() {
                recipe_image_gallery(
                    recipe_id: recipe.id().as_str(),
                    image_ids: body.images(),
                    images: &images
                )
            }
            recipe_view(
                recipe_id: recipe.id().as_str(),
                content: recipe.content(),
                images: &images
            )
            source_document_gallery(
                recipe_id: recipe.id().as_str(),
                documents: &source_documents
            )
            if is_author {
                <section class="rounded-lg border border-border bg-card p-4">
                    <h3 class="mb-3 text-lg font-semibold">(t!(scope_heading))</h3>
                    recipe_scope_form(
                        recipe_id: recipe.id().as_str().to_string(),
                        scope: scope
                    )
                </section>
            }
        </div>
    })
}

#[component]
pub async fn recipe_menu(
    cx: &Cx,
    user: &AuthenticatedUser,
    recipe: &RecipeWithContent,
) -> Result<impl View> {
    let is_author = recipe.author() == user.id();
    let can_edit = recipe.can_edit(user.id());
    let delete_action = href!(delete_recipe, RecipeId(recipe.id().as_str())).resolve(cx);
    let delete_confirm = t!(recipe_delete_confirm).resolve(cx);
    Ok(view! {
        cx =>
        <details class="relative" data-menu="">
            <summary
                class=(class!(
                    button_variants(ButtonVariant::Ghost, ButtonSize::Icon),
                    "cursor-pointer list-none [&::-webkit-details-marker]:hidden",
                ))
                aria-label=(t!(recipe_actions))
                title=(t!(recipe_actions))
            >
                icon(data: MORE_VERT)
            </summary>
            <div
                class="absolute right-0 z-10 mt-2 flex w-48 flex-col gap-1 rounded-lg border border-border bg-card p-1 shadow-md"
            >
                if can_edit {
                    <a
                        class=(class!(
                            button_variants(ButtonVariant::Ghost, ButtonSize::Sm),
                            "w-full justify-start",
                        ))
                        href=(href!(edit_recipe, RecipeId(recipe.id().as_str())))
                    >
                        if recipe.needs_review() {
                            (t!(recipe_verify))
                        } else {
                            (t!(recipe_edit))
                        }
                    </a>
                }
                <a
                    class=(class!(
                        button_variants(ButtonVariant::Ghost, ButtonSize::Sm),
                        "w-full justify-start",
                    ))
                    href=(href!(recipe_history, RecipeId(recipe.id().as_str())))
                >
                    (t!(recipe_history))
                </a>
                <a
                    class=(class!(
                        button_variants(ButtonVariant::Ghost, ButtonSize::Sm),
                        "w-full justify-start",
                    ))
                    href=(href!(export_recipe, RecipeId(recipe.id().as_str())))
                >
                    (t!(recipe_export))
                </a>
                if is_author {
                    <form
                        method="post"
                        action=(delete_action)
                        onsubmit=(format!("return confirm({delete_confirm:?})"))
                    >
                        button(
                            variant: ButtonVariant::Ghost,
                            size: ButtonSize::Sm,
                            attrs: attributes! {
                                type="submit"
                                class="w-full justify-start text-destructive hover:bg-destructive/10"
                            },
                            (t!(recipe_delete))
                        )
                    </form>
                }
            </div>
        </details>
    })
}
