use cookbook_domain::recipe::models::RecipeCategory;
use topcoat::context::Cx;

use crate::t;

/// Localized display label for a recipe category.
pub(crate) fn category_label(cx: &Cx, category: RecipeCategory) -> String {
    match category {
        RecipeCategory::Aperitif => t!(category_aperitif).resolve(cx).into_owned(),
        RecipeCategory::Starter => t!(category_starter).resolve(cx).into_owned(),
        RecipeCategory::Main => t!(category_main).resolve(cx).into_owned(),
        RecipeCategory::Side => t!(category_side).resolve(cx).into_owned(),
        RecipeCategory::Dessert => t!(category_dessert).resolve(cx).into_owned(),
        RecipeCategory::Breakfast => t!(category_breakfast).resolve(cx).into_owned(),
        RecipeCategory::Snack => t!(category_snack).resolve(cx).into_owned(),
        RecipeCategory::Drink => t!(category_drink).resolve(cx).into_owned(),
        RecipeCategory::Other => t!(category_other).resolve(cx).into_owned(),
    }
}
