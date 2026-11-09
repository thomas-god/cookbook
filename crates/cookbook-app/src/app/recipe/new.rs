use cookbook_domain::recipe::ports::RecipeService;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Form,
        error::{SeeOther, see_other},
        page, route,
    },
    view::{View, view},
};

use crate::{
    app::{AppRecipeService, auth},
    components::{
        base::card::{card, card_content, card_header, card_title},
        recipe::recipe_form::{RecipeFormMode, create_recipe_request, recipe_form},
    },
    t,
};

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    auth::require_user(cx)?;

    Ok(view! {
        card(
            card_header(card_title((t!(form_new_recipe))))
            card_content(recipe_form(mode: RecipeFormMode::New))
        )
    })
}

#[route(POST)]
pub async fn create(cx: &Cx, Form(form): Form<Vec<(String, String)>>) -> Result<SeeOther> {
    let user = auth::require_user(cx)?;

    let recipe_service: &AppRecipeService = app_context(cx);

    recipe_service
        .create_recipe(create_recipe_request(form, user.id().clone())?)
        .await?;

    Ok(see_other("/recipes"))
}
