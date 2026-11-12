use cookbook_domain::recipe::ports::RecipeService;
use cookbook_vision::{SourcePhoto, VisionClient, VisionError};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        StatusCode,
        content::multipart::Multipart,
        error::{not_found, see_other},
        href, page,
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
        field::{field, field_description, field_error, field_label},
        image_upload::image_upload,
    },
    t,
};

pub const MAX_UPLOAD_BYTES: usize = 100 * 1024 * 1024;

/// A user-facing reason why an import did not produce a recipe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ImportError {
    Empty,
    Unsupported,
    Failed,
    SaveFailed,
}

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    auth::require_user(cx)?;
    require_enabled(cx)?;

    import_page(cx, None)
}

#[page(POST)]
pub async fn create(cx: &Cx, mut multipart: Multipart) -> Result<impl View> {
    let user = auth::require_user(cx)?;
    require_enabled(cx)?;

    let photos = match read_photos(&mut multipart).await {
        Ok(photos) => photos,
        Err(_) => return import_page(cx, Some(ImportError::Failed)),
    };

    if photos.is_empty() {
        return import_page(cx, Some(ImportError::Empty));
    }

    let client: &VisionClient = app_context(cx);
    let vision_recipe = match client.parse_recipe(photos).await {
        Ok(recipe) => recipe,
        Err(VisionError::UnsupportedImage { .. } | VisionError::ImageProcessing { .. }) => {
            return import_page(cx, Some(ImportError::Unsupported));
        }
        Err(VisionError::NoImages | VisionError::EmptyImage { .. }) => {
            return import_page(cx, Some(ImportError::Empty));
        }
        Err(_) => return import_page(cx, Some(ImportError::Failed)),
    };

    let request = vision_recipe.into_create_request(user.id().clone());
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = match recipe_service.create_imported_recipe(request).await {
        Ok(recipe) => recipe,
        Err(_) => return import_page(cx, Some(ImportError::SaveFailed)),
    };

    Err(see_other(href!(edit_recipe, RecipeId(recipe.id().as_str())).resolve(cx)).into())
}

fn import_page(cx: &Cx, error: Option<ImportError>) -> Result<impl View> {
    let images_id = element_id(cx);
    let has_error = error.is_some();

    Ok(view! {
        cx =>
        card(
            card_header(card_title((t!(recipe_import_heading))))
            card_content(
                <form
                    method="post"
                    action="/recipe/import"
                    enctype="multipart/form-data"
                    class="flex flex-col gap-5"
                    onsubmit="if (this.dataset.submitting) { event.preventDefault(); return; } this.dataset.submitting = 'true'; this.querySelector('[data-import-submit]').disabled = true; this.querySelector('[data-import-spinner]').classList.remove('hidden'); this.querySelector('[data-import-working]').classList.remove('hidden'); this.setAttribute('aria-busy', 'true');"
                >
                    if has_error {
                        (StatusCode::BAD_REQUEST)
                        <div class="flex flex-col gap-1.5">
                            if matches!(error, Some(ImportError::Empty)) {
                                field_error((t!(recipe_import_empty)))
                            }
                            if matches!(error, Some(ImportError::Unsupported)) {
                                field_error((t!(recipe_import_unsupported)))
                            }
                            if matches!(error, Some(ImportError::Failed)) {
                                field_error((t!(recipe_import_failed)))
                            }
                            if matches!(error, Some(ImportError::SaveFailed)) {
                                field_error((t!(recipe_import_save_failed)))
                            }
                            field_description((t!(recipe_import_retry)))
                        </div>
                    }
                    field(
                        field_label(
                            attrs: attributes! { for=(&images_id) },
                            (t!(recipe_import_images))
                        )
                        image_upload(
                            multiple: true,
                            attrs: attributes! { id=(&images_id) name="images" required="" }
                        )
                        field_description((t!(recipe_import_hint)))
                    )
                    <div class="flex flex-col items-end gap-2">
                        <div class="flex justify-end">
                            button(
                                attrs: attributes! { type="submit" data-import-submit="" },
                                <span
                                    data-import-spinner=""
                                    aria-hidden="true"
                                    class="hidden size-4 animate-spin rounded-full border-2 border-current border-t-transparent"
                                ></span>
                                (t!(recipe_import_submit))
                            )
                        </div>
                        <p
                            data-import-working=""
                            role="status"
                            class="hidden text-sm text-muted-foreground"
                        >
                            (t!(recipe_import_working))
                        </p>
                    </div>
                </form>
            )
        )
    })
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
