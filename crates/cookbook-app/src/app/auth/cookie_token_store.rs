use std::time::Duration;

use topcoat::{
    context::Cx,
    cookie::{Cookie, Cookies, SameSite, cookies, time},
    router::request::headers,
    session::{Token, TokenStore, TokenStoreFuture},
};

const SECURE_COOKIE_NAME: &str = "__Host-session";
const INSECURE_COOKIE_NAME: &str = "session";

/// Carries the session token in a cookie whose `Secure` attribute follows the
/// scheme reported by a TLS-terminating proxy.
///
/// HTTPS requests use the hardened `__Host-session` cookie. Plain HTTP requests
/// use a regular `session` cookie, so the app can run without a certificate on
/// a trusted network.
#[derive(Debug, Clone, Copy, Default)]
pub struct ForwardedProtoCookieTokenStore;

impl ForwardedProtoCookieTokenStore {
    pub fn new() -> Self {
        Self
    }
}

fn request_is_secure(cx: &Cx) -> bool {
    headers(cx)
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .is_some_and(|scheme| scheme.trim().eq_ignore_ascii_case("https"))
}

fn read_token(cx: &Cx, name: &str) -> Option<Token> {
    cookies(cx)
        .get(name)
        .and_then(|cookie| Token::decode(cookie.value_trimmed()).ok())
}

impl TokenStore for ForwardedProtoCookieTokenStore {
    fn read<'a>(&'a self, cx: &'a Cx) -> TokenStoreFuture<'a, Option<Token>> {
        Box::pin(async move {
            Ok(read_token(cx, SECURE_COOKIE_NAME).or_else(|| read_token(cx, INSECURE_COOKIE_NAME)))
        })
    }

    fn write<'a>(
        &'a self,
        cx: &'a Cx,
        token: Token,
        max_age: Duration,
    ) -> TokenStoreFuture<'a, ()> {
        Box::pin(async move {
            let secure = request_is_secure(cx);
            let name = if secure {
                SECURE_COOKIE_NAME
            } else {
                INSECURE_COOKIE_NAME
            };
            let max_age = time::Duration::try_from(max_age)?;

            cookies(cx).add(
                Cookie::build((name, token.encode()))
                    .path("/")
                    .http_only(true)
                    .same_site(SameSite::Lax)
                    .secure(secure)
                    .max_age(max_age)
                    .build(),
            );

            Ok(())
        })
    }

    fn delete<'a>(&'a self, cx: &'a Cx) -> TokenStoreFuture<'a, ()> {
        Box::pin(async move {
            let jar = cookies(cx);

            jar.remove(
                Cookie::build((SECURE_COOKIE_NAME, ""))
                    .path("/")
                    .http_only(true)
                    .same_site(SameSite::Lax)
                    .secure(true)
                    .build(),
            );
            jar.remove(
                Cookie::build((INSECURE_COOKIE_NAME, ""))
                    .path("/")
                    .http_only(true)
                    .same_site(SameSite::Lax)
                    .secure(false)
                    .build(),
            );

            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use topcoat::{
        context::CxTestBuilder,
        cookie::CookieJarCell,
        router::{HeaderMap, header::SET_COOKIE, request::Request},
    };

    use super::*;

    fn cx_with(headers: &[(&str, &str)]) -> Cx {
        let mut builder = Request::builder();
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        let (parts, ()) = builder.body(()).unwrap().into_parts();

        CxTestBuilder::new()
            .request_context(parts)
            .request_context(CookieJarCell::new())
            .build()
    }

    fn set_cookies(cx: &Cx) -> Vec<String> {
        let mut headers = HeaderMap::new();
        topcoat::cookie::write_cookies(cx, &mut headers);
        headers
            .get_all(SET_COOKIE)
            .iter()
            .map(|value| value.to_str().unwrap().to_owned())
            .collect()
    }

    #[tokio::test]
    async fn https_writes_a_secure_host_cookie() {
        let cx = cx_with(&[("x-forwarded-proto", "https")]);
        let store = ForwardedProtoCookieTokenStore::new();
        let token = Token::random();
        let expected = *token.dangerous_as_array();

        store
            .write(&cx, token, Duration::from_hours(1))
            .await
            .unwrap();

        let written = &set_cookies(&cx)[0];
        assert!(written.starts_with("__Host-session="), "{written}");
        assert!(written.contains("Secure"), "{written}");

        let separator = written.find(';').unwrap_or(written.len());
        let echoed = &written[..separator];
        let reader = cx_with(&[("cookie", echoed)]);
        let token = store.read(&reader).await.unwrap().unwrap();

        assert_eq!(*token.dangerous_as_array(), expected);
    }

    #[tokio::test]
    async fn http_writes_a_non_secure_cookie() {
        let cx = cx_with(&[]);
        let store = ForwardedProtoCookieTokenStore::new();

        store
            .write(&cx, Token::random(), Duration::from_hours(1))
            .await
            .unwrap();

        let written = &set_cookies(&cx)[0];
        assert!(written.starts_with("session="), "{written}");
        assert!(!written.contains("Secure"), "{written}");
    }

    #[tokio::test]
    async fn reads_either_cookie_name() {
        let store = ForwardedProtoCookieTokenStore::new();
        let token = Token::random();
        let cookie = format!("session={}", token.encode());
        let cx = cx_with(&[("cookie", &cookie)]);

        let read = store.read(&cx).await.unwrap().unwrap();

        assert_eq!(read.dangerous_as_array(), token.dangerous_as_array());
    }

    #[tokio::test]
    async fn deletes_both_cookie_names() {
        let cx = cx_with(&[("cookie", "__Host-session=abc; session=def")]);
        let store = ForwardedProtoCookieTokenStore::new();

        store.delete(&cx).await.unwrap();

        let removed = set_cookies(&cx);
        assert!(
            removed
                .iter()
                .any(|cookie| cookie.starts_with("__Host-session="))
        );
        assert!(removed.iter().any(|cookie| cookie.starts_with("session=")));
        assert!(removed.iter().all(|cookie| cookie.contains("Max-Age=0")));
    }
}
