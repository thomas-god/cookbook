use cookbook_infra::auth::models::AuthenticatedUser;
use topcoat::{
    Result,
    context::Cx,
    icon::icon,
    router::href,
    view::{View, attributes, class, component, view},
};

use crate::{
    app::{admin, auth::account},
    components::base::{
        button::{ButtonSize, ButtonVariant, button, button_variants},
        icons::SETTINGS,
    },
    t,
};

#[component]
pub async fn user_menu(cx: &Cx, user: &AuthenticatedUser) -> Result<impl View> {
    Ok(view! {
        cx =>
        <details class="relative" data-menu="">
            <summary
                class=(class!(
                    button_variants(ButtonVariant::Ghost, ButtonSize::Sm),
                    "cursor-pointer list-none [&::-webkit-details-marker]:hidden",
                ))
                aria-label=(t!(account_menu_label))
            >
                (user.display_name())
                icon(data: SETTINGS)
            </summary>
            <div
                class="absolute right-0 z-10 mt-1 flex w-48 flex-col gap-1 rounded-lg border border-border bg-card p-1 shadow-md"
            >
                <a
                    class=(class!(
                        button_variants(ButtonVariant::Ghost, ButtonSize::Sm),
                        "w-full justify-start",
                    ))
                    href=(href!(account::page))
                >
                    (t!(account_heading))
                </a>
                if user.role().is_admin() {
                    <a
                        class=(class!(
                            button_variants(ButtonVariant::Ghost, ButtonSize::Sm),
                            "w-full justify-start",
                        ))
                        href=(href!(admin::users::page))
                    >
                        (t!(admin_users_link))
                    </a>
                }
                <form method="post" action="/logout">
                    button(
                        variant: ButtonVariant::Ghost,
                        size: ButtonSize::Sm,
                        attrs: attributes! { type="submit" class="w-full justify-start" },
                        (t!(auth_logout))
                    )
                </form>
            </div>
        </details>
    })
}
