use cookbook_domain::recipe::{
    models::{RecipeId as RecipeIdValue, Unit},
    ports::RecipeService,
};
use cookbook_infra::auth::ports::UserRepository;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{error::RouterErrorExt, href, page, path_param},
    view::{View, view},
};

use crate::{
    app::{AppRecipeService, AppUserRepository, auth},
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

path_param!(recipe_id);

#[page("./{recipe_id}")]
pub async fn recipe_detail(cx: &Cx) -> Result<impl View> {
    auth::require_user(cx)?;

    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(path_param::<RecipeId>(cx).to_string()))
        .await?
        .ok_or_not_found()?;

    let user_repository: &AppUserRepository = app_context(cx);
    let author_name = match user_repository.find_by_id(recipe.author()).await? {
        Some(author) => author
            .username()
            .map(|username| username.as_str().to_string())
            .unwrap_or_else(|| author.email().as_str().to_string()),
        None => recipe.author().as_str().to_string(),
    };

    let total_minutes = recipe.metadata().total_duration().as_minutes();
    let mut ingredients: Vec<_> = recipe
        .ingredients()
        .iter()
        .map(|(name, quantity)| {
            (
                name.as_str().to_string(),
                quantity.value(),
                quantity.unit().clone(),
            )
        })
        .collect();
    ingredients.sort_by(|left, right| left.0.cmp(&right.0));

    Ok(view! {
        <div class="flex flex-col gap-8">
            <div>
                <a
                    class="text-sm text-muted-foreground underline-offset-4 hover:underline"
                    href=(href!(page))
                >
                    (t!(recipe_back_to_list))
                </a>
                <h2 class="mt-3 text-3xl font-semibold">(recipe.title().as_str())</h2>
                if let Some(description) = recipe.description() {
                    <p class="mt-2 text-sm text-muted-foreground">(description.as_str())</p>
                }
                <p class="mt-2 text-sm text-muted-foreground">
                    (t!(recipe_author)) ": " (author_name)
                </p>
                if total_minutes > 0 {
                    <p class="mt-2 text-sm text-muted-foreground">
                        (t!(recipe_total_duration)) ": " (total_minutes) " " (t!(recipe_minutes))
                    </p>
                }
            </div>

            <section>
                <h3 class="mb-3 text-lg font-semibold">(t!(recipe_ingredients))</h3>
                <ul class="flex flex-col gap-2">
                    for (name, quantity, unit) in ingredients {
                        <li
                            class="flex items-center justify-between rounded-lg border border-border bg-card px-4 py-3 text-sm"
                        >
                            <span>(name)</span>
                            <span class="text-muted-foreground">
                                (quantity) " "
                                (match unit {
                                    Unit::Gram => t!(unit_gram),
                                    Unit::MilliLitre => t!(unit_millilitre),
                                    Unit::Litre => t!(unit_litre),
                                })
                            </span>
                        </li>
                    }
                </ul>
            </section>

            <section>
                <h3 class="mb-3 text-lg font-semibold">(t!(recipe_steps))</h3>
                <ol class="flex flex-col gap-4">
                    for (index, step) in recipe.steps().iter().enumerate() {
                        <li class="rounded-lg border border-border bg-card p-4 text-sm">
                            <h4 class="font-semibold">
                                if let Some(title) = step.title() {
                                    (title)
                                } else {
                                    (t!(recipe_step)) " " (index + 1)
                                }
                            </h4>
                            <p class="mt-2 text-muted-foreground">(step.instruction())</p>
                        </li>
                    }
                </ol>
            </section>
        </div>
    })
}
