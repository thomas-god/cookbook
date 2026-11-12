use cookbook_domain::recipe::models::RecipeSourceDocumentSummary;
use topcoat::{
    Result,
    context::Cx,
    router::href,
    view::{View, attributes, component, element_id, view},
};

use crate::{
    app::recipes::{
        RecipeId, SourceDocumentId, delete_source_document, source_document,
        upload_source_documents,
    },
    components::{
        base::{
            button::{ButtonSize, ButtonVariant, button},
            field::{field, field_description, field_label},
            image_upload::document_upload,
        },
        recipe::image_thumbnail::{IMAGE_GRID_CLASS, image_thumbnail},
    },
    t,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum SourceDocumentKind {
    Image,
    Pdf,
}

impl SourceDocumentKind {
    fn from_content_type(content_type: &str) -> Self {
        if content_type == "application/pdf" {
            Self::Pdf
        } else {
            Self::Image
        }
    }
}

struct GalleryDocument {
    url: String,
    alt: String,
    kind: SourceDocumentKind,
}

struct EditableDocument {
    url: String,
    delete_url: String,
    alt: String,
    kind: SourceDocumentKind,
}

#[component]
pub async fn source_document_gallery(
    cx: &Cx,
    recipe_id: &str,
    documents: &[RecipeSourceDocumentSummary],
) -> Result<impl View> {
    let fallback_alt = t!(source_document_alt).resolve(cx).into_owned();
    let documents = documents
        .iter()
        .map(|document| {
            let url = href!(
                source_document,
                RecipeId(recipe_id),
                SourceDocumentId(document.id().as_str())
            )
            .resolve(cx);

            GalleryDocument {
                url,
                alt: document
                    .original_name()
                    .map(str::to_string)
                    .unwrap_or_else(|| fallback_alt.clone()),
                kind: SourceDocumentKind::from_content_type(document.content_type()),
            }
        })
        .collect::<Vec<_>>();

    Ok(view! {
        cx =>
        <section>
            <h3 class="mb-3 text-lg font-semibold">(t!(source_documents_heading))</h3>
            if documents.is_empty() {
                <p class="text-sm text-muted-foreground">
                    (t!(source_documents_empty))
                </p>
            } else {
                <ul class=(IMAGE_GRID_CLASS)>
                    for document in documents {
                        <li>
                            if document.kind == SourceDocumentKind::Pdf {
                                pdf_document_link(url: document.url, alt: document.alt)
                            } else {
                                image_thumbnail(url: document.url, alt: document.alt)
                            }
                        </li>
                    }
                </ul>
            }
        </section>
    })
}

#[component]
pub async fn source_document_editor(
    cx: &Cx,
    recipe_id: &str,
    documents: &[RecipeSourceDocumentSummary],
) -> Result<impl View> {
    let documents_id = element_id(cx);
    let fallback_alt = t!(source_document_alt).resolve(cx).into_owned();
    let documents = documents
        .iter()
        .map(|document| {
            let url = href!(
                source_document,
                RecipeId(recipe_id),
                SourceDocumentId(document.id().as_str())
            )
            .resolve(cx);
            let delete_url = href!(
                delete_source_document,
                RecipeId(recipe_id),
                SourceDocumentId(document.id().as_str())
            )
            .resolve(cx);

            EditableDocument {
                url,
                delete_url,
                alt: document
                    .original_name()
                    .map(str::to_string)
                    .unwrap_or_else(|| fallback_alt.clone()),
                kind: SourceDocumentKind::from_content_type(document.content_type()),
            }
        })
        .collect::<Vec<_>>();

    Ok(view! {
        cx =>
        <div class="flex flex-col gap-6">
            if documents.is_empty() {
                <p class="text-sm text-muted-foreground">
                    (t!(source_documents_empty))
                </p>
            } else {
                <ul class=(IMAGE_GRID_CLASS)>
                    for document in documents {
                        <li class="flex flex-col gap-2">
                            if document.kind == SourceDocumentKind::Pdf {
                                pdf_document_link(url: document.url, alt: document.alt)
                            } else {
                                image_thumbnail(url: document.url, alt: document.alt)
                            }
                            <form method="post" action=(document.delete_url)>
                                button(
                                    variant: ButtonVariant::Ghost,
                                    size: ButtonSize::Sm,
                                    attrs: attributes! { type="submit" },
                                    (t!(source_documents_delete))
                                )
                            </form>
                        </li>
                    }
                </ul>
            }

            <form
                method="post"
                action=(href!(upload_source_documents, RecipeId(recipe_id)))
                enctype="multipart/form-data"
                class="flex flex-col gap-4"
            >
                field(
                    field_label(
                        attrs: attributes! { for=(&documents_id) },
                        (t!(source_documents_upload))
                    )
                    document_upload(
                        multiple: true,
                        attrs: attributes! { id=(&documents_id) name="documents" required="" }
                    )
                    field_description((t!(source_documents_upload_hint)))
                )
                <div class="flex justify-end">
                    button(
                        variant: ButtonVariant::Outline,
                        size: ButtonSize::Sm,
                        attrs: attributes! { type="submit" },
                        (t!(source_documents_upload_submit))
                    )
                </div>
            </form>
        </div>
    })
}

/// A linked document tile that opens the PDF in a new tab.
#[component]
async fn pdf_document_link(cx: &Cx, url: String, alt: String) -> Result<impl View> {
    Ok(view! {
        cx =>
        <a
            href=(url)
            target="_blank"
            rel="noreferrer"
            class="flex aspect-square w-full flex-col items-center justify-center gap-2 rounded-lg border border-border bg-muted/40 px-3 text-center"
        >
            <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
                class="size-8 text-muted-foreground"
            >
                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                <path d="M14 2v6h6"></path>
                <path d="M8 13h8"></path>
                <path d="M8 17h8"></path>
            </svg>
            <span class="text-xs font-medium break-all">(alt)</span>
            <span class="text-xs text-muted-foreground">
                (t!(source_document_pdf_open))
            </span>
        </a>
    })
}
