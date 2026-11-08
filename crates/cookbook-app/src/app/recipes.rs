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
    view::{View, attributes, element_id, view},
};

use crate::{
    app::AppRecipeService,
    components::{
        button::button,
        card::{card, card_content, card_description, card_header, card_title},
        field::{field, field_label},
        input::input,
    },
};

#[derive(Deserialize)]
struct CreateRecipeForm {
    title: String,
}

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipes = recipe_service.get_recipes().await?;

    Ok(view! {
        let title_id = element_id(cx);

        <h2 class="mb-6 text-2xl font-semibold">"Recipe list"</h2>
        card(
            card_header(
                card_title("Add a recipe")
                card_description("Give your recipe a title to get started.")
            )
            card_content(
                <form method="post" action="/recipes" class="flex flex-col gap-4">
                    field(
                        field_label(attrs: attributes! { for=(&title_id) }, "Title")
                        input(attrs: attributes! {
                            id=(&title_id)
                            name="title"
                            type="text"
                            placeholder="Grandma's apple pie"
                            required=""
                        })
                    )
                    <div class="flex justify-end">
                        button(attrs: attributes! { type="submit" }, "Add recipe")
                    </div>
                </form>
            )
        )
        if recipes.is_empty() {
            <p class="mt-8 text-sm text-muted-foreground">"No recipes yet."</p>
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
pub async fn create(cx: &Cx, Form(form): Form<CreateRecipeForm>) -> Result<SeeOther> {
    let recipe_service: &AppRecipeService = app_context(cx);

    recipe_service
        .create_recipe(CreateRecipeRequest {
            title: RecipeTitle::new(form.title),
            metadata: RecipeMetadata::new(None, None, None),
            ingredients: HashMap::new(),
            steps: Vec::new(),
        })
        .await?;

    Ok(see_other("/recipes"))
}
