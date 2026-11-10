use cookbook_domain::recipe::ports::RecipeService;
use cookbook_vision::{SourcePhoto, VisionClient, VisionError};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::multipart::Multipart,
        error::{SeeOther, bad_request, not_found, see_other},
        href, page, route,
    },
    view::{View, attributes, element_id, view},
};

use crate::{
    app::{
        AppRecipeService, auth,
        recipes::{RecipeId, edit_recipe},
        vision::VisionSettings,
    },
    components::base::{
        button::button,
        card::{card, card_content, card_header, card_title},
        field::{field, field_description, field_label},
        image_upload::image_upload,
    },
    t,
};

pub const MAX_UPLOAD_BYTES: usize = 100 * 1024 * 1024;

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    auth::require_user(cx)?;
    require_enabled(cx)?;

    let images_id = element_id(cx);

    Ok(view! {
        card(
            card_header(card_title((t!(recipe_import_heading))))
            card_content(
                <form
                    method="post"
                    action="/recipe/import"
                    enctype="multipart/form-data"
                    class="flex flex-col gap-5"
                >
                    field(
                        field_label(
                            attrs: attributes! { for=(&images_id) },
                            (t!(recipe_import_images))
                        )
                        image_upload(
                            multiple: true,
                            attrs: attributes! {
                                id=(&images_id)
                                name="images"
                                required=""
                            }
                        )
                        field_description((t!(recipe_import_hint)))
                    )
                    <div class="flex justify-end">
                        button(
                            attrs: attributes! { type="submit" },
                            (t!(recipe_import_submit))
                        )
                    </div>
                </form>
            )
        )
    })
}

#[route(POST)]
pub async fn create(cx: &Cx, mut multipart: Multipart) -> Result<SeeOther> {
    let user = auth::require_user(cx)?;
    require_enabled(cx)?;

    let photos = read_photos(&mut multipart).await?;

    if photos.is_empty() {
        return Err(bad_request(t!(recipe_import_empty).resolve(cx)).into());
    }

    let client: &VisionClient = app_context(cx);
    let vision_recipe = match client.parse_recipe(photos).await {
        Ok(recipe) => recipe,
        Err(VisionError::UnsupportedImage { .. } | VisionError::ImageProcessing { .. }) => {
            return Err(bad_request(t!(recipe_import_unsupported).resolve(cx)).into());
        }
        Err(VisionError::NoImages | VisionError::EmptyImage { .. }) => {
            return Err(bad_request(t!(recipe_import_empty).resolve(cx)).into());
        }
        Err(_) => {
            return Err(bad_request(t!(recipe_import_failed).resolve(cx)).into());
        }
    };

    let request = vision_recipe.into_create_request(user.id().clone());
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = recipe_service.create_recipe(request).await?;

    Ok(see_other(
        href!(edit_recipe, RecipeId(recipe.id().as_str())).resolve(cx),
    ))
}

async fn read_photos(multipart: &mut Multipart) -> Result<Vec<SourcePhoto>> {
    let mut photos = Vec::new();

    while let Some(part) = multipart.next_field().await? {
        if part.name() != Some("images") {
            continue;
        }

        let original_name = part.file_name().map(str::to_owned);
        let bytes = part.bytes().await?;
        photos.push(SourcePhoto::new(original_name, bytes.to_vec()));
    }

    Ok(photos)
}

fn require_enabled(cx: &Cx) -> Result<()> {
    let settings: &VisionSettings = app_context(cx);

    if settings.enabled() {
        Ok(())
    } else {
        Err(not_found().into())
    }
}
