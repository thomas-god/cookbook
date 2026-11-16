use cookbook_domain::user::models::{UserId as UserIdValue, UserRole, Username};
use cookbook_infra::auth::{
    models::{AdminActor, UserEmail, UserSummary},
    ports::{
        ChangeUserEmailError, ChangeUserUsernameError, CreateUserError, CreateUserRequest,
        DeleteUserError, ResetUserPasswordError, SetUserRoleError, UserAdminService,
    },
};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Form,
        error::{SeeOther, bad_request, forbidden, not_found, see_other},
        href, page, path_param, query_params, route,
    },
    view::{View, attributes, element_id, view},
};

use crate::{
    app::{AppAuthService, auth::require_admin},
    components::base::{
        button::{ButtonSize, ButtonVariant, button, button_variants},
        card::{card, card_content, card_header, card_title},
        field::{field, field_error, field_label},
        input::input,
    },
    t,
};

path_param!(pub user_id);

#[query_params(error = bad_request)]
struct ListQuery {
    updated: Option<String>,
}

impl ListQuery {
    fn update(&self) -> Option<Update> {
        parse_update(self.updated.as_deref())
    }
}

#[query_params(error = bad_request)]
struct EditQuery {
    updated: Option<String>,
    error: Option<String>,
}

impl EditQuery {
    fn update(&self) -> Option<Update> {
        parse_update(self.updated.as_deref())
    }

    fn error(&self) -> Option<EditError> {
        match self.error.as_deref() {
            Some("last-admin") => Some(EditError::LastAdmin),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum Update {
    Created,
    Deleted,
    Email,
    Username,
    Password,
    Role,
}

fn parse_update(value: Option<&str>) -> Option<Update> {
    match value {
        Some("created") => Some(Update::Created),
        Some("deleted") => Some(Update::Deleted),
        Some("email") => Some(Update::Email),
        Some("username") => Some(Update::Username),
        Some("password") => Some(Update::Password),
        Some("role") => Some(Update::Role),
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum EditError {
    LastAdmin,
}

#[derive(Clone, Copy)]
enum CreateFormError {
    InvalidForm,
    InvalidEmail,
    PasswordMismatch,
    EmailTaken,
}

#[derive(Clone, Copy)]
enum EmailFormError {
    InvalidForm,
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
    PasswordMismatch,
}

#[derive(Default)]
struct CreateFormState {
    email: String,
    username: String,
    as_admin: bool,
}

struct CreateUserForm {
    email: String,
    username: Option<String>,
    password: String,
    password_confirm: String,
    as_admin: bool,
}

impl CreateUserForm {
    fn parse(pairs: Vec<(String, String)>) -> Option<Self> {
        let mut email = None;
        let mut username = None;
        let mut password = None;
        let mut password_confirm = None;
        let mut as_admin = false;

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
                "as_admin" => {
                    as_admin = matches!(value.as_str(), "true" | "on" | "1");
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
            as_admin,
        })
    }

    fn state(&self) -> CreateFormState {
        CreateFormState {
            email: self.email.clone(),
            username: self.username.clone().unwrap_or_default(),
            as_admin: self.as_admin,
        }
    }
}

struct EmailForm {
    email: String,
}

impl EmailForm {
    fn parse(pairs: Vec<(String, String)>) -> Option<Self> {
        let mut email = None;

        for (key, value) in pairs {
            match key.as_str() {
                "email" => {
                    if email.replace(value).is_some() {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        let email = email?.trim().to_string();

        if email.is_empty() {
            return None;
        }

        Some(Self { email })
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
    password: String,
    password_confirm: String,
}

impl PasswordForm {
    fn parse(pairs: Vec<(String, String)>) -> Option<Self> {
        let mut password = None;
        let mut password_confirm = None;

        for (key, value) in pairs {
            match key.as_str() {
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

        let password = password?;
        let password_confirm = password_confirm?;

        if password.is_empty() {
            return None;
        }

        Some(Self {
            password,
            password_confirm,
        })
    }
}

#[derive(Default)]
struct EditFormState {
    new_email: String,
    new_username: String,
    email_error: Option<EmailFormError>,
    username_error: Option<UsernameFormError>,
    password_error: Option<PasswordFormError>,
}

impl EditFormState {
    fn for_user(user: &UserSummary) -> Self {
        Self {
            new_email: user.email().as_str().to_string(),
            new_username: user
                .username()
                .map(|username| username.as_str().to_string())
                .unwrap_or_default(),
            ..Self::default()
        }
    }
}

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let admin = require_admin(cx)?;
    let query = query_params::<ListQuery>(cx)?;
    let auth: &AppAuthService = app_context(cx);

    let users = auth.list_users(&admin).await?;

    users_page(cx, admin.id().clone(), users, query.update()).await
}

#[page("./new")]
pub async fn new_page(cx: &Cx) -> Result<impl View> {
    require_admin(cx)?;

    create_page(cx, CreateFormState::default(), None).await
}

#[page(POST)]
pub async fn create(cx: &Cx, Form(form): Form<Vec<(String, String)>>) -> Result<impl View> {
    let admin = require_admin(cx)?;

    let Some(form) = CreateUserForm::parse(form) else {
        return create_page(
            cx,
            CreateFormState::default(),
            Some(CreateFormError::InvalidForm),
        )
        .await;
    };

    if form.password != form.password_confirm {
        return create_page(cx, form.state(), Some(CreateFormError::PasswordMismatch)).await;
    }

    let state = form.state();
    let Ok(email) = UserEmail::try_new(form.email.clone()) else {
        return create_page(cx, state, Some(CreateFormError::InvalidEmail)).await;
    };

    let auth: &AppAuthService = app_context(cx);
    let request = CreateUserRequest {
        email,
        username: form.username.clone().map(Username::new),
        password: form.password,
        role: if form.as_admin {
            UserRole::Admin
        } else {
            UserRole::User
        },
    };

    match auth.create_user(&admin, request).await {
        Ok(_) => Err(see_other(href!(page).resolve(cx)).into()),
        Err(CreateUserError::EmailTaken) => {
            create_page(cx, state, Some(CreateFormError::EmailTaken)).await
        }
        Err(error) => Err(error.into()),
    }
}

#[page("./{user_id}")]
pub async fn edit_page(cx: &Cx) -> Result<impl View> {
    let admin = require_admin(cx)?;
    let query = query_params::<EditQuery>(cx)?;
    let user = load_user(cx, &admin, &path_user_id(cx)).await?;

    let state = EditFormState::for_user(&user);

    edit_view(
        cx,
        admin.id().clone(),
        user,
        state,
        query.update(),
        query.error(),
    )
    .await
}

#[page(POST "./{user_id}/email")]
pub async fn change_email(cx: &Cx, Form(form): Form<Vec<(String, String)>>) -> Result<impl View> {
    let admin = require_admin(cx)?;
    let id = path_user_id(cx);
    let user = load_user(cx, &admin, &id).await?;

    let Some(form) = EmailForm::parse(form) else {
        let mut state = EditFormState::for_user(&user);
        state.email_error = Some(EmailFormError::InvalidForm);
        return edit_view(cx, admin.id().clone(), user, state, None, None).await;
    };

    let Ok(email) = UserEmail::try_new(form.email.clone()) else {
        let mut state = EditFormState::for_user(&user);
        state.new_email = form.email;
        state.email_error = Some(EmailFormError::InvalidEmail);
        return edit_view(cx, admin.id().clone(), user, state, None, None).await;
    };

    let auth: &AppAuthService = app_context(cx);

    match auth.change_user_email(&admin, &id, email).await {
        Ok(_) => Err(see_other(edit_url(cx, &id, "updated=email")).into()),
        Err(ChangeUserEmailError::EmailTaken) => {
            let mut state = EditFormState::for_user(&user);
            state.new_email = form.email;
            state.email_error = Some(EmailFormError::EmailTaken);
            edit_view(cx, admin.id().clone(), user, state, None, None).await
        }
        Err(error) => Err(error.into()),
    }
}

#[page(POST "./{user_id}/username")]
pub async fn change_username(
    cx: &Cx,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<impl View> {
    let admin = require_admin(cx)?;
    let id = path_user_id(cx);
    let user = load_user(cx, &admin, &id).await?;

    let Some(form) = UsernameForm::parse(form) else {
        let mut state = EditFormState::for_user(&user);
        state.username_error = Some(UsernameFormError::InvalidForm);
        return edit_view(cx, admin.id().clone(), user, state, None, None).await;
    };

    let auth: &AppAuthService = app_context(cx);

    match auth
        .change_user_username(&admin, &id, form.username.map(Username::new))
        .await
    {
        Ok(_) => Err(see_other(edit_url(cx, &id, "updated=username")).into()),
        Err(ChangeUserUsernameError::NotFound) => Err(not_found().into()),
        Err(error) => Err(error.into()),
    }
}

#[page(POST "./{user_id}/password")]
pub async fn change_password(
    cx: &Cx,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<impl View> {
    let admin = require_admin(cx)?;
    let id = path_user_id(cx);
    let user = load_user(cx, &admin, &id).await?;

    let Some(form) = PasswordForm::parse(form) else {
        let mut state = EditFormState::for_user(&user);
        state.password_error = Some(PasswordFormError::InvalidForm);
        return edit_view(cx, admin.id().clone(), user, state, None, None).await;
    };

    if form.password != form.password_confirm {
        let mut state = EditFormState::for_user(&user);
        state.password_error = Some(PasswordFormError::PasswordMismatch);
        return edit_view(cx, admin.id().clone(), user, state, None, None).await;
    }

    let auth: &AppAuthService = app_context(cx);

    match auth.reset_user_password(&admin, &id, form.password).await {
        Ok(_) => Err(see_other(edit_url(cx, &id, "updated=password")).into()),
        Err(ResetUserPasswordError::NotFound) => Err(not_found().into()),
        Err(error) => Err(error.into()),
    }
}

#[route(POST "./{user_id}/role")]
pub async fn set_role(cx: &Cx, Form(form): Form<Vec<(String, String)>>) -> Result<SeeOther> {
    let admin = require_admin(cx)?;
    let id = path_user_id(cx);
    let Some(role) = parse_role_form(form) else {
        return Err(bad_request("role is required").into());
    };

    let auth: &AppAuthService = app_context(cx);

    match auth.set_user_role(&admin, &id, role).await {
        Ok(_) if id.as_str() == admin.id().as_str() && role == UserRole::User => Ok(see_other("/")),
        Ok(_) => Ok(see_other(edit_url(cx, &id, "updated=role"))),
        Err(SetUserRoleError::NotFound) => Err(not_found().into()),
        Err(SetUserRoleError::LastAdmin) => Ok(see_other(edit_url(cx, &id, "error=last-admin"))),
        Err(error) => Err(error.into()),
    }
}

#[route(POST "./{user_id}/delete")]
pub async fn delete_user(cx: &Cx) -> Result<SeeOther> {
    let admin = require_admin(cx)?;
    let id = path_user_id(cx);
    let auth: &AppAuthService = app_context(cx);

    match auth.delete_user(&admin, &id).await {
        Ok(()) => Ok(see_other(format!(
            "{}?updated=deleted",
            href!(page).resolve(cx)
        ))),
        Err(DeleteUserError::NotFound) => Err(not_found().into()),
        Err(DeleteUserError::CannotDeleteSelf) => Err(forbidden().into()),
        Err(DeleteUserError::LastAdmin) => Ok(see_other(edit_url(cx, &id, "error=last-admin"))),
        Err(error) => Err(error.into()),
    }
}

fn parse_role_form(pairs: Vec<(String, String)>) -> Option<UserRole> {
    let mut role = None;

    for (key, value) in pairs {
        match key.as_str() {
            "role" => {
                if role.replace(value).is_some() {
                    return None;
                }
            }
            _ => return None,
        }
    }

    UserRole::parse(role.as_deref()?.trim())
}

fn path_user_id(cx: &Cx) -> UserIdValue {
    UserIdValue::new(path_param::<UserId>(cx).to_string())
}

async fn load_user(cx: &Cx, admin: &AdminActor, id: &UserIdValue) -> Result<UserSummary> {
    let auth: &AppAuthService = app_context(cx);

    auth.get_user(admin, id)
        .await?
        .ok_or_else(|| not_found().into())
}

fn edit_url(cx: &Cx, id: &UserIdValue, query: &str) -> String {
    let base = href!(edit_page, UserId(id.as_str())).resolve(cx);

    format!("{base}?{query}")
}

async fn users_page(
    cx: &Cx,
    admin_id: UserIdValue,
    users: Vec<UserSummary>,
    updated: Option<Update>,
) -> Result<impl View + use<>> {
    Ok(view! {
        cx =>
        <div class="mb-6 flex items-center justify-between gap-4">
            <h2 class="text-2xl font-semibold">(t!(admin_users_heading))</h2>
            <a
                class=(button_variants(ButtonVariant::Primary, ButtonSize::Sm))
                href=(href!(new_page))
            >
                (t!(admin_users_new))
            </a>
        </div>
        if let Some(updated) = updated {
            <p
                role="status"
                class="mb-6 rounded-lg border border-border bg-muted px-4 py-3 text-sm"
            >
                if matches!(updated, Update::Created) {
                    (t!(admin_user_created))
                }
                if matches!(updated, Update::Deleted) {
                    (t!(admin_user_deleted))
                }
            </p>
        }
        if users.is_empty() {
            <p class="text-sm text-muted-foreground">(t!(admin_users_empty))</p>
        } else {
            <ul class="flex flex-col gap-2">
                for user in &users {
                    <li
                        class="flex flex-col gap-3 rounded-lg border border-border bg-card px-4 py-3 sm:flex-row sm:items-center"
                    >
                        <div class="min-w-0 flex-1">
                            <p class="font-medium">
                                (user.display_name())
                                if user.id() == &admin_id {
                                    <span class="ml-2 text-xs text-muted-foreground">
                                        (t!(admin_user_you))
                                    </span>
                                }
                            </p>
                            <p class="truncate text-sm text-muted-foreground">
                                (user.email().as_str())
                            </p>
                        </div>
                        <div class="flex flex-wrap items-center gap-2">
                            <span
                                class="rounded-full border border-border px-2 py-0.5 text-xs font-medium text-muted-foreground"
                            >
                                if user.role().is_admin() {
                                    (t!(admin_role_admin))
                                } else {
                                    (t!(admin_role_user))
                                }
                            </span>
                            <a
                                class=(button_variants(ButtonVariant::Ghost, ButtonSize::Sm))
                                href=(href!(edit_page, UserId(user.id().as_str())))
                            >
                                (t!(admin_user_edit))
                            </a>
                        </div>
                    </li>
                }
            </ul>
        }
    })
}

async fn create_page(
    cx: &Cx,
    state: CreateFormState,
    error: Option<CreateFormError>,
) -> Result<impl View + use<>> {
    let email_id = element_id(cx);
    let username_id = element_id(cx);
    let password_id = element_id(cx);
    let password_confirm_id = element_id(cx);

    let invalid_form = matches!(error, Some(CreateFormError::InvalidForm));
    let invalid_email = matches!(error, Some(CreateFormError::InvalidEmail));
    let password_mismatch = matches!(error, Some(CreateFormError::PasswordMismatch));
    let email_taken = matches!(error, Some(CreateFormError::EmailTaken));

    Ok(view! {
        cx =>
        <div class="mb-6">
            <a class="text-sm underline underline-offset-4" href=(href!(page))>
                (t!(admin_users_back))
            </a>
            <h2 class="mt-2 text-2xl font-semibold">(t!(admin_user_create_heading))</h2>
        </div>
        card(
            card_content(
                <form method="post" action=(href!(create)) class="flex flex-col gap-4">
                    if invalid_form {
                        field_error((t!(auth_form_invalid)))
                    }
                    if invalid_email {
                        field_error((t!(auth_invalid_email)))
                    }
                    if password_mismatch {
                        field_error((t!(auth_password_mismatch)))
                    }
                    if email_taken {
                        field_error((t!(auth_email_taken)))
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
                                value=(state.email)
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
                                value=(state.username)
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
                    <label class="flex items-center gap-2 text-sm">
                        if state.as_admin {
                            <input
                                type="checkbox"
                                name="as_admin"
                                value="true"
                                checked=""
                            >
                        } else {
                            <input type="checkbox" name="as_admin" value="true">
                        }
                        <span>(t!(admin_user_create_as_admin))</span>
                    </label>
                    <div class="flex items-center gap-2">
                        button(
                            attrs: attributes! { type="submit" },
                            (t!(admin_user_create_submit))
                        )
                        <a
                            class=(button_variants(ButtonVariant::Ghost, ButtonSize::Md))
                            href=(href!(page))
                        >
                            (t!(form_cancel))
                        </a>
                    </div>
                </form>
            )
        )
    })
}

async fn edit_view(
    cx: &Cx,
    admin_id: UserIdValue,
    user: UserSummary,
    state: EditFormState,
    updated: Option<Update>,
    error: Option<EditError>,
) -> Result<impl View + use<>> {
    let email_id = element_id(cx);
    let username_id = element_id(cx);
    let password_id = element_id(cx);
    let password_confirm_id = element_id(cx);

    let invalid_email_form = matches!(state.email_error, Some(EmailFormError::InvalidForm));
    let invalid_email = matches!(state.email_error, Some(EmailFormError::InvalidEmail));
    let email_taken = matches!(state.email_error, Some(EmailFormError::EmailTaken));
    let invalid_username_form =
        matches!(state.username_error, Some(UsernameFormError::InvalidForm));
    let invalid_password_form =
        matches!(state.password_error, Some(PasswordFormError::InvalidForm));
    let password_mismatch = matches!(
        state.password_error,
        Some(PasswordFormError::PasswordMismatch)
    );
    let delete_action = href!(delete_user, UserId(user.id().as_str())).resolve(cx);
    let delete_confirm = t!(admin_user_delete_confirm).resolve(cx);

    Ok(view! {
        cx =>
        <div class="mb-6">
            <a class="text-sm underline underline-offset-4" href=(href!(page))>
                (t!(admin_users_back))
            </a>
            <h2 class="mt-2 text-2xl font-semibold">
                (t!(admin_user_edit_heading))
                ": "
                (user.display_name())
            </h2>
        </div>
        if let Some(updated) = updated {
            <p
                role="status"
                class="mb-6 rounded-lg border border-border bg-muted px-4 py-3 text-sm"
            >
                if matches!(updated, Update::Email) {
                    (t!(account_updated_email))
                }
                if matches!(updated, Update::Username) {
                    (t!(account_updated_username))
                }
                if matches!(updated, Update::Password) {
                    (t!(account_updated_password))
                }
                if matches!(updated, Update::Role) {
                    (t!(admin_user_updated_role))
                }
            </p>
        }
        if matches!(error, Some(EditError::LastAdmin)) {
            <p
                role="alert"
                class="mb-6 rounded-lg border border-destructive bg-destructive/10 px-4 py-3 text-sm"
            >
                (t!(admin_user_last_admin))
            </p>
        }
        <div class="flex flex-col gap-6">
            card(
                card_header(card_title((t!(account_email_heading))))
                card_content(
                    <form
                        method="post"
                        action=(href!(change_email, UserId(user.id().as_str())))
                        class="flex flex-col gap-4"
                    >
                        if invalid_email_form {
                            field_error((t!(auth_form_invalid)))
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
                            <span class="font-medium">(user.email().as_str())</span>
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
                        action=(href!(change_username, UserId(user.id().as_str())))
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
                            if let Some(username) = user.username() {
                                <span class="font-medium">(username.as_str())</span>
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
                        action=(href!(change_password, UserId(user.id().as_str())))
                        class="flex flex-col gap-4"
                    >
                        if invalid_password_form {
                            field_error((t!(auth_form_invalid)))
                        }
                        if password_mismatch {
                            field_error((t!(auth_password_mismatch)))
                        }
                        field(
                            field_label(
                                attrs: attributes! { for=(&password_id) },
                                (t!(account_new_password))
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
                        button(
                            attrs: attributes! { type="submit" },
                            (t!(account_save_password))
                        )
                    </form>
                )
            )
            card(
                card_header(card_title((t!(admin_user_role_heading))))
                card_content(
                    <div class="flex flex-col gap-4">
                        <div class="text-sm">
                            <span class="text-muted-foreground">
                                (t!(admin_user_current_role))
                            </span>
                            ": "
                            <span class="font-medium">
                                if user.role().is_admin() {
                                    (t!(admin_role_admin))
                                } else {
                                    (t!(admin_role_user))
                                }
                            </span>
                            if user.id() == &admin_id {
                                <span class="ml-2 text-xs text-muted-foreground">
                                    (t!(admin_user_you))
                                </span>
                            }
                        </div>
                        <form
                            method="post"
                            action=(href!(set_role, UserId(user.id().as_str())))
                        >
                            if user.role().is_admin() {
                                <input type="hidden" name="role" value="user">
                                button(
                                    variant: ButtonVariant::Outline,
                                    attrs: attributes! { type="submit" },
                                    (t!(admin_user_demote))
                                )
                            } else {
                                <input type="hidden" name="role" value="admin">
                                button(
                                    variant: ButtonVariant::Primary,
                                    attrs: attributes! { type="submit" },
                                    (t!(admin_user_promote))
                                )
                            }
                        </form>
                    </div>
                )
            )
            if user.id() != &admin_id {
                card(
                    card_header(card_title((t!(admin_user_danger_heading))))
                    card_content(
                        <div class="flex flex-col gap-4">
                            <p class="text-sm text-muted-foreground">
                                (t!(admin_user_delete_hint))
                            </p>
                            <form
                                method="post"
                                action=(delete_action)
                                onsubmit=(format!("return confirm({delete_confirm:?})"))
                            >
                                button(
                                    variant: ButtonVariant::Destructive,
                                    attrs: attributes! { type="submit" },
                                    (t!(admin_user_delete_submit))
                                )
                            </form>
                        </div>
                    )
                )
            }
        </div>
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
    fn create_form_parses_trimmed_fields_and_the_admin_checkbox() {
        let form = CreateUserForm::parse(pairs(&[
            ("email", "  New@Example.com "),
            ("username", " Alice "),
            ("password", "secret"),
            ("password_confirm", "secret"),
            ("as_admin", "true"),
        ]))
        .unwrap();

        assert_eq!(form.email, "New@Example.com");
        assert_eq!(form.username.as_deref(), Some("Alice"));
        assert!(form.as_admin);
    }

    #[test]
    fn create_form_defaults_to_a_regular_user() {
        let form = CreateUserForm::parse(pairs(&[
            ("email", "new@example.com"),
            ("username", "   "),
            ("password", "secret"),
            ("password_confirm", "secret"),
        ]))
        .unwrap();

        assert!(!form.as_admin);
        assert!(form.username.is_none());
    }

    #[test]
    fn create_form_rejects_unknown_or_incomplete_fields() {
        assert!(CreateUserForm::parse(pairs(&[("email", "new@example.com")])).is_none());
        assert!(
            CreateUserForm::parse(pairs(&[
                ("email", "new@example.com"),
                ("password", "secret"),
                ("password_confirm", "secret"),
                ("next", "/"),
            ]))
            .is_none()
        );
    }

    #[test]
    fn username_form_clears_blank_values() {
        assert_eq!(
            UsernameForm::parse(pairs(&[("username", "   ")]))
                .unwrap()
                .username,
            None
        );
        assert_eq!(
            UsernameForm::parse(pairs(&[("username", " Alice ")]))
                .unwrap()
                .username
                .as_deref(),
            Some("Alice")
        );
    }

    #[test]
    fn password_form_rejects_an_empty_password() {
        assert!(
            PasswordForm::parse(pairs(&[("password", ""), ("password_confirm", ""),])).is_none()
        );
    }

    #[test]
    fn parses_role_values() {
        assert!(matches!(
            parse_role_form(pairs(&[("role", "admin")])),
            Some(UserRole::Admin)
        ));
        assert!(matches!(
            parse_role_form(pairs(&[("role", "user")])),
            Some(UserRole::User)
        ));
        assert!(parse_role_form(pairs(&[("role", "owner")])).is_none());
    }

    #[test]
    fn maps_update_and_error_queries() {
        assert!(matches!(parse_update(Some("email")), Some(Update::Email)));
        assert!(matches!(
            parse_update(Some("deleted")),
            Some(Update::Deleted)
        ));
        assert!(parse_update(Some("unknown")).is_none());

        let query = EditQuery {
            updated: None,
            error: Some("last-admin".to_string()),
        };
        assert!(matches!(query.error(), Some(EditError::LastAdmin)));
    }
}
