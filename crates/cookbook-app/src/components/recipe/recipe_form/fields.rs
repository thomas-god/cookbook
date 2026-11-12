//! Wire vocabulary shared by the recipe form view and its parser.

use cookbook_domain::recipe::models::Unit;

pub(super) const TITLE_FIELD: &str = "title";
pub(super) const DESCRIPTION_FIELD: &str = "description";
pub(super) const CATEGORY_FIELD: &str = "category";
pub(super) const RECIPE_TYPE_FIELD: &str = "recipe_type";
pub(super) const PREPARATION_FIELD: &str = "preparation";
pub(super) const REST_FIELD: &str = "rest";
pub(super) const COOKING_FIELD: &str = "cooking";
pub(super) const SERVINGS_TYPE_FIELD: &str = "servings_type";
pub(super) const SERVINGS_COUNT_FIELD: &str = "servings_count";
pub(super) const SERVINGS_LABEL_FIELD: &str = "servings_label";
pub(super) const MESSAGE_FIELD: &str = "message";
pub(crate) const LATEST_REVISION_FIELD: &str = "latest_revision";
pub(super) const IMAGES_FIELD: &str = "images";
pub(super) const IMAGE_IDS_FIELD: &str = "image_ids";

/// The servings mode selected in the form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ServingsType {
    None,
    People,
    Portions,
}

impl ServingsType {
    pub(super) const ALL: [Self; 3] = [Self::None, Self::People, Self::Portions];

    pub(super) const fn wire_name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::People => "people",
            Self::Portions => "portions",
        }
    }

    pub(super) fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "" | "none" => Some(Self::None),
            "people" => Some(Self::People),
            "portions" => Some(Self::Portions),
            _ => None,
        }
    }
}

/// How an ingredient amount is expressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AmountType {
    Measured,
    Text,
}

impl AmountType {
    pub(super) const ALL: [Self; 2] = [Self::Measured, Self::Text];

    pub(super) const fn wire_name(self) -> &'static str {
        match self {
            Self::Measured => "measured",
            Self::Text => "text",
        }
    }

    pub(super) fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "" | "measured" => Some(Self::Measured),
            "text" => Some(Self::Text),
            _ => None,
        }
    }
}

/// Whether a recipe has one body or multiple parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RecipeFormType {
    Simple,
    Multipart,
}

impl RecipeFormType {
    pub(super) const fn wire_name(self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Multipart => "multipart",
        }
    }

    pub(super) fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "" | "simple" => Some(Self::Simple),
            "multipart" => Some(Self::Multipart),
            _ => None,
        }
    }
}

pub(super) const DEFAULT_UNIT: &str = "gram";
pub(super) const OTHER_UNIT: &str = "other";

pub(super) const BUILT_IN_UNITS: [&str; 8] = [
    "gram",
    "kilogram",
    "millilitre",
    "centilitre",
    "litre",
    "piece",
    "teaspoon",
    "tablespoon",
];

pub(super) fn unit_wire(unit: &Unit) -> &'static str {
    match unit {
        Unit::Gram => "gram",
        Unit::Kilogram => "kilogram",
        Unit::MilliLitre => "millilitre",
        Unit::CentiLitre => "centilitre",
        Unit::Litre => "litre",
        Unit::Piece => "piece",
        Unit::Teaspoon => "teaspoon",
        Unit::Tablespoon => "tablespoon",
        Unit::Other(_) => OTHER_UNIT,
    }
}

pub(super) fn unit_from_wire(value: &str) -> Option<Unit> {
    match value.trim() {
        "gram" => Some(Unit::Gram),
        "kilogram" => Some(Unit::Kilogram),
        "millilitre" => Some(Unit::MilliLitre),
        "centilitre" => Some(Unit::CentiLitre),
        "litre" => Some(Unit::Litre),
        "piece" => Some(Unit::Piece),
        "teaspoon" => Some(Unit::Teaspoon),
        "tablespoon" => Some(Unit::Tablespoon),
        _ => None,
    }
}

/// The fields of one ingredient row on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum IngredientField {
    Name,
    AmountType,
    Quantity,
    QuantityText,
    Unit,
    UnitOther,
}

impl IngredientField {
    pub(super) const fn wire_name(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::AmountType => "amount_type",
            Self::Quantity => "quantity",
            Self::QuantityText => "quantity_text",
            Self::Unit => "unit",
            Self::UnitOther => "unit_other",
        }
    }

    pub(super) fn key(self, row: usize, prefix: &str) -> String {
        if prefix.is_empty() {
            format!("ingredients[{row}][{}]", self.wire_name())
        } else {
            format!("{prefix}[ingredients][{row}][{}]", self.wire_name())
        }
    }

    pub(super) fn parse(wire_field: &str) -> Option<Self> {
        match wire_field {
            "name" => Some(Self::Name),
            "amount_type" => Some(Self::AmountType),
            "quantity" => Some(Self::Quantity),
            "quantity_text" => Some(Self::QuantityText),
            "unit" => Some(Self::Unit),
            "unit_other" => Some(Self::UnitOther),
            _ => None,
        }
    }
}

/// The fields of one step row on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum StepField {
    Id,
    Title,
    Instruction,
    Images,
    ImageIds,
}

impl StepField {
    pub(super) const fn wire_name(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Title => "title",
            Self::Instruction => "instruction",
            Self::Images => "images",
            Self::ImageIds => "image_ids",
        }
    }

    pub(super) fn key(self, row: usize, prefix: &str) -> String {
        if prefix.is_empty() {
            format!("steps[{row}][{}]", self.wire_name())
        } else {
            format!("{prefix}[steps][{row}][{}]", self.wire_name())
        }
    }

    pub(super) fn parse(wire_field: &str) -> Option<Self> {
        match wire_field {
            "id" => Some(Self::Id),
            "title" => Some(Self::Title),
            "instruction" => Some(Self::Instruction),
            // File and repeated image id fields are collected separately.
            "images" | "image_ids" => None,
            _ => None,
        }
    }
}

/// The fields of one multipart recipe part on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum PartField {
    Id,
    Title,
    Description,
}

impl PartField {
    pub(super) const fn wire_name(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Title => "title",
            Self::Description => "description",
        }
    }

    pub(super) fn key(self, row: usize) -> String {
        format!("parts[{row}][{}]", self.wire_name())
    }

    pub(super) fn parse(wire_field: &str) -> Option<Self> {
        match wire_field {
            "id" => Some(Self::Id),
            "title" => Some(Self::Title),
            "description" => Some(Self::Description),
            _ => None,
        }
    }
}

pub(super) fn body_field(prefix: &str, wire_field: &str) -> String {
    if prefix.is_empty() {
        wire_field.to_string()
    } else {
        format!("{prefix}[{wire_field}]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servings_types_round_trip_through_the_wire() {
        for servings_type in ServingsType::ALL {
            assert_eq!(
                ServingsType::parse(servings_type.wire_name()),
                Some(servings_type)
            );
        }
        assert_eq!(ServingsType::parse(""), Some(ServingsType::None));
        assert_eq!(ServingsType::parse("unknown"), None);
    }

    #[test]
    fn amount_types_round_trip_through_the_wire() {
        for amount_type in AmountType::ALL {
            assert_eq!(
                AmountType::parse(amount_type.wire_name()),
                Some(amount_type)
            );
        }
        assert_eq!(AmountType::parse(""), Some(AmountType::Measured));
        assert_eq!(AmountType::parse("unknown"), None);
    }

    #[test]
    fn recipe_form_types_round_trip_through_the_wire() {
        for recipe_type in [RecipeFormType::Simple, RecipeFormType::Multipart] {
            assert_eq!(
                RecipeFormType::parse(recipe_type.wire_name()),
                Some(recipe_type)
            );
        }
        assert_eq!(RecipeFormType::parse(""), Some(RecipeFormType::Simple));
        assert_eq!(RecipeFormType::parse("unknown"), None);
    }

    #[test]
    fn built_in_units_round_trip_through_the_wire() {
        for wire in BUILT_IN_UNITS {
            let unit = unit_from_wire(wire).expect("built-in unit should parse");
            assert_eq!(unit_wire(&unit), wire);
        }
        assert_eq!(unit_from_wire("other"), None);
    }
}
