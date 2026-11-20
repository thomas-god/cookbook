use topcoat::{
    Result,
    context::Cx,
    router::href,
    view::{View, attributes, class, component, element_id, view},
};

use crate::{
    app::recipe::{import as import_recipe, new as new_recipe},
    components::base::button::{ButtonSize, ButtonVariant, button, button_variants},
    t,
};

/// Navbar entry point for starting a recipe from anywhere in the app.
///
/// The trigger is a link to the new recipe page, so it still works without
/// JavaScript. With JavaScript, it opens a native modal instead.
#[component]
pub async fn add_recipe_dialog(cx: &Cx, import_enabled: bool) -> Result<impl View> {
    let dialog_id = element_id(cx);
    let title_id = element_id(cx);

    Ok(view! {
        <div class="contents">
            <a
                href=(href!(new_recipe::page))
                class=(button_variants(ButtonVariant::Primary, ButtonSize::Sm))
                data-dialog-open=(&dialog_id)
                aria-haspopup="dialog"
                aria-controls=(&dialog_id)
            >
                <span aria-hidden="true">("+")</span>
                (t!(recipe))
            </a>
            <dialog
                id=(&dialog_id)
                aria-labelledby=(&title_id)
                class="m-auto w-[calc(100%-2rem)] max-w-md rounded-xl border border-border bg-card p-0 text-card-foreground shadow-sm backdrop:bg-black/50"
            >
                <div class="flex flex-col gap-5 p-6">
                    <div class="flex flex-col gap-1.5">
                        <h2 id=(&title_id) class="text-lg leading-none font-semibold">
                            (t!(form_add_recipe))
                        </h2>
                    </div>
                    <div class="flex flex-col gap-2">
                        <a
                            href=(href!(new_recipe::page))
                            class=(class!(
                                "flex flex-col gap-0.5 rounded-lg border border-border px-4 py-3",
                                "outline-none transition-colors hover:bg-foreground/5",
                                "focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
                                "focus-visible:ring-offset-background",
                            ))
                        >
                            <span class="text-sm font-medium">(t!(new_recipe))</span>
                            <span class="text-xs text-muted-foreground">
                                (t!(add_recipe_new_hint))
                            </span>
                        </a>
                        if import_enabled {
                            <a
                                href=(href!(import_recipe::page))
                                class=(class!(
                                    "flex flex-col gap-0.5 rounded-lg border border-border px-4 py-3",
                                    "outline-none transition-colors hover:bg-foreground/5",
                                    "focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
                                    "focus-visible:ring-offset-background",
                                ))
                            >
                                <span class="text-sm font-medium">
                                    (t!(add_recipe_import))
                                </span>
                                <span class="text-xs text-muted-foreground">
                                    (t!(add_recipe_import_hint))
                                </span>
                            </a>
                        }
                    </div>
                    <form method="dialog" class="flex justify-end">
                        button(
                            variant: ButtonVariant::Ghost,
                            size: ButtonSize::Sm,
                            attrs: attributes! { type="submit" },
                            (t!(form_cancel))
                        )
                    </form>
                </div>
            </dialog>
        </div>
    })
}
