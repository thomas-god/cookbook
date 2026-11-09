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
    app::{
        AppRecipeService,
        recipe_form::{create_recipe_request, recipe_form},
    },
    components::card::{card, card_content, card_header, card_title},
    t,
};

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipes = recipe_service.get_recipes().await?;

    Ok(view! {
        <h2 class="mb-6 text-2xl font-semibold">(t!(recipes_list_heading))</h2>
        card(
            card_header(
                card_title((t!(form_new_recipe)))
            )
            card_content(
                recipe_form()
            )
        )
        if recipes.is_empty() {
            <p class="mt-8 text-sm text-muted-foreground">(t!(recipes_empty))</p>
        } else {
            <ul class="mt-8 flex flex-col gap-2">
                for recipe in &recipes {
                    <li class="rounded-lg border border-border bg-card px-4 py-3 text-sm shadow-sm">
                        (recipe.title().as_str())
                    </li>
                }
            </ul>
        }
    })
}

#[route(POST)]
pub async fn create(cx: &Cx, Form(form): Form<Vec<(String, String)>>) -> Result<SeeOther> {
    let recipe_service: &AppRecipeService = app_context(cx);

    recipe_service
        .create_recipe(create_recipe_request(form)?)
        .await?;

    Ok(see_other("/recipes"))
}
