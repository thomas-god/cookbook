//! Editing primitives shared by the recipe form sections.

use topcoat::{
    Result,
    context::Cx,
    runtime::{Event, Signal, signal},
    view::{Attributes, Child, View, component, element_id, view},
};

use crate::{components::recipe::recipe_form::view::TEXTAREA_CLASS, t};

/// A visually distinct group of related form fields.
#[component]
pub(super) async fn form_section(
    #[into] heading: String,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <section
            class="flex flex-col gap-4 rounded-xl border border-border bg-foreground/[0.02] p-4 sm:p-5"
        >
            <h3 class="text-lg font-semibold">(heading)</h3>
            (child)
        </section>
    })
}

/// A pencil button that switches a field or row into edit mode, and a check
/// button that switches it back.
#[component]
async fn edit_toggle_icons(editing: Signal<bool>) -> Result<impl View> {
    Ok(view! {
        <span :hidden=$(editing.get()) aria-hidden="true">
            <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
                stroke-linejoin="round"
                class="size-4"
            >
                <path d="M12 20h9"></path>
                <path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z"></path>
            </svg>
        </span>
        <span :hidden=$(!editing.get()) aria-hidden="true">
            <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                class="size-4"
            >
                <path d="m5 12 5 5L20 7"></path>
            </svg>
        </span>
    })
}

/// A button that toggles an editing signal without validation.
#[component]
pub(super) async fn edit_toggle_button(
    cx: &Cx,
    editing: Signal<bool>,
    #[into] field: String,
) -> Result<impl View> {
    let edit_label = t!(form_edit_field, field = field.clone())
        .resolve(cx)
        .into_owned();
    let done_label = t!(form_done_editing, field = field)
        .resolve(cx)
        .into_owned();

    Ok(view! {
        <button
            type="button"
            class="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-foreground/5 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
            :aria-expanded=$(editing.get())
            :aria-label=$(if editing.get() {
                done_label.clone()
            } else {
                edit_label.clone()
            })
            :title=$(if editing.get() {
                done_label.clone()
            } else {
                edit_label.clone()
            })
            @click=$(|_e: Event| editing.toggle())
        >
            edit_toggle_icons(editing: editing.clone())
        </button>
    })
}

/// An icon-only delete button that matches the edit toggle's size and weight.
#[component]
pub(super) async fn delete_button(
    cx: &Cx,
    #[into] label: String,
    #[default] mut attrs: Attributes,
) -> Result<impl View> {
    attrs.insert(cx, "type", "button");
    attrs.insert(cx, "aria-label", label.clone());
    attrs.insert(cx, "title", label);

    Ok(view! {
        <button
            class="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
            (attrs)
        >
            <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
                class="size-4"
            >
                <path d="M3 6h18"></path>
                <path d="M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2"></path>
                <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"></path>
                <path d="M10 11v6"></path>
                <path d="M14 11v6"></path>
            </svg>
        </button>
    })
}

/// A toggle button that cannot leave edit mode while a required value is empty.
#[component]
pub(super) async fn edit_toggle_guarded_button(
    cx: &Cx,
    editing: Signal<bool>,
    #[into] field: String,
    value: Signal<String>,
) -> Result<impl View> {
    let edit_label = t!(form_edit_field, field = field.clone())
        .resolve(cx)
        .into_owned();
    let done_label = t!(form_done_editing, field = field)
        .resolve(cx)
        .into_owned();

    Ok(view! {
        <button
            type="button"
            class="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-foreground/5 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
            :aria-expanded=$(editing.get())
            :aria-label=$(if editing.get() {
                done_label.clone()
            } else {
                edit_label.clone()
            })
            :title=$(if editing.get() {
                done_label.clone()
            } else {
                edit_label.clone()
            })
            @click=$(|_e: Event| {
                if editing.get() {
                    if !value.get().trim().is_empty() {
                        editing.set(false)
                    }
                } else {
                    editing.set(true)
                }
            })
        >
            edit_toggle_icons(editing: editing.clone())
        </button>
    })
}

/// A text, textarea, or number field that is always visible.
#[component]
#[allow(clippy::too_many_arguments)]
pub(super) async fn form_field(
    cx: &Cx,
    #[into] name: String,
    #[into] label: String,
    #[into] initial: String,
    #[default]
    #[into]
    placeholder: String,
    #[default] required: bool,
    #[default] multiline: bool,
    #[default]
    #[into]
    input_type: String,
    #[default]
    #[into]
    min: String,
    #[default]
    #[into]
    step: String,
) -> Result<impl View> {
    let value = signal(cx, move || initial.clone());
    let input_id = element_id(cx);
    let input_type = if input_type.is_empty() {
        "text".to_string()
    } else {
        input_type
    };

    Ok(view! {
        <div class="flex flex-col gap-2">
            <label
                for=(&input_id)
                class="text-sm font-medium text-muted-foreground"
            >
                (label)
            </label>

            if multiline {
                <textarea
                    id=(&input_id)
                    name=(name)
                    rows="3"
                    class=(TEXTAREA_CLASS)
                    placeholder=(placeholder)
                    :required=$(required)
                    :value=$(value.get())
                    @input=$(|e: Event| value.set(e.target.value))
                ></textarea>
            } else {
                <input
                    id=(&input_id)
                    name=(name)
                    type=(input_type)
                    min=(min)
                    step=(step)
                    placeholder=(placeholder)
                    :required=$(required)
                    class="h-9 w-full min-w-0 rounded-lg border border-border bg-transparent px-3 text-sm transition-colors outline-none placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
                    :value=$(value.get())
                    @input=$(|e: Event| value.set(e.target.value))
                >
            }
        </div>
    })
}
