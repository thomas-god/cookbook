use topcoat::{
    Result,
    context::Cx,
    cookie::{Cookie, Cookies, SameSite, cookies, time},
    l10n::{Locale, accept_language, locale, negotiate},
    router::{
        content::Form,
        error::{SeeOther, bad_request, see_other},
        route,
    },
};

use crate::t;

const DEFAULT_LOCALE: Locale = locale!("en");
const LOCALE_COOKIE: &str = "locale";

/// Extract language from the locale cookie or the browser's preferred languages.
pub async fn get_locale(cx: &Cx) -> Locale {
    let from_cookie = cookies(cx)
        .get(LOCALE_COOKIE)
        .and_then(|cookie| cookie.value().parse::<Locale>().ok());

    from_cookie
        .as_ref()
        .and_then(|candidate| negotiate(std::iter::once(candidate), t::locales()).cloned())
        .or_else(|| negotiate(accept_language(cx), t::locales()).cloned())
        .unwrap_or_else(|| DEFAULT_LOCALE.clone())
}

#[route(POST)]
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
