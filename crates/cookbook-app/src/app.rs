use cookbook_domain::recipe::service::RecipeServiceImpl;
use cookbook_infra::recipe::InMemoryRecipeRepository;
use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt},
    context::Cx,
    cookie::RouterBuilderCookieExt,
    font::{fontsource::fontsource_font, link},
    l10n::locale,
    router::{
        Body, HeaderValue, Next, RouterBuilderDiscoverExt, Slot,
        header::VARY,
        href, layer, layout, module_router, page,
        response::{Response, response_headers},
    },
    runtime::RouterBuilderRuntimeExt,
    view::{View, view},
};

mod locale;
mod recipe;
mod recipes;

use crate::{
    app::locale::get_locale,
    components::{
        base::button::{ButtonSize, ButtonVariant, button},
        misc::locale_select::locale_select,
    },
    t,
};

pub(crate) type AppRecipeService = RecipeServiceImpl<InMemoryRecipeRepository>;

const GEIST: topcoat::font::Font = fontsource_font!(GEIST);

pub fn router() -> topcoat::router::Router {
    let recipe_service = AppRecipeService::new(InMemoryRecipeRepository::new());

    module_router!()
        .app_context(recipe_service)
        .discover()
        .cookies()
        .assets(
            AssetBundle::load()
                .expect("no asset bundle found; run `topcoat asset bundle` or use `topcoat dev`"),
        )
        .runtime()
        .build()
}

#[layout]
async fn root_layout(cx: &Cx, slot: Slot<'_>) -> Result<impl View> {
    let current_locale = locale(cx);

    Ok(view! {
        <!DOCTYPE html>
        <html lang=(current_locale.id()) dir=(current_locale.id().direction())>
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>(t!(app_name))</title>
                link(font: GEIST, preload: false)
                <link rel="stylesheet" href=(topcoat::tailwind::stylesheet!())>
                topcoat::runtime::script()
                topcoat::dev::script()
            </head>
            <body class="min-h-screen">
                <header class="border-b border-border">
                    <div class="mx-auto flex h-14 max-w-2xl items-center gap-4 px-4">
                        <a href=(href!(page))>
                            <h1 class="text-lg font-semibold">(t!(app_name))</h1>
                        </a>
                        locale_select()
                    </div>
                </header>
                <main class="mx-auto max-w-2xl px-4 py-8">(slot)</main>
            </body>
        </html>
    })
}

#[layer]
async fn locale_layer(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let locale = get_locale(cx).await;

    response_headers(cx).append(VARY, HeaderValue::from_static("Accept-Language, Cookie"));

    next.run(&cx.with(locale), body).await
}

#[page]
pub async fn page() -> Result<impl View> {
    Ok(view! {
        <h2 class="mb-2 text-2xl font-semibold">(t!(recipes_heading))</h2>
        <p class="mb-6 text-sm text-muted-foreground">(t!(recipes_welcome))</p>
        <div class="flex flex-row gap-2">
            <a href=(href!(recipes::page))>
                button(
                    variant: ButtonVariant::Primary,
                    size: ButtonSize::Md,
                    (t!(recipe_list))
                )
            </a>
            <a href=(href!(recipe::new::page))>
                button(
                    variant: ButtonVariant::Primary,
                    size: ButtonSize::Md,
                    (t!(new_recipe))
                )
            </a>
        </div>
    })
}
