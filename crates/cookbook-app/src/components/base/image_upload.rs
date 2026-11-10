//! File input for image uploads.

use topcoat::{
    Result,
    context::Cx,
    view::{Attributes, View, component, view},
};

use super::input::input;

/// The MIME types advertised by every image picker.
///
/// iOS converts HEIC photos to JPEG when a file input only accepts JPEG and PNG.
/// Keep all image uploads on this list so the conversion also applies to them.
pub const IMAGE_ACCEPT: &str = "image/jpeg,image/png";

/// A file input restricted to JPEG and PNG images.
///
/// This is a thin wrapper around [`input`] that always sets `type="file"` and
/// `accept` to [`IMAGE_ACCEPT`]. Callers cannot override either attribute, so an
/// image picker cannot accidentally lose the iOS HEIC conversion.
///
/// Set `multiple: true` to let the picker return several images at once.
///
/// ```ignore
/// view! {
///     image_upload(
///         multiple: true,
///         attrs: attributes! {
///             id=(&image_id)
///             name="images"
///             required=""
///         }
///     )
/// }
/// ```
#[component]
pub async fn image_upload(
    cx: &Cx,
    #[default] multiple: bool,
    #[default] mut attrs: Attributes,
) -> Result<impl View> {
    attrs.insert(cx, "type", "file");
    attrs.insert(cx, "accept", IMAGE_ACCEPT);

    if multiple {
        attrs.insert(cx, "multiple", "");
    }

    Ok(view! {
        cx =>
        input(attrs: attrs)
    })
}
