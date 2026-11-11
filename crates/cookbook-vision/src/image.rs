use std::io::Cursor;

use image::{
    DynamicImage, ImageDecoder, ImageFormat, ImageReader, codecs::jpeg::JpegEncoder,
    metadata::Orientation,
};

use crate::VisionError;

/// The longest edge of an image sent to the VLM. Larger photos are resized
/// before being encoded into the request.
const MAX_VLM_IMAGE_EDGE: u32 = 2048;
const VLM_JPEG_QUALITY: u8 = 85;

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

/// A photo supplied by the application.
#[derive(Debug, Clone)]
pub struct SourcePhoto {
    original_name: Option<String>,
    bytes: Vec<u8>,
}

impl SourcePhoto {
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

#[derive(Debug, Clone)]
pub(crate) struct PreparedImage {
    pub(crate) content_type: &'static str,
    pub(crate) original_name: Option<String>,
    pub(crate) original_bytes: Vec<u8>,
    pub(crate) vlm_bytes: Vec<u8>,
    pub(crate) vlm_content_type: &'static str,
}

pub(crate) fn prepare_images(photos: Vec<SourcePhoto>) -> Result<Vec<PreparedImage>, VisionError> {
    if photos.is_empty() {
        return Err(VisionError::NoImages);
    }

    photos
        .into_iter()
        .enumerate()
        .map(|(index, photo)| prepare_image(index, photo))
        .collect()
}

fn prepare_image(index: usize, photo: SourcePhoto) -> Result<PreparedImage, VisionError> {
    if photo.bytes.is_empty() {
        return Err(VisionError::EmptyImage { index });
    }

    let original_bytes = photo.bytes;
    let original_name = photo.original_name;

    let (content_type, vlm_bytes, vlm_content_type) = {
        let reader = ImageReader::new(Cursor::new(original_bytes.as_slice()))
            .with_guessed_format()
            .map_err(|error| VisionError::ImageProcessing {
                index,
                reason: error.to_string(),
            })?;

        let content_type = match reader.format() {
            Some(ImageFormat::Jpeg) => "image/jpeg",
            Some(ImageFormat::Png) => "image/png",
            Some(format) => {
                return Err(VisionError::UnsupportedImage {
                    index,
                    reason: format!("{format:?}"),
                });
            }
            None => {
                return Err(VisionError::UnsupportedImage {
                    index,
                    reason: "unknown image format".to_string(),
                });
            }
        };

        let mut decoder = reader
            .into_decoder()
            .map_err(|error| VisionError::ImageProcessing {
                index,
                reason: error.to_string(),
            })?;
        let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
        let mut image =
            DynamicImage::from_decoder(decoder).map_err(|error| VisionError::ImageProcessing {
                index,
                reason: error.to_string(),
            })?;
        image.apply_orientation(orientation);

        if image.width() <= MAX_VLM_IMAGE_EDGE && image.height() <= MAX_VLM_IMAGE_EDGE {
            (content_type, original_bytes.clone(), content_type)
        } else {
            let resized = image.thumbnail(MAX_VLM_IMAGE_EDGE, MAX_VLM_IMAGE_EDGE);
            let mut encoded = Vec::new();
            JpegEncoder::new_with_quality(&mut encoded, VLM_JPEG_QUALITY)
                .encode_image(&resized)
                .map_err(|error| VisionError::ImageProcessing {
                    index,
                    reason: error.to_string(),
                })?;

            (content_type, encoded, "image/jpeg")
        }
    };

    Ok(PreparedImage {
        content_type,
        original_name,
        original_bytes,
        vlm_bytes,
        vlm_content_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
            prepare_images(Vec::new()),
            Err(VisionError::NoImages)
        ));
    }

    #[test]
    fn rejects_an_empty_photo() {
        let result = prepare_images(vec![SourcePhoto::new(None, Vec::new())]);

        assert!(matches!(result, Err(VisionError::EmptyImage { index: 0 })));
    }

    #[test]
    fn detects_supported_content_types() {
        assert_eq!(
            detect_image_content_type(&[0xFF, 0xD8, 0xFF, 0xE0]),
            Some("image/jpeg")
        );
        assert_eq!(
            detect_image_content_type(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
            Some("image/png")
        );
        assert_eq!(detect_image_content_type(b"not an image"), None);
    }

    #[test]
    fn rejects_an_unknown_format() {
        let result = prepare_images(vec![SourcePhoto::new(None, b"not an image".to_vec())]);

        assert!(matches!(
            result,
            Err(VisionError::UnsupportedImage { index: 0, .. })
        ));
    }

    #[test]
    fn keeps_a_small_jpeg_as_is() {
        let bytes = jpeg_bytes(64, 32);
        let prepared = prepare_images(vec![SourcePhoto::new(
            Some("small.jpg".to_string()),
            bytes.clone(),
        )])
        .unwrap();

        assert_eq!(prepared[0].content_type, "image/jpeg");
        assert_eq!(prepared[0].vlm_content_type, "image/jpeg");
        assert_eq!(prepared[0].original_bytes, bytes);
        assert_eq!(prepared[0].vlm_bytes, bytes);
    }

    #[test]
    fn resizes_a_large_jpeg_for_the_vlm() {
        // Just over the edge is enough to exercise resizing; a photo-sized
        // canvas would only make encoding and resizing slower.
        let prepared = prepare_images(vec![SourcePhoto::new(
            Some("large.jpg".to_string()),
            jpeg_bytes(MAX_VLM_IMAGE_EDGE + 1, 8),
        )])
        .unwrap();

        assert_eq!(prepared[0].content_type, "image/jpeg");
        assert_eq!(prepared[0].vlm_content_type, "image/jpeg");
        assert_ne!(prepared[0].original_bytes, prepared[0].vlm_bytes);
        assert!(prepared[0].vlm_bytes.len() < prepared[0].original_bytes.len());
    }
}
