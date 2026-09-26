use std::sync::Arc;

use crate::Button;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerDescriptor {
    pub port: usize,
    pub name: Arc<str>,
    pub buttons: Vec<Button>,
}

impl ControllerDescriptor {
    pub fn supports(&self, button: Button) -> bool {
        self.buttons.contains(&button)
    }
}
