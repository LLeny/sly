use std::ops::Range;

use debugger_model::{LUA_KEYWORDS, LuaTokenKind};
use gpui::SharedString;

pub(super) const MAX_MATCHES: usize = 8;

#[derive(Clone)]
pub struct Completion {
    pub label: SharedString,
    pub detail: SharedString,
    pub doc: SharedString,
    pub insert: SharedString,
}

impl Completion {
    pub fn new(
        label: impl Into<SharedString>,
        detail: impl Into<SharedString>,
        insert: impl Into<SharedString>,
    ) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
            doc: SharedString::default(),
            insert: insert.into(),
        }
    }

    pub fn doc(mut self, doc: impl Into<SharedString>) -> Self {
        self.doc = doc.into();
        self
    }

    fn keyword(word: &'static str) -> Self {
        Self::new(word, "keyword", word)
    }

    fn local(word: &str) -> Self {
        Self::new(word.to_string(), "in file", word.to_string())
    }
}

pub(super) struct Suggestions {
    pub query: Range<usize>,
    pub matches: Vec<Completion>,
    pub selected: usize,
}

impl Suggestions {
    pub fn selected(&self) -> Option<&Completion> {
        self.matches.get(self.selected)
    }

    pub fn move_selection(&mut self, delta: isize) {
        let len = self.matches.len();
        if len == 0 {
            return;
        }

        let next = self.selected as isize + delta;
        self.selected = next.rem_euclid(len as isize) as usize;
    }
}

impl super::CodeEditor {
    pub fn set_completions(&mut self, completions: Vec<Completion>) {
        self.completions = completions;
    }

    pub(super) fn close_suggestions(&mut self) -> bool {
        self.suggestions.take().is_some()
    }

    pub(super) fn refresh_suggestions(&mut self) {
        let caret = self.cursor_offset();
        if !self.selected_range.is_empty() {
            self.suggestions = None;
            return;
        }

        let query = self.query_range(caret);
        let text = &self.text[query.clone()];
        if text.is_empty() || self.in_comment_or_string(caret) {
            self.suggestions = None;
            return;
        }

        let matches = self.matches_for(text);
        self.suggestions = (!matches.is_empty()).then(|| {
            let selected = self
                .suggestions
                .as_ref()
                .and_then(|open| open.selected().map(|item| item.label.clone()))
                .and_then(|label| matches.iter().position(|item| item.label == label))
                .unwrap_or(0);

            Suggestions {
                query,
                matches,
                selected,
            }
        });
    }

    pub(super) fn documented_at(&self, offset: usize) -> Option<Completion> {
        let word = &self.text[self.word_at(offset)];
        if word.is_empty() {
            return None;
        }

        self.completions
            .iter()
            .find(|item| item.label == word && !item.doc.is_empty())
            .cloned()
    }

    pub(super) fn word_at(&self, offset: usize) -> Range<usize> {
        let is_word = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '.';
        let offset = offset.min(self.text.len());

        let start = self.text[..offset]
            .char_indices()
            .rev()
            .take_while(|(_, ch)| is_word(*ch))
            .last()
            .map(|(ix, _)| ix)
            .unwrap_or(offset);
        let end = self.text[offset..]
            .char_indices()
            .find(|(_, ch)| !is_word(*ch))
            .map(|(ix, _)| offset + ix)
            .unwrap_or(self.text.len());

        start..end
    }

    fn query_range(&self, caret: usize) -> Range<usize> {
        self.word_at(caret).start..caret
    }

    fn in_comment_or_string(&self, caret: usize) -> bool {
        self.tokens.iter().any(|token| {
            matches!(token.kind, LuaTokenKind::Comment | LuaTokenKind::Str)
                && (token.start..token.end).contains(&caret.saturating_sub(1))
        })
    }

    fn matches_for(&self, query: &str) -> Vec<Completion> {
        let needle = query.to_lowercase();

        let words = self.buffer_words(query);
        let candidates = self
            .completions
            .iter()
            .cloned()
            .chain(LUA_KEYWORDS.iter().copied().map(Completion::keyword))
            .chain(words.iter().copied().map(Completion::local));

        let mut matches: Vec<(u8, Completion)> = Vec::new();
        for candidate in candidates {
            let label = candidate.label.to_lowercase();
            let rank = if label.starts_with(&needle) {
                0
            } else if label
                .rsplit('.')
                .next()
                .is_some_and(|tail| tail.starts_with(&needle))
            {
                1
            } else {
                continue;
            };

            if label == needle {
                continue;
            }
            if matches
                .iter()
                .any(|(_, other)| other.label == candidate.label)
            {
                continue;
            }

            matches.push((rank, candidate));
        }

        matches.sort_by(|(a_rank, a), (b_rank, b)| {
            a_rank.cmp(b_rank).then_with(|| a.label.cmp(&b.label))
        });
        matches.truncate(MAX_MATCHES);
        matches.into_iter().map(|(_, item)| item).collect()
    }

    fn buffer_words(&self, query: &str) -> Vec<&str> {
        let mut words: Vec<&str> = self
            .tokens
            .iter()
            .filter(|token| token.kind == LuaTokenKind::Ident)
            .map(|token| &self.text[token.start..token.end])
            .filter(|word| word.len() > query.len())
            .collect();

        words.sort_unstable();
        words.dedup();
        words
    }

    pub(super) fn accept_suggestion(&mut self, cx: &mut gpui::Context<Self>) -> bool {
        let Some(open) = self.suggestions.take() else {
            return false;
        };
        let Some(item) = open.matches.get(open.selected) else {
            return false;
        };

        let insert = item.insert.to_string();
        self.replace_text_with_history(open.query.clone(), &insert);

        let caret = open.query.start + insert.len();
        self.selected_range = caret..caret;
        self.selection_reversed = false;
        self.reveal_cursor = true;

        self.reparse();
        cx.emit(super::CodeEditorEvent::Changed);
        cx.notify();

        true
    }
}
