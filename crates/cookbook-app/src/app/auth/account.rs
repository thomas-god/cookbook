use cookbook_domain::user::models::Username;
use cookbook_infra::auth::{
    models::{AuthenticatedUser, UserEmail},
    ports::{AuthService, ChangeEmailError, ChangePasswordError},
};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{content::Form, error::see_other, page, query_params},
    view::{View, attributes, element_id, view},
};

use crate::{
    app::{AppAuthService, auth::require_user},
    components::{
        base::{
            button::button,
            card::{card, card_content, card_header, card_title},
            field::{field, field_error, field_label},
            input::input,
        },
        misc::locale_select::locale_select,
    },
    t,
};

#[query_params(error = bad_request)]
struct AccountQuery {
    updated: Option<String>,
}

#[derive(Clone, Copy)]
enum AccountUpdate {
    Email,
    Username,
    Password,
}

impl AccountQuery {
    fn update(&self) -> Option<AccountUpdate> {
        match self.updated.as_deref() {
            Some("email") => Some(AccountUpdate::Email),
            Some("username") => Some(AccountUpdate::Username),
            Some("password") => Some(AccountUpdate::Password),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum EmailFormError {
    InvalidForm,
    InvalidPassword,
    InvalidEmail,
    EmailTaken,
}

#[derive(Clone, Copy)]
enum UsernameFormError {
    InvalidForm,
}

#[derive(Clone, Copy)]
enum PasswordFormError {
    InvalidForm,
    InvalidPassword,
    PasswordMismatch,
}

#[derive(Default)]
struct AccountFormState {
    current_email: String,
    current_username: Option<String>,
    new_email: String,
    new_username: String,
    email_error: Option<EmailFormError>,
    username_error: Option<UsernameFormError>,
    password_error: Option<PasswordFormError>,
}

impl AccountFormState {
    fn for_user(user: &AuthenticatedUser) -> Self {
        Self {
            current_email: user.email().as_str().to_string(),
            current_username: user
                .username()
                .map(|username| username.as_str().to_string()),
            ..Self::default()
        }
    }
}

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let user = require_user(cx)?;
    let query = query_params::<AccountQuery>(cx)?;

    account_page(cx, AccountFormState::for_user(user), query.update()).await
}

#[page(POST "./email")]
pub async fn change_email(cx: &Cx, Form(form): Form<Vec<(String, String)>>) -> Result<impl View> {
    let user = require_user(cx)?;

    let Some(form) = EmailForm::parse(form) else {
        let mut state = AccountFormState::for_user(user);
        state.new_email.clear();
        state.email_error = Some(EmailFormError::InvalidForm);
        return account_page(cx, state, None).await;
    };

    let auth: &AppAuthService = app_context(cx);

    match auth
        .change_email(
            user.id(),
            form.current_password,
            UserEmail::new(form.email.clone()),
        )
        .await
    {
        Ok(_) => Err(see_other("/account?updated=email").into()),
        Err(ChangeEmailError::InvalidPassword) => {
            let mut state = AccountFormState::for_user(user);
            state.new_email = form.email;
            state.email_error = Some(EmailFormError::InvalidPassword);
            account_page(cx, state, None).await
        }
        Err(ChangeEmailError::InvalidEmail) => {
            let mut state = AccountFormState::for_user(user);
            state.new_email = form.email;
            state.email_error = Some(EmailFormError::InvalidEmail);
            account_page(cx, state, None).await
        }
        Err(ChangeEmailError::EmailTaken) => {
            let mut state = AccountFormState::for_user(user);
            state.new_email = form.email;
            state.email_error = Some(EmailFormError::EmailTaken);
            account_page(cx, state, None).await
        }
        Err(error) => Err(error.into()),
    }
}

#[page(POST "./username")]
pub async fn change_username(
    cx: &Cx,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<impl View> {
    let user = require_user(cx)?;

    let Some(form) = UsernameForm::parse(form) else {
        let mut state = AccountFormState::for_user(user);
        state.username_error = Some(UsernameFormError::InvalidForm);
        return account_page(cx, state, None).await;
    };

    let auth: &AppAuthService = app_context(cx);

    match auth
        .change_username(user.id(), form.username.map(Username::new))
        .await
    {
        Ok(_) => Err(see_other("/account?updated=username").into()),
        Err(error) => Err(error.into()),
    }
}

#[page(POST "./password")]
pub async fn change_password(
    cx: &Cx,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<impl View> {
    let user = require_user(cx)?;

    let Some(form) = PasswordForm::parse(form) else {
        let mut state = AccountFormState::for_user(user);
        state.password_error = Some(PasswordFormError::InvalidForm);
        return account_page(cx, state, None).await;
    };

    if form.password != form.password_confirm {
        let mut state = AccountFormState::for_user(user);
        state.password_error = Some(PasswordFormError::PasswordMismatch);
        return account_page(cx, state, None).await;
    }

    let auth: &AppAuthService = app_context(cx);

    match auth
        .change_password(user.id(), form.current_password, form.password)
        .await
    {
        Ok(_) => Err(see_other("/account?updated=password").into()),
        Err(ChangePasswordError::InvalidPassword) => {
            let mut state = AccountFormState::for_user(user);
            state.password_error = Some(PasswordFormError::InvalidPassword);
            account_page(cx, state, None).await
        }
        Err(error) => Err(error.into()),
    }
}

struct EmailForm {
    email: String,
    current_password: String,
}

impl EmailForm {
    fn parse(pairs: Vec<(String, String)>) -> Option<Self> {
        let mut email = None;
        let mut current_password = None;

        for (key, value) in pairs {
            match key.as_str() {
                "email" => {
                    if email.replace(value).is_some() {
                        return None;
                    }
                }
                "current_password" => {
                    if current_password.replace(value).is_some() {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        let email = email?.trim().to_string();
        let current_password = current_password?;

        if email.is_empty() || current_password.is_empty() {
            return None;
        }

        Some(Self {
            email,
            current_password,
        })
    }
}

struct UsernameForm {
    username: Option<String>,
}

impl UsernameForm {
    fn parse(pairs: Vec<(String, String)>) -> Option<Self> {
        let mut username = None;

        for (key, value) in pairs {
            match key.as_str() {
                "username" => {
                    if username.replace(value).is_some() {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        Some(Self {
            username: username
                .map(|username| username.trim().to_string())
                .filter(|username| !username.is_empty()),
        })
    }
}

struct PasswordForm {
    current_password: String,
    password: String,
    password_confirm: String,
}

impl PasswordForm {
    fn parse(pairs: Vec<(String, String)>) -> Option<Self> {
        let mut current_password = None;
        let mut password = None;
        let mut password_confirm = None;

        for (key, value) in pairs {
            match key.as_str() {
                "current_password" => {
                    if current_password.replace(value).is_some() {
                        return None;
                    }
                }
                "password" => {
                    if password.replace(value).is_some() {
                        return None;
                    }
                }
                "password_confirm" => {
                    if password_confirm.replace(value).is_some() {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        let current_password = current_password?;
        let password = password?;
        let password_confirm = password_confirm?;

        if current_password.is_empty() || password.is_empty() {
            return None;
        }

        Some(Self {
            current_password,
            password,
            password_confirm,
        })
    }
}

async fn account_page(
    cx: &Cx,
    state: AccountFormState,
    updated: Option<AccountUpdate>,
) -> Result<impl View> {
    let email_id = element_id(cx);
    let email_password_id = element_id(cx);
    let username_id = element_id(cx);
    let current_password_id = element_id(cx);
    let new_password_id = element_id(cx);
    let confirm_password_id = element_id(cx);

    let invalid_email_form = matches!(state.email_error, Some(EmailFormError::InvalidForm));
    let invalid_email_password = matches!(state.email_error, Some(EmailFormError::InvalidPassword));
    let invalid_email = matches!(state.email_error, Some(EmailFormError::InvalidEmail));
    let email_taken = matches!(state.email_error, Some(EmailFormError::EmailTaken));
    let invalid_username_form =
        matches!(state.username_error, Some(UsernameFormError::InvalidForm));
    let invalid_password_form =
        matches!(state.password_error, Some(PasswordFormError::InvalidForm));
    let invalid_password = matches!(
        state.password_error,
        Some(PasswordFormError::InvalidPassword)
    );
    let password_mismatch = matches!(
        state.password_error,
        Some(PasswordFormError::PasswordMismatch)
    );

    Ok(view! {
        cx =>
        <h2 class="mb-6 text-2xl font-semibold">(t!(account_heading))</h2>
        if let Some(updated) = updated {
            <p
                role="status"
                class="mb-6 rounded-lg border border-border bg-muted px-4 py-3 text-sm"
            >
                if matches!(updated, AccountUpdate::Email) {
                    (t!(account_updated_email))
                }
                if matches!(updated, AccountUpdate::Username) {
                    (t!(account_updated_username))
                }
                if matches!(updated, AccountUpdate::Password) {
                    (t!(account_updated_password))
                }
            </p>
        }
        <div class="flex flex-col gap-6">
            card(
                card_header(card_title((t!(account_email_heading))))
                card_content(
                    <form
                        method="post"
                        action="/account/email"
                        class="flex flex-col gap-4"
                    >
                        if invalid_email_form {
                            field_error((t!(auth_form_invalid)))
                        }
                        if invalid_email_password {
                            field_error((t!(account_invalid_current_password)))
                        }
                        if invalid_email {
                            field_error((t!(auth_invalid_email)))
                        }
                        if email_taken {
                            field_error((t!(auth_email_taken)))
                        }
                        <div class="text-sm">
                            <span class="text-muted-foreground">
                                (t!(account_current_email))
                            </span>
                            ": "
                            <span class="font-medium">(state.current_email)</span>
                        </div>
                        field(
                            field_label(
                                attrs: attributes! { for=(&email_id) },
                                (t!(account_new_email))
                            )
                            input(
                                attrs: attributes! {
                                    id=(&email_id)
                                    name="email"
                                    type="email"
                                    autocomplete="email"
                                    required=""
                                    value=(state.new_email)
                                }
                            )
                        )
                        field(
                            field_label(
                                attrs: attributes! { for=(&email_password_id) },
                                (t!(account_current_password))
                            )
                            input(
                                attrs: attributes! {
                                    id=(&email_password_id)
                                    name="current_password"
                                    type="password"
                                    autocomplete="current-password"
                                    required=""
                                }
                            )
                        )
                        button(
                            attrs: attributes! { type="submit" },
                            (t!(account_save_email))
                        )
                    </form>
                )
            )
            card(
                card_header(card_title((t!(account_username_heading))))
                card_content(
                    <form
                        method="post"
                        action="/account/username"
                        class="flex flex-col gap-4"
                    >
                        if invalid_username_form {
                            field_error((t!(auth_form_invalid)))
                        }
                        <div class="text-sm">
                            <span class="text-muted-foreground">
                                (t!(account_current_username))
                            </span>
                            ": "
                            if let Some(current_username) = &state.current_username {
                                <span class="font-medium">(current_username)</span>
                            } else {
                                <span class="text-muted-foreground">
                                    (t!(account_username_none))
                                </span>
                            }
                        </div>
                        field(
                            field_label(
                                attrs: attributes! { for=(&username_id) },
                                (t!(account_new_username))
                            )
                            input(
                                attrs: attributes! {
                                    id=(&username_id)
                                    name="username"
                                    type="text"
                                    autocomplete="nickname"
                                    value=(state.new_username)
                                }
                            )
                        )
                        <p class="text-sm text-muted-foreground">
                            (t!(account_username_hint))
                        </p>
                        button(
                            attrs: attributes! { type="submit" },
                            (t!(account_save_username))
                        )
                    </form>
                )
            )
            card(
                card_header(card_title((t!(account_password_heading))))
                card_content(
                    <form
                        method="post"
                        action="/account/password"
                        class="flex flex-col gap-4"
                    >
                        if invalid_password_form {
                            field_error((t!(auth_form_invalid)))
                        }
                        if invalid_password {
                            field_error((t!(account_invalid_current_password)))
                        }
                        if password_mismatch {
                            field_error((t!(auth_password_mismatch)))
                        }
                        field(
                            field_label(
                                attrs: attributes! { for=(&current_password_id) },
                                (t!(account_current_password))
                            )
                            input(
                                attrs: attributes! {
                                    id=(&current_password_id)
                                    name="current_password"
                                    type="password"
                                    autocomplete="current-password"
                                    required=""
                                }
                            )
                        )
                        field(
                            field_label(
                                attrs: attributes! { for=(&new_password_id) },
                                (t!(account_new_password))
                            )
                            input(
                                attrs: attributes! {
                                    id=(&new_password_id)
                                    name="password"
                                    type="password"
                                    autocomplete="new-password"
                                    required=""
                                }
                            )
                        )
                        field(
                            field_label(
                                attrs: attributes! { for=(&confirm_password_id) },
                                (t!(auth_password_confirm))
                            )
                            input(
                                attrs: attributes! {
                                    id=(&confirm_password_id)
                                    name="password_confirm"
                                    type="password"
                                    autocomplete="new-password"
                                    required=""
                                }
                            )
                        )
                        button(
                            attrs: attributes! { type="submit" },
                            (t!(account_save_password))
                        )
                    </form>
                )
            )
            card(
                card_header(card_title((t!(language_label))))
                card_content(locale_select())
            )
        </div>
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use cookbook_domain::user::models::{UserId, UserRole};

    fn pairs(entries: &[(&str, &str)]) -> Vec<(String, String)> {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn form_state_shows_current_values_and_starts_new_values_empty() {
        let user = AuthenticatedUser::new(
            UserId::new("user-1".to_string()),
            UserEmail::new("Alice@Example.com"),
            Some(Username::new("Alice".to_string())),
            UserRole::User,
        );

        let state = AccountFormState::for_user(&user);

        assert_eq!(state.current_email, "alice@example.com");
        assert_eq!(state.current_username.as_deref(), Some("Alice"));
        assert!(state.new_email.is_empty());
        assert!(state.new_username.is_empty());
    }

    #[test]
    fn parses_and_trims_email_fields() {
        let form = EmailForm::parse(pairs(&[
            ("email", "  New@Example.com "),
            ("current_password", "secret"),
        ]))
        .unwrap();

        assert_eq!(form.email, "New@Example.com");
        assert_eq!(form.current_password, "secret");
    }

    #[test]
    fn rejects_incomplete_or_unknown_email_fields() {
        assert!(EmailForm::parse(pairs(&[("email", "alice@example.com")])).is_none());
        assert!(
            EmailForm::parse(pairs(&[
                ("email", "alice@example.com"),
                ("current_password", "secret"),
                ("next", "/recipes"),
            ]))
            .is_none()
        );
        assert!(
            EmailForm::parse(pairs(&[
                ("email", "alice@example.com"),
                ("email", "bob@example.com"),
                ("current_password", "secret"),
            ]))
            .is_none()
        );
    }

    #[test]
    fn parses_and_trims_optional_usernames() {
        let form = UsernameForm::parse(pairs(&[("username", " Alice ")])).unwrap();
        assert_eq!(form.username.as_deref(), Some("Alice"));

        let form = UsernameForm::parse(pairs(&[("username", "   ")])).unwrap();
        assert_eq!(form.username, None);

        let form = UsernameForm::parse(pairs(&[("username", "")])).unwrap();
        assert_eq!(form.username, None);
    }

    #[test]
    fn rejects_unknown_username_fields() {
        assert!(UsernameForm::parse(pairs(&[("email", "alice@example.com")])).is_none());
    }

    #[test]
    fn parses_password_fields() {
        let form = PasswordForm::parse(pairs(&[
            ("current_password", "old"),
            ("password", "new"),
            ("password_confirm", "new"),
        ]))
        .unwrap();

        assert_eq!(form.current_password, "old");
        assert_eq!(form.password, "new");
        assert_eq!(form.password_confirm, "new");
    }

    #[test]
    fn rejects_incomplete_password_fields() {
        assert!(
            PasswordForm::parse(pairs(&[("current_password", "old"), ("password", "new"),]))
                .is_none()
        );
        assert!(
            PasswordForm::parse(pairs(&[
                ("current_password", "old"),
                ("password", ""),
                ("password_confirm", ""),
            ]))
            .is_none()
        );
    }

    #[test]
    fn maps_success_messages() {
        let query = AccountQuery {
            updated: Some("email".to_string()),
        };
        assert!(matches!(query.update(), Some(AccountUpdate::Email)));

        let query = AccountQuery {
            updated: Some("unknown".to_string()),
        };
        assert!(query.update().is_none());
    }
}
