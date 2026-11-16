mod layout;

use std::fmt;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExportOptions {
    pub include_images: bool,
    pub include_sources: bool,
    pub hide_author: bool,
    pub show_timestamp: bool,
}

#[derive(Debug, Clone)]
pub struct ExportRecipe {
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub author_name: Option<String>,
    pub total_minutes: Option<usize>,
    pub servings: Option<String>,
    pub revision_timestamp: Option<String>,
    pub parts: Vec<ExportPart>,
    pub sources: Vec<ExportSource>,
}

#[derive(Debug, Clone)]
pub struct ExportPart {
    pub title: Option<String>,
    pub description: Option<String>,
    pub images: Vec<ExportImage>,
    pub ingredients: Vec<ExportIngredient>,
    pub steps: Vec<ExportStep>,
}

#[derive(Debug, Clone)]
pub struct ExportIngredient {
    pub name: String,
    pub quantity: String,
}

#[derive(Debug, Clone)]
pub struct ExportStep {
    pub title: Option<String>,
    pub instruction: String,
    pub images: Vec<ExportImage>,
}

#[derive(Debug, Clone)]
pub struct ExportImage {
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum ExportSource {
    Image(Vec<u8>),
    Pdf { name: String },
}

/// Localized labels used inside the PDF.
#[derive(Debug, Clone)]
pub struct ExportLabels {
    pub author: String,
    pub total_time: String,
    pub minutes: String,
    pub servings: String,
    pub revision: String,
    pub ingredients: String,
    pub steps: String,
    pub step: String,
    pub sources: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportError(String);

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ExportError {}

pub fn render_recipe(
    recipe: &ExportRecipe,
    options: &ExportOptions,
    labels: &ExportLabels,
) -> Result<Vec<u8>, ExportError> {
    layout::render(recipe, options, labels).map_err(ExportError)
}

#[cfg(test)]
mod tests {
    use super::*;
    use printpdf::{PdfDocument, PdfParseOptions};

    fn labels() -> ExportLabels {
        ExportLabels {
            author: "Author".to_string(),
            total_time: "Total time".to_string(),
            minutes: "minutes".to_string(),
            servings: "Servings".to_string(),
            revision: "Revision".to_string(),
            ingredients: "Ingredients".to_string(),
            steps: "Steps".to_string(),
            step: "Step".to_string(),
            sources: "Sources".to_string(),
        }
    }

    fn recipe() -> ExportRecipe {
        ExportRecipe {
            title: "Apple tart".to_string(),
            description: Some("A family favourite.".to_string()),
            category: Some("Dessert".to_string()),
            author_name: Some("Alice".to_string()),
            total_minutes: Some(60),
            servings: Some("8 people".to_string()),
            revision_timestamp: Some("2026-11-16 10:30".to_string()),
            parts: vec![ExportPart {
                title: None,
                description: None,
                images: Vec::new(),
                ingredients: vec![
                    ExportIngredient {
                        name: "Apples".to_string(),
                        quantity: "6".to_string(),
                    },
                    ExportIngredient {
                        name: "Flour".to_string(),
                        quantity: "200 g".to_string(),
                    },
                ],
                steps: vec![ExportStep {
                    title: Some("Prepare".to_string()),
                    instruction: "Peel and slice the apples, then arrange them on the dough."
                        .to_string(),
                    images: Vec::new(),
                }],
            }],
            sources: vec![ExportSource::Pdf {
                name: "grandma.pdf".to_string(),
            }],
        }
    }

    fn text_of(bytes: &[u8]) -> String {
        let document = PdfDocument::parse(bytes, &PdfParseOptions::default(), &mut Vec::new())
            .expect("valid pdf");
        document
            .extract_text()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn renders_a_valid_pdf_with_recipe_content() {
        let bytes = render_recipe(&recipe(), &options(), &labels()).unwrap();

        assert!(bytes.starts_with(b"%PDF"));
        let text = text_of(&bytes);
        assert!(text.contains("Apple tart"));
        assert!(text.contains("Ingredients"));
        assert!(text.contains("Apples"));
        assert!(text.contains("Step 1: Prepare"));
        assert!(text.contains("Alice"));
        assert!(text.contains("2026-11-16 10:30"));
    }

    #[test]
    fn hides_author_and_timestamp_when_requested() {
        let options = ExportOptions {
            hide_author: true,
            show_timestamp: false,
            ..options()
        };
        let bytes = render_recipe(&recipe(), &options, &labels()).unwrap();
        let text = text_of(&bytes);

        assert!(!text.contains("Alice"));
        assert!(!text.contains("2026-11-16 10:30"));
    }

    #[test]
    fn includes_sources_only_when_requested() {
        let bytes = render_recipe(&recipe(), &options(), &labels()).unwrap();
        assert!(!text_of(&bytes).contains("grandma.pdf"));

        let options = ExportOptions {
            include_sources: true,
            ..options()
        };
        let bytes = render_recipe(&recipe(), &options, &labels()).unwrap();
        assert!(text_of(&bytes).contains("grandma.pdf"));
    }

    #[test]
    fn embeds_recipe_images_when_requested() {
        const PIXEL_PNG: &[u8] = &[
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x08, 0x02, 0x00, 0x00,
            0x00, 0xfd, 0xd4, 0x9a, 0x73, 0x00, 0x00, 0x00, 0x13, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0, 0x9f, 0x01, 0x8c, 0xff, 0x33, 0x30, 0x00, 0x00,
            0x1f, 0xee, 0x03, 0xfd, 0x35, 0x1b, 0x00, 0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
            0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ];

        let mut recipe = recipe();
        recipe.parts[0].images = vec![ExportImage {
            bytes: PIXEL_PNG.to_vec(),
        }];
        recipe.parts[0].steps[0].images = vec![ExportImage {
            bytes: PIXEL_PNG.to_vec(),
        }];

        let bytes = render_recipe(&recipe, &options(), &labels()).unwrap();
        let document = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new())
            .expect("valid pdf");

        assert_eq!(document.resources.xobjects.map.len(), 2);

        let without_images = ExportOptions {
            include_images: false,
            ..options()
        };
        let bytes = render_recipe(&recipe, &without_images, &labels()).unwrap();
        let document = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new())
            .expect("valid pdf");

        assert!(document.resources.xobjects.map.is_empty());
    }

    #[test]
    fn long_recipes_span_multiple_pages() {
        let mut recipe = recipe();
        recipe.parts[0].steps = (0..80)
            .map(|index| ExportStep {
                title: None,
                instruction: format!(
                    "This is instruction number {index} with enough text to fill a line or two on the page."
                ),
                images: Vec::new(),
            })
            .collect();

        let bytes = render_recipe(&recipe, &options(), &labels()).unwrap();
        let document = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new())
            .expect("valid pdf");

        assert!(document.page_count() > 1);
    }

    fn options() -> ExportOptions {
        ExportOptions {
            include_images: true,
            include_sources: false,
            hide_author: false,
            show_timestamp: true,
        }
    }
}
