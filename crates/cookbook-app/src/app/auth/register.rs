use cookbook_domain::user::models::{UserRole, Username};
use cookbook_infra::auth::{
    models::UserEmail,
    ports::{AuthService, RegisterError, RegisterRequest},
};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Form,
        error::{forbidden, see_other},
        page,
    },
    view::{View, attributes, element_id, view},
};

use crate::{
    app::{
        AppAuthService,
        auth::{AuthSettings, current_user, write_issued_session},
    },
    components::base::{
        button::button,
        card::{card, card_content, card_header, card_title},
        field::{field, field_error, field_label},
        input::input,
    },
    t,
};

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    if current_user(cx).is_some() {
        return Err(see_other("/recipes").into());
    }

    if !registration_enabled(cx) {
        return register_form(
            cx,
            String::new(),
            String::new(),
            Some(RegisterFormError::RegistrationDisabled),
        )
        .await;
    }

    register_form(cx, String::new(), String::new(), None).await
}

#[page(POST)]
pub async fn register(cx: &Cx, Form(form): Form<Vec<(String, String)>>) -> Result<impl View> {
    if !registration_enabled(cx) {
        return Err(forbidden().into());
    }

    let Some(form) = RegisterForm::parse(form) else {
        return register_form(
            cx,
            String::new(),
            String::new(),
            Some(RegisterFormError::InvalidForm),
        )
        .await;
    };

    if form.password != form.password_confirm {
        return register_form(
            cx,
            form.email.clone(),
            form.username.clone().unwrap_or_default(),
            Some(RegisterFormError::PasswordMismatch),
        )
        .await;
    }

    let auth: &AppAuthService = app_context(cx);
    let request = RegisterRequest {
        email: UserEmail::new(form.email.clone()),
        username: form.username.clone().map(Username::new),
        password: form.password,
        role: UserRole::User,
    };

    match auth.register(request).await {
        Ok(res) => {
            write_issued_session(cx, res.session()).await?;
            Err(see_other("/recipes").into())
        }
        Err(RegisterError::EmailTaken) => {
            register_form(
                cx,
                form.email.clone(),
                form.username.clone().unwrap_or_default(),
                Some(RegisterFormError::EmailTaken),
            )
            .await
        }
        Err(RegisterError::InvalidEmail) => {
            register_form(
                cx,
                form.email.clone(),
                form.username.clone().unwrap_or_default(),
                Some(RegisterFormError::InvalidEmail),
            )
            .await
        }
        Err(RegisterError::PasswordTooShort { .. }) => {
            register_form(
                cx,
                form.email.clone(),
                form.username.clone().unwrap_or_default(),
                Some(RegisterFormError::PasswordTooShort),
            )
            .await
        }
        Err(error) => Err(error.into()),
    }
}

#[derive(Clone, Copy)]
pub enum RegisterFormError {
    RegistrationDisabled,
    InvalidForm,
    InvalidEmail,
    PasswordTooShort,
    PasswordMismatch,
    EmailTaken,
}

struct RegisterForm {
    email: String,
    username: Option<String>,
    password: String,
    password_confirm: String,
}

impl RegisterForm {
    fn parse(pairs: Vec<(String, String)>) -> Option<Self> {
        let mut email = None;
        let mut username = None;
        let mut password = None;
        let mut password_confirm = None;

        for (key, value) in pairs {
            match key.as_str() {
                "email" => {
                    if email.replace(value).is_some() {
                        return None;
                    }
                }
                "username" => {
                    if username.replace(value).is_some() {
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

        let email = email?.trim().to_string();
        let username = username
            .map(|username| username.trim().to_string())
            .filter(|username| !username.is_empty());
        let password = password?;
        let password_confirm = password_confirm?;

        if email.is_empty() || password.is_empty() {
            return None;
        }

        Some(Self {
            email,
            username,
            password,
            password_confirm,
        })
    }
}

fn registration_enabled(cx: &Cx) -> bool {
    let settings: &AuthSettings = app_context(cx);
    settings.registration_enabled()
}

async fn register_form(
    cx: &Cx,
    email: String,
    username: String,
    error: Option<RegisterFormError>,
) -> Result<impl View> {
    let email_id = element_id(cx);
    let username_id = element_id(cx);
    let password_id = element_id(cx);
    let password_confirm_id = element_id(cx);

    let registration_disabled = matches!(error, Some(RegisterFormError::RegistrationDisabled));
    let invalid_form = matches!(error, Some(RegisterFormError::InvalidForm));
    let invalid_email = matches!(error, Some(RegisterFormError::InvalidEmail));
    let password_too_short = matches!(error, Some(RegisterFormError::PasswordTooShort));
    let password_mismatch = matches!(error, Some(RegisterFormError::PasswordMismatch));
    let email_taken = matches!(error, Some(RegisterFormError::EmailTaken));

    Ok(view! {
        cx =>
        card(
            attrs: attributes! { class="mx-auto max-w-md" },
            card_header(card_title((t!(auth_register_heading))))
            card_content(
                if registration_disabled {
                    <p class="text-sm text-muted-foreground">
                        (t!(auth_registration_disabled))
                    </p>
                    <p class="mt-4 text-sm text-muted-foreground">
                        <a class="underline underline-offset-4" href="/login">
                            (t!(auth_login_link))
                        </a>
                    </p>
                } else {
                <form method="post" action="/register" class="flex flex-col gap-4">
                    if invalid_form {
                        field_error((t!(auth_form_invalid)))
                    }
                    if invalid_email {
                        field_error((t!(auth_invalid_email)))
                    }
                    if password_too_short {
                        field_error((t!(auth_password_too_short)))
                    }
                    if password_mismatch {
                        field_error((t!(auth_password_mismatch)))
                    }
                    if email_taken {
                        field_error((t!(auth_email_taken)))
                    }
                    field(
                        field_label(attrs: attributes! { for=(&email_id) }, (t!(auth_email)))
                        input(
                            attrs: attributes! {
                                id=(&email_id)
                                name="email"
                                type="email"
                                autocomplete="email"
                                required=""
                                value=(email)
                            }
                        )
                    )
                    field(
                        field_label(
                            attrs: attributes! { for=(&username_id) },
                            (t!(auth_username))
                        )
                        input(
                            attrs: attributes! {
                                id=(&username_id)
                                name="username"
                                type="text"
                                autocomplete="nickname"
                                value=(username)
                            }
                        )
                    )
                    field(
                        field_label(
                            attrs: attributes! { for=(&password_id) },
                            (t!(auth_password))
                        )
                        input(
                            attrs: attributes! {
                                id=(&password_id)
                                name="password"
                                type="password"
                                autocomplete="new-password"
                                required=""
                            }
                        )
                    )
                    field(
                        field_label(
                            attrs: attributes! { for=(&password_confirm_id) },
                            (t!(auth_password_confirm))
                        )
                        input(
                            attrs: attributes! {
                                id=(&password_confirm_id)
                                name="password_confirm"
                                type="password"
                                autocomplete="new-password"
                                required=""
                            }
                        )
                    )
                    button(attrs: attributes! { type="submit" }, (t!(auth_register_submit)))
                    <p class="text-sm text-muted-foreground">
                        (t!(auth_have_account))
                        <a class="underline underline-offset-4" href="/login">
                            (t!(auth_login_link))
                        </a>
                    </p>
                </form>
                }
            )
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(entries: &[(&str, &str)]) -> Vec<(String, String)> {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn parses_registration_fields_and_optional_username() {
        let form = RegisterForm::parse(pairs(&[
            ("email", " alice@example.com "),
            ("username", " Alice "),
            ("password", "secret123"),
            ("password_confirm", "secret123"),
        ]))
        .unwrap();

        assert_eq!(form.email, "alice@example.com");
        assert_eq!(form.username.as_deref(), Some("Alice"));
        assert_eq!(form.password, "secret123");

        let form = RegisterForm::parse(pairs(&[
            ("email", "alice@example.com"),
            ("username", "   "),
            ("password", "secret123"),
            ("password_confirm", "secret123"),
        ]))
        .unwrap();

        assert_eq!(form.username, None);
    }

    #[test]
    fn rejects_incomplete_or_unknown_registration_fields() {
        assert!(
            RegisterForm::parse(pairs(&[
                ("email", "alice@example.com"),
                ("password", "secret123"),
            ]))
            .is_none()
        );
        assert!(
            RegisterForm::parse(pairs(&[
                ("email", "alice@example.com"),
                ("username", "Alice"),
                ("password", "secret123"),
                ("password_confirm", "secret123"),
                ("admin", "true"),
            ]))
            .is_none()
        );
    }
}
