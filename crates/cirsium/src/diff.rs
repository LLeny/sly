use std::collections::HashMap;
use std::hash::Hash;

use gpui::Hsla;

use crate::theme::Theme;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Debug)]
pub enum Highlight {
    #[default]
    None,
    Read,
    Written,
    Changed,
}

impl Highlight {
    pub fn merge(self, other: Highlight) -> Highlight {
        self.max(other)
    }
}

pub fn highlight_color(theme: &Theme, highlight: Highlight) -> Hsla {
    match highlight {
        Highlight::None => theme.text_primary,
        Highlight::Changed => theme.danger,
        Highlight::Read => theme.info,
        Highlight::Written => theme.warning,
    }
}

#[derive(Default)]
pub struct ChangeTracker<K: Eq + Hash> {
    previous: HashMap<K, u64>,
}

impl<K: Eq + Hash> ChangeTracker<K> {
    pub fn clear(&mut self) {
        self.previous.clear();
    }

    pub fn highlight(&mut self, key: K, value: u64) -> Highlight {
        match self.previous.insert(key, value) {
            Some(prev) if prev != value => Highlight::Changed,
            _ => Highlight::None,
        }
    }

    pub fn snapshot(&mut self, key: K, value: u64) {
        self.previous.insert(key, value);
    }
}
