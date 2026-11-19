use cookbook_domain::recipe::models::RecipeId as RecipeIdValue;
use cookbook_domain::recipe::ports::RecipeService;
use topcoat::{
    Result,
    context::{Cx, app_context},
    icon::icon,
    router::{
        error::{RouterErrorExt, forbidden},
        href, page, path_param,
    },
    view::{View, view},
};

use crate::app::recipes::RecipeId;
use crate::app::{AppRecipeService, auth};
use crate::components::base::card::{card, card_content, card_header, card_title};
use crate::components::recipe::recipe_scope_form::recipe_scope_form;
use crate::t;
use crate::{app::recipe::view::recipe_detail, components::base::icons::BACK_ARROW};

#[page("/recipe/{recipe_id}/scope")]
pub async fn recipe_scope(cx: &Cx) -> Result<impl View> {
    let user = auth::require_user(cx)?;

    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id.clone()), user.id().clone())
        .await?
        .ok_or_not_found()?;

    if recipe.author() != user.id() {
        return Err(forbidden().into());
    }

    Ok(view! {
        <div class="flex flex-col gap-6">
            <a
                class="text-sm text-muted-foreground underline-offset-4 hover:underline italic opacity-80 flex items-center gap-1.5"
                href=(href!(recipe_detail, RecipeId(recipe_id.as_str())))
            >
                icon(data: BACK_ARROW)
                (t!(recipe_back_to_recipe))
            </a>
            <h2 class="text-2xl font-semibold">
                (t!(scope_heading))
                ": "
                (recipe.title().as_str())
            </h2>
            card(
                card_header(card_title((t!(scope_heading))))
                card_content(
                    recipe_scope_form(
                        recipe_id: recipe.id().as_str().to_string(),
                        scope: recipe.scope()
                    )
                )
            )
        </div>
    })
}
