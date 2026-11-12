use std::collections::HashMap;

use cookbook_domain::recipe::models::{RecipeBody, RecipeContent, RecipeImageMetadata, RecipeStep};
use topcoat::{
    Result,
    context::Cx,
    router::href,
    runtime::{Event, signal},
    view::{View, attributes, class, component, view},
};

use crate::{
    app::recipes::{ImageId, RecipeId, recipe_detail, recipe_image},
    components::base::button::{ButtonSize, ButtonVariant, button, button_variants},
    components::recipe::recipe_view::quantity_label,
    t,
};

struct CookIngredient {
    name: String,
    quantity: String,
}

struct CookIngredientGroup {
    title: Option<String>,
    ingredients: Vec<CookIngredient>,
}

struct CookStepImage {
    url: String,
    alt: String,
}

struct CookStep {
    part_title: Option<String>,
    title: Option<String>,
    instruction: String,
    images: Vec<CookStepImage>,
}

fn ingredient_group(cx: &Cx, title: Option<String>, body: &RecipeBody) -> CookIngredientGroup {
    let mut ingredients = body
        .ingredients()
        .iter()
        .map(|ingredient| CookIngredient {
            name: ingredient.name().as_str().to_string(),
            quantity: quantity_label(cx, ingredient.quantity()),
        })
        .collect::<Vec<_>>();

    ingredients.sort_by(|left, right| left.name.cmp(&right.name));

    CookIngredientGroup { title, ingredients }
}

fn cook_step(
    cx: &Cx,
    recipe_id: &str,
    part_title: Option<String>,
    step: &RecipeStep,
    images_by_id: &HashMap<&str, &RecipeImageMetadata>,
    fallback_alt: &str,
) -> CookStep {
    let images = step
        .images()
        .iter()
        .filter_map(|image_id| images_by_id.get(image_id.as_str()).copied())
        .map(|image| CookStepImage {
            url: href!(
                recipe_image,
                RecipeId(recipe_id),
                ImageId(image.id().as_str())
            )
            .resolve(cx),
            alt: fallback_alt.to_string(),
        })
        .collect();

    CookStep {
        part_title,
        title: step.title().map(str::to_string),
        instruction: step.instruction().to_string(),
        images,
    }
}

/// Full-screen cooking assistant: an ingredient checklist followed by one
/// large step at a time, navigated with big previous/next buttons.
#[component]
pub async fn cook_recipe_view(
    cx: &Cx,
    recipe_id: &str,
    content: &RecipeContent,
    images: &[RecipeImageMetadata],
) -> Result<impl View> {
    let showing_steps = signal(cx, || false);
    let ingredients_open = signal(cx, || false);
    let step_index = signal(cx, || 0usize);
    let text_size = signal(cx, || 1usize);

    let fallback_alt = t!(recipe_image_alt).resolve(cx).into_owned();
    let images_by_id: HashMap<&str, &RecipeImageMetadata> = images
        .iter()
        .map(|image| (image.id().as_str(), image))
        .collect();

    let mut ingredient_groups = Vec::new();
    let mut steps = Vec::new();

    if let Some(body) = content.simple_body() {
        ingredient_groups.push(ingredient_group(cx, None, body));
        steps.extend(
            body.steps()
                .iter()
                .map(|step| cook_step(cx, recipe_id, None, step, &images_by_id, &fallback_alt)),
        );
    } else {
        for part in content.parts() {
            let part_title = part.title().as_str().to_string();
            ingredient_groups.push(ingredient_group(cx, Some(part_title.clone()), part.body()));
            steps.extend(part.body().steps().iter().map(|step| {
                cook_step(
                    cx,
                    recipe_id,
                    Some(part_title.clone()),
                    step,
                    &images_by_id,
                    &fallback_alt,
                )
            }));
        }
    }

    let total_steps = steps.len();
    let recipe_href = href!(recipe_detail, RecipeId(recipe_id)).resolve(cx);

    Ok(view! {
        <div class="fixed inset-0 z-50 flex flex-col bg-background text-foreground">
            <header class="flex items-center gap-4 border-b border-border px-4 py-3">
                <a
                    class="shrink-0 text-sm text-muted-foreground underline-offset-4 hover:underline"
                    href=(recipe_href.clone())
                >
                    "← "
                    (t!(cook_exit))
                </a>
                <h1 class="min-w-0 flex-1 truncate text-lg font-semibold">
                    (content.title().as_str())
                </h1>
                <div
                    :hidden=$(!showing_steps.get())
                    class="flex shrink-0 items-center gap-1"
                >
                    button(
                        variant: ButtonVariant::Ghost,
                        size: ButtonSize::Sm,
                        attrs: attributes! {
                            type="button"
                            aria-label=(t!(cook_text_smaller))
                            title=(t!(cook_text_smaller))
                            :disabled=$(text_size.get() == 0)
                            @click=$(|_e: Event| {
                                if text_size.get() > 0 {
                                    text_size.decrement()
                                }
                            })
                        },
                        ("A−")
                    )
                    button(
                        variant: ButtonVariant::Ghost,
                        size: ButtonSize::Sm,
                        attrs: attributes! {
                            type="button"
                            aria-label=(t!(cook_text_larger))
                            title=(t!(cook_text_larger))
                            :disabled=$(text_size.get() == 3)
                            @click=$(|_e: Event| {
                                if text_size.get() < 3 {
                                    text_size.increment()
                                }
                            })
                        },
                        ("A+")
                    )
                </div>
            </header>

            <div
                :hidden=$(if showing_steps.get() {
                    !ingredients_open.get()
                } else {
                    true
                })
                aria-hidden="true"
                class="absolute inset-0 z-20 bg-foreground/25"
                @click=$(|_e: Event| ingredients_open.set(false))
            ></div>

            <section
                id="cook-ingredients"
                :class=$(if !showing_steps.get() {
                    "flex min-h-0 flex-1 flex-col"
                } else if ingredients_open.get() {
                    "absolute inset-x-0 bottom-0 z-30 flex max-h-[80dvh] flex-col rounded-t-xl border-t border-border bg-background shadow-sm lg:inset-y-0 lg:left-auto lg:right-0 lg:max-h-none lg:w-96 lg:rounded-none lg:border-l lg:border-t-0"
                } else {
                    "hidden"
                })
            >
                <div
                    class="mx-auto flex w-full max-w-3xl min-h-0 flex-1 flex-col px-4 py-5"
                >
                    <div class="flex items-start justify-between gap-4">
                        <div>
                            <h2 class="text-2xl font-semibold">
                                (t!(recipe_ingredients))
                            </h2>
                        </div>
                        button(
                            variant: ButtonVariant::Ghost,
                            size: ButtonSize::Sm,
                            attrs: attributes! {
                                type="button"
                                class="-mr-1 -mt-1 shrink-0"
                                aria-label=(t!(cook_close_ingredients))
                                :hidden=$(if showing_steps.get() {
                                    !ingredients_open.get()
                                } else {
                                    true
                                })
                                @click=$(|_e: Event| ingredients_open.set(false))
                            },
                            ("✕")
                        )
                    </div>
                    <div class="mt-4 min-h-0 flex-1 overflow-y-auto pr-1">
                        for group in &ingredient_groups {
                            <section class="mb-6 last:mb-0">
                                if let Some(title) = &group.title {
                                    <h3
                                        class="mb-2 text-sm font-medium uppercase tracking-wide text-muted-foreground"
                                    >
                                        (title)
                                    </h3>
                                }
                                if group.ingredients.is_empty() {
                                    <p class="text-sm text-muted-foreground">
                                        (t!(cook_no_ingredients))
                                    </p>
                                } else {
                                    <ul class="flex flex-col gap-2">
                                        for ingredient in &group.ingredients {
                                            <li>
                                                <label
                                                    class="flex cursor-pointer items-center gap-3 rounded-lg border border-border bg-card px-4 py-3"
                                                >
                                                    <input
                                                        type="checkbox"
                                                        class="peer size-5 shrink-0 accent-primary"
                                                    >
                                                    <span
                                                        class="min-w-0 flex-1 text-base peer-checked:text-muted-foreground peer-checked:line-through"
                                                    >
                                                        (ingredient.name.as_str())
                                                    </span>
                                                    <span
                                                        class="shrink-0 text-sm text-muted-foreground peer-checked:line-through"
                                                    >
                                                        (ingredient.quantity.as_str())
                                                    </span>
                                                </label>
                                            </li>
                                        }
                                    </ul>
                                }
                            </section>
                        }
                    </div>
                </div>
                <footer
                    :hidden=$(showing_steps.get())
                    class="border-t border-border p-4"
                >
                    <div class="mx-auto w-full max-w-3xl">
                        button(
                            size: ButtonSize::Lg,
                            attrs: attributes! {
                                type="button"
                                class="min-h-14 w-full"
                                :disabled=$({
                                    total_steps == 0
                                })
                                @click=$(|_e: Event| showing_steps.set(true))
                            },
                            (t!(cook_start))
                        )
                    </div>
                </footer>
            </section>

            <section
                :hidden=$(!showing_steps.get())
                class="flex min-h-0 flex-1 flex-col"
            >
                if total_steps == 0 {
                    <div class="mx-auto flex w-full max-w-3xl flex-1 items-center px-4">
                        <p class="text-lg text-muted-foreground">(t!(cook_no_steps))</p>
                    </div>
                    <footer class="border-t border-border p-4">
                        <div class="mx-auto w-full max-w-3xl">
                            button(
                                variant: ButtonVariant::Secondary,
                                size: ButtonSize::Lg,
                                attrs: attributes! {
                                    type="button"
                                    class="min-h-14 w-full"
                                    @click=$(|_e: Event| showing_steps.set(false))
                                },
                                (t!(cook_back_to_ingredients))
                            )
                        </div>
                    </footer>
                } else {
                    <div class="min-h-0 flex-1 overflow-y-auto">
                        <div class="mx-auto flex w-full max-w-3xl flex-col px-4 py-6">
                            for (index, step) in steps.iter().enumerate() {
                                <article
                                    :hidden=$(step_index.get() != index)
                                    class="flex flex-col gap-5"
                                >
                                    if let Some(part_title) = &step.part_title {
                                        <p
                                            class="text-xs font-medium uppercase tracking-wide text-muted-foreground"
                                        >
                                            (part_title)
                                        </p>
                                    }
                                    <div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
                                        <span class="text-sm text-muted-foreground">
                                            (t!(recipe_step))
                                            " "
                                            $(step_index.get() + 1)
                                            " / "
                                            (total_steps)
                                        </span>
                                        if let Some(title) = &step.title {
                                            <h2
                                                :class=$(if text_size.get() == 0 {
                                                    "text-xl font-semibold sm:text-2xl"
                                                } else if text_size.get() == 1 {
                                                    "text-2xl font-semibold sm:text-3xl"
                                                } else if text_size.get() == 2 {
                                                    "text-3xl font-semibold sm:text-4xl"
                                                } else {
                                                    "text-4xl font-semibold sm:text-5xl"
                                                })
                                            >
                                                (title)
                                            </h2>
                                        }
                                    </div>
                                    if !step.images.is_empty() {
                                        <ul class="grid grid-cols-1 gap-3 sm:grid-cols-2">
                                            for image in &step.images {
                                                <li>
                                                    <a
                                                        href=(image.url.clone())
                                                        target="_blank"
                                                        rel="noreferrer"
                                                    >
                                                        <img
                                                            src=(image.url.as_str())
                                                            alt=(image.alt.as_str())
                                                            loading="lazy"
                                                            class="max-h-[45vh] w-full rounded-lg border border-border bg-card object-contain"
                                                        >
                                                    </a>
                                                </li>
                                            }
                                        </ul>
                                    }
                                    <p
                                        :class=$(if text_size.get() == 0 {
                                            "whitespace-pre-line text-lg leading-relaxed sm:text-xl"
                                        } else if text_size.get() == 1 {
                                            "whitespace-pre-line text-xl leading-relaxed sm:text-2xl"
                                        } else if text_size.get() == 2 {
                                            "whitespace-pre-line text-2xl leading-relaxed sm:text-3xl"
                                        } else {
                                            "whitespace-pre-line text-3xl leading-relaxed sm:text-4xl"
                                        })
                                    >
                                        (step.instruction.as_str())
                                    </p>
                                </article>
                            }
                        </div>
                    </div>
                    <footer class="border-t border-border p-4">
                        <div class="mx-auto flex w-full max-w-3xl items-stretch gap-2">
                            button(
                                variant: ButtonVariant::Secondary,
                                size: ButtonSize::Lg,
                                attrs: attributes! {
                                    type="button"
                                    class="min-h-16"
                                    aria-controls="cook-ingredients"
                                    :aria-expanded=$(ingredients_open.get())
                                    @click=$(|_e: Event| ingredients_open.set(true))
                                },
                                (t!(cook_back_to_ingredients))
                            )
                            button(
                                variant: ButtonVariant::Outline,
                                size: ButtonSize::Lg,
                                attrs: attributes! {
                                    type="button"
                                    class="min-h-16 flex-1"
                                    :disabled=$(step_index.get() == 0)
                                    @click=$(|_e: Event| {
                                        if step_index.get() > 0 {
                                            step_index.decrement()
                                        }
                                    })
                                },
                                ("← ")
                                (t!(cook_previous))
                            )
                            <div class="flex min-h-16 flex-1">
                                button(
                                    size: ButtonSize::Lg,
                                    attrs: attributes! {
                                        type="button"
                                        class="min-h-16 w-full"
                                        :hidden=$(step_index.get() + 1 == total_steps)
                                        @click=$(|_e: Event| step_index.set(step_index.get() + 1))
                                    },
                                    (t!(cook_next))
                                    " →"
                                )
                                <a
                                    class=(class!(
                                        button_variants(ButtonVariant::Primary, ButtonSize::Lg),
                                        "min-h-16 w-full",
                                    ))
                                    :hidden=$(step_index.get() + 1 != total_steps)
                                    href=(recipe_href)
                                >
                                    (t!(cook_finish))
                                </a>
                            </div>
                        </div>
                    </footer>
                }
            </section>
        </div>
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cookbook_domain::recipe::models::{
        Ingredient, IngredientName, Quantity, RecipeDurations, RecipeImageId, RecipePart,
        RecipePartId, RecipePartTitle, RecipeStepId, Unit,
    };

    fn ingredient(name: &str, quantity: Quantity) -> Ingredient {
        Ingredient::new(IngredientName::new(name.to_string()), quantity)
    }

    fn body(ingredients: Vec<Ingredient>, steps: Vec<RecipeStep>) -> RecipeBody {
        RecipeBody::new(
            RecipeDurations::new(None, None, None),
            ingredients,
            Vec::new(),
            steps,
        )
    }

    fn step(instruction: &str) -> RecipeStep {
        RecipeStep::new(
            RecipeStepId::generate(),
            None,
            instruction.to_string(),
            Vec::new(),
        )
    }

    #[test]
    fn sorts_ingredients_by_name_within_a_body() {
        let cx = Cx::default();
        let body = body(
            vec![
                ingredient("salt", Quantity::from_text("to taste")),
                ingredient("flour", Quantity::from_measure(200.0, Unit::Gram)),
            ],
            Vec::new(),
        );

        let group = ingredient_group(&cx, None, &body);
        let names = group
            .ingredients
            .iter()
            .map(|ingredient| ingredient.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["flour", "salt"]);
    }

    #[test]
    fn flattens_multipart_bodies_and_keeps_part_titles_on_steps() {
        let cx = Cx::default();
        let content = RecipeContent::new_multipart(
            cookbook_domain::recipe::models::RecipeTitle::new("Cake".to_string()),
            None,
            None,
            None,
            vec![
                RecipePart::new(
                    RecipePartId::generate(),
                    RecipePartTitle::new("Sponge".to_string()),
                    None,
                    body(Vec::new(), vec![step("Whisk.")]),
                ),
                RecipePart::new(
                    RecipePartId::generate(),
                    RecipePartTitle::new("Filling".to_string()),
                    None,
                    body(Vec::new(), vec![step("Whip.")]),
                ),
            ],
        )
        .unwrap();

        let mut steps = Vec::new();
        for part in content.parts() {
            let part_title = part.title().as_str().to_string();
            steps.extend(part.body().steps().iter().map(|step| {
                cook_step(
                    &cx,
                    "cake",
                    Some(part_title.clone()),
                    step,
                    &HashMap::new(),
                    "",
                )
            }));
        }

        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].part_title.as_deref(), Some("Sponge"));
        assert_eq!(steps[0].instruction, "Whisk.");
        assert_eq!(steps[1].part_title.as_deref(), Some("Filling"));
        assert_eq!(steps[1].instruction, "Whip.");
    }

    #[test]
    fn ignores_images_that_are_no_longer_stored() {
        let cx = Cx::default();
        let step = RecipeStep::new(
            RecipeStepId::generate(),
            None,
            "Stir.".to_string(),
            vec![RecipeImageId::new("missing".to_string())],
        );

        let cooked = cook_step(&cx, "soup", None, &step, &HashMap::new(), "Recipe photo");

        assert!(cooked.images.is_empty());
    }
}
