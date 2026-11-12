use std::collections::HashMap;

use cookbook_domain::{
    recipe::{
        models::{
            Quantity, RecipeBody, RecipeId as RecipeIdValue, RecipeImageMetadata,
            RecipeWithContent, Servings, Unit,
        },
        ports::RecipeService,
    },
    user::models::UserId,
};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{error::RouterErrorExt, href},
};

use crate::{
    app::{
        AppRecipeService,
        recipes::{ImageId, RecipeId, recipe_image},
    },
    t,
};

use super::fields::{AmountType, DEFAULT_UNIT, ServingsType, unit_wire};

#[derive(Clone, Default)]
pub(super) struct IngredientInitial {
    pub(super) name: String,
    pub(super) amount_type: String,
    pub(super) quantity: String,
    pub(super) quantity_text: String,
    pub(super) unit: String,
    pub(super) unit_other: String,
}

#[derive(Clone, Default)]
pub(super) struct FormImageInitial {
    pub(super) id: String,
    pub(super) url: String,
    pub(super) alt: String,
}

#[derive(Clone, Default)]
pub(super) struct StepInitial {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) instruction: String,
    pub(super) images: Vec<String>,
}

#[derive(Clone, Default)]
pub(super) struct BodyInitial {
    pub(super) preparation: String,
    pub(super) rest: String,
    pub(super) cooking: String,
    pub(super) ingredients: Vec<IngredientInitial>,
    pub(super) steps: Vec<StepInitial>,
    pub(super) images: Vec<FormImageInitial>,
}

#[derive(Clone, Default)]
pub(super) struct PartInitial {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) description: String,
    pub(super) body: BodyInitial,
}

pub(super) enum StructureInitial {
    Simple(BodyInitial),
    Multipart(Vec<PartInitial>),
}

impl Default for StructureInitial {
    fn default() -> Self {
        Self::Simple(BodyInitial::default())
    }
}

#[derive(Default)]
pub(super) struct FormInitial {
    pub(super) title: String,
    pub(super) description: String,
    pub(super) category: String,
    pub(super) servings_type: String,
    pub(super) servings_count: String,
    pub(super) servings_label: String,
    pub(super) structure: StructureInitial,
}

pub(super) fn form_image_initial(
    image: &RecipeImageMetadata,
    recipe_id: &str,
    cx: &Cx,
) -> FormImageInitial {
    FormImageInitial {
        id: image.id().as_str().to_string(),
        url: href!(
            recipe_image,
            RecipeId(recipe_id),
            ImageId(image.id().as_str())
        )
        .resolve(cx),
        alt: t!(recipe_image_alt).resolve(cx).into_owned(),
    }
}

impl BodyInitial {
    fn from_body(body: &RecipeBody, image_lookup: &HashMap<String, FormImageInitial>) -> Self {
        let mut ingredients: Vec<_> = body
            .ingredients()
            .iter()
            .map(|ingredient| ingredient_initial(ingredient.name().as_str(), ingredient.quantity()))
            .collect();
        ingredients.sort_by(|left, right| left.name.cmp(&right.name));

        Self {
            preparation: body
                .durations()
                .preparation()
                .map(|duration| duration.as_minutes().to_string())
                .unwrap_or_default(),
            rest: body
                .durations()
                .rest()
                .map(|duration| duration.as_minutes().to_string())
                .unwrap_or_default(),
            cooking: body
                .durations()
                .cooking()
                .map(|duration| duration.as_minutes().to_string())
                .unwrap_or_default(),
            ingredients,
            steps: body
                .steps()
                .iter()
                .map(|step| StepInitial {
                    id: step.id().as_str().to_string(),
                    title: step.title().unwrap_or_default().to_string(),
                    instruction: step.instruction().to_string(),
                    images: step
                        .images()
                        .iter()
                        .filter(|image_id| image_lookup.contains_key(image_id.as_str()))
                        .map(|image_id| image_id.as_str().to_string())
                        .collect(),
                })
                .collect(),
            images: body
                .images()
                .iter()
                .filter_map(|image_id| image_lookup.get(image_id.as_str()).cloned())
                .collect(),
        }
    }
}

impl FormInitial {
    #[cfg(test)]
    fn simple_body(&self) -> &BodyInitial {
        match &self.structure {
            StructureInitial::Simple(body) => body,
            StructureInitial::Multipart(_) => panic!("expected a simple recipe form"),
        }
    }

    pub(super) fn new(multipart: bool) -> Self {
        let mut initial = Self::default();

        if multipart {
            initial.structure = StructureInitial::Multipart(vec![PartInitial::default()]);
        }

        initial
    }

    pub(super) fn from_recipe(
        recipe: &RecipeWithContent,
        image_lookup: &HashMap<String, FormImageInitial>,
    ) -> Self {
        let (servings_type, servings_count, servings_label) = match recipe.servings() {
            Some(Servings::People(count)) => (
                ServingsType::People.wire_name().to_string(),
                count.to_string(),
                String::new(),
            ),
            Some(Servings::Portions { count, label }) => (
                ServingsType::Portions.wire_name().to_string(),
                count.to_string(),
                label.clone().unwrap_or_default(),
            ),
            None => (
                ServingsType::None.wire_name().to_string(),
                String::new(),
                String::new(),
            ),
        };

        let structure = if let Some(body) = recipe.content().simple_body() {
            StructureInitial::Simple(BodyInitial::from_body(body, image_lookup))
        } else {
            StructureInitial::Multipart(
                recipe
                    .content()
                    .parts()
                    .iter()
                    .map(|part| PartInitial {
                        id: part.id().as_str().to_string(),
                        title: part.title().as_str().to_string(),
                        description: part
                            .description()
                            .map(|description| description.as_str().to_string())
                            .unwrap_or_default(),
                        body: BodyInitial::from_body(part.body(), image_lookup),
                    })
                    .collect(),
            )
        };

        Self {
            title: recipe.title().as_str().to_string(),
            description: recipe
                .description()
                .map(|description| description.as_str().to_string())
                .unwrap_or_default(),
            category: recipe
                .category()
                .map(|category| category.as_str().to_string())
                .unwrap_or_default(),
            servings_type,
            servings_count,
            servings_label,
            structure,
        }
    }
}

pub(super) async fn load_current_recipe(
    cx: &Cx,
    recipe_id: &str,
    user: &UserId,
) -> Result<Option<RecipeWithContent>> {
    if recipe_id.is_empty() {
        return Ok(None);
    }

    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id.to_string()), user.clone())
        .await?;

    Ok(Some(recipe.ok_or_not_found()?))
}

fn ingredient_initial(name: &str, quantity: &Quantity) -> IngredientInitial {
    match quantity {
        Quantity::Measured { value, unit } => {
            let (unit, unit_other) = match unit {
                Unit::Other(label) => (unit_wire(unit), label.clone()),
                unit => (unit_wire(unit), String::new()),
            };

            IngredientInitial {
                name: name.to_string(),
                amount_type: AmountType::Measured.wire_name().to_string(),
                quantity: value.to_string(),
                quantity_text: String::new(),
                unit: unit.to_string(),
                unit_other,
            }
        }
        Quantity::Text(text) => IngredientInitial {
            name: name.to_string(),
            amount_type: AmountType::Text.wire_name().to_string(),
            quantity: String::new(),
            quantity_text: text.clone(),
            unit: DEFAULT_UNIT.to_string(),
            unit_other: String::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use cookbook_domain::{
        recipe::models::{
            DurationMinutes, Ingredient, IngredientName, Recipe, RecipeBody, RecipeCategory,
            RecipeContent, RecipeDescription, RecipeDurations, RecipeRevision, RecipeRevisionId,
            RecipeStep, RecipeStepId, RecipeTitle, RecipeWithContent, RevisionVerification,
            Servings,
        },
        user::models::UserId,
    };

    use super::*;

    #[test]
    fn prefills_form_values_from_a_recipe() {
        let recipe = Recipe::new(
            RecipeIdValue::new("soup".to_string()),
            UserId::new("user-1".to_string()),
        );
        let content = RecipeContent::new_simple(
            RecipeTitle::new("Soup".to_string()),
            Some(RecipeDescription::new("A warming soup.".to_string())),
            Some(Servings::portions_with_label(12, "cookies")),
            Some(RecipeCategory::Main),
            RecipeBody::new(
                RecipeDurations::new(
                    Some(DurationMinutes::new(10)),
                    None,
                    Some(DurationMinutes::new(20)),
                ),
                vec![
                    Ingredient::new(
                        IngredientName::new("milk".to_string()),
                        Quantity::from_measure(0.5, Unit::Litre),
                    ),
                    Ingredient::new(
                        IngredientName::new("flour".to_string()),
                        Quantity::from_measure(200.0, Unit::Gram),
                    ),
                    Ingredient::new(
                        IngredientName::new("salt".to_string()),
                        Quantity::from_text("to taste"),
                    ),
                    Ingredient::new(
                        IngredientName::new("yeast".to_string()),
                        Quantity::from_measure(1.0, Unit::Other("sachet".to_string())),
                    ),
                ],
                Vec::new(),
                vec![RecipeStep::new(
                    RecipeStepId::generate(),
                    Some("Mix".to_string()),
                    "Stir everything together.".to_string(),
                    Vec::new(),
                )],
            ),
        )
        .unwrap();
        let revision = RecipeRevision::new(
            RecipeRevisionId::new("revision-1".to_string()),
            recipe.id().clone(),
            content,
            UserId::new("user-1".to_string()),
            SystemTime::UNIX_EPOCH,
            None,
            RevisionVerification::Verified,
        );
        let current = RecipeWithContent::new(recipe, revision).unwrap();

        let initial = FormInitial::from_recipe(&current, &HashMap::new());

        assert_eq!(initial.title, "Soup");
        assert_eq!(initial.description, "A warming soup.");
        assert_eq!(initial.category, "main");
        assert_eq!(initial.servings_type, "portions");
        assert_eq!(initial.servings_count, "12");
        assert_eq!(initial.servings_label, "cookies");
        assert_eq!(initial.simple_body().preparation, "10");
        assert_eq!(initial.simple_body().rest, "");
        assert_eq!(initial.simple_body().cooking, "20");
        assert_eq!(initial.simple_body().ingredients[0].name, "flour");
        assert_eq!(initial.simple_body().ingredients[0].amount_type, "measured");
        assert_eq!(initial.simple_body().ingredients[0].quantity, "200");
        assert_eq!(initial.simple_body().ingredients[0].quantity_text, "");
        assert_eq!(initial.simple_body().ingredients[0].unit, "gram");
        assert_eq!(initial.simple_body().ingredients[0].unit_other, "");
        assert_eq!(initial.simple_body().ingredients[1].name, "milk");
        assert_eq!(initial.simple_body().ingredients[1].amount_type, "measured");
        assert_eq!(initial.simple_body().ingredients[1].quantity, "0.5");
        assert_eq!(initial.simple_body().ingredients[1].unit, "litre");
        assert_eq!(initial.simple_body().ingredients[2].name, "salt");
        assert_eq!(initial.simple_body().ingredients[2].amount_type, "text");
        assert_eq!(initial.simple_body().ingredients[2].quantity, "");
        assert_eq!(
            initial.simple_body().ingredients[2].quantity_text,
            "to taste"
        );
        assert_eq!(initial.simple_body().ingredients[3].name, "yeast");
        assert_eq!(initial.simple_body().ingredients[3].amount_type, "measured");
        assert_eq!(initial.simple_body().ingredients[3].quantity, "1");
        assert_eq!(initial.simple_body().ingredients[3].unit, "other");
        assert_eq!(initial.simple_body().ingredients[3].unit_other, "sachet");
        assert_eq!(initial.simple_body().steps[0].title, "Mix");
        assert_eq!(
            initial.simple_body().steps[0].instruction,
            "Stir everything together."
        );
    }
}
