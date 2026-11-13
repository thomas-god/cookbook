use std::time::Instant;

use cookbook_domain::recipe::service::RecipeServiceImpl;
use cookbook_infra::{
    auth::{
        Argon2PasswordHasher, AuthServiceImpl, InMemorySessionRepository, InMemoryUserRepository,
        RandomSessionTokenGenerator, SystemClock,
    },
    recipe::InMemoryRecipeRepository,
};
use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt},
    context::{Cx, app_context},
    cookie::RouterBuilderCookieExt,
    font::{fontsource::fontsource_font, link},
    l10n::locale,
    router::{
        Body, BodyLimit, HeaderValue, Next, RouterBuilderDiscoverExt, Slot,
        header::VARY,
        href, layer, layout, module_router, page,
        request::{method, uri},
        response::{Response, response_headers},
    },
    runtime::RouterBuilderRuntimeExt,
    session::{RouterBuilderSessionExt, SessionConfig},
    view::{View, attributes, view},
};

pub(crate) mod auth;
mod locale;
pub(crate) mod recipe;
pub(crate) mod recipes;
mod vision;

use crate::{
    app::{
        auth::{AuthSettings, SESSION_POLICY},
        locale::get_locale,
    },
    components::{
        base::button::{ButtonSize, ButtonVariant, button},
        misc::locale_select::locale_select,
    },
    t,
};

pub(crate) type AppRecipeService = RecipeServiceImpl<InMemoryRecipeRepository, SystemClock>;
pub(crate) type AppUserRepository = InMemoryUserRepository;
pub(crate) type AppAuthService = AuthServiceImpl<
    InMemoryUserRepository,
    Argon2PasswordHasher,
    InMemorySessionRepository,
    SystemClock,
    RandomSessionTokenGenerator,
>;

const GEIST: topcoat::font::Font = fontsource_font!(GEIST);
const APP_JS: topcoat::asset::Asset = topcoat::asset::asset!("assets/app.js");

pub async fn router() -> topcoat::router::Router {
    let recipe_service =
        AppRecipeService::new(InMemoryRecipeRepository::seeded(), SystemClock::new());
    let auth_config = auth::AuthConfig::from_env().expect("invalid authentication configuration");
    let vision_config = vision::VisionConfig::from_env().expect("invalid vision configuration");
    let user_repository = AppUserRepository::new();
    let auth_service = AppAuthService::new(
        user_repository.clone(),
        Argon2PasswordHasher::new(),
        InMemorySessionRepository::new(),
        SystemClock::new(),
        RandomSessionTokenGenerator::new(),
        SESSION_POLICY,
    );

    auth_config
        .seed_initial_admin(&auth_service)
        .await
        .expect("failed to create the initial admin");

    module_router!()
        .app_context(recipe_service)
        .app_context(auth_service)
        .app_context(user_repository)
        .app_context(auth_config.settings())
        .app_context(vision_config.settings())
        .app_context(vision_config.client())
        .discover()
        .layer(BodyLimit::max(recipe::import::MAX_UPLOAD_BYTES).at("/recipe/import"))
        .layer(BodyLimit::max(recipes::MAX_UPLOAD_BYTES).at("/recipe/new"))
        .layer(BodyLimit::max(recipes::MAX_UPLOAD_BYTES).at("/recipe/new/multipart"))
        .layer(BodyLimit::max(recipes::MAX_UPLOAD_BYTES).at("/recipes"))
        .layer(auth::AuthenticationLayer::new())
        .cookies()
        .sessions(
            SessionConfig::builder()
                .token_store(auth::ForwardedProtoCookieTokenStore::new())
                .lifetime(SESSION_POLICY.lifetime())
                .build(),
        )
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
    let current_user = auth::current_user(cx);
    let settings: &AuthSettings = app_context(cx);

    Ok(view! {
        <!DOCTYPE html>
        <html lang=(current_locale.id()) dir=(current_locale.id().direction())>
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>(t!(app_name))</title>
                link(font: GEIST, preload: false)
                <link rel="stylesheet" href=(topcoat::tailwind::stylesheet!())>
                <script type="module" src=(APP_JS)></script>
                topcoat::runtime::script()
                topcoat::dev::script()
            </head>
            <body class="min-h-screen">
                <header class="border-b border-border">
                    <div class="mx-auto flex h-14 max-w-2xl items-center gap-4 px-4">
                        <a href=(href!(page))>
                            <h1 class="text-lg font-semibold">(t!(app_name))</h1>
                        </a>
                        <div class="ml-auto flex items-center gap-3">
                            if let Some(current) = current_user {
                                <span class="text-sm text-muted-foreground">
                                    (current.display_name())
                                </span>
                                <form method="post" action="/logout">
                                    button(
                                        variant: ButtonVariant::Ghost,
                                        size: ButtonSize::Sm,
                                        attrs: attributes! { type="submit" },
                                        (t!(auth_logout))
                                    )
                                </form>
                            } else {
                                <a
                                    class="text-sm underline underline-offset-4"
                                    href="/login"
                                >
                                    (t!(auth_login_link))
                                </a>
                                if settings.registration_enabled() {
                                    <a
                                        class="text-sm underline underline-offset-4"
                                        href="/register"
                                    >
                                        (t!(auth_register_link))
                                    </a>
                                }
                            }
                            locale_select()
                        </div>
                    </div>
                </header>
                <main class="mx-auto max-w-2xl px-4 py-8">(slot)</main>
            </body>
        </html>
    })
}

#[layer("/")]
async fn request_logging(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let method = method(cx).clone();
    let path = uri(cx).path().to_owned();
    let started = Instant::now();

    let result = next.run(cx, body).await;
    let latency_ms = started.elapsed().as_millis();

    match &result {
        Ok(response) => tracing::info!(
            %method,
            path,
            status = response.status().as_u16(),
            latency_ms,
            "request completed"
        ),
        Err(error) => tracing::warn!(
            %method,
            path,
            latency_ms,
            error = %error,
            "request failed"
        ),
    }

    result
}

#[layer]
async fn locale_layer(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let locale = get_locale(cx).await;

    response_headers(cx).append(VARY, HeaderValue::from_static("Accept-Language, Cookie"));

    next.run(&cx.with(locale), body).await
}

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    auth::require_user(cx)?;
    let vision_settings: &vision::VisionSettings = app_context(cx);

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
            if vision_settings.enabled() {
                <a href=(href!(recipe::import::page))>
                    button(
                        variant: ButtonVariant::Primary,
                        size: ButtonSize::Md,
                        (t!(recipe_import_link))
                    )
                </a>
            }
        </div>
    })
}
