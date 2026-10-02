use std::collections::HashSet;

use core_model::{GameId, System};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

pub const MAX_QUERY_CHARS: usize = 96;
pub const MAX_RESULTS: usize = 200;
pub const COLS: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchDocument {
    pub id: GameId,
    pub title: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchFocus {
    Keyboard,
    #[default]
    Results,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardAction {
    Insert(&'static str),
    Backspace,
    Clear,
    Results,
}

#[derive(Clone, Copy, Debug)]
pub struct SearchKey {
    pub label: &'static str,
    pub action: KeyboardAction,
}

const fn letter(label: &'static str) -> SearchKey {
    SearchKey {
        label,
        action: KeyboardAction::Insert(label),
    }
}

pub const KEYS: [SearchKey; 40] = [
    letter("A"),
    letter("B"),
    letter("C"),
    letter("D"),
    letter("E"),
    letter("F"),
    letter("G"),
    letter("H"),
    letter("I"),
    letter("J"),
    letter("K"),
    letter("L"),
    letter("M"),
    letter("N"),
    letter("O"),
    letter("P"),
    letter("Q"),
    letter("R"),
    letter("S"),
    letter("T"),
    letter("U"),
    letter("V"),
    letter("W"),
    letter("X"),
    letter("Y"),
    letter("Z"),
    letter("0"),
    letter("1"),
    letter("2"),
    letter("3"),
    letter("4"),
    letter("5"),
    letter("6"),
    letter("7"),
    letter("8"),
    letter("9"),
    SearchKey {
        label: "Space",
        action: KeyboardAction::Insert(" "),
    },
    SearchKey {
        label: "Delete",
        action: KeyboardAction::Backspace,
    },
    SearchKey {
        label: "Clear",
        action: KeyboardAction::Clear,
    },
    SearchKey {
        label: "Results",
        action: KeyboardAction::Results,
    },
];

#[derive(Debug)]
struct IndexedDocument {
    document: SearchDocument,
    normalized: String,
    compact: String,
}

#[derive(Debug)]
pub struct SearchState {
    pub query: String,
    pub results: Vec<SearchDocument>,
    pub selected: usize,
    pub focus: SearchFocus,
    pub keyboard_index: usize,
    pub total_matches: usize,
    pub current_system: System,
    documents: Vec<IndexedDocument>,
}

impl SearchState {
    pub fn new(
        current_system: System,
        documents: impl IntoIterator<Item = SearchDocument>,
    ) -> Self {
        let mut state = Self {
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            focus: SearchFocus::Results,
            keyboard_index: 0,
            total_matches: 0,
            current_system,
            documents: Vec::new(),
        };

        state.refresh(documents);
        state
    }

    pub fn refresh(&mut self, documents: impl IntoIterator<Item = SearchDocument>) {
        let selected = self.selected_result().map(|result| result.id.clone());

        self.documents = documents
            .into_iter()
            .map(|document| {
                let normalized = normalize(&document.title);

                IndexedDocument {
                    compact: normalized.replace(' ', ""),
                    normalized,
                    document,
                }
            })
            .collect();

        self.documents.sort_by(|a, b| {
            a.normalized
                .cmp(&b.normalized)
                .then_with(|| a.document.id.system.slug().cmp(b.document.id.system.slug()))
                .then_with(|| a.document.id.sha1.cmp(&b.document.id.sha1))
                .then_with(|| a.document.title.cmp(&b.document.title))
        });

        let mut seen = HashSet::new();
        self.documents
            .retain(|document| seen.insert(document.document.id.clone()));
        self.rebuild(selected.as_ref());
    }

    pub fn insert_text(&mut self, text: &str) -> bool {
        let remaining = MAX_QUERY_CHARS.saturating_sub(self.query.chars().count());
        let original_len = self.query.len();
        self.query
            .extend(text.chars().filter(|ch| !ch.is_control()).take(remaining));

        if self.query.len() == original_len {
            return false;
        }

        self.rebuild(None);
        true
    }

    pub fn backspace(&mut self) -> bool {
        if self.query.is_empty() {
            return false;
        }

        // Delete a base character together with its combining marks.
        let start = self
            .query
            .char_indices()
            .rev()
            .find(|(_, ch)| !is_combining_mark(*ch))
            .map_or(0, |(index, _)| index);

        self.query.truncate(start);
        self.rebuild(None);
        true
    }

    pub fn clear(&mut self) -> bool {
        if self.query.is_empty() {
            return false;
        }

        self.query.clear();
        self.rebuild(None);
        true
    }

    pub fn selected_result(&self) -> Option<&SearchDocument> {
        self.results.get(self.selected)
    }

    pub fn move_selection(&mut self, delta: i32) -> bool {
        if self.results.is_empty() {
            return false;
        }

        let selected = (self.selected as i64 + i64::from(delta))
            .rem_euclid(self.results.len() as i64) as usize;
        let changed = selected != self.selected;
        self.selected = selected;
        changed
    }

    pub fn move_keyboard(&mut self, dx: i32, dy: i32) -> bool {
        let col = (self.keyboard_index % COLS) as i64;
        let row = (self.keyboard_index / COLS) as i64;
        let col = (col + i64::from(dx)).rem_euclid(COLS as i64) as usize;
        let row = (row + i64::from(dy)).rem_euclid((KEYS.len() / COLS) as i64) as usize;
        let index = row * COLS + col;
        let changed = index != self.keyboard_index;
        self.keyboard_index = index;
        changed
    }

    pub fn keyboard_action(&self) -> KeyboardAction {
        KEYS[self.keyboard_index % KEYS.len()].action
    }

    fn rebuild(&mut self, preserved_selection: Option<&GameId>) {
        let query = normalize(&self.query);
        let compact_query = query.replace(' ', "");
        let tokens: Vec<&str> = query.split_whitespace().collect();

        let mut matches: Vec<_> = self
            .documents
            .iter()
            .filter_map(|document| {
                rank(&document.normalized, &query, &tokens)
                    .or_else(|| {
                        // Fall back to ignoring spaces, so FZero finds F-Zero.
                        document
                            .compact
                            .find(&compact_query)
                            .map(|position| (5, position))
                    })
                    .map(|rank| (document, rank))
            })
            .collect();

        let order = |(a, a_rank): &(&IndexedDocument, (u8, usize)),
                     (b, b_rank): &(&IndexedDocument, (u8, usize))| {
            (a.document.id.system != self.current_system)
                .cmp(&(b.document.id.system != self.current_system))
                .then_with(|| a_rank.cmp(b_rank))
                .then_with(|| a.normalized.cmp(&b.normalized))
                .then_with(|| a.document.id.system.slug().cmp(b.document.id.system.slug()))
                .then_with(|| a.document.id.sha1.cmp(&b.document.id.sha1))
                .then_with(|| a.document.title.cmp(&b.document.title))
        };

        self.total_matches = matches.len();

        if matches.len() > MAX_RESULTS {
            matches.select_nth_unstable_by(MAX_RESULTS, order);
            matches.truncate(MAX_RESULTS);
        }

        matches.sort_by(order);

        self.results = matches
            .into_iter()
            .map(|(document, _)| document.document.clone())
            .collect();

        self.selected = preserved_selection
            .and_then(|id| self.results.iter().position(|result| &result.id == id))
            .unwrap_or(0);
    }
}

/// Lowercases, strips accents and turns punctuation into single spaces, so plain A-Z input finds
/// accented titles.
fn normalize(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut separator = false;

    for ch in text.nfkd().filter(|ch| !is_combining_mark(*ch)) {
        if ch.is_alphanumeric() {
            if separator && !normalized.is_empty() {
                normalized.push(' ');
            }

            normalized.extend(ch.to_lowercase());
            separator = false;
        } else {
            separator = true;
        }
    }

    normalized
}

fn rank(title: &str, query: &str, tokens: &[&str]) -> Option<(u8, usize)> {
    if query.is_empty() {
        return Some((0, 0));
    }

    if title == query {
        return Some((0, 0));
    }

    if title.starts_with(query) {
        return Some((1, 0));
    }

    if let Some(position) = title.find(query) {
        let starts_word = title[..position].ends_with(' ');
        return Some((if starts_word { 2 } else { 3 }, position));
    }

    // Every word must appear, in any order: "zelda past" finds A Link to the Past.
    let mut position_sum = 0;

    for token in tokens {
        position_sum += title.find(token)?;
    }

    Some((4, position_sum))
}
