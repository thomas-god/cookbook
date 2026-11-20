use chrono::{Local, NaiveDate};
use cookbook_domain::recipe::models::{MAX_COOKED_RECIPE_NOTE_CHARS, RecipeCookedEvent};
use topcoat::{
    Result,
    context::Cx,
    router::href,
    view::{View, attributes, class, component, element_id, view},
};

use crate::{
    app::{
        cooked::{CookedEventId, delete_cooked_event, update_cooked_event},
        recipe::view::recipe_detail,
        recipes::RecipeId,
    },
    components::base::{
        button::{ButtonSize, ButtonVariant, button, button_variants},
        card::{card, card_content, card_header, card_title},
    },
    t,
};

const TEXTAREA_CLASS: &str = "min-h-24 w-full rounded-lg border border-border bg-transparent px-3 py-2 text-sm transition-colors outline-none placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background";

fn format_cooked_on(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn today() -> String {
    format_cooked_on(Local::now().date_naive())
}

fn update_action(cx: &Cx, event_id: &str) -> String {
    href!(update_cooked_event, CookedEventId(event_id)).resolve(cx)
}

fn delete_action(cx: &Cx, event_id: &str) -> String {
    href!(delete_cooked_event, CookedEventId(event_id)).resolve(cx)
}

fn recipe_href(cx: &Cx, recipe_id: &str) -> String {
    href!(recipe_detail, RecipeId(recipe_id)).resolve(cx)
}

/// A date and optional note form shared by creating and editing an event.
#[component]
pub async fn cooked_event_form(
    cx: &Cx,
    action: String,
    cooked_on: String,
    note: String,
    next: Option<String>,
    submit_label: String,
) -> Result<impl View> {
    let date_id = element_id(cx);
    let note_id = element_id(cx);

    Ok(view! {
        cx =>
        <form method="post" action=(action) class="flex flex-col gap-3">
            if let Some(next) = &next {
                <input type="hidden" name="next" value=(next)>
            }
            <div class="flex flex-col gap-1.5">
                <label for=(&date_id) class="text-sm font-medium text-muted-foreground">
                    (t!(cooked_event_date))
                </label>
                <input
                    id=(&date_id)
                    type="date"
                    name="cooked_on"
                    value=(cooked_on)
                    required=""
                    class="h-9 w-full rounded-lg border border-border bg-transparent px-3 py-1 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background sm:w-56"
                >
            </div>
            <div class="flex flex-col gap-1.5">
                <label for=(&note_id) class="text-sm font-medium text-muted-foreground">
                    (t!(cooked_event_note))
                </label>
                <textarea
                    id=(&note_id)
                    name="note"
                    rows="3"
                    maxlength=(MAX_COOKED_RECIPE_NOTE_CHARS.to_string())
                    placeholder=(t!(cooked_event_note_placeholder))
                    class=(TEXTAREA_CLASS)
                >
                    (note)
                </textarea>
            </div>
            <div class="flex items-center gap-2">
                button(attrs: attributes! { type="submit" }, (submit_label))
            </div>
        </form>
    })
}

#[component]
pub async fn cooked_event_actions(
    cx: &Cx,
    event: &RecipeCookedEvent,
    next: &str,
) -> Result<impl View> {
    let update_action = update_action(cx, event.id().as_str());
    let delete_action = delete_action(cx, event.id().as_str());
    let delete_confirm = t!(cooked_event_delete_confirm).resolve(cx);

    Ok(view! {
        cx =>
        <div class="mt-2 flex flex-wrap items-center gap-2">
            <details class="inline-block">
                <summary
                    class=(class!(
                        button_variants(ButtonVariant::Ghost, ButtonSize::Sm),
                        "cursor-pointer list-none [&::-webkit-details-marker]:hidden",
                    ))
                >
                    (t!(cooked_event_edit))
                </summary>
                <div class="mt-2 rounded-lg border border-border bg-card p-3">
                    cooked_event_form(
                        action: update_action,
                        cooked_on: format_cooked_on(event.cooked_on()),
                        note: event.note().as_str().to_string(),
                        next: Some(next.to_string()),
                        submit_label: t!(cooked_event_save).resolve(cx).into_owned()
                    )
                </div>
            </details>
            <form
                method="post"
                action=(delete_action)
                onsubmit=(format!("return confirm({delete_confirm:?})"))
            >
                <input type="hidden" name="next" value=(next)>
                button(
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Sm,
                    attrs: attributes! {
                        type="submit"
                        class="text-destructive hover:bg-destructive/10"
                    },
                    (t!(cooked_event_delete))
                )
            </form>
        </div>
    })
}

/// The cooking log form and entries shown on the dedicated journal page.
#[component]
pub async fn recipe_cooked_events(
    cx: &Cx,
    recipe_id: &str,
    events: &[RecipeCookedEvent],
) -> Result<impl View> {
    let add_action = href!(
        crate::app::cooked::add_recipe_cooked_event,
        RecipeId(recipe_id)
    )
    .resolve(cx);
    let next = href!(crate::app::cooked::recipe_cook_journal, RecipeId(recipe_id)).resolve(cx);

    Ok(view! {
        cx =>
        <div class="flex flex-col gap-4">
            card(
                card_header(card_title((t!(cooked_event_new))))
                card_content(
                    cooked_event_form(
                        action: add_action,
                        cooked_on: today(),
                        note: String::new(),
                        next: None,
                        submit_label: t!(cooked_event_add).resolve(cx).into_owned()
                    )
                )
            )

            if events.is_empty() {
                <p class="text-sm text-muted-foreground">(t!(cooked_event_empty))</p>
            } else {
                <ul class="flex flex-col gap-3">
                    #[key(event.id().as_str())]
                    for event in events {
                        <li class="rounded-lg border border-border px-3 py-3 text-sm">
                            <div
                                class="flex flex-wrap items-baseline justify-between gap-2"
                            >
                                <time
                                    datetime=(format_cooked_on(event.cooked_on()))
                                    class="font-medium"
                                >
                                    (format_cooked_on(event.cooked_on()))
                                </time>
                                <a
                                    class="text-sm text-muted-foreground underline underline-offset-4"
                                    href=(href!(
                                        crate::app::recipes::recipe_revision,
                                        RecipeId(recipe_id),
                                        crate::app::recipes::RevisionId(
                                            event.snapshot().revision_id().as_str(),
                                        ),
                                    ))
                                >
                                    (t!(cooked_event_view_version))
                                </a>
                            </div>
                            if !event.note().as_str().is_empty() {
                                <p class="mt-1 whitespace-pre-line">
                                    (event.note().as_str())
                                </p>
                            }
                            cooked_event_actions(event: event, next: next.as_str())
                        </li>
                    }
                </ul>
            }
        </div>
    })
}

#[component]
pub async fn cooked_event_log_entry(
    cx: &Cx,
    entry: &cookbook_domain::recipe::models::RecipeCookedEventEntry,
    next: &str,
) -> Result<impl View> {
    let event = entry.event();

    Ok(view! {
        cx =>
        <li class="rounded-lg border border-border bg-card px-4 py-3 text-sm">
            <div class="flex flex-wrap items-baseline justify-between gap-2">
                <div class="flex flex-wrap items-center gap-2">
                    if let Some(link) = entry.recipe_link() {
                        <a
                            class="font-medium underline underline-offset-4"
                            href=(recipe_href(cx, link.recipe_id().as_str()))
                        >
                            (entry.display_title().as_str())
                        </a>
                    } else {
                        <span class="font-medium">
                            (entry.display_title().as_str())
                        </span>
                        <span
                            class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-xs font-medium text-muted-foreground"
                        >
                            if event.recipe_id().is_none() {
                                (t!(cooked_event_recipe_deleted))
                            } else {
                                (t!(cooked_event_recipe_unavailable))
                            }
                        </span>
                    }
                </div>
                <div class="flex flex-wrap items-baseline justify-end gap-3">
                    if let Some(link) = entry.recipe_link() {
                        <a
                            class="text-sm text-muted-foreground underline underline-offset-4"
                            href=(href!(
                                crate::app::recipes::recipe_revision,
                                RecipeId(link.recipe_id().as_str()),
                                crate::app::recipes::RevisionId(link.revision_id().as_str()),
                            ))
                        >
                            (t!(cooked_event_view_version))
                        </a>
                    }
                    <time
                        datetime=(format_cooked_on(event.cooked_on()))
                        class="text-muted-foreground"
                    >
                        (format_cooked_on(event.cooked_on()))
                    </time>
                </div>
            </div>
            if !event.note().as_str().is_empty() {
                <p class="mt-1 whitespace-pre-line">(event.note().as_str())</p>
            }
            cooked_event_actions(event: event, next: next)
        </li>
    })
}

/// The current user's cooking log across all recipes.
#[component]
pub async fn cooked_event_log(
    cx: &Cx,
    entries: &[cookbook_domain::recipe::models::RecipeCookedEventEntry],
    next: &str,
) -> Result<impl View> {
    Ok(view! {
        cx =>
        if entries.is_empty() {
            <p class="mt-8 text-sm text-muted-foreground">
                (t!(cooked_event_log_empty))
            </p>
        } else {
            <ul class="mt-3 flex flex-col gap-3">
                #[key(entry.event().id().as_str())]
                for entry in entries {
                    cooked_event_log_entry(entry: entry, next: next)
                }
            </ul>
        }
    })
}
