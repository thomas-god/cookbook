use cookbook_domain::recipe::models::RecipeId as RecipeIdValue;
use cookbook_domain::recipe::ports::RecipeService;
use topcoat::{
    Result,
    context::{Cx, context},
    router::{error::RouterErrorExt, page},
    view::{View, view},
};

use crate::app::recipes::recipe_id;
use crate::app::{AppRecipeService, auth};
use crate::components::recipe::cook_view::cook_recipe_view;

#[page("/recipe/{recipe_id}/cook")]
pub async fn cook_recipe(cx: &Cx) -> Result<impl View> {
    let user = auth::require_user(cx)?;
    let recipe_id = recipe_id(cx).to_string();
    let recipe_service: &AppRecipeService = context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id), user.id().clone())
        .await?
        .ok_or_not_found()?;
    let images = recipe_service
        .get_recipe_images(recipe.id().clone(), user.id().clone())
        .await?;

    Ok(view! {
        cook_recipe_view(
            recipe_id: recipe.id().as_str(),
            content: recipe.content(),
            images: &images
        )
    })
}
