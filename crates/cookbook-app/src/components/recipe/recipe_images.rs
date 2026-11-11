use cookbook_domain::recipe::models::{RecipeImageId, RecipeImageMetadata};
use topcoat::{
    Result,
    context::Cx,
    router::href,
    view::{View, component, view},
};

use crate::{
    app::recipes::{ImageId, RecipeId, recipe_image},
    t,
};

struct ImageView {
    url: String,
    alt: String,
}

/// Displays the images attached directly to a recipe.
#[component]
pub async fn recipe_image_gallery(
    cx: &Cx,
    recipe_id: &str,
    image_ids: &[RecipeImageId],
    images: &[RecipeImageMetadata],
) -> Result<impl View> {
    let fallback_alt = t!(recipe_image_alt).resolve(cx).into_owned();
    let images_by_id: std::collections::HashMap<&str, &RecipeImageMetadata> = images
        .iter()
        .map(|image| (image.id().as_str(), image))
        .collect();
    let images = image_ids
        .iter()
        .filter_map(|image_id| images_by_id.get(image_id.as_str()).copied())
        .map(|image| ImageView {
            url: href!(
                recipe_image,
                RecipeId(recipe_id),
                ImageId(image.id().as_str())
            )
            .resolve(cx),
            alt: fallback_alt.clone(),
        })
        .collect::<Vec<_>>();

    Ok(view! {
        cx =>
        if !images.is_empty() {
            <section>
                <h3 class="mb-3 text-lg font-semibold">(t!(recipe_images_heading))</h3>
                <ul class="grid grid-cols-1 gap-3 sm:grid-cols-2">
                    for image in images {
                        <li>
                            <a
                                href=(image.url.clone())
                                target="_blank"
                                rel="noreferrer"
                            >
                                <img
                                    src=(image.url)
                                    alt=(image.alt)
                                    loading="lazy"
                                    class="w-full rounded-lg border border-border object-cover"
                                >
                            </a>
                        </li>
                    }
                </ul>
            </section>
        }
    })
}
