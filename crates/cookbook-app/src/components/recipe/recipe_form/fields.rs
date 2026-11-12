//! Wire vocabulary shared by the recipe form view and its parser.

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
    Preparation,
    Rest,
    Cooking,
}

impl PartField {
    pub(super) const fn wire_name(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Title => "title",
            Self::Description => "description",
            Self::Preparation => "preparation",
            Self::Rest => "rest",
            Self::Cooking => "cooking",
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
            "preparation" => Some(Self::Preparation),
            "rest" => Some(Self::Rest),
            "cooking" => Some(Self::Cooking),
            // Repeated image id fields are collected separately.
            "images" | "image_ids" => None,
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
