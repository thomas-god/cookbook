use cookbook_domain::recipe::models::{RecipeContent, Unit};
use topcoat::{
    Result,
    context::Cx,
    view::{View, component, view},
};

use crate::t;

/// Read-only rendering of a recipe's ingredients and steps.
#[component]
pub async fn recipe_view(cx: &Cx, content: &RecipeContent) -> Result<impl View> {
    let mut ingredients: Vec<_> = content
        .ingredients()
        .iter()
        .map(|(name, quantity)| {
            (
                name.as_str().to_string(),
                quantity.value(),
                quantity.unit().clone(),
            )
        })
        .collect();
    ingredients.sort_by(|left, right| left.0.cmp(&right.0));

    Ok(view! {
        cx =>
        <section>
            <h3 class="mb-3 text-lg font-semibold">(t!(recipe_ingredients))</h3>
            <ul class="flex flex-col gap-2">
                for (name, quantity, unit) in ingredients {
                    <li
                        class="flex items-center justify-between rounded-lg border border-border bg-card px-4 py-3 text-sm"
                    >
                        <span>(name)</span>
                        <span class="text-muted-foreground">
                            (quantity)
                            " "
                            (match unit {
                                Unit::Gram => t!(unit_gram),
                                Unit::MilliLitre => t!(unit_millilitre),
                                Unit::Litre => t!(unit_litre),
                            })
                        </span>
                    </li>
                }
            </ul>
        </section>

        <section>
            <h3 class="mb-3 text-lg font-semibold">(t!(recipe_steps))</h3>
            <ol class="flex flex-col gap-4">
                for (index, step) in content.steps().iter().enumerate() {
                    <li class="rounded-lg border border-border bg-card p-4 text-sm">
                        <h4 class="font-semibold">
                            if let Some(title) = step.title() {
                                (title)
                            } else {
                                (t!(recipe_step))
                                " "
                                (index + 1)
                            }
                        </h4>
                        <p class="mt-2 text-muted-foreground">(step.instruction())</p>
                    </li>
                }
            </ol>
        </section>
    })
}
