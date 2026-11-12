use cookbook_domain::recipe::ports::RecipeService;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::multipart::Multipart,
        error::{SeeOther, see_other},
        href, page, route,
    },
    view::{View, view},
};

use crate::{
    app::{AppRecipeService, auth},
    components::{
        base::card::{card, card_content, card_header, card_title},
        recipe::recipe_form::{
            RecipeFormMode, create_recipe_request, read_recipe_form, recipe_form,
        },
    },
    t,
};

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    auth::require_user(cx)?;

    Ok(view! {
        <div class="flex flex-col gap-4">
            <p class="text-sm">
                <a class="underline underline-offset-4" href=(href!(page_multipart))>
                    (t!(form_new_multipart_recipe))
                </a>
            </p>
            card(
                card_header(card_title((t!(form_new_recipe))))
                card_content(
                    recipe_form(mode: RecipeFormMode::New { multipart: false })
                )
            )
        </div>
    })
}

#[route(POST)]
pub async fn create(cx: &Cx, mut multipart: Multipart) -> Result<SeeOther> {
    let user = auth::require_user(cx)?;

    let recipe_service: &AppRecipeService = app_context(cx);
    let submission = read_recipe_form(&mut multipart).await?;

    recipe_service
        .create_verified_recipe(create_recipe_request(submission, user.id().clone())?)
        .await?;

    Ok(see_other("/recipes"))
}

#[page("./multipart")]
pub async fn page_multipart(cx: &Cx) -> Result<impl View> {
    auth::require_user(cx)?;

    Ok(view! {
        <div class="flex flex-col gap-4">
            <p class="text-sm">
                <a class="underline underline-offset-4" href=(href!(page))>
                    (t!(form_new_simple_recipe))
                </a>
            </p>
            card(
                card_header(card_title((t!(form_new_multipart_recipe))))
                card_content(recipe_form(mode: RecipeFormMode::New { multipart: true }))
            )
        </div>
    })
}

#[route(POST "./multipart")]
pub async fn create_multipart(cx: &Cx, mut multipart: Multipart) -> Result<SeeOther> {
    let user = auth::require_user(cx)?;

    let recipe_service: &AppRecipeService = app_context(cx);
    let submission = read_recipe_form(&mut multipart).await?;

    recipe_service
        .create_verified_recipe(create_recipe_request(submission, user.id().clone())?)
        .await?;

    Ok(see_other("/recipes"))
}
