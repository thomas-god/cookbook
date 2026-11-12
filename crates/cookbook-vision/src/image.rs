use std::io::Cursor;

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::{LoadPdfError, Pdf};
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::vello_cpu::peniko::ImageAlphaType;
use hayro::{PixmapSettings, RenderCache, RenderSettings, render};
use image::{
    DynamicImage, ImageDecoder, ImageFormat, ImageReader, RgbaImage, codecs::jpeg::JpegEncoder,
    metadata::Orientation,
};

use crate::VisionError;

/// The longest edge of an image sent to the VLM. Larger photos and PDF pages
/// are resized before being encoded into the request.
const MAX_VLM_IMAGE_EDGE: u32 = 2048;
const VLM_JPEG_QUALITY: u8 = 85;

/// The maximum number of pages accepted from a single PDF. One PDF is treated
/// as one recipe, so a larger document is more likely to be a whole cookbook.
pub const MAX_PDF_PAGES: usize = 10;

/// PDF readers are allowed to place a byte-order mark or comments before the
/// header, so look for the magic bytes near the start of the file.
const PDF_MAGIC_SCAN_BYTES: usize = 1024;

/// Detects the content type of a supported image from its magic bytes.
///
/// This is intentionally a lightweight check for application-side upload
/// validation. The vision pipeline performs a full decode before sending the
/// image to a model.
pub fn detect_image_content_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }

    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }

    None
}

/// Detects the content type of a supported source document from its magic
/// bytes. Source documents may be images or PDF files.
pub fn detect_source_document_content_type(bytes: &[u8]) -> Option<&'static str> {
    if is_pdf(bytes) {
        return Some("application/pdf");
    }

    detect_image_content_type(bytes)
}

fn is_pdf(bytes: &[u8]) -> bool {
    let window = &bytes[..bytes.len().min(PDF_MAGIC_SCAN_BYTES)];

    window.windows(5).any(|window| window == b"%PDF-")
}

/// A source document supplied by the application.
#[derive(Debug, Clone)]
pub struct SourceDocument {
    original_name: Option<String>,
    bytes: Vec<u8>,
}

impl SourceDocument {
    pub fn new(original_name: Option<String>, bytes: Vec<u8>) -> Self {
        Self {
            original_name,
            bytes,
        }
    }

    pub fn original_name(&self) -> Option<&str> {
        self.original_name.as_deref()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// A stored source document, kept in its original form.
#[derive(Debug, Clone)]
pub(crate) struct PreparedSourceDocument {
    pub(crate) content_type: &'static str,
    pub(crate) original_name: Option<String>,
    pub(crate) original_bytes: Vec<u8>,
}

/// A page image ready to be sent to the VLM.
#[derive(Debug, Clone)]
pub(crate) struct PreparedImage {
    pub(crate) vlm_bytes: Vec<u8>,
    pub(crate) vlm_content_type: &'static str,
}

/// The stored documents and the transient images sent to the VLM.
#[derive(Debug, Clone, Default)]
pub(crate) struct PreparedSources {
    pub(crate) documents: Vec<PreparedSourceDocument>,
    pub(crate) vlm_images: Vec<PreparedImage>,
}

pub(crate) fn prepare_sources(
    documents: Vec<SourceDocument>,
) -> Result<PreparedSources, VisionError> {
    if documents.is_empty() {
        return Err(VisionError::NoDocuments);
    }

    let pdf_count = documents
        .iter()
        .filter(|document| is_pdf(&document.bytes))
        .count();

    if pdf_count > 1 {
        return Err(VisionError::MultiplePdfDocuments);
    }

    if pdf_count == 1 && documents.len() > 1 {
        return Err(VisionError::MixedDocumentTypes);
    }

    let mut prepared = PreparedSources::default();

    for (index, document) in documents.into_iter().enumerate() {
        if document.bytes.is_empty() {
            return Err(VisionError::EmptyDocument { index });
        }

        if is_pdf(&document.bytes) {
            let (source, pages) = prepare_pdf(index, document)?;
            prepared.documents.push(source);
            prepared.vlm_images.extend(pages);
        } else {
            let (source, image) = prepare_image(index, document)?;
            prepared.documents.push(source);
            prepared.vlm_images.push(image);
        }
    }

    Ok(prepared)
}

fn prepare_image(
    index: usize,
    document: SourceDocument,
) -> Result<(PreparedSourceDocument, PreparedImage), VisionError> {
    let SourceDocument {
        original_name,
        bytes: original_bytes,
    } = document;

    let (content_type, vlm_bytes, vlm_content_type) = {
        let reader = ImageReader::new(Cursor::new(original_bytes.as_slice()))
            .with_guessed_format()
            .map_err(|error| VisionError::DocumentProcessing {
                index,
                reason: error.to_string(),
            })?;

        let content_type = match reader.format() {
            Some(ImageFormat::Jpeg) => "image/jpeg",
            Some(ImageFormat::Png) => "image/png",
            Some(format) => {
                return Err(VisionError::UnsupportedDocument {
                    index,
                    reason: format!("{format:?}"),
                });
            }
            None => {
                return Err(VisionError::UnsupportedDocument {
                    index,
                    reason: "unknown document format".to_string(),
                });
            }
        };

        let mut decoder =
            reader
                .into_decoder()
                .map_err(|error| VisionError::DocumentProcessing {
                    index,
                    reason: error.to_string(),
                })?;
        let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
        let mut image = DynamicImage::from_decoder(decoder).map_err(|error| {
            VisionError::DocumentProcessing {
                index,
                reason: error.to_string(),
            }
        })?;
        image.apply_orientation(orientation);

        if image.width() <= MAX_VLM_IMAGE_EDGE && image.height() <= MAX_VLM_IMAGE_EDGE {
            (content_type, original_bytes.clone(), content_type)
        } else {
            let resized = image.thumbnail(MAX_VLM_IMAGE_EDGE, MAX_VLM_IMAGE_EDGE);
            let encoded =
                encode_jpeg(&resized).map_err(|error| VisionError::DocumentProcessing {
                    index,
                    reason: error.to_string(),
                })?;

            (content_type, encoded, "image/jpeg")
        }
    };

    Ok((
        PreparedSourceDocument {
            content_type,
            original_name,
            original_bytes,
        },
        PreparedImage {
            vlm_bytes,
            vlm_content_type,
        },
    ))
}

fn prepare_pdf(
    index: usize,
    document: SourceDocument,
) -> Result<(PreparedSourceDocument, Vec<PreparedImage>), VisionError> {
    let SourceDocument {
        original_name,
        bytes: original_bytes,
    } = document;

    let pdf = Pdf::new(original_bytes.clone()).map_err(|error| match error {
        LoadPdfError::Decryption(_) => VisionError::EncryptedPdf { index },
        LoadPdfError::Invalid => VisionError::DocumentProcessing {
            index,
            reason: "invalid PDF".to_string(),
        },
    })?;

    let page_count = pdf.pages().iter().count();

    if page_count == 0 {
        return Err(VisionError::EmptyDocument { index });
    }

    if page_count > MAX_PDF_PAGES {
        return Err(VisionError::TooManyPages {
            index,
            count: page_count,
            max: MAX_PDF_PAGES,
        });
    }

    let cache = RenderCache::new();
    let interpreter_settings = InterpreterSettings::default();
    let render_settings = RenderSettings::default();
    let mut images = Vec::with_capacity(page_count);

    for page in pdf.pages().iter() {
        let (width, height) = page.render_dimensions();
        let longest_edge = width.max(height).max(1.0);
        let scale = MAX_VLM_IMAGE_EDGE as f32 / longest_edge;
        let pixmap = render(
            page,
            &cache,
            &interpreter_settings,
            &render_settings,
            &PixmapSettings {
                x_scale: scale,
                y_scale: scale,
                bg_color: WHITE,
            },
        );

        let width = u32::from(pixmap.width());
        let height = u32::from(pixmap.height());
        let rgba = pixmap.take_rgba8(ImageAlphaType::Alpha);
        let rgba_image = RgbaImage::from_raw(width, height, rgba).ok_or_else(|| {
            VisionError::DocumentProcessing {
                index,
                reason: "rendered PDF page has an invalid size".to_string(),
            }
        })?;
        let image = DynamicImage::ImageRgba8(rgba_image);
        let image = if image.width() > MAX_VLM_IMAGE_EDGE || image.height() > MAX_VLM_IMAGE_EDGE {
            image.thumbnail(MAX_VLM_IMAGE_EDGE, MAX_VLM_IMAGE_EDGE)
        } else {
            image
        };
        let encoded = encode_jpeg(&image).map_err(|error| VisionError::DocumentProcessing {
            index,
            reason: error.to_string(),
        })?;

        images.push(PreparedImage {
            vlm_bytes: encoded,
            vlm_content_type: "image/jpeg",
        });
    }

    Ok((
        PreparedSourceDocument {
            content_type: "application/pdf",
            original_name,
            original_bytes,
        },
        images,
    ))
}

fn encode_jpeg(image: &DynamicImage) -> image::ImageResult<Vec<u8>> {
    let mut encoded = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, VLM_JPEG_QUALITY).encode_image(image)?;

    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_PAGE_PDF: &[u8] = include_bytes!("../tests/fixtures/two-pages.pdf");
    const ELEVEN_PAGE_PDF: &[u8] = include_bytes!("../tests/fixtures/eleven-pages.pdf");

    fn jpeg_bytes(width: u32, height: u32) -> Vec<u8> {
        let image = DynamicImage::new_rgb8(width, height);
        let mut bytes = Vec::new();
        JpegEncoder::new_with_quality(&mut bytes, 90)
            .encode_image(&image)
            .unwrap();
        bytes
    }

    #[test]
    fn rejects_an_empty_request() {
        assert!(matches!(
            prepare_sources(Vec::new()),
            Err(VisionError::NoDocuments)
        ));
    }

    #[test]
    fn rejects_an_empty_document() {
        let result = prepare_sources(vec![SourceDocument::new(None, Vec::new())]);

        assert!(matches!(
            result,
            Err(VisionError::EmptyDocument { index: 0 })
        ));
    }

    #[test]
    fn detects_supported_content_types() {
        assert_eq!(
            detect_source_document_content_type(&[0xFF, 0xD8, 0xFF, 0xE0]),
            Some("image/jpeg")
        );
        assert_eq!(
            detect_source_document_content_type(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
            Some("image/png")
        );
        assert_eq!(
            detect_source_document_content_type(b"%PDF-1.7\n"),
            Some("application/pdf")
        );
        assert_eq!(
            detect_source_document_content_type(b"comment\n%PDF-1.4\n"),
            Some("application/pdf")
        );
        assert_eq!(detect_source_document_content_type(b"not a document"), None);
    }

    #[test]
    fn rejects_an_unknown_format() {
        let result = prepare_sources(vec![SourceDocument::new(None, b"not a document".to_vec())]);

        assert!(matches!(
            result,
            Err(VisionError::UnsupportedDocument { index: 0, .. })
        ));
    }

    #[test]
    fn keeps_a_small_jpeg_as_is() {
        let bytes = jpeg_bytes(64, 32);
        let prepared = prepare_sources(vec![SourceDocument::new(
            Some("small.jpg".to_string()),
            bytes.clone(),
        )])
        .unwrap();

        assert_eq!(prepared.documents[0].content_type, "image/jpeg");
        assert_eq!(prepared.documents[0].original_bytes, bytes);
        assert_eq!(prepared.vlm_images[0].vlm_content_type, "image/jpeg");
        assert_eq!(prepared.vlm_images[0].vlm_bytes, bytes);
    }

    #[test]
    fn resizes_a_large_jpeg_for_the_vlm() {
        // Just over the edge is enough to exercise resizing; a photo-sized
        // canvas would only make encoding and resizing slower.
        let prepared = prepare_sources(vec![SourceDocument::new(
            Some("large.jpg".to_string()),
            jpeg_bytes(MAX_VLM_IMAGE_EDGE + 1, 8),
        )])
        .unwrap();

        assert_eq!(prepared.documents[0].content_type, "image/jpeg");
        assert_eq!(prepared.vlm_images[0].vlm_content_type, "image/jpeg");
        assert_ne!(
            prepared.documents[0].original_bytes,
            prepared.vlm_images[0].vlm_bytes
        );
        assert!(
            prepared.vlm_images[0].vlm_bytes.len() < prepared.documents[0].original_bytes.len()
        );
    }

    #[test]
    fn renders_each_pdf_page_for_the_vlm_but_keeps_the_original_document() {
        let prepared = prepare_sources(vec![SourceDocument::new(
            Some("recipe.pdf".to_string()),
            TWO_PAGE_PDF.to_vec(),
        )])
        .unwrap();

        assert_eq!(prepared.documents.len(), 1);
        assert_eq!(prepared.documents[0].content_type, "application/pdf");
        assert_eq!(
            prepared.documents[0].original_name.as_deref(),
            Some("recipe.pdf")
        );
        assert_eq!(prepared.documents[0].original_bytes, TWO_PAGE_PDF);
        assert_eq!(prepared.vlm_images.len(), 2);
        assert!(
            prepared
                .vlm_images
                .iter()
                .all(|image| image.vlm_content_type == "image/jpeg")
        );
    }

    #[test]
    fn rejects_a_pdf_with_too_many_pages() {
        let result = prepare_sources(vec![SourceDocument::new(
            Some("cookbook.pdf".to_string()),
            ELEVEN_PAGE_PDF.to_vec(),
        )]);

        assert!(matches!(
            result,
            Err(VisionError::TooManyPages {
                index: 0,
                count: 11,
                max: MAX_PDF_PAGES
            })
        ));
    }

    #[test]
    fn rejects_more_than_one_pdf() {
        let result = prepare_sources(vec![
            SourceDocument::new(Some("one.pdf".to_string()), TWO_PAGE_PDF.to_vec()),
            SourceDocument::new(Some("two.pdf".to_string()), TWO_PAGE_PDF.to_vec()),
        ]);

        assert!(matches!(result, Err(VisionError::MultiplePdfDocuments)));
    }

    #[test]
    fn rejects_mixing_a_pdf_with_an_image() {
        let result = prepare_sources(vec![
            SourceDocument::new(Some("recipe.pdf".to_string()), TWO_PAGE_PDF.to_vec()),
            SourceDocument::new(Some("photo.jpg".to_string()), jpeg_bytes(8, 8)),
        ]);

        assert!(matches!(result, Err(VisionError::MixedDocumentTypes)));
    }

    #[test]
    fn rejects_an_invalid_pdf() {
        let result = prepare_sources(vec![SourceDocument::new(
            None,
            b"%PDF-1.7 not really a PDF".to_vec(),
        )]);

        assert!(matches!(
            result,
            Err(VisionError::DocumentProcessing { index: 0, .. })
        ));
    }
}
