use topcoat::{
    Result,
    view::{View, component, view},
};

/// The grid layout used for thumbnail collections.
pub(super) const IMAGE_GRID_CLASS: &str = "grid grid-cols-2 gap-3 sm:grid-cols-3";

const IMAGE_CLASS: &str = "aspect-square w-full rounded-lg border border-border object-cover";

/// A linked image thumbnail that opens the full image in a new tab.
#[component]
pub async fn image_thumbnail(url: String, alt: String) -> Result<impl View> {
    Ok(view! {
        <a href=(url.clone()) target="_blank" rel="noreferrer">
            <img src=(url) alt=(alt) loading="lazy" class=(IMAGE_CLASS)>
        </a>
    })
}
