use cookbook_domain::recipe::models::RecipeCategory;
use cookbook_domain::recipe::ports::{RecipeSort, SortDirection};
use topcoat::{
    Result,
    context::Cx,
    router::href,
    runtime::{Event, Signal},
    view::{View, attributes, component, view},
};

use crate::{
    app::recipes::page,
    components::base::{
        button::{ButtonSize, ButtonVariant, button},
        field::{field, field_label, field_legend, field_set},
        input::input,
    },
    t,
};

use super::category::category_label;

const SELECT_CLASS: &str = "h-9 w-full rounded-lg border border-border bg-transparent px-3 text-sm transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background";

/// A sort order offered by the recipe search form.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SortChoice {
    #[default]
    UpdatedDesc,
    UpdatedAsc,
    TitleAsc,
    TitleDesc,
}

impl SortChoice {
    pub(crate) const ALL: [Self; 4] = [
        Self::UpdatedDesc,
        Self::UpdatedAsc,
        Self::TitleAsc,
        Self::TitleDesc,
    ];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::UpdatedDesc => "updated-desc",
            Self::UpdatedAsc => "updated-asc",
            Self::TitleAsc => "title-asc",
            Self::TitleDesc => "title-desc",
        }
    }

    /// Parses a wire value. A missing value selects the default order.
    pub(crate) fn parse(value: Option<&str>) -> Option<Self> {
        match value.unwrap_or("updated-desc") {
            "updated-desc" => Some(Self::UpdatedDesc),
            "updated-asc" => Some(Self::UpdatedAsc),
            "title-asc" => Some(Self::TitleAsc),
            "title-desc" => Some(Self::TitleDesc),
            _ => None,
        }
    }

    pub(crate) fn to_domain(self) -> RecipeSort {
        match self {
            Self::UpdatedDesc => RecipeSort::LastEdited(SortDirection::Descending),
            Self::UpdatedAsc => RecipeSort::LastEdited(SortDirection::Ascending),
            Self::TitleAsc => RecipeSort::Title(SortDirection::Ascending),
            Self::TitleDesc => RecipeSort::Title(SortDirection::Descending),
        }
    }

    fn label(self, cx: &Cx) -> String {
        match self {
            Self::UpdatedDesc => t!(recipe_search_sort_updated_desc).resolve(cx).into_owned(),
            Self::UpdatedAsc => t!(recipe_search_sort_updated_asc).resolve(cx).into_owned(),
            Self::TitleAsc => t!(recipe_search_sort_title_asc).resolve(cx).into_owned(),
            Self::TitleDesc => t!(recipe_search_sort_title_desc).resolve(cx).into_owned(),
        }
    }
}

fn category_field_name(category: RecipeCategory) -> String {
    format!("category_{}", category.as_str())
}

#[component]
pub async fn recipe_search_form(
    cx: &Cx,
    query: Signal<String>,
    categories: &[RecipeCategory],
    exclude_needs_review: bool,
    sort: SortChoice,
) -> Result<impl View> {
    let sort_options = SortChoice::ALL.map(|choice| (choice, choice.label(cx)));
    let options_active =
        !categories.is_empty() || exclude_needs_review || sort != SortChoice::default();

    Ok(view! {
        cx =>
        <form
            method="get"
            action=(href!(page))
            class="flex flex-col gap-5 rounded-lg border border-border bg-card p-4"
        >
            field(
                field_label(
                    attrs: attributes! { for="recipe-search-q" },
                    (t!(recipe_search))
                )
                input(
                    attrs: attributes! {
                        id="recipe-search-q"
                        name="q"
                        type="search"
                        :value=$(query.get())
                        @input=$(|_e: Event| {
                            raw!(
                                "(clearTimeout(${_e}.target.__cookbookSearchDebounce), ${_e}.target.__cookbookSearchDebounce = setTimeout(() => ${query}.set(${_e}.target.value), 250))",
                                (),
                            )
                        })
                        placeholder=(t!(recipe_search_placeholder))
                    }
                )
            )

            <details class="rounded-lg border border-border" open=(options_active)>
                <summary
                    class="cursor-pointer px-3 py-2 text-sm font-medium select-none"
                >
                    (t!(recipe_search_options))
                </summary>
                <div class="flex flex-col gap-5 border-t border-border p-3">
                    field_set(
                        field_legend((t!(recipe_search_categories)))
                        <div class="grid grid-cols-2 gap-2 sm:grid-cols-3">
                            for category in RecipeCategory::ALL {
                                <label class="flex items-center gap-2 text-sm">
                                    <input
                                        type="checkbox"
                                        name=(category_field_name(category))
                                        value="true"
                                        checked=(categories.contains(&category))
                                        class="size-4 rounded border-border accent-primary"
                                    >
                                    (category_label(cx, category))
                                </label>
                            }
                        </div>
                    )

                    <label class="flex items-center gap-2 text-sm">
                        <input
                            type="checkbox"
                            name="exclude_needs_review"
                            value="true"
                            checked=(exclude_needs_review)
                            class="size-4 rounded border-border accent-primary"
                        >
                        (t!(recipe_search_exclude_needs_review))
                    </label>

                    field(
                        field_label(
                            attrs: attributes! { for="recipe-search-sort" },
                            (t!(recipe_search_sort))
                        )
                        <select
                            id="recipe-search-sort"
                            name="sort"
                            class=(SELECT_CLASS)
                        >
                            for (choice, label) in sort_options {
                                <option value=(choice.as_str()) selected=(choice == sort)>
                                    (label)
                                </option>
                            }
                        </select>
                    )
                </div>
            </details>

            <div class="flex flex-wrap items-center gap-3">
                button(
                    variant: ButtonVariant::Primary,
                    size: ButtonSize::Sm,
                    attrs: attributes! { type="submit" },
                    (t!(recipe_search_submit))
                )
                <a class="text-sm underline underline-offset-4" href=(href!(page))>
                    (t!(recipe_search_clear))
                </a>
            </div>
        </form>
    })
}
