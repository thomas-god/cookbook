use std::collections::HashMap;

use cookbook_domain::recipe::{
    models::{RecipeMetadata, RecipeTitle},
    ports::{CreateRecipeRequest, RecipeService},
};
use serde::Deserialize;
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

use crate::app::AppRecipeService;

#[derive(Deserialize)]
struct CreateRecipeForm {
    title: String,
}

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipes = recipe_service.get_recipes().await?;

    Ok(view! {
        <h2>"Recipe list"</h2>
        <form method="post" action="/recipes">
            <label>
                "Title"
                <input type="text" name="title" required="">
            </label>
            <button type="submit">"Add recipe"</button>
        </form>
        if recipes.is_empty() {
            <p>"No recipes yet."</p>
        } else {
            <ul>
                for recipe in &recipes {
                    <li>(recipe.title().as_str())</li>
                }
            </ul>
        }
    })
}

#[route(POST)]
pub async fn create(cx: &Cx, Form(input): Form<CreateRecipeForm>) -> Result<SeeOther> {
    let recipe_service: &AppRecipeService = app_context(cx);

    recipe_service
        .create_recipe(CreateRecipeRequest {
            title: RecipeTitle::new(input.title),
            metadata: RecipeMetadata::new(None, None, None),
            ingredients: HashMap::new(),
            steps: Vec::new(),
        })
        .await?;

    Ok(see_other("/recipes"))
}
