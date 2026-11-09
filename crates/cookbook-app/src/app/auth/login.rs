use cookbook_infra::auth::{
    models::UserEmail,
    ports::{AuthService, LoginError},
};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{content::Form, error::see_other, page},
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

    login_form(cx, String::new(), None).await
}

#[page(POST)]
pub async fn login(cx: &Cx, Form(form): Form<Vec<(String, String)>>) -> Result<impl View> {
    let Some(form) = LoginForm::parse(form) else {
        return login_form(cx, String::new(), Some(LoginFormError::InvalidForm)).await;
    };

    let auth: &AppAuthService = app_context(cx);

    match auth
        .login(UserEmail::new(form.email.clone()), form.password)
        .await
    {
        Ok(result) => {
            write_issued_session(cx, result.session()).await?;
            Err(see_other("/recipes").into())
        }
        Err(LoginError::InvalidCredentials) => {
            login_form(cx, form.email, Some(LoginFormError::InvalidCredentials)).await
        }
        Err(error) => Err(error.into()),
    }
}

#[derive(Clone, Copy)]
pub enum LoginFormError {
    InvalidForm,
    InvalidCredentials,
}

struct LoginForm {
    email: String,
    password: String,
}

impl LoginForm {
    fn parse(pairs: Vec<(String, String)>) -> Option<Self> {
        let mut email = None;
        let mut password = None;

        for (key, value) in pairs {
            match key.as_str() {
                "email" => {
                    if email.replace(value).is_some() {
                        return None;
                    }
                }
                "password" => {
                    if password.replace(value).is_some() {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        let email = email?.trim().to_string();
        let password = password?;

        if email.is_empty() || password.is_empty() {
            return None;
        }

        Some(Self { email, password })
    }
}

async fn login_form(cx: &Cx, email: String, error: Option<LoginFormError>) -> Result<impl View> {
    let email_id = element_id(cx);
    let password_id = element_id(cx);
    let settings: &AuthSettings = app_context(cx);
    let registration_enabled = settings.registration_enabled();
    let invalid_form = matches!(error, Some(LoginFormError::InvalidForm));
    let invalid_credentials = matches!(error, Some(LoginFormError::InvalidCredentials));

    Ok(view! {
        cx =>
        card(
            attrs: attributes! { class="mx-auto max-w-md" },
            card_header(card_title((t!(auth_login_heading))))
            card_content(
                <form method="post" action="/login" class="flex flex-col gap-4">
                    if invalid_form {
                        field_error((t!(auth_form_invalid)))
                    }
                    if invalid_credentials {
                        field_error((t!(auth_invalid_credentials)))
                    }
                    field(
                        field_label(
                            attrs: attributes! { for=(&email_id) },
                            (t!(auth_email))
                        )
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
                            attrs: attributes! { for=(&password_id) },
                            (t!(auth_password))
                        )
                        input(
                            attrs: attributes! {
                                id=(&password_id)
                                name="password"
                                type="password"
                                autocomplete="current-password"
                                required=""
                            }
                        )
                    )
                    button(
                        attrs: attributes! { type="submit" },
                        (t!(auth_login_submit))
                    )
                    if registration_enabled {
                        <p class="text-sm text-muted-foreground">
                            (t!(auth_no_account))
                            <a class="underline underline-offset-4" href="/register">
                                (t!(auth_register_link))
                            </a>
                        </p>
                    }
                </form>
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
    fn parses_and_trims_login_fields() {
        let form = LoginForm::parse(pairs(&[
            ("email", "  alice@example.com "),
            ("password", "secret"),
        ]))
        .unwrap();

        assert_eq!(form.email, "alice@example.com");
        assert_eq!(form.password, "secret");
    }

    #[test]
    fn rejects_incomplete_or_unknown_login_fields() {
        assert!(LoginForm::parse(pairs(&[("email", "alice@example.com")])).is_none());
        assert!(
            LoginForm::parse(pairs(&[("email", "alice@example.com"), ("password", "")])).is_none()
        );
        assert!(
            LoginForm::parse(pairs(&[
                ("email", "alice@example.com"),
                ("password", "secret"),
                ("next", "/recipes"),
            ]))
            .is_none()
        );
        assert!(
            LoginForm::parse(pairs(&[
                ("email", "alice@example.com"),
                ("email", "bob@example.com"),
                ("password", "secret"),
            ]))
            .is_none()
        );
    }
}
