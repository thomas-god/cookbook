use cookbook_domain::recipe::models::RecipeSourceImageSummary;
use topcoat::{
    Result,
    context::Cx,
    router::href,
    view::{View, attributes, component, element_id, view},
};

use crate::{
    app::recipes::{
        RecipeId, SourceImageId, delete_source_image, source_image, upload_source_images,
    },
    components::base::{
        button::{ButtonSize, ButtonVariant, button},
        field::{field, field_description, field_label},
        image_upload::image_upload,
    },
    t,
};

struct GalleryImage {
    url: String,
    alt: String,
}

struct EditableImage {
    url: String,
    delete_url: String,
    alt: String,
}

#[component]
pub async fn source_image_gallery(
    cx: &Cx,
    recipe_id: &str,
    images: &[RecipeSourceImageSummary],
) -> Result<impl View> {
    let fallback_alt = t!(source_image_alt).resolve(cx).into_owned();
    let images = images
        .iter()
        .map(|image| {
            let url = href!(
                source_image,
                RecipeId(recipe_id),
                SourceImageId(image.id().as_str())
            )
            .resolve(cx);

            GalleryImage {
                url,
                alt: image
                    .original_name()
                    .map(str::to_string)
                    .unwrap_or_else(|| fallback_alt.clone()),
            }
        })
        .collect::<Vec<_>>();

    Ok(view! {
        cx =>
        <section>
            <h3 class="mb-3 text-lg font-semibold">(t!(source_images_heading))</h3>
            if images.is_empty() {
                <p class="text-sm text-muted-foreground">(t!(source_images_empty))</p>
            } else {
                <ul class="grid grid-cols-2 gap-3 sm:grid-cols-3">
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
                                    class="aspect-square w-full rounded-lg border border-border object-cover"
                                >
                            </a>
                        </li>
                    }
                </ul>
            }
        </section>
    })
}

#[component]
pub async fn source_image_editor(
    cx: &Cx,
    recipe_id: &str,
    images: &[RecipeSourceImageSummary],
) -> Result<impl View> {
    let images_id = element_id(cx);
    let fallback_alt = t!(source_image_alt).resolve(cx).into_owned();
    let images = images
        .iter()
        .map(|image| {
            let url = href!(
                source_image,
                RecipeId(recipe_id),
                SourceImageId(image.id().as_str())
            )
            .resolve(cx);
            let delete_url = href!(
                delete_source_image,
                RecipeId(recipe_id),
                SourceImageId(image.id().as_str())
            )
            .resolve(cx);

            EditableImage {
                url,
                delete_url,
                alt: image
                    .original_name()
                    .map(str::to_string)
                    .unwrap_or_else(|| fallback_alt.clone()),
            }
        })
        .collect::<Vec<_>>();

    Ok(view! {
        cx =>
        <div class="flex flex-col gap-6">
            if images.is_empty() {
                <p class="text-sm text-muted-foreground">(t!(source_images_empty))</p>
            } else {
                <ul class="grid grid-cols-2 gap-3 sm:grid-cols-3">
                    for image in images {
                        <li class="flex flex-col gap-2">
                            <a
                                href=(image.url.clone())
                                target="_blank"
                                rel="noreferrer"
                            >
                                <img
                                    src=(image.url)
                                    alt=(image.alt)
                                    loading="lazy"
                                    class="aspect-square w-full rounded-lg border border-border object-cover"
                                >
                            </a>
                            <form method="post" action=(image.delete_url)>
                                button(
                                    variant: ButtonVariant::Ghost,
                                    size: ButtonSize::Sm,
                                    attrs: attributes! { type="submit" },
                                    (t!(source_images_delete))
                                )
                            </form>
                        </li>
                    }
                </ul>
            }

            <form
                method="post"
                action=(href!(upload_source_images, RecipeId(recipe_id)))
                enctype="multipart/form-data"
                class="flex flex-col gap-4"
            >
                field(
                    field_label(
                        attrs: attributes! { for=(&images_id) },
                        (t!(source_images_upload))
                    )
                    image_upload(
                        multiple: true,
                        attrs: attributes! { id=(&images_id) name="images" required="" }
                    )
                    field_description((t!(source_images_upload_hint)))
                )
                <div class="flex justify-end">
                    button(
                        variant: ButtonVariant::Outline,
                        size: ButtonSize::Sm,
                        attrs: attributes! { type="submit" },
                        (t!(source_images_upload_submit))
                    )
                </div>
            </form>
        </div>
    })
}
