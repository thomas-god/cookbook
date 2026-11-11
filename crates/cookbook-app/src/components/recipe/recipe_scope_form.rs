use cookbook_domain::recipe::models::RecipeScope;
use topcoat::{
    Result,
    context::Cx,
    view::{View, attributes, component, element_id, view},
};

use crate::{
    components::base::{
        button::{ButtonSize, ButtonVariant, button},
        field::{field, field_description, field_label},
    },
    t,
};

const SELECT_CLASS: &str = "h-9 w-full rounded-lg border border-border bg-transparent px-3 text-sm transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background";

#[component]
pub async fn recipe_scope_form(
    cx: &Cx,
    recipe_id: String,
    scope: RecipeScope,
) -> Result<impl View> {
    let scope_id = element_id(cx);
    let action = format!("/recipes/{recipe_id}/scope");

    let options = [
        (
            RecipeScope::Private,
            t!(scope_private).resolve(cx).into_owned(),
        ),
        (
            RecipeScope::Viewable,
            t!(scope_viewable).resolve(cx).into_owned(),
        ),
        (
            RecipeScope::Editable,
            t!(scope_editable).resolve(cx).into_owned(),
        ),
    ];

    Ok(view! {
        <form method="post" action=(action) class="flex flex-col gap-4">
            field(
                field_label(attrs: attributes! { for=(&scope_id) }, (t!(scope_label)))
                <select id=(&scope_id) name="scope" class=(SELECT_CLASS)>
                    for (option, label) in options {
                        <option value=(option.as_str()) selected=(option == scope)>
                            (label)
                        </option>
                    }
                </select>
                field_description((t!(scope_hint)))
            )
            <div class="flex justify-end">
                button(
                    variant: ButtonVariant::Secondary,
                    size: ButtonSize::Sm,
                    attrs: attributes! { type="submit" },
                    (t!(scope_save))
                )
            </div>
        </form>
    })
}
