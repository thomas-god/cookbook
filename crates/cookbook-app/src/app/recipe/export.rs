use std::collections::{HashMap, HashSet};

use cookbook_domain::recipe::models::{
    RecipeBody, RecipeId as RecipeIdValue, RecipeImageId, RecipeImageId as RecipeImageIdValue,
    RecipeRevision, RecipeSourceDocumentId as RecipeSourceDocumentIdValue,
    RecipeSourceDocumentSummary, RecipeWithContent,
};
use cookbook_domain::recipe::ports::RecipeService;
use cookbook_domain::user::models::UserId;
use cookbook_export::{
    ExportImage, ExportIngredient, ExportLabels, ExportOptions, ExportPart, ExportRecipe,
    ExportSource, ExportStep,
};
use topcoat::{
    Result,
    context::{Cx, context},
    router::{
        Body,
        content::Form,
        error::{RouterErrorExt, bad_request, internal_server_error},
        href, page, query_params,
        response::Response,
        route,
    },
    view::{View, attributes, view},
};

use crate::app::recipe::view::recipe_detail;
use crate::{
    app::{
        AppRecipeService, auth,
        recipes::{display_name, format_timestamp, recipe_id, servings_label},
    },
    components::{
        base::{
            button::{ButtonSize, ButtonVariant, button, button_variants},
            card::{card, card_content, card_description, card_header, card_title},
        },
        recipe::recipe_view::quantity_label,
    },
    t,
};

#[query_params(error = bad_request)]
struct ExportQuery {
    revision: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ExportForm {
    include_images: bool,
    include_sources: bool,
    hide_author: bool,
    show_timestamp: bool,
    revision_id: Option<String>,
}

impl ExportForm {
    fn parse(pairs: Vec<(String, String)>) -> topcoat::Result<Self> {
        let mut form = Self::default();
        let mut seen = HashSet::new();

        for (key, value) in pairs {
            if !seen.insert(key.clone()) {
                return Err(bad_request(format!("duplicate form field `{key}`")).into());
            }

            match key.as_str() {
                "images" => form.include_images = parse_checkbox(&key, &value)?,
                "sources" => form.include_sources = parse_checkbox(&key, &value)?,
                "hide_author" => form.hide_author = parse_checkbox(&key, &value)?,
                "show_timestamp" => form.show_timestamp = parse_checkbox(&key, &value)?,
                "revision_id" => {
                    let revision = value.trim();
                    if revision.is_empty() {
                        return Err(bad_request("revision id is required").into());
                    }
                    form.revision_id = Some(revision.to_string());
                }
                _ => return Err(bad_request(format!("unknown form field `{key}`")).into()),
            }
        }

        Ok(form)
    }

    fn options(&self) -> ExportOptions {
        ExportOptions {
            include_images: self.include_images,
            include_sources: self.include_sources,
            hide_author: self.hide_author,
            show_timestamp: self.show_timestamp,
        }
    }
}

fn parse_checkbox(key: &str, value: &str) -> topcoat::Result<bool> {
    match value {
        "true" | "on" => Ok(true),
        "false" => Ok(false),
        _ => Err(bad_request(format!("invalid value for `{key}`")).into()),
    }
}

#[page("/recipe/{recipe_id}/export")]
pub async fn export_recipe(cx: &Cx) -> Result<impl View> {
    let user = auth::require_user(cx)?;
    let query = query_params::<ExportQuery>(cx)?;
    let recipe_id = recipe_id(cx).to_string();
    let recipe_service: &AppRecipeService = context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id.clone()), user.id().clone())
        .await?
        .ok_or_not_found()?;
    let revision = select_revision(cx, user.id(), &recipe, query.revision.as_deref()).await?;

    let action = href!(export_recipe, recipe_id: recipe_id.as_str()).resolve(cx);
    let back = href!(recipe_detail, recipe_id: recipe_id.as_str()).resolve(cx);
    let title = revision.content().title().as_str().to_string();
    let timestamp = format_timestamp(revision.edited_at());
    let revision_value =
        (revision.id() != recipe.revision().id()).then(|| revision.id().as_str().to_string());

    Ok(view! {
        <div class="flex flex-col gap-6">
            <a
                class="text-sm text-muted-foreground underline-offset-4 hover:underline"
                href=(back.clone())
            >
                (t!(recipe_export_back))
            </a>
            card(
                card_header(
                    card_title((t!(recipe_export_heading)))
                    card_description((t!(recipe_export_description)))
                )
                card_content(
                    <form
                        method="post"
                        action=(action)
                        class="flex flex-col gap-4"
                        onsubmit="cookbook.downloadForm(event)"
                        data-download-error=(t!(recipe_export_failed))
                    >
                        <div class="flex flex-col gap-1">
                            <p class="font-medium">(title)</p>
                            <p class="text-sm text-muted-foreground">
                                (t!(recipe_export_revision_info, timestamp: timestamp))
                            </p>
                        </div>
                        if let Some(revision) = revision_value {
                            <input type="hidden" name="revision_id" value=(revision)>
                        }
                        <label class="flex items-center gap-2 text-sm">
                            <input
                                type="checkbox"
                                name="images"
                                value="true"
                                checked=""
                            >
                            <span>(t!(recipe_export_images))</span>
                        </label>
                        <label class="flex items-center gap-2 text-sm">
                            <input type="checkbox" name="sources" value="true">
                            <span>(t!(recipe_export_sources))</span>
                        </label>
                        <label class="flex items-center gap-2 text-sm">
                            <input type="checkbox" name="hide_author" value="true">
                            <span>(t!(recipe_export_hide_author))</span>
                        </label>
                        <label class="flex items-center gap-2 text-sm">
                            <input
                                type="checkbox"
                                name="show_timestamp"
                                value="true"
                                checked=""
                            >
                            <span>(t!(recipe_export_show_timestamp))</span>
                        </label>
                        <div class="flex items-center gap-2">
                            button(
                                attrs: attributes! { type="submit" data-download-submit="" },
                                <span
                                    data-download-spinner=""
                                    aria-hidden="true"
                                    class="hidden size-4 animate-spin rounded-full border-2 border-current border-t-transparent"
                                ></span>
                                (t!(recipe_export_submit))
                            )
                            <a
                                class=(button_variants(ButtonVariant::Ghost, ButtonSize::Md))
                                href=(back)
                            >
                                (t!(form_cancel))
                            </a>
                        </div>
                    </form>
                )
            )
        </div>
    })
}

#[route(POST "/recipe/{recipe_id}/export")]
pub async fn export_recipe_pdf(
    cx: &Cx,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let user = auth::require_user(cx)?;
    let form = ExportForm::parse(form)?;
    let options = form.options();
    let recipe_id = recipe_id(cx).to_string();
    let recipe_service: &AppRecipeService = context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id), user.id().clone())
        .await?
        .ok_or_not_found()?;
    let revision = select_revision(cx, user.id(), &recipe, form.revision_id.as_deref()).await?;
    let export_data =
        build_export_recipe(cx, recipe_service, user.id(), &recipe, &revision, options).await?;
    let labels = export_labels(cx);

    let recipe_title = export_data.title.clone();
    let bytes = tokio::task::spawn_blocking(move || {
        cookbook_export::render_recipe(&export_data, &options, &labels)
    })
    .await
    .map_err(internal_server_error)?
    .map_err(internal_server_error)?;

    Ok(Response::builder()
        .header("Content-Type", "application/pdf")
        .header("Content-Disposition", attachment_file_name(&recipe_title))
        .header("Cache-Control", "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(bytes))?)
}

async fn select_revision(
    cx: &Cx,
    viewer: &UserId,
    recipe: &RecipeWithContent,
    revision_id: Option<&str>,
) -> Result<RecipeRevision> {
    let Some(revision_id) = revision_id else {
        return Ok(recipe.revision().clone());
    };

    let recipe_service: &AppRecipeService = context(cx);
    let revisions = recipe_service
        .get_recipe_history(recipe.id().clone(), viewer.clone())
        .await?;

    Ok(revisions
        .into_iter()
        .find(|revision| revision.id().as_str() == revision_id)
        .ok_or_not_found()?)
}

async fn build_export_recipe(
    cx: &Cx,
    recipe_service: &AppRecipeService,
    user_id: &UserId,
    recipe: &RecipeWithContent,
    revision: &RecipeRevision,
    options: ExportOptions,
) -> Result<ExportRecipe> {
    let content = revision.content();
    let author_name = if options.hide_author {
        None
    } else {
        Some(display_name(cx, recipe.author()).await?)
    };

    let mut images = HashMap::new();

    if options.include_images {
        let metadata = recipe_service
            .get_recipe_images(recipe.id().clone(), user_id.clone())
            .await?;

        for image in metadata {
            let id = image.id().as_str().to_string();
            if let Some(image) = recipe_service
                .get_recipe_image(
                    recipe.id().clone(),
                    RecipeImageIdValue::new(id.clone()),
                    user_id.clone(),
                )
                .await?
            {
                images.insert(id, image.bytes().to_vec());
            }
        }
    }

    let parts = match content.simple_body() {
        Some(body) => vec![build_part(cx, None, None, body, &images)],
        None => content
            .parts()
            .iter()
            .map(|part| {
                build_part(
                    cx,
                    Some(part.title().as_str().to_string()),
                    part.description()
                        .map(|description| description.as_str().to_string()),
                    part.body(),
                    &images,
                )
            })
            .collect(),
    };

    let sources = if options.include_sources {
        let documents = recipe_service
            .get_source_documents(recipe.id().clone(), user_id.clone())
            .await?;
        build_sources(recipe_service, user_id, recipe, documents).await?
    } else {
        Vec::new()
    };

    let total_minutes = content.durations().total_duration().as_minutes();

    Ok(ExportRecipe {
        title: content.title().as_str().to_string(),
        description: content
            .description()
            .map(|description| description.as_str().to_string()),
        category: content
            .category()
            .map(|category| crate::components::recipe::category::category_label(cx, category)),
        author_name,
        total_minutes: (total_minutes > 0).then_some(total_minutes),
        servings: content
            .servings()
            .map(|servings| servings_label(cx, servings)),
        revision_timestamp: Some(format_timestamp(revision.edited_at())),
        parts,
        sources,
    })
}

fn build_part(
    cx: &Cx,
    title: Option<String>,
    description: Option<String>,
    body: &RecipeBody,
    images: &HashMap<String, Vec<u8>>,
) -> ExportPart {
    let mut ingredients = body
        .ingredients()
        .iter()
        .map(|ingredient| ExportIngredient {
            name: ingredient.name().as_str().to_string(),
            quantity: quantity_label(cx, ingredient.quantity()),
        })
        .collect::<Vec<_>>();
    ingredients.sort_by(|left, right| left.name.cmp(&right.name));

    ExportPart {
        title,
        description,
        images: map_images(body.images(), images),
        ingredients,
        steps: body
            .steps()
            .iter()
            .map(|step| ExportStep {
                title: step.title().map(str::to_string),
                instruction: step.instruction().to_string(),
                images: map_images(step.images(), images),
            })
            .collect(),
    }
}

fn map_images(ids: &[RecipeImageId], images: &HashMap<String, Vec<u8>>) -> Vec<ExportImage> {
    ids.iter()
        .filter_map(|id| {
            images.get(id.as_str()).map(|bytes| ExportImage {
                bytes: bytes.clone(),
            })
        })
        .collect()
}

async fn build_sources(
    recipe_service: &AppRecipeService,
    user_id: &UserId,
    recipe: &RecipeWithContent,
    documents: Vec<RecipeSourceDocumentSummary>,
) -> Result<Vec<ExportSource>> {
    let mut sources = Vec::new();

    for document in documents {
        if document.content_type() == "application/pdf" {
            sources.push(ExportSource::Pdf {
                name: document
                    .original_name()
                    .filter(|name| !name.is_empty())
                    .unwrap_or("document.pdf")
                    .to_string(),
            });
            continue;
        }

        if let Some(document) = recipe_service
            .get_source_document(
                recipe.id().clone(),
                RecipeSourceDocumentIdValue::new(document.id().as_str().to_string()),
                user_id.clone(),
            )
            .await?
        {
            sources.push(ExportSource::Image(document.bytes().to_vec()));
        }
    }

    Ok(sources)
}

fn export_labels(cx: &Cx) -> ExportLabels {
    ExportLabels {
        author: t!(recipe_author).resolve(cx).into_owned(),
        total_time: t!(recipe_total_duration).resolve(cx).into_owned(),
        minutes: t!(recipe_minutes).resolve(cx).into_owned(),
        servings: t!(recipe_servings).resolve(cx).into_owned(),
        revision: t!(recipe_export_revision).resolve(cx).into_owned(),
        ingredients: t!(recipe_ingredients).resolve(cx).into_owned(),
        steps: t!(recipe_steps).resolve(cx).into_owned(),
        step: t!(recipe_step).resolve(cx).into_owned(),
        sources: t!(source_documents_heading).resolve(cx).into_owned(),
    }
}

fn attachment_file_name(title: &str) -> String {
    let mut slug = String::new();

    for character in title.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if (character.is_whitespace() || character == '-' || character == '_')
            && !slug.is_empty()
            && !slug.ends_with('-')
        {
            slug.push('-');
        }
    }

    let slug = slug.trim_matches('-');
    let name = if slug.is_empty() {
        "recipe.pdf".to_string()
    } else {
        format!("{slug}.pdf")
    };

    format!("attachment; filename=\"{name}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(entries: &[(&str, &str)]) -> Vec<(String, String)> {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn parses_the_export_options() {
        let form = ExportForm::parse(pairs(&[
            ("images", "true"),
            ("sources", "true"),
            ("hide_author", "true"),
            ("revision_id", " rev-1 "),
        ]))
        .unwrap();

        assert!(form.include_images);
        assert!(form.include_sources);
        assert!(form.hide_author);
        assert!(!form.show_timestamp);
        assert_eq!(form.revision_id.as_deref(), Some("rev-1"));
    }

    #[test]
    fn unchecks_unselected_options() {
        let form = ExportForm::parse(pairs(&[("images", "true"), ("sources", "false")])).unwrap();

        assert!(form.include_images);
        assert!(!form.include_sources);
        assert!(!form.hide_author);
        assert!(!form.show_timestamp);
    }

    #[test]
    fn rejects_unknown_duplicate_and_invalid_fields() {
        assert!(ExportForm::parse(pairs(&[("unknown", "true")])).is_err());
        assert!(ExportForm::parse(pairs(&[("images", "true"), ("images", "true")])).is_err());
        assert!(ExportForm::parse(pairs(&[("images", "yes")])).is_err());
        assert!(ExportForm::parse(pairs(&[("revision_id", "  ")])).is_err());
    }

    #[test]
    fn builds_a_safe_pdf_file_name() {
        assert_eq!(
            attachment_file_name("Tarte aux pommes"),
            "attachment; filename=\"tarte-aux-pommes.pdf\""
        );
        assert_eq!(
            attachment_file_name("Grandma's \"best\" cake!"),
            "attachment; filename=\"grandmas-best-cake.pdf\""
        );
        assert_eq!(
            attachment_file_name("   "),
            "attachment; filename=\"recipe.pdf\""
        );
        assert_eq!(
            attachment_file_name("Crème brûlée"),
            "attachment; filename=\"crme-brle.pdf\""
        );
    }

    #[test]
    fn prefers_an_ascii_file_name() {
        assert!(attachment_file_name("Café").is_ascii());
    }
}
