use gpui::SharedString;

/// A labeled value with optional accent, shared by field-list widgets.
#[derive(Clone)]
pub struct Field {
    pub label: SharedString,
    pub value: SharedString,
    pub accent: bool,
}

impl From<debugger_model::Field> for Field {
    fn from(field: debugger_model::Field) -> Self {
        Self {
            label: field.label.into(),
            value: field.value.into(),
            accent: field.accent,
        }
    }
}
