use cookbook_infra::auth::{
    models::{AuthenticatedUser, SessionTokenHash},
    ports::AuthService,
};
use topcoat::{
    Result,
    context::{Cx, app_context, try_request_context},
    router::{Body, Layer, LayerFuture, Next, Path, error::see_other},
    session,
};

use crate::app::AppAuthService;

pub fn current_user(cx: &Cx) -> Option<&AuthenticatedUser> {
    try_request_context::<AuthenticatedUser>(cx)
}

/// Returns the authenticated user, or redirects to the login page.
pub fn require_user(cx: &Cx) -> Result<&AuthenticatedUser> {
    current_user(cx).ok_or_else(|| see_other("/login").into())
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AuthenticationLayer;

impl AuthenticationLayer {
    pub fn new() -> Self {
        Self
    }
}

impl Layer for AuthenticationLayer {
    fn path(&self) -> Option<&Path> {
        Some(Path::new("/"))
    }

    fn handle<'a>(&'a self, cx: &'a Cx, body: Body, next: Next<'a>) -> LayerFuture<'a> {
        Box::pin(async move {
            let Some(hash) = session::token_hash(cx).await? else {
                return next.run(cx, body).await;
            };

            let auth: &AppAuthService = app_context(cx);
            let token_hash = SessionTokenHash::new(*hash);

            let Some((user, expires_at)) = auth.authenticate_session(token_hash.clone()).await?
            else {
                session::stop(cx).await?;
                return next.run(cx, body).await;
            };

            if auth.session_needs_renewal(expires_at) {
                if session::refresh(cx).await?.is_some() {
                    auth.renew_session(token_hash).await?;
                }
            }

            let cx = cx.with(user);
            next.run(&cx, body).await
        })
    }
}
