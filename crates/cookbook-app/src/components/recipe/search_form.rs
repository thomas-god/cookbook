use topcoat::{
    Result,
    context::Cx,
    icon::{IconData, icon},
    runtime::{Event, Signal},
    view::{View, attributes, component, svg::ViewBox, view},
};

use crate::{
    components::base::{field::field, input::input},
    t,
};

const SEARCH: IconData = IconData::unescaped_unchecked(
    ViewBox::new(0.0, 0.0, 24.0, 24.0),
    r#"<g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></g>"#,
);

#[component]
pub async fn recipe_search_form(
    cx: &Cx,
    query: Signal<String>,
    page: Signal<String>,
) -> Result<impl View> {
    Ok(view! {
        cx =>
        <div role="search">
            field(
                <div class="relative">
                    icon(
                        data: SEARCH,
                        size: 16,
                        attrs: attributes! {
                            class="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-muted-foreground"
                        }
                    )
                    input(
                        attrs: attributes! {
                            id="recipe-search-q"
                            name="q"
                            type="search"
                            class="pl-9"
                            :value=$(query.get())
                            @input=$(|_e: Event| {
                                raw!(
                                    "cookbook.debounce(
                                        ${_e}.target.id, 250,
                                        () => (
                                            ${query}.set(${_e}.target.value), ${page}.set(\"1\")
                                        )
                                    )",
                                    (),
                                )
                            })
                            placeholder=(t!(recipe_search_placeholder))
                        }
                    )
                </div>
            )
        </div>
    })
}
