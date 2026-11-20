use cookbook_domain::recipe::models::RecipeCategory;
use topcoat::{
    Result,
    context::Cx,
    icon::icon,
    runtime::{Event, Signal},
    view::{View, attributes, component, view},
};

use crate::{
    components::{
        base::{
            button::{ButtonSize, ButtonVariant, button},
            field::field,
            icons::{FILTER, SEARCH},
            input::input,
        },
        recipe::category::category_label,
    },
    t,
};

#[component]
pub async fn recipe_search_form(
    cx: &Cx,
    query: Signal<String>,
    page: Signal<String>,
    filters_open: Signal<bool>,
    categories: Vec<(RecipeCategory, Signal<bool>)>,
    exclude_needs_review: Signal<bool>,
    favorites_only: Signal<bool>,
) -> Result<impl View> {
    Ok(view! {
        <div role="search" class="flex flex-col gap-2">
            <div class="flex flex-row items-stretch gap-2">
                <div class="min-w-0 flex-1">
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
                <div>
                    button(
                        variant: ButtonVariant::Outline,
                        size: ButtonSize::Sm,
                        attrs: attributes! {
                            type="button"
                            aria-controls="recipe-search-filters"
                            :aria-expanded=$(filters_open.get())
                            @click=$(|_e: Event| filters_open.toggle())
                            class="h-full"
                        },
                        icon(data: FILTER, size: 22)
                    )
                </div>
            </div>

            <div
                id="recipe-search-filters"
                :hidden=$(!filters_open.get())
                class="flex flex-col gap-3 rounded-lg border border-border bg-card p-3"
            >
                <div
                    role="group"
                    aria-label=(t!(recipe_search_categories))
                    class="flex flex-wrap gap-2"
                >
                    for (category, selected) in &categories {
                        category_chip(
                            category: *category,
                            selected: selected.clone(),
                            page: page.clone()
                        )
                    }
                </div>
                <div class="flex flex-wrap gap-x-4 gap-y-2">
                    filter_toggle(
                        label: t!(recipe_search_favorites_only).resolve(cx).into_owned(),
                        selected: favorites_only.clone(),
                        page: page.clone()
                    )
                    filter_toggle(
                        label: t!(recipe_search_exclude_needs_review)
                            .resolve(cx)
                            .into_owned(),
                        selected: exclude_needs_review.clone(),
                        page: page.clone()
                    )
                </div>
            </div>
        </div>
    })
}

#[component]
async fn category_chip(
    cx: &Cx,
    category: RecipeCategory,
    selected: Signal<bool>,
    page: Signal<String>,
) -> Result<impl View> {
    let label = category_label(cx, category);

    Ok(view! {
        <label
            class="inline-flex cursor-pointer items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground transition-colors select-none hover:bg-foreground/5 hover:text-foreground has-[:checked]:border-primary has-[:checked]:bg-primary has-[:checked]:text-primary-foreground has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-ring has-[:focus-visible]:ring-offset-2 has-[:focus-visible]:ring-offset-background"
        >
            <input
                type="checkbox"
                class="sr-only"
                :checked=$(selected.get())
                @change=$(|_e: Event| {
                    selected.toggle();
                    page.set("1".to_owned());
                })
            >
            (label)
        </label>
    })
}

#[component]
async fn filter_toggle(
    cx: &Cx,
    #[into] label: String,
    selected: Signal<bool>,
    page: Signal<String>,
) -> Result<impl View> {
    Ok(view! {
        <label class="inline-flex cursor-pointer items-center gap-2 text-sm">
            <input
                type="checkbox"
                class="size-4 rounded border-border accent-primary"
                :checked=$(selected.get())
                @change=$(|_e: Event| {
                    selected.toggle();
                    page.set("1".to_owned());
                })
            >
            (label)
        </label>
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use topcoat::{runtime::signal, view::ViewExt};

    #[component]
    async fn test_form(cx: &Cx) -> Result<impl View> {
        Ok(view! {
            recipe_search_form(
                query: signal(cx, String::new),
                page: signal(cx, || "1".to_string()),
                filters_open: signal(cx, || false),
                categories: RecipeCategory::ALL
                    .into_iter()
                    .map(
                        |category| {
                            let selected = category == RecipeCategory::Main;
                            (
                                category,
                                signal(&cx.keyed(category.as_str()), move || selected),
                            )
                        },
                    )
                    .collect(),
                exclude_needs_review: signal(cx, || true),
                favorites_only: signal(cx, || false)
            )
        })
    }

    #[tokio::test]
    async fn renders_the_filters_collapsed_with_the_initial_selection() {
        let cx = &Cx::default();
        let html = view! { test_form() }.single().await.unwrap().render(cx);

        assert!(html.contains("id=\"recipe-search-filters\""), "{html}");
        assert!(html.contains("hidden"), "{html}");
        assert!(html.contains("aria-expanded"), "{html}");
        assert!(html.contains("data-topcoat-bind:checked"), "{html}");
        assert_eq!(html.matches("type=\"checkbox\"").count(), 11, "{html}");
    }
}
