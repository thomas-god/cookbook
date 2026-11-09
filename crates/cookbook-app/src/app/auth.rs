use std::time::Duration;

use cookbook_infra::auth::models::SessionPolicy;
use topcoat::router::segment;

segment!(kind = Group);

pub const SESSION_POLICY: SessionPolicy = SessionPolicy::new(
    Duration::from_secs(60 * 60 * 24 * 30),
    Duration::from_secs(60 * 60 * 24 * 7),
);

pub mod login;
pub mod logout;
pub mod register;

mod cookie_token_store;
mod layer;
mod session;
mod settings;

pub use cookie_token_store::ForwardedProtoCookieTokenStore;
pub use layer::{AuthenticationLayer, current_user, require_user};
pub use session::{stop_user_session, write_issued_session};
pub use settings::{AuthConfig, AuthSettings};
