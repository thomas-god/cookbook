use cookbook_domain::recipe::models::{Quantity, RecipeContent, Servings, Unit};
use topcoat::{
    Result,
    context::Cx,
    view::{View, component, view},
};

use crate::t;

/// A language-neutral optional plural marker. We do not guess actual plural
/// forms, so any amount other than exactly one gets the same `(s)` suffix.
const OPTIONAL_PLURAL_SUFFIX: &str = "(s)";

fn unit_label(cx: &Cx, unit: &Unit) -> String {
    match unit {
        // Metric units are displayed with their symbol and never take a plural.
        Unit::Gram => "g".to_string(),
        Unit::Kilogram => "kg".to_string(),
        Unit::MilliLitre => "mL".to_string(),
        Unit::CentiLitre => "cL".to_string(),
        Unit::Litre => "L".to_string(),
        // Word-like units are localized and can carry the optional plural marker.
        Unit::Piece => t!(unit_piece).resolve(cx).into_owned(),
        Unit::Teaspoon => t!(unit_teaspoon).resolve(cx).into_owned(),
        Unit::Tablespoon => t!(unit_tablespoon).resolve(cx).into_owned(),
        Unit::Other(label) => label.clone(),
    }
}

fn unit_takes_optional_plural(unit: &Unit) -> bool {
    matches!(
        unit,
        Unit::Piece | Unit::Teaspoon | Unit::Tablespoon | Unit::Other(_)
    )
}

fn quantity_label(cx: &Cx, quantity: &Quantity) -> String {
    match quantity {
        Quantity::Measured { value, unit } => {
            let suffix = if *value != 1.0 && unit_takes_optional_plural(unit) {
                OPTIONAL_PLURAL_SUFFIX
            } else {
                ""
            };

            format!("{value} {}{suffix}", unit_label(cx, unit))
        }
        Quantity::Text(text) => text.clone(),
    }
}

fn servings_label(cx: &Cx, servings: &Servings) -> String {
    match servings {
        Servings::People(count) if *count == 1 => t!(serving_person, count = count.to_string())
            .resolve(cx)
            .into_owned(),
        Servings::People(count) => t!(serving_people, count = count.to_string())
            .resolve(cx)
            .into_owned(),
        Servings::Portions {
            count,
            label: Some(label),
        } => format!("{count} {label}"),
        Servings::Portions { count, label: None } => count.to_string(),
    }
}

/// Read-only rendering of a recipe's ingredients and steps.
#[component]
pub async fn recipe_view(cx: &Cx, content: &RecipeContent) -> Result<impl View> {
    let mut ingredients: Vec<_> = content
        .ingredients()
        .iter()
        .map(|(name, quantity)| (name.as_str().to_string(), quantity_label(cx, quantity)))
        .collect();
    ingredients.sort_by(|left, right| left.0.cmp(&right.0));

    Ok(view! {
        cx =>
        if let Some(servings) = content.servings() {
            <p class="text-sm text-muted-foreground">
                (t!(recipe_servings))
                ": "
                (servings_label(cx, servings))
            </p>
        }
        <section>
            <h3 class="mb-3 text-lg font-semibold">(t!(recipe_ingredients))</h3>
            <ul class="flex flex-col gap-2">
                for (name, quantity) in ingredients {
                    <li
                        class="flex items-center justify-between rounded-lg border border-border bg-card px-4 py-3 text-sm"
                    >
                        <span>(name)</span>
                        <span class="text-muted-foreground">(quantity)</span>
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_the_optional_plural_suffix_to_word_like_units() {
        let cx = Cx::default();
        let unit = Unit::Other("botte".to_string());

        assert_eq!(
            quantity_label(&cx, &Quantity::from_measure(1.0, unit.clone())),
            "1 botte"
        );
        assert_eq!(
            quantity_label(&cx, &Quantity::from_measure(23.0, unit)),
            "23 botte(s)"
        );
    }

    #[test]
    fn displays_metric_units_as_abbreviations_without_a_plural_suffix() {
        let cx = Cx::default();

        assert_eq!(
            quantity_label(&cx, &Quantity::from_measure(1.0, Unit::Gram)),
            "1 g"
        );
        assert_eq!(
            quantity_label(&cx, &Quantity::from_measure(200.0, Unit::Gram)),
            "200 g"
        );
        assert_eq!(
            quantity_label(&cx, &Quantity::from_measure(50.0, Unit::MilliLitre)),
            "50 mL"
        );
        assert_eq!(
            quantity_label(&cx, &Quantity::from_measure(2.0, Unit::Litre)),
            "2 L"
        );
    }

    #[test]
    fn leaves_free_text_amounts_unchanged() {
        let cx = Cx::default();

        assert_eq!(quantity_label(&cx, &Quantity::from_text("libre")), "libre");
    }

    #[test]
    fn localizes_people_servings_with_singular_and_plural_forms() {
        let cx = Cx::default().with(topcoat::l10n::locale!("en"));

        // Fluent wraps interpolated values in bidi isolation marks.
        let label = |servings| {
            servings_label(&cx, &servings)
                .replace('\u{2068}', "")
                .replace('\u{2069}', "")
        };

        assert_eq!(label(Servings::people(1)), "1 person");
        assert_eq!(label(Servings::people(4)), "4 people");
    }

    #[test]
    fn displays_portion_servings_with_their_free_form_label() {
        let cx = Cx::default();

        assert_eq!(
            servings_label(&cx, &Servings::portions_with_label(12, "cookies")),
            "12 cookies"
        );
        assert_eq!(servings_label(&cx, &Servings::portions(12)), "12");
    }
}
