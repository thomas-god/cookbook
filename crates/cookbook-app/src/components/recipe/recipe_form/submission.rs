use cookbook_domain::{
    recipe::{
        models::{
            NewRecipeImage, RecipeId as RecipeIdValue, RecipeImageId, RecipeRevisionId,
            RevisionMessage, RevisionVerification,
        },
        ports::{CreateRecipeRequest, UpdateRecipeRequest},
    },
    user::models::UserId,
};
use cookbook_vision::detect_image_content_type;
use topcoat::{Result, router::content::multipart::Multipart};

use super::{
    fields::{IMAGES_FIELD, StepField},
    parse::{
        BodyFormData, RecipeFormData, StructureFormData, indexed_row, invalid, parse_part_key,
        parse_recipe_form,
    },
};

fn parse_recipe_uploads(
    form: &mut RecipeFormData,
    uploads: Vec<RecipeFormUpload>,
) -> Result<Vec<NewRecipeImage>> {
    let mut images = Vec::with_capacity(uploads.len());

    for upload in uploads {
        let content_type = detect_image_content_type(&upload.bytes).ok_or_else(|| {
            invalid(format!(
                "image upload `{}` is not a supported JPEG or PNG image",
                upload.field
            ))
        })?;
        let id = RecipeImageId::generate();
        image_upload_target(form, &upload.field)?.push(id.clone());

        images.push(NewRecipeImage::new(
            id,
            content_type.to_string(),
            upload.bytes,
        ));
    }

    Ok(images)
}

fn image_upload_target<'a>(
    form: &'a mut RecipeFormData,
    upload_field: &str,
) -> Result<&'a mut Vec<RecipeImageId>> {
    let (part_index, field) = match parse_part_key(upload_field)? {
        Some((part_index, rest)) => (Some(part_index), rest),
        None => (None, upload_field.to_string()),
    };

    let body = upload_body(form, part_index, upload_field)?;

    if field == IMAGES_FIELD {
        return Ok(&mut body.images);
    }

    if let Some(row) = indexed_row(&field, "steps", StepField::Images.wire_name())? {
        return body
            .steps
            .iter_mut()
            .find(|step| step.row == row)
            .map(|step| &mut step.images)
            .ok_or_else(|| {
                invalid(format!(
                    "image upload `{upload_field}` targets an unknown step row"
                ))
            });
    }

    Err(invalid(format!(
        "unknown image upload field `{upload_field}`"
    )))
}

fn upload_body<'a>(
    form: &'a mut RecipeFormData,
    part_index: Option<usize>,
    upload_field: &str,
) -> Result<&'a mut BodyFormData> {
    match (part_index, &mut form.structure) {
        (None, StructureFormData::Simple(body)) => Ok(body),
        (None, StructureFormData::Multipart(_)) => Err(invalid(format!(
            "image upload `{upload_field}` targets the recipe body of a multipart recipe"
        ))),
        (Some(_), StructureFormData::Simple(_)) => Err(invalid(format!(
            "image upload `{upload_field}` targets a part on a simple recipe"
        ))),
        (Some(part_index), StructureFormData::Multipart(parts)) => parts
            .iter_mut()
            .find(|part| part.row == part_index)
            .map(|part| &mut part.body)
            .ok_or_else(|| {
                invalid(format!(
                    "image upload `{upload_field}` targets an unknown part row"
                ))
            }),
    }
}

pub(crate) struct RecipeFormSubmission {
    pub fields: Vec<(String, String)>,
    pub uploads: Vec<RecipeFormUpload>,
}

pub(crate) struct RecipeFormUpload {
    field: String,
    bytes: Vec<u8>,
}

pub(crate) async fn read_recipe_form(multipart: &mut Multipart) -> Result<RecipeFormSubmission> {
    let mut fields = Vec::new();
    let mut uploads = Vec::new();

    while let Some(part) = multipart.next_field().await? {
        let Some(field_name) = part.name().map(str::to_owned) else {
            continue;
        };

        let file_name = part.file_name().map(str::to_owned);

        if let Some(file_name) = file_name {
            let bytes = part.bytes().await?;

            if file_name.is_empty() && bytes.is_empty() {
                continue;
            }

            uploads.push(RecipeFormUpload {
                field: field_name,
                bytes: bytes.to_vec(),
            });
        } else {
            fields.push((field_name, part.text().await?));
        }
    }

    Ok(RecipeFormSubmission { fields, uploads })
}

pub(crate) fn create_recipe_request(
    submission: RecipeFormSubmission,
    author: UserId,
) -> Result<CreateRecipeRequest> {
    let mut form = parse_recipe_form(&submission.fields)?;
    let images = parse_recipe_uploads(&mut form, submission.uploads)?;

    Ok(CreateRecipeRequest {
        content: form.into_content()?,
        author,
        source_images: Vec::new(),
        images,
        verification: RevisionVerification::Verified,
    })
}

pub(crate) fn update_recipe_request(
    submission: RecipeFormSubmission,
    id: RecipeIdValue,
    editor: UserId,
) -> Result<UpdateRecipeRequest> {
    let mut form = parse_recipe_form(&submission.fields)?;
    let images = parse_recipe_uploads(&mut form, submission.uploads)?;

    let latest_revision = form
        .latest_revision
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid("latest revision is required"))?
        .to_string();
    let message = form
        .message
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|message| RevisionMessage::new(message).map_err(|error| invalid(error.to_string())))
        .transpose()?;

    Ok(UpdateRecipeRequest {
        id,
        content: form.into_content()?,
        editor,
        latest_revision: RecipeRevisionId::new(latest_revision),
        message,
        new_images: images,
    })
}

#[cfg(test)]
mod tests {
    use cookbook_domain::recipe::models::{
        DurationMinutes, RecipeCategory, RecipeDescription, RevisionVerification,
    };

    use super::*;

    fn pairs(entries: &[(&str, &str)]) -> Vec<(String, String)> {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn submission(entries: &[(&str, &str)]) -> RecipeFormSubmission {
        RecipeFormSubmission {
            fields: pairs(entries),
            uploads: Vec::new(),
        }
    }

    #[test]
    fn converts_the_form_into_a_create_request() {
        let request = create_recipe_request(
            submission(&[
                ("title", "Soup"),
                ("description", "A warming soup."),
                ("category", "main"),
                ("ingredients[0][name]", "Flour"),
                ("ingredients[0][quantity]", "200"),
                ("ingredients[0][unit]", "gram"),
                ("steps[0][instruction]", "Stir."),
            ]),
            UserId::new("user-1".to_string()),
        )
        .unwrap();

        assert_eq!(request.content.title().as_str(), "Soup");
        assert_eq!(request.author.as_str(), "user-1");
        assert_eq!(request.verification, RevisionVerification::Verified);
        assert_eq!(
            request.content.description().map(RecipeDescription::as_str),
            Some("A warming soup.")
        );
        assert_eq!(request.content.category(), Some(RecipeCategory::Main));
        let body = request.content.simple_body().unwrap();
        assert_eq!(body.ingredients().len(), 1);
        assert_eq!(body.steps().len(), 1);
    }

    #[test]
    fn converts_the_form_into_an_update_request() {
        let request = update_recipe_request(
            submission(&[
                ("title", "Soup"),
                ("message", "reduced the salt"),
                ("latest_revision", "revision-1"),
            ]),
            RecipeIdValue::new("soup".to_string()),
            UserId::new("editor-1".to_string()),
        )
        .unwrap();

        assert_eq!(request.id.as_str(), "soup");
        assert_eq!(request.editor.as_str(), "editor-1");
        assert_eq!(request.latest_revision.as_str(), "revision-1");
        assert_eq!(
            request.message.as_ref().map(RevisionMessage::as_str),
            Some("reduced the salt")
        );
        assert_eq!(request.content.title().as_str(), "Soup");
    }

    #[test]
    fn update_request_requires_a_latest_revision() {
        assert!(
            update_recipe_request(
                submission(&[("title", "Soup")]),
                RecipeIdValue::new("soup".to_string()),
                UserId::new("editor-1".to_string()),
            )
            .is_err()
        );
    }

    #[test]
    fn update_request_treats_a_blank_message_as_missing() {
        let request = update_recipe_request(
            submission(&[
                ("title", "Soup"),
                ("message", "   "),
                ("latest_revision", "revision-1"),
            ]),
            RecipeIdValue::new("soup".to_string()),
            UserId::new("editor-1".to_string()),
        )
        .unwrap();

        assert!(request.message.is_none());
    }

    #[test]
    fn parses_recipe_and_step_image_uploads() {
        let mut form = parse_recipe_form(&pairs(&[
            ("title", "Soup"),
            ("steps[0][id]", "step-1"),
            ("steps[0][instruction]", "Stir."),
        ]))
        .unwrap();
        let png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];

        let images = parse_recipe_uploads(
            &mut form,
            vec![
                RecipeFormUpload {
                    field: IMAGES_FIELD.to_string(),
                    bytes: png.clone(),
                },
                RecipeFormUpload {
                    field: "steps[0][images]".to_string(),
                    bytes: png,
                },
            ],
        )
        .unwrap();

        assert_eq!(images.len(), 2);
        assert_eq!(images[0].content_type(), "image/png");
        assert_eq!(form.simple_body().images, vec![images[0].id().clone()]);
        assert_eq!(
            form.simple_body().steps[0].images,
            vec![images[1].id().clone()]
        );
    }

    #[test]
    fn rejects_an_image_for_an_unknown_step_row() {
        let mut form = parse_recipe_form(&pairs(&[("title", "Soup")])).unwrap();

        let result = parse_recipe_uploads(
            &mut form,
            vec![RecipeFormUpload {
                field: "steps[4][images]".to_string(),
                bytes: vec![0xFF, 0xD8, 0xFF, 1],
            }],
        );

        assert!(result.is_err());
    }

    #[test]
    fn rejects_an_unsupported_image_upload() {
        let mut form = parse_recipe_form(&pairs(&[("title", "Soup")])).unwrap();

        let result = parse_recipe_uploads(
            &mut form,
            vec![RecipeFormUpload {
                field: IMAGES_FIELD.to_string(),
                bytes: b"not an image".to_vec(),
            }],
        );

        assert!(result.is_err());
    }

    #[test]
    fn parses_multipart_image_uploads() {
        let mut form = parse_recipe_form(&pairs(&[
            ("recipe_type", "multipart"),
            ("title", "Cake"),
            ("parts[0][title]", "Sponge"),
            ("parts[1][title]", "Filling"),
            ("parts[1][steps][0][instruction]", "Whip."),
        ]))
        .unwrap();
        let png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];

        let images = parse_recipe_uploads(
            &mut form,
            vec![
                RecipeFormUpload {
                    field: "parts[0][images]".to_string(),
                    bytes: png.clone(),
                },
                RecipeFormUpload {
                    field: "parts[1][steps][0][images]".to_string(),
                    bytes: png,
                },
            ],
        )
        .unwrap();

        let StructureFormData::Multipart(parts) = &form.structure else {
            panic!("expected a multipart form");
        };

        assert_eq!(parts[0].body.images, vec![images[0].id().clone()]);
        assert_eq!(parts[1].body.steps[0].images, vec![images[1].id().clone()]);
    }

    #[test]
    fn multipart_form_builds_multipart_content() {
        let form = parse_recipe_form(&pairs(&[
            ("recipe_type", "multipart"),
            ("title", "Cake"),
            ("parts[0][id]", "part-1"),
            ("parts[0][title]", "Sponge"),
            ("parts[0][preparation]", "10"),
            ("parts[1][id]", "part-2"),
            ("parts[1][title]", "Filling"),
            ("parts[1][cooking]", "5"),
        ]))
        .unwrap();

        let content = form.into_content().unwrap();

        assert!(content.is_multipart());
        assert_eq!(content.parts().len(), 2);
        assert_eq!(content.parts()[0].id().as_str(), "part-1");
        assert_eq!(content.parts()[0].title().as_str(), "Sponge");
        assert_eq!(
            content
                .durations()
                .preparation()
                .map(DurationMinutes::as_minutes),
            Some(10)
        );
        assert_eq!(
            content
                .durations()
                .cooking()
                .map(DurationMinutes::as_minutes),
            Some(5)
        );
    }
}
