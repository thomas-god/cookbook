use topcoat::{
    Result,
    context::Cx,
    router::{
        error::{SeeOther, see_other},
        route,
    },
};

use crate::app::auth::stop_user_session;

#[route(POST)]
pub async fn logout(cx: &Cx) -> Result<SeeOther> {
    stop_user_session(cx).await?;
    Ok(see_other("/login"))
}
