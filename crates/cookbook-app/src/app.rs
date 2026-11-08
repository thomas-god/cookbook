use cookbook_domain::recipe::service::RecipeServiceImpl;
use cookbook_infra::recipe::InMemoryRecipeRepository;
use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt},
    context::Cx,
    font::{RouterBuilderFontExt, fontsource::fontsource_font, link},
    router::{Slot, layout, module_router, page},
    view::{View, attributes, element_id, view},
};

mod recipes;

use crate::components::{
    button::button,
    card::{card, card_content, card_description, card_header, card_title},
    field::{field, field_label},
    input::input,
};

pub(crate) type AppRecipeService = RecipeServiceImpl<InMemoryRecipeRepository>;

const GEIST: topcoat::font::Font = fontsource_font!(GEIST);

pub fn router() -> topcoat::router::Router {
    let recipe_service = AppRecipeService::new(InMemoryRecipeRepository::new());

    module_router!()
        .app_context(recipe_service)
        .font(GEIST)
        .assets(
            AssetBundle::load()
                .expect("no asset bundle found; run `topcoat asset bundle` or use `topcoat dev`"),
        )
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
pub async fn page(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        let title_id = element_id(cx);

        <h2 class="mb-2 text-2xl font-semibold">"Recipes"</h2>
        <p class="mb-6 text-sm text-muted-foreground">"Welcome to the family cookbook."</p>
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
    })
}
