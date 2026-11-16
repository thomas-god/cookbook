use printpdf::{
    BuiltinFont, Color, Line, LineDashPattern, LinePoint, Mm, Op, ParsedFont, PdfDocument,
    PdfFontHandle, PdfPage, PdfSaveOptions, Point, Pt, RawImage, Rgb, TextItem, XObjectTransform,
};

use crate::{
    ExportImage, ExportIngredient, ExportLabels, ExportOptions, ExportPart, ExportRecipe,
    ExportSource, ExportStep,
};

const PAGE_WIDTH: f32 = 595.28;
const PAGE_HEIGHT: f32 = 841.89;
const MARGIN: f32 = 51.02;
const CONTENT_WIDTH: f32 = PAGE_WIDTH - 2.0 * MARGIN;
const CONTENT_TOP: f32 = 51.02;
const CONTENT_BOTTOM: f32 = PAGE_HEIGHT - 56.69;
const FOOTER_Y: f32 = PAGE_HEIGHT - 34.0;

const BODY_SIZE: f32 = 10.5;
const LINE_FACTOR: f32 = 1.4;

fn body_color() -> Color {
    Color::Rgb(Rgb {
        r: 0.12,
        g: 0.12,
        b: 0.12,
        icc_profile: None,
    })
}

fn muted_color() -> Color {
    Color::Rgb(Rgb {
        r: 0.42,
        g: 0.42,
        b: 0.42,
        icc_profile: None,
    })
}

fn rule_color() -> Color {
    Color::Rgb(Rgb {
        r: 0.82,
        g: 0.82,
        b: 0.82,
        icc_profile: None,
    })
}

struct Metrics {
    regular: ParsedFont,
    bold: ParsedFont,
}

impl Metrics {
    fn new() -> Self {
        Self {
            regular: BuiltinFont::Helvetica
                .get_parsed_font()
                .expect("built-in Helvetica font"),
            bold: BuiltinFont::HelveticaBold
                .get_parsed_font()
                .expect("built-in Helvetica bold font"),
        }
    }

    fn width(&self, bold: bool, size: f32, text: &str) -> f32 {
        let font = if bold { &self.bold } else { &self.regular };
        let units = f32::from(font.units_per_em.max(1));

        text.chars()
            .map(|character| {
                let fallback = font.lookup_glyph_index('?' as u32).unwrap_or(0);
                let glyph = font
                    .lookup_glyph_index(character as u32)
                    .unwrap_or(fallback);
                let advance = font.get_glyph_width(glyph).unwrap_or(0);

                f32::from(advance) / units * size
            })
            .sum()
    }

    fn line_height(&self, size: f32) -> f32 {
        size * LINE_FACTOR
    }
}

struct Layout<'a> {
    doc: PdfDocument,
    pages: Vec<Vec<Op>>,
    ops: Vec<Op>,
    y: f32,
    metrics: Metrics,
    labels: &'a ExportLabels,
}

impl<'a> Layout<'a> {
    fn new(doc: PdfDocument, labels: &'a ExportLabels) -> Self {
        Self {
            doc,
            pages: Vec::new(),
            ops: Vec::new(),
            y: CONTENT_TOP,
            metrics: Metrics::new(),
            labels,
        }
    }

    fn new_page(&mut self) {
        self.pages.push(std::mem::take(&mut self.ops));
        self.y = CONTENT_TOP;
    }

    fn has_content(&self) -> bool {
        !self.ops.is_empty()
    }

    fn ensure_space(&mut self, height: f32) {
        if self.y + height > CONTENT_BOTTOM && self.has_content() {
            self.new_page();
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if !self.ops.is_empty() || self.pages.is_empty() {
            self.pages.push(std::mem::take(&mut self.ops));
        }

        let total = self.pages.len();
        let footers = (0..total)
            .map(|index| {
                let label = format!("{} / {}", index + 1, total);
                let size = 8.5;
                let width = self.metrics.width(false, size, &label);

                self.text_ops(
                    &label,
                    false,
                    size,
                    muted_color(),
                    PAGE_WIDTH - MARGIN - width,
                    FOOTER_Y,
                )
            })
            .collect::<Vec<_>>();

        for (page, footer) in self.pages.iter_mut().zip(footers) {
            page.extend(footer);
        }

        let pages = self
            .pages
            .into_iter()
            .map(|ops| PdfPage::new(Mm(210.0), Mm(297.0), ops))
            .collect();

        self.doc
            .with_pages(pages)
            .save(&PdfSaveOptions::default(), &mut Vec::new())
    }

    fn text_ops(
        &self,
        text: &str,
        bold: bool,
        size: f32,
        color: Color,
        x: f32,
        top: f32,
    ) -> Vec<Op> {
        let font = if bold {
            BuiltinFont::HelveticaBold
        } else {
            BuiltinFont::Helvetica
        };
        let baseline = top + size * 0.82;

        vec![
            Op::StartTextSection,
            Op::SetTextCursor {
                pos: Point {
                    x: Pt(x),
                    y: Pt(PAGE_HEIGHT - baseline),
                },
            },
            Op::SetFont {
                font: PdfFontHandle::Builtin(font),
                size: Pt(size),
            },
            Op::SetLineHeight {
                lh: Pt(self.metrics.line_height(size)),
            },
            Op::SetFillColor { col: color },
            Op::ShowText {
                items: vec![TextItem::Text(text.to_string())],
            },
            Op::EndTextSection,
        ]
    }

    fn render_line(&mut self, text: &str, bold: bool, size: f32, color: Color, x: f32, top: f32) {
        self.ops
            .extend(self.text_ops(text, bold, size, color, x, top));
    }

    fn wrap(&self, text: &str, bold: bool, size: f32, max_width: f32) -> Vec<String> {
        let mut lines = Vec::new();
        // Fluent can insert bidi isolation marks around placeables. They are
        // meaningful in HTML but would become `?` in the built-in fonts.
        let text = text
            .chars()
            .filter(|character| !is_invisible(*character))
            .collect::<String>();

        for paragraph in text.split('\n') {
            if paragraph.trim().is_empty() {
                lines.push(String::new());
                continue;
            }

            let mut current = String::new();

            for word in paragraph.split_whitespace() {
                let candidate = if current.is_empty() {
                    word.to_string()
                } else {
                    format!("{current} {word}")
                };

                if self.metrics.width(bold, size, &candidate) <= max_width {
                    current = candidate;
                    continue;
                }

                if !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                }

                if self.metrics.width(bold, size, word) <= max_width {
                    current = word.to_string();
                    continue;
                }

                for character in word.chars() {
                    let mut trial = current.clone();
                    trial.push(character);

                    if self.metrics.width(bold, size, &trial) > max_width && !current.is_empty() {
                        lines.push(std::mem::take(&mut current));
                    }

                    current.push(character);
                }
            }

            if !current.is_empty() {
                lines.push(current);
            }
        }

        if lines.is_empty() {
            lines.push(String::new());
        }

        lines
    }

    fn heading(
        &mut self,
        text: &str,
        size: f32,
        space_before: f32,
        space_after: f32,
        bold: bool,
        color: Color,
    ) {
        let lines = self.wrap(text, bold, size, CONTENT_WIDTH);
        let line_height = self.metrics.line_height(size);
        let height = lines.len() as f32 * line_height;

        self.ensure_space(space_before + height + space_after);
        self.y += space_before;

        for line in lines {
            self.render_line(&line, bold, size, color.clone(), MARGIN, self.y);
            self.y += line_height;
        }

        self.y += space_after;
    }

    fn paragraph_with_style(
        &mut self,
        text: &str,
        bold: bool,
        size: f32,
        color: Color,
        space_after: f32,
    ) {
        let lines = self.wrap(text, bold, size, CONTENT_WIDTH);
        let line_height = self.metrics.line_height(size);

        for line in lines {
            self.ensure_space(line_height);
            self.render_line(&line, bold, size, color.clone(), MARGIN, self.y);
            self.y += line_height;
        }

        self.y += space_after;
    }

    fn paragraph(&mut self, text: &str, size: f32, color: Color, space_after: f32) {
        self.paragraph_with_style(text, false, size, color, space_after);
    }

    fn ingredients(&mut self, ingredients: &[ExportIngredient]) {
        if ingredients.is_empty() {
            return;
        }

        self.heading(
            &self.labels.ingredients,
            14.0,
            14.0,
            6.0,
            true,
            body_color(),
        );

        let quantity_width = ingredients
            .iter()
            .map(|ingredient| self.metrics.width(false, BODY_SIZE, &ingredient.quantity))
            .fold(0.0_f32, f32::max);
        let name_width = (CONTENT_WIDTH - quantity_width - 16.0).max(80.0);
        let line_height = self.metrics.line_height(BODY_SIZE);

        for ingredient in ingredients {
            let name_lines = self.wrap(&ingredient.name, false, BODY_SIZE, name_width);
            let row_height = name_lines.len() as f32 * line_height;

            self.ensure_space(row_height + 5.0);

            for (index, line) in name_lines.iter().enumerate() {
                let top = self.y + index as f32 * line_height;
                self.render_line(line, false, BODY_SIZE, body_color(), MARGIN, top);

                if index == 0 {
                    let quantity_x = PAGE_WIDTH - MARGIN - quantity_width;
                    let name_end = MARGIN + self.metrics.width(false, BODY_SIZE, line) + 6.0;

                    if quantity_x - name_end > 14.0 {
                        self.leader_line(name_end, quantity_x - 6.0, top);
                    }

                    self.render_line(
                        &ingredient.quantity,
                        false,
                        BODY_SIZE,
                        muted_color(),
                        quantity_x,
                        top,
                    );
                }
            }

            self.y += row_height + 5.0;
        }

        self.y += 6.0;
    }

    fn steps(&mut self, steps: &[ExportStep], options: &ExportOptions) {
        if steps.is_empty() {
            return;
        }

        self.heading(&self.labels.steps, 14.0, 8.0, 6.0, true, body_color());

        for (index, step) in steps.iter().enumerate() {
            let heading = match step.title.as_deref() {
                Some(title) if !title.trim().is_empty() => {
                    format!("{} {}: {}", self.labels.step, index + 1, title)
                }
                _ => format!("{} {}", self.labels.step, index + 1),
            };
            let heading_lines = self.wrap(&heading, true, 11.5, CONTENT_WIDTH);
            let heading_height = heading_lines.len() as f32 * self.metrics.line_height(11.5) + 4.0;

            self.ensure_space(heading_height + self.metrics.line_height(BODY_SIZE) * 2.0);

            for line in heading_lines {
                self.render_line(&line, true, 11.5, body_color(), MARGIN, self.y);
                self.y += self.metrics.line_height(11.5);
            }
            self.y += 3.0;

            self.paragraph(&step.instruction, BODY_SIZE, body_color(), 7.0);

            if options.include_images && !step.images.is_empty() {
                self.image_grid(&step.images, 2, 150.0);
            }

            self.y += 7.0;
        }
    }

    fn image_grid(&mut self, images: &[ExportImage], columns: usize, max_height: f32) {
        let columns = columns.max(1);
        let gap = 8.0;
        let cell_width =
            (CONTENT_WIDTH - gap * (columns.saturating_sub(1) as f32)) / columns as f32;

        struct Placed {
            raw: RawImage,
            width: f32,
            height: f32,
        }

        let mut placed = Vec::new();

        for image in images {
            let Ok(raw) = RawImage::decode_from_bytes(&image.bytes, &mut Vec::new()) else {
                continue;
            };

            if raw.width == 0 || raw.height == 0 {
                continue;
            }

            let natural_width = raw.width as f32;
            let natural_height = raw.height as f32;
            let scale = (cell_width / natural_width).min(max_height / natural_height);
            let width = natural_width * scale;
            let height = natural_height * scale;

            placed.push(Placed { raw, width, height });
        }

        for row in placed.chunks(columns) {
            let row_height = row.iter().map(|image| image.height).fold(0.0_f32, f32::max);

            self.ensure_space(row_height + gap);

            for (column, image) in row.iter().enumerate() {
                let x =
                    MARGIN + column as f32 * (cell_width + gap) + (cell_width - image.width) / 2.0;
                let top = self.y + (row_height - image.height) / 2.0;

                let id = self.doc.add_image(&image.raw);
                self.ops.push(Op::UseXobject {
                    id,
                    transform: XObjectTransform {
                        translate_x: Some(Pt(x)),
                        translate_y: Some(Pt(PAGE_HEIGHT - top - image.height)),
                        scale_x: Some(image.width / image.raw.width as f32),
                        scale_y: Some(image.height / image.raw.height as f32),
                        dpi: Some(72.0),
                        ..Default::default()
                    },
                });
            }

            self.y += row_height + gap;
        }

        self.y += 4.0;
    }

    fn leader_line(&mut self, from: f32, to: f32, top: f32) {
        if to <= from {
            return;
        }

        let y = PAGE_HEIGHT - (top + BODY_SIZE * 0.82 - 1.0);

        self.ops.push(Op::SetOutlineColor { col: rule_color() });
        self.ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });
        self.ops.push(Op::SetLineDashPattern {
            dash: LineDashPattern::new(0.0, &[1.0, 2.5]),
        });
        self.ops.push(Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint {
                        p: Point {
                            x: Pt(from),
                            y: Pt(y),
                        },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point {
                            x: Pt(to),
                            y: Pt(y),
                        },
                        bezier: false,
                    },
                ],
                is_closed: false,
            },
        });
        self.ops.push(Op::SetLineDashPattern {
            dash: LineDashPattern::solid(),
        });
    }

    fn divider(&mut self) {
        self.ensure_space(12.0);
        self.y += 6.0;
        self.ops.push(Op::SetOutlineColor { col: rule_color() });
        self.ops.push(Op::DrawLine {
            line: printpdf::Line {
                points: vec![
                    printpdf::LinePoint {
                        p: Point {
                            x: Pt(MARGIN),
                            y: Pt(PAGE_HEIGHT - self.y),
                        },
                        bezier: false,
                    },
                    printpdf::LinePoint {
                        p: Point {
                            x: Pt(PAGE_WIDTH - MARGIN),
                            y: Pt(PAGE_HEIGHT - self.y),
                        },
                        bezier: false,
                    },
                ],
                is_closed: false,
            },
        });
        self.y += 6.0;
    }

    fn sources(&mut self, sources: &[ExportSource]) {
        if sources.is_empty() {
            return;
        }

        self.heading(&self.labels.sources, 16.0, 22.0, 8.0, true, body_color());

        let images = sources
            .iter()
            .filter_map(|source| match source {
                ExportSource::Image(bytes) => Some(ExportImage {
                    bytes: bytes.clone(),
                }),
                ExportSource::Pdf { .. } => None,
            })
            .collect::<Vec<_>>();

        if !images.is_empty() {
            self.image_grid(&images, 2, 220.0);
        }

        for source in sources {
            if let ExportSource::Pdf { name } = source {
                self.paragraph(&format!("• {name}"), BODY_SIZE, body_color(), 3.0);
            }
        }
    }
}

pub(crate) fn render(
    recipe: &ExportRecipe,
    options: &ExportOptions,
    labels: &ExportLabels,
) -> Result<Vec<u8>, String> {
    let mut doc = PdfDocument::new(&recipe.title);
    doc.metadata.info.creator = "Cookbook".to_string();
    doc.metadata.info.producer = "Cookbook".to_string();
    doc.metadata.info.subject = recipe.description.clone().unwrap_or_default();

    if !options.hide_author {
        if let Some(author) = &recipe.author_name {
            doc.metadata.info.author = author.clone();
        }
    }

    let mut layout = Layout::new(doc, labels);

    layout.heading(&recipe.title, 24.0, 0.0, 6.0, true, body_color());

    if let Some(category) = &recipe.category {
        layout.paragraph(category, 9.5, muted_color(), 10.0);
    }

    let mut metadata = Vec::new();

    if !options.hide_author {
        if let Some(author) = &recipe.author_name {
            metadata.push(format!("{}: {author}", labels.author));
        }
    }

    if let Some(minutes) = recipe.total_minutes.filter(|minutes| *minutes > 0) {
        metadata.push(format!(
            "{}: {minutes} {}",
            labels.total_time, labels.minutes
        ));
    }

    if let Some(servings) = &recipe.servings {
        metadata.push(format!("{}: {servings}", labels.servings));
    }

    if !metadata.is_empty() {
        layout.paragraph(&metadata.join("  ·  "), 9.5, muted_color(), 3.0);
    }

    if options.show_timestamp {
        if let Some(timestamp) = &recipe.revision_timestamp {
            layout.paragraph(
                &format!("{}: {timestamp}", labels.revision),
                9.5,
                muted_color(),
                8.0,
            );
        }
    }

    if let Some(description) = &recipe.description {
        layout.paragraph(description, 11.0, body_color(), 10.0);
    }

    for (index, part) in recipe.parts.iter().enumerate() {
        render_part(&mut layout, part, options, index + 1 < recipe.parts.len());
    }

    if options.include_sources {
        layout.sources(&recipe.sources);
    }

    Ok(layout.finish())
}

fn is_invisible(character: char) -> bool {
    character != '\n'
        && (character.is_control()
            || matches!(
                character as u32,
                0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x206F | 0xFEFF
            ))
}

fn render_part(layout: &mut Layout<'_>, part: &ExportPart, options: &ExportOptions, divider: bool) {
    if let Some(title) = &part.title {
        layout.heading(title, 16.0, 16.0, 4.0, true, body_color());
    }

    if let Some(description) = &part.description {
        layout.paragraph(description, 10.5, muted_color(), 8.0);
    }

    if options.include_images && !part.images.is_empty() {
        layout.image_grid(&part.images, 2, 240.0);
    }

    layout.ingredients(&part.ingredients);
    layout.steps(&part.steps, options);

    if divider {
        layout.divider();
    }
}
