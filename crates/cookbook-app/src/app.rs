use cookbook_domain::recipe::service::RecipeServiceImpl;
use cookbook_infra::recipe::InMemoryRecipeRepository;
use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt},
    context::Cx,
    font::{fontsource::fontsource_font, link},
    router::{RouterBuilderDiscoverExt, Slot, layout, module_router, page},
    runtime::RouterBuilderRuntimeExt,
    view::{View, view},
};

mod recipe_form;
mod recipes;

use crate::components::card::{card, card_content, card_description, card_header, card_title};

pub(crate) type AppRecipeService = RecipeServiceImpl<InMemoryRecipeRepository>;

const GEIST: topcoat::font::Font = fontsource_font!(GEIST);

pub fn router() -> topcoat::router::Router {
    let recipe_service = AppRecipeService::new(InMemoryRecipeRepository::new());

    module_router!()
        .app_context(recipe_service)
        .discover()
        .assets(
            AssetBundle::load()
                .expect("no asset bundle found; run `topcoat asset bundle` or use `topcoat dev`"),
        )
        .runtime()
        .build()
}

#[layout]
async fn root_layout(cx: &Cx, slot: Slot<'_>) -> Result<impl View> {
    let _ = cx;

    Ok(view! {
        <!DOCTYPE html>
        <html>
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"Cookbook"</title>
                link(font: GEIST, preload: false)
                <link rel="stylesheet" href=(topcoat::tailwind::stylesheet!())>
                topcoat::runtime::script()
                topcoat::dev::script()
            </head>
            <body class="min-h-screen">
                <header class="border-b border-border">
                    <div class="mx-auto flex h-14 max-w-2xl items-center px-4">
                        <h1 class="text-lg font-semibold">"Cookbook"</h1>
                    </div>
                </header>
                <main class="mx-auto max-w-2xl px-4 py-8">
                    (slot)
                </main>
            </body>
        </html>
    })
}

#[page]
pub async fn page() -> Result<impl View> {
    Ok(view! {
        <h2 class="mb-2 text-2xl font-semibold">"Recipes"</h2>
        <p class="mb-6 text-sm text-muted-foreground">"Welcome to the family cookbook."</p>
        card(
            card_header(
                card_title("Add a recipe")
                card_description("Add the ingredients, timings, and steps for your recipe.")
            )
            card_content(
                recipe_form::recipe_form()
            )
        )
    })
}
