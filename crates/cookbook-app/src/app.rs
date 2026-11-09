use cookbook_domain::recipe::service::RecipeServiceImpl;
use cookbook_infra::recipe::InMemoryRecipeRepository;
use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt},
    context::Cx,
    cookie::{Cookie, Cookies, RouterBuilderCookieExt, SameSite, cookies, time},
    font::{fontsource::fontsource_font, link},
    l10n::{Locale, accept_language, locale, negotiate},
    router::{
        Body, Next, RouterBuilderDiscoverExt, Slot,
        content::Form,
        error::{SeeOther, bad_request, see_other},
        header::{HeaderValue, VARY},
        layer, layout, module_router, page,
        response::{Response, response_headers},
        route,
    },
    runtime::RouterBuilderRuntimeExt,
    view::{View, view},
};

mod locale_select;
mod recipe_form;
mod recipes;

use crate::{
    components::card::{card, card_content, card_header, card_title},
    t,
};

pub(crate) type AppRecipeService = RecipeServiceImpl<InMemoryRecipeRepository>;

const GEIST: topcoat::font::Font = fontsource_font!(GEIST);

const DEFAULT_LOCALE: Locale = locale!("en");
const LOCALE_COOKIE: &str = "locale";

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

/// Selects the request's language from the locale cookie or the browser's
/// preferred languages, and makes it available to layouts and pages.
#[layer]
async fn locale_layer(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let from_cookie = cookies(cx)
        .get(LOCALE_COOKIE)
        .and_then(|cookie| cookie.value().parse::<Locale>().ok());

    let locale = from_cookie
        .as_ref()
        .and_then(|candidate| negotiate(std::iter::once(candidate), t::locales()).cloned())
        .or_else(|| negotiate(accept_language(cx), t::locales()).cloned())
        .unwrap_or_else(|| DEFAULT_LOCALE.clone());

    response_headers(cx).append(VARY, HeaderValue::from_static("Accept-Language, Cookie"));

    next.run(&cx.with(locale), body).await
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
                        <h1 class="text-lg font-semibold">(t!(app_name))</h1>
                        locale_select::locale_select()
                    </div>
                </header>
                <main class="mx-auto max-w-2xl px-4 py-8">
                    (slot)
                </main>
            </body>
        </html>
    })
}

#[route(POST "./locale")]
async fn switch_locale(cx: &Cx, Form(form): Form<Vec<(String, String)>>) -> Result<SeeOther> {
    let mut requested = None;
    let mut next = None;

    for (key, value) in form {
        match key.as_str() {
            LOCALE_COOKIE => requested = Some(value),
            "next" => next = Some(value),
            _ => return Err(bad_request(format!("unknown form field `{key}`")).into()),
        }
    }

    let Some(requested) = requested else {
        return Err(bad_request("locale is required").into());
    };

    let Ok(requested) = requested.parse::<Locale>() else {
        return Err(bad_request("invalid locale").into());
    };

    let Some(selected) = negotiate(std::iter::once(&requested), t::locales()).cloned() else {
        return Err(bad_request("unsupported locale").into());
    };

    let next = next.unwrap_or_else(|| "/".to_owned());

    // Only allow same-origin redirect targets.
    if !next.starts_with('/') || next.starts_with("//") || next.contains('\\') {
        return Err(bad_request("invalid redirect target").into());
    }

    cookies(cx).add(
        Cookie::build((LOCALE_COOKIE, selected.id().to_string()))
            .path("/")
            .same_site(SameSite::Lax)
            .max_age(time::Duration::days(365))
            .build(),
    );

    Ok(see_other(next))
}

#[page]
pub async fn page() -> Result<impl View> {
    Ok(view! {
        <h2 class="mb-2 text-2xl font-semibold">(t!(recipes_heading))</h2>
        <p class="mb-6 text-sm text-muted-foreground">(t!(recipes_welcome))</p>
        card(
            card_header(
                card_title((t!(form_new_recipe)))
            )
            card_content(
                recipe_form::recipe_form()
            )
        )
    })
}
