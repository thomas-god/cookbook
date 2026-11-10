//! Development-only upload probes.
//!
//! These routes exist to observe what a browser actually sends. They are not part of the
//! cookbook feature set.

use std::fmt::Write as _;

use topcoat::{
    Result,
    context::Cx,
    router::{content::multipart::Multipart, error::bad_request, page},
    view::{View, attributes, element_id, view},
};

use crate::{
    app::auth,
    components::base::{
        button::button,
        card::{card, card_content, card_header, card_title},
        field::{field, field_label},
        image_upload::{IMAGE_ACCEPT, image_upload},
    },
};

/// The maximum image size this probe accepts. It matches the [`BodyLimit`] registered
/// for `/dev/heic` in the router.
pub const MAX_UPLOAD_BYTES: usize = 25 * 1024 * 1024;

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    auth::require_user(cx)?;

    render(cx, Vec::new())
}

#[page(POST)]
pub async fn upload(cx: &Cx, mut multipart: Multipart) -> Result<impl View> {
    auth::require_user(cx)?;

    let mut reports = Vec::new();

    while let Some(part) = multipart.next_field().await? {
        let field_name = part.name().map(str::to_owned);
        let file_name = part.file_name().map(str::to_owned);
        let declared_content_type = part.content_type().map(str::to_owned);
        let headers = part
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    value.to_str().unwrap_or("<non-UTF-8>").to_owned(),
                )
            })
            .collect::<Vec<_>>();
        let bytes = part.bytes().await?;

        let is_image_field = matches!(field_name.as_deref(), Some("image" | "images"));
        if is_image_field || file_name.is_some() {
            reports.push(UploadReport::new(
                field_name,
                file_name,
                declared_content_type,
                headers,
                &bytes,
            ));
        }
    }

    if reports.is_empty() {
        return Err(bad_request("the upload did not include a file").into());
    }

    render(cx, reports)
}

fn render(cx: &Cx, reports: Vec<UploadReport>) -> Result<impl View> {
    let image_id = element_id(cx);

    Ok(view! {
        cx =>
        <div class="flex flex-col gap-6">
            <div>
                <h2 class="text-2xl font-semibold">"HEIC upload test"</h2>
                <p class="mt-2 text-sm text-muted-foreground">
                    "Pick one or more photos from the iOS Photos app. The file input advertises "
                    <code>(format!("accept=\"{IMAGE_ACCEPT}\""))</code>
                    ", so iOS may convert HEIC originals before uploading them. This page "
                    "inspects the bytes that actually arrive on the server."
                </p>
            </div>

            card(
                card_header(card_title("Upload photos"))
                card_content(
                    <form
                        method="post"
                        action="/dev/heic"
                        enctype="multipart/form-data"
                        class="flex flex-col gap-4"
                    >
                        field(
                            field_label(
                                attrs: attributes! { for=(&image_id) },
                                "Images"
                            )
                            image_upload(
                                multiple: true,
                                attrs: attributes! {
                                    id=(&image_id)
                                    name="images"
                                    required=""
                                }
                            )
                        )
                        <p class="text-sm text-muted-foreground">
                            "Tip: use the Photos picker rather than the Files app. Picking from "
                            "Files can bypass the conversion iOS applies to photo-library items."
                        </p>
                        button(attrs: attributes! { type="submit" }, "Upload and inspect")
                    </form>
                )
            )

            if !reports.is_empty() {
                <div class="flex flex-col gap-4">
                    <h3 class="text-lg font-semibold">"What the server received"</h3>
                    for (index, report) in reports.iter().enumerate() {
                        card(
                            card_header(card_title((format!("Image {}", index + 1))))
                            card_content(
                                <div class="flex flex-col gap-4 text-sm">
                                    <div class="rounded-lg border border-border bg-muted/40 p-4">
                                        <p class="font-medium">(report.verdict)</p>
                                        <p class="mt-1 text-muted-foreground">
                                            (report.verdict_detail.as_str())
                                        </p>
                                    </div>

                                    <dl class="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-2">
                                        <dt class="text-muted-foreground">"Field name"</dt>
                                        <dd>(report.field_name.as_deref().unwrap_or("—"))</dd>

                                        <dt class="text-muted-foreground">"File name"</dt>
                                        <dd class="break-all">
                                            (report.file_name.as_deref().unwrap_or("—"))
                                        </dd>

                                        <dt class="text-muted-foreground">
                                            "Declared Content-Type"
                                        </dt>
                                        <dd class="break-all">
                                            (report.declared_content_type.as_deref().unwrap_or("—"))
                                        </dd>

                                        <dt class="text-muted-foreground">
                                            "Detected from bytes"
                                        </dt>
                                        <dd>
                                            (report.detected_format)
                                            if let Some(brand) = &report.heif_brand {
                                                " (brand: "
                                                (brand)
                                                ")"
                                            }
                                        </dd>

                                        <dt class="text-muted-foreground">"Size"</dt>
                                        <dd>(report.size) " bytes"</dd>

                                        <dt class="text-muted-foreground">"First bytes (hex)"</dt>
                                        <dd class="break-all font-mono">(report.magic_hex.as_str())</dd>
                                    </dl>

                                    <details>
                                        <summary class="cursor-pointer font-medium">
                                            "Part headers"
                                        </summary>
                                        <ul class="mt-2 flex flex-col gap-1 font-mono text-xs">
                                            for (name, value) in &report.headers {
                                                <li class="break-all">
                                                    (name)
                                                    ": "
                                                    (value)
                                                </li>
                                            }
                                        </ul>
                                    </details>
                                </div>
                            )
                        )
                    }
                </div>
            }
        </div>
    })
}

struct UploadReport {
    field_name: Option<String>,
    file_name: Option<String>,
    declared_content_type: Option<String>,
    headers: Vec<(String, String)>,
    size: usize,
    magic_hex: String,
    detected_format: &'static str,
    heif_brand: Option<String>,
    verdict: &'static str,
    verdict_detail: String,
}

impl UploadReport {
    fn new(
        field_name: Option<String>,
        file_name: Option<String>,
        declared_content_type: Option<String>,
        headers: Vec<(String, String)>,
        bytes: &[u8],
    ) -> Self {
        let (detected_format, heif_brand) = detect_format(bytes);
        let (verdict, verdict_detail) = verdict(
            detected_format,
            declared_content_type.as_deref(),
            file_name.as_deref(),
        );

        Self {
            field_name,
            file_name,
            declared_content_type,
            headers,
            size: bytes.len(),
            magic_hex: hex_prefix(bytes, 32),
            detected_format,
            heif_brand,
            verdict,
            verdict_detail,
        }
    }
}

fn detect_format(bytes: &[u8]) -> (&'static str, Option<String>) {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return ("JPEG", None);
    }

    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return ("PNG", None);
    }

    // ISO base media files (HEIC/HEIF/AVIF) have an `ftyp` box at offset 4 and a
    // four-character brand at offset 8.
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        let brand = String::from_utf8_lossy(&bytes[8..12]).trim().to_string();
        let format = match brand.as_str() {
            "heic" | "heix" | "hevc" | "hevx" | "mif1" | "msf1" => "HEIF/HEIC",
            "avif" | "avis" => "AVIF",
            _ => "ISO BMFF",
        };

        return (format, Some(brand));
    }

    ("Unknown", None)
}

fn verdict(
    detected_format: &str,
    declared_content_type: Option<&str>,
    file_name: Option<&str>,
) -> (&'static str, String) {
    let source_looks_heic = declared_content_type.is_some_and(|value| {
        value.eq_ignore_ascii_case("image/heic") || value.eq_ignore_ascii_case("image/heif")
    }) || file_name.is_some_and(|value| {
        let value = value.to_ascii_lowercase();
        value.ends_with(".heic") || value.ends_with(".heif")
    });

    match detected_format {
        "HEIF/HEIC" => (
            "Still HEIC/HEIF",
            "The uploaded bytes are still HEIC/HEIF. iOS did not convert this file before \
             upload."
                .to_owned(),
        ),
        "JPEG" if source_looks_heic => (
            "Converted to JPEG",
            "The file name or declared type looked like HEIC, but the bytes are JPEG. The \
             browser converted the image before uploading it."
                .to_owned(),
        ),
        "JPEG" => (
            "JPEG",
            "The uploaded bytes are JPEG. If you selected a HEIC photo, the browser converted \
             it before upload."
                .to_owned(),
        ),
        "PNG" => (
            "PNG",
            "The uploaded bytes are PNG. The browser either selected a PNG original or \
             converted the image to PNG."
                .to_owned(),
        ),
        "AVIF" => (
            "AVIF",
            "The uploaded bytes are AVIF. No HEIC-to-JPEG conversion was applied.".to_owned(),
        ),
        _ => (
            "Unrecognized format",
            "The first bytes do not match a format this probe knows. Check the hex dump and \
             part headers below."
                .to_owned(),
        ),
    }
}

fn hex_prefix(bytes: &[u8], limit: usize) -> String {
    let mut output = String::new();

    for (index, byte) in bytes.iter().take(limit).enumerate() {
        if index > 0 {
            output.push(' ');
        }
        let _ = write!(output, "{byte:02X}");
    }

    if bytes.len() > limit {
        output.push_str(" …");
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_jpeg() {
        assert_eq!(
            detect_format(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10]),
            ("JPEG", None)
        );
    }

    #[test]
    fn detects_png() {
        assert_eq!(
            detect_format(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
            ("PNG", None)
        );
    }

    #[test]
    fn detects_heic_brand() {
        assert_eq!(
            detect_format(b"\0\0\0\x18ftypheic\0\0\0\0"),
            ("HEIF/HEIC", Some("heic".to_owned()))
        );
        assert_eq!(
            detect_format(b"\0\0\0\x18ftypmif1\0\0\0\0"),
            ("HEIF/HEIC", Some("mif1".to_owned()))
        );
    }

    #[test]
    fn reports_conversion_when_filename_still_says_heic() {
        let (verdict, _) = verdict("JPEG", Some("image/jpeg"), Some("IMG_0001.HEIC"));

        assert_eq!(verdict, "Converted to JPEG");
    }

    #[test]
    fn reports_unconverted_heic() {
        let (verdict, _) = verdict("HEIF/HEIC", Some("image/heic"), Some("IMG_0001.HEIC"));

        assert_eq!(verdict, "Still HEIC/HEIF");
    }

    #[test]
    fn formats_hex_prefix() {
        assert_eq!(hex_prefix(&[0x00, 0x0F, 0xA0, 0xFF], 32), "00 0F A0 FF");
        assert_eq!(hex_prefix(&[0x00, 0x0F, 0xA0, 0xFF], 2), "00 0F …");
    }
}
