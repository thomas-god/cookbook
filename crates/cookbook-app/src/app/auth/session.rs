use cookbook_infra::auth::{
    models::{IssuedSession, SessionTokenHash},
    ports::AuthService,
};
use topcoat::{
    Result,
    context::Cx,
    session::{self, Token, TokenStore},
};

use crate::app::{
    AppAuthService,
    auth::{ForwardedProtoCookieTokenStore, SESSION_POLICY},
};

/// Sends an already-issued session token to the client.
pub async fn write_issued_session(cx: &Cx, issued: &IssuedSession) -> Result<()> {
    let previous = session::token_hash(cx).await?;

    // Keep this store in sync with the `token_store` configured in `app::router`.
    let token = Token::decode(issued.token())?;
    ForwardedProtoCookieTokenStore::new()
        .write(cx, token, SESSION_POLICY.lifetime())
        .await?;

    if let Some(previous) = previous {
        let previous = SessionTokenHash::new(*previous);
        if &previous != issued.token_hash() {
            let auth: &AppAuthService = topcoat::context::app_context(cx);
            auth.delete_session(previous).await?;
        }
    }

    Ok(())
}

/// Stops the current web session and deletes its stored record.
pub async fn stop_user_session(cx: &Cx) -> Result<()> {
    let Some(hash) = session::stop(cx).await? else {
        return Ok(());
    };

    let auth: &AppAuthService = topcoat::context::app_context(cx);
    auth.delete_session(SessionTokenHash::new(*hash)).await?;

    Ok(())
}
