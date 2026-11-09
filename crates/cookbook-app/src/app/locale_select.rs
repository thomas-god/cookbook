use topcoat::{
    Result,
    context::Cx,
    l10n::{Locale, locale},
    router::request::uri,
    view::{View, attributes, component, view},
};

use crate::{
    components::button::{ButtonSize, ButtonVariant, button},
    t,
};

fn locale_name(locale: &Locale) -> &'static str {
    match locale.id().to_string().as_str() {
        "en" => "English",
        "fr" => "Français",
        _ => "?",
    }
}

#[component]
pub async fn locale_select(cx: &Cx) -> Result<impl View> {
    let current_locale = locale(cx);
    let current_path = uri(cx).path_and_query().map_or("/", |path| path.as_str());

    Ok(view! {
        <form method="post" action="/locale" class="ml-auto flex items-center gap-2">
            <label for="locale-select" class="sr-only">(t!(language_label))</label>
            <select
                id="locale-select"
                name="locale"
                class="h-8 rounded-md border border-border bg-transparent px-2 text-sm"
                onchange="this.form.submit()"
            >
                for available in t::locales() {
                    <option
                        value=(available.id().to_string())
                        selected=(available == current_locale)
                    >
                        (locale_name(available))
                    </option>
                }
            </select>
            <input type="hidden" name="next" value=(current_path)>
            <noscript>
                button(
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Sm,
                    attrs: attributes! { type="submit" },
                    (t!(language_apply))
                )
            </noscript>
        </form>
    })
}
