use std::collections::{HashMap, HashSet};

use cookbook_domain::recipe::models::{
    Quantity, RecipeBody, RecipeContent, RecipeImageMetadata, Unit,
};
use topcoat::{
    Result,
    context::Cx,
    router::href,
    view::{View, component, view},
};

use crate::{
    app::recipes::{ImageId, RecipeId, recipe_image},
    components::recipe::{image_thumbnail::image_thumbnail, recipe_images::recipe_image_gallery},
    t,
};

struct StepImageView {
    url: String,
    alt: String,
}

/// Localizes the unit label for a measured amount. Fluent selects the singular
/// or plural form from the locale's own plural rules.
fn unit_label(cx: &Cx, unit: &Unit, value: f32) -> String {
    match unit {
        Unit::Gram => t!(quantity_unit_gram, count = value)
            .resolve(cx)
            .into_owned(),
        Unit::Kilogram => t!(quantity_unit_kilogram, count = value)
            .resolve(cx)
            .into_owned(),
        Unit::MilliLitre => t!(quantity_unit_millilitre, count = value)
            .resolve(cx)
            .into_owned(),
        Unit::CentiLitre => t!(quantity_unit_centilitre, count = value)
            .resolve(cx)
            .into_owned(),
        Unit::Litre => t!(quantity_unit_litre, count = value)
            .resolve(cx)
            .into_owned(),
        // Pieces have no unit label; the ingredient name carries the noun.
        Unit::Piece => String::new(),
        Unit::Teaspoon => t!(quantity_unit_teaspoon, count = value)
            .resolve(cx)
            .into_owned(),
        Unit::Tablespoon => t!(quantity_unit_tablespoon, count = value)
            .resolve(cx)
            .into_owned(),
        // Free-text custom units are shown exactly as entered.
        Unit::Other(label) => label.clone(),
    }
}

pub(crate) fn quantity_label(cx: &Cx, quantity: &Quantity) -> String {
    match quantity {
        Quantity::Measured { value, unit } => {
            let label = unit_label(cx, unit, *value);

            if label.is_empty() {
                format!("{value}")
            } else {
                format!("{value} {label}")
            }
        }
        Quantity::Text(text) => text.clone(),
    }
}

/// Read-only rendering of a recipe's ingredients and steps.
#[component]
pub async fn recipe_view(
    cx: &Cx,
    recipe_id: &str,
    content: &RecipeContent,
    images: &[RecipeImageMetadata],
) -> Result<impl View> {
    Ok(view! {
        cx =>
        if let Some(body) = content.simple_body() {
            recipe_body_view(recipe_id: recipe_id, body: body, images: images)
        } else {
            for part in content.parts() {
                <section class="flex flex-col gap-4">
                    <div>
                        <h3 class="text-xl font-semibold">(part.title().as_str())</h3>
                        if let Some(description) = part.description() {
                            <p
                                class="mt-2 whitespace-pre-line text-sm text-muted-foreground"
                            >
                                (description.as_str())
                            </p>
                        }
                    </div>
                    recipe_image_gallery(
                        recipe_id: recipe_id,
                        image_ids: part.body().images(),
                        images: images
                    )
                    recipe_body_view(
                        recipe_id: recipe_id,
                        body: part.body(),
                        images: images
                    )
                </section>
            }
        }
    })
}

/// Read-only rendering of one recipe body's ingredients and steps.
#[component]
async fn recipe_body_view(
    cx: &Cx,
    recipe_id: &str,
    body: &RecipeBody,
    images: &[RecipeImageMetadata],
) -> Result<impl View> {
    let mut ingredients: Vec<_> = body
        .ingredients()
        .iter()
        .map(|ingredient| {
            (
                ingredient.name().as_str().to_string(),
                quantity_label(cx, ingredient.quantity()),
            )
        })
        .collect();
    ingredients.sort_by(|left, right| left.0.cmp(&right.0));

    let conflicts: HashSet<&str> = body
        .ingredient_conflicts()
        .into_iter()
        .map(|name| name.as_str())
        .collect();

    let fallback_alt = t!(recipe_image_alt).resolve(cx).into_owned();
    let images_by_id: HashMap<&str, &RecipeImageMetadata> = images
        .iter()
        .map(|image| (image.id().as_str(), image))
        .collect();
    let mut step_images: HashMap<String, Vec<StepImageView>> = HashMap::new();

    for step in body.steps() {
        let images = step
            .images()
            .iter()
            .filter_map(|image_id| images_by_id.get(image_id.as_str()).copied())
            .map(|image| StepImageView {
                url: href!(
                    recipe_image,
                    RecipeId(recipe_id),
                    ImageId(image.id().as_str())
                )
                .resolve(cx),
                alt: fallback_alt.clone(),
            })
            .collect::<Vec<_>>();

        if !images.is_empty() {
            step_images.insert(step.id().as_str().to_string(), images);
        }
    }

    Ok(view! {
        if !conflicts.is_empty() {
            <p
                role="status"
                class="rounded-lg border border-warning/40 bg-warning/10 px-4 py-3 text-sm text-warning-foreground"
            >
                (t!(recipe_ingredient_conflicts_notice))
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
                for (index, step) in body.steps().iter().enumerate() {
                    <li class="rounded-lg border border-border bg-card p-4 text-sm">
                        <h4 class="font-semibold">
                            (t!(recipe_step))
                            " "
                            (index + 1)
                            if let Some(title) = step.title() {
                                ": "
                                (title)
                            }
                        </h4>
                        <p class="mt-2 text-muted-foreground">(step.instruction())</p>
                        if let Some(images) = step_images.get(step.id().as_str()) {
                            <ul class="mt-3 grid grid-cols-2 gap-3">
                                for image in images {
                                    <li>
                                        image_thumbnail(
                                            url: image.url.clone(),
                                            alt: image.alt.clone()
                                        )
                                    </li>
                                }
                            </ul>
                        }
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
    fn localizes_word_like_units_with_singular_and_plural_forms() {
        let en = Cx::default().with(topcoat::l10n::locale!("en"));
        let fr = Cx::default().with(topcoat::l10n::locale!("fr"));

        assert_eq!(
            quantity_label(&en, &Quantity::from_measure(1.0, Unit::Teaspoon)),
            "1 Teaspoon"
        );
        assert_eq!(
            quantity_label(&en, &Quantity::from_measure(2.0, Unit::Teaspoon)),
            "2 Teaspoons"
        );
        assert_eq!(
            quantity_label(&fr, &Quantity::from_measure(1.0, Unit::Teaspoon)),
            "1 Cuillère à café"
        );
        assert_eq!(
            quantity_label(&fr, &Quantity::from_measure(2.0, Unit::Teaspoon)),
            "2 Cuillères à café"
        );
        // French plural rules treat zero as singular.
        assert_eq!(
            quantity_label(&fr, &Quantity::from_measure(0.0, Unit::Tablespoon)),
            "0 Cuillère à soupe"
        );
    }

    #[test]
    fn leaves_custom_units_exactly_as_entered() {
        let cx = Cx::default();
        let unit = Unit::Other("botte".to_string());

        assert_eq!(
            quantity_label(&cx, &Quantity::from_measure(1.0, unit.clone())),
            "1 botte"
        );
        assert_eq!(
            quantity_label(&cx, &Quantity::from_measure(23.0, unit)),
            "23 botte"
        );
    }

    #[test]
    fn displays_pieces_without_a_unit_label() {
        let cx = Cx::default();

        assert_eq!(
            quantity_label(&cx, &Quantity::from_measure(3.0, Unit::Piece)),
            "3"
        );
    }

    #[test]
    fn displays_metric_units_as_abbreviations_without_a_plural_suffix() {
        let cx = Cx::default().with(topcoat::l10n::locale!("en"));

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
}
