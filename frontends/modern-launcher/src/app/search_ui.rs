use super::*;
use crate::input::ActiveInput;
use crate::search::{COLS, KEYS, KeyboardAction, SearchDocument, SearchFocus, SearchState};

impl App {
    fn search_documents(&self) -> Vec<SearchDocument> {
        self.entries
            .iter()
            .chain(self.other_consoles.values().flatten())
            .map(|entry| SearchDocument {
                id: entry.game_id(),
                title: entry.title.clone(),
            })
            .collect()
    }

    pub(super) fn open_search(&mut self) {
        let mut search = SearchState::new(
            self.active_console().unwrap_or(System::ALL[0]),
            self.search_documents(),
        );

        if self.input.active == ActiveInput::Gamepad {
            search.focus = SearchFocus::Keyboard;
        }

        self.search = Some(search);
        self.search_preedit.clear();
        self.menu = Some(Menu::Search);
        self.menu_focus = 0;
        self.status.clear();
        self.clear_inputs();
        self.play_sound(Cue::Select);
    }

    pub(super) fn refresh_search(&mut self) {
        let documents = self.search_documents();
        let current_system = self.active_console();

        if let Some(search) = &mut self.search {
            if let Some(system) = current_system {
                search.current_system = system;
            }

            search.refresh(documents);
            self.menu_focus = search.selected;
        }

        self.needs_redraw = true;
    }

    pub fn search_text(&mut self, text: &str) {
        if self.menu == Some(Menu::Search)
            && let Some(search) = &mut self.search
            && search.insert_text(text)
        {
            search.focus = SearchFocus::Results;
            self.menu_focus = search.selected;
            self.search_preedit.clear();
            self.input.active = ActiveInput::Keyboard;
            self.needs_redraw = true;
        }
    }

    pub fn search_preedit(&mut self, text: &str) {
        let text: String = if self.menu == Some(Menu::Search) {
            text.chars()
                .filter(|c| !c.is_control())
                .take(crate::search::MAX_QUERY_CHARS)
                .collect()
        } else {
            String::new()
        };

        if self.search_preedit != text {
            self.search_preedit = text;
            self.needs_redraw = true;
        }
    }

    pub fn search_has_preedit(&self) -> bool {
        !self.search_preedit.is_empty()
    }

    pub fn search_input_rect(&self, size: [u32; 2], scale: f32) -> Option<[f32; 4]> {
        (self.menu == Some(Menu::Search)).then(|| {
            let scale = valid_scale(scale);

            SearchLayout::new(
                size.map(|value| value as f32 / scale),
                self.settings.text_scale,
            )
            .query
        })
    }

    fn close_search(&mut self) {
        self.menu = None;
        self.search = None;
        self.search_preedit.clear();
        self.menu_focus = 0;
        self.clear_inputs();
        self.needs_redraw = true;
    }

    fn choose_search_result(&mut self) {
        let Some(id) = self
            .search
            .as_ref()
            .and_then(SearchState::selected_result)
            .map(|r| r.id.clone())
        else {
            return;
        };

        let entries = if self.active_console() == Some(id.system) {
            Some(&self.entries)
        } else {
            self.other_consoles.get(&id.system)
        };

        let Some(index) =
            entries.and_then(|entries| entries.iter().position(|entry| entry.sha1 == id.sha1))
        else {
            self.refresh_search();
            return;
        };

        if self.active_console() != Some(id.system) && !self.activate_console(id.system) {
            return;
        }

        self.close_search();
        self.select(index);
        self.play_sound(Cue::Select);
    }

    pub(super) fn search_action(&mut self, action: Action) {
        if matches!(action, Action::Back | Action::Search) {
            self.close_search();
            self.play_sound(Cue::Back);
            return;
        }

        if matches!(action, Action::SearchBackspace | Action::SearchClear) {
            self.search_preedit.clear();
        }

        let Some(search) = &mut self.search else {
            return;
        };

        if action == Action::Launch && search.focus == SearchFocus::Results {
            self.choose_search_result();
            return;
        }

        let mut cue = Cue::Navigate;

        let changed = match action {
            Action::Navigate(delta) if search.focus == SearchFocus::Keyboard => {
                search.move_keyboard(0, delta)
            }
            Action::Adjust(delta) if search.focus == SearchFocus::Keyboard => {
                search.move_keyboard(delta, 0)
            }
            Action::Navigate(delta) => search.move_selection(delta),
            Action::SearchFocus => {
                search.focus = if search.focus == SearchFocus::Results {
                    SearchFocus::Keyboard
                } else {
                    SearchFocus::Results
                };

                cue = Cue::Select;
                true
            }
            Action::SearchBackspace => {
                cue = Cue::Adjust;
                search.backspace()
            }
            Action::SearchClear => {
                cue = Cue::Adjust;
                search.clear()
            }
            Action::Launch => {
                cue = Cue::Select;

                match search.keyboard_action() {
                    KeyboardAction::Insert(text) => search.insert_text(text),
                    KeyboardAction::Backspace => search.backspace(),
                    KeyboardAction::Clear => search.clear(),
                    KeyboardAction::Results => {
                        search.focus = SearchFocus::Results;
                        true
                    }
                }
            }
            _ => false,
        };

        self.menu_focus = search.selected;

        if changed {
            self.search_preedit.clear();
            self.play_sound(cue);
        }
    }

    pub(super) fn advance_search_motion(&mut self, dt: f32) {
        let active = self.menu.filter(|menu| *menu == Menu::Search);

        let index = self
            .search
            .as_ref()
            .map_or(0, |search| search.keyboard_index);

        for (motion, target) in self
            .search_key_motion
            .iter_mut()
            .zip([index % COLS, index / COLS])
        {
            self.needs_redraw |= motion.advance(active, target, dt, self.settings.reduced_motion);
        }
    }

    pub(super) fn search_ui(&self, ui: &mut UiFrame) {
        let Some(search) = &self.search else {
            return;
        };

        let theme = UiTheme::new(self.theme_accent());
        let layout = SearchLayout::new(ui.logical_size, ui.text_scale);
        let [left, top, width, height] = layout.panel;

        ui.rect(
            [0.0, 0.0, ui.logical_size[0], ui.logical_size[1]],
            [22, 20, 27, 145],
            0.0,
        );

        ui.rect(layout.panel, theme.background, 18.0);

        if layout.heading_height > 0.0 {
            ui.text(
                left + layout.padding,
                top + layout.padding,
                width - layout.padding * 2.0,
                22.0,
                "Search games",
                UI_INK,
            )
            .font = FontFace::Semibold;
        }

        ui.rect(layout.query, theme.info, 9.0);
        let [qx, qy, qw, qh] = layout.query;
        let content = format!("{}{}", search.query, self.search_preedit);
        let placeholder = content.is_empty();
        let limit = ((qw - 30.0) / (20.0 * ui.text_scale * 0.65)).max(4.0) as usize;
        let chars: Vec<_> = content.chars().collect();

        let display = if placeholder {
            "Type a game name".to_owned()
        } else if chars.len() > limit {
            format!(
                "…{}",
                chars[chars.len() - limit..].iter().collect::<String>()
            )
        } else {
            content
        };

        let text = ui.text(
            qx + 14.0,
            qy,
            qw - 28.0,
            20.0,
            display,
            if placeholder { UI_MUTED } else { UI_INK },
        );

        text.max_height = qh;
        text.vertically_centered = true;

        let matches = if search.total_matches > search.results.len() {
            format!(
                "{} of {} matches",
                search.results.len(),
                search.total_matches
            )
        } else {
            format!(
                "{} {}",
                search.total_matches,
                if search.total_matches == 1 {
                    "match"
                } else {
                    "matches"
                }
            )
        };

        let priority = if self.available_consoles().is_empty() {
            "All consoles".to_owned()
        } else {
            format!("{} first", search.current_system.name())
        };

        let caption = if layout.wide {
            format!("{matches} · {priority}")
        } else {
            format!(
                "{} · {matches}",
                if search.focus == SearchFocus::Keyboard {
                    "Keyboard"
                } else {
                    &priority
                }
            )
        };

        let subtitle = ui.text(qx, qy + qh + 8.0, qw, 12.0, caption, UI_MUTED);
        subtitle.max_height = layout.summary_height - 8.0;
        subtitle.vertically_centered = true;

        if layout.wide || search.focus == SearchFocus::Results {
            self.search_results_ui(ui, search, &layout, theme);
        }

        if layout.wide || search.focus == SearchFocus::Keyboard {
            self.search_keyboard_ui(ui, search, &layout, theme);
        }

        let saved_scale = ui.text_scale;
        let mut hints = self.input.search_hints();

        for (_, label) in &mut hints {
            if *label == "Pick" {
                *label = if search.focus == SearchFocus::Keyboard {
                    "Type"
                } else {
                    "Choose"
                };
            }

            if *label == "Keyboard / results" {
                *label = if search.focus == SearchFocus::Keyboard {
                    "Results"
                } else {
                    "Keyboard"
                };
            }
        }

        let available = width - layout.padding * 2.0;

        if hints
            .iter()
            .map(|(control, label)| prompt_width(control, label, saved_scale, self.input.active))
            .sum::<f32>()
            > available
        {
            hints.retain(|(_, label)| !matches!(*label, "Delete" | "Clear"));
        }

        let total = hints
            .iter()
            .map(|(control, label)| prompt_width(control, label, saved_scale, self.input.active))
            .sum::<f32>();

        ui.text_scale *= (available / total.max(1.0)).min(1.0);
        let mut fragments = Vec::new();

        for (control, label) in hints {
            let mark = ui.mark();

            prompt_ui(
                ui,
                0.0,
                0.0,
                layout.footer_height,
                &control,
                label,
                self.input.active,
            );

            fragments.push(ui.fragment(mark));
        }

        ui.bottom_right_row(
            fragments,
            left + width - layout.padding,
            top + height - layout.padding,
            10.0,
        );

        ui.text_scale = saved_scale;
    }

    fn search_results_ui(
        &self,
        ui: &mut UiFrame,
        search: &SearchState,
        layout: &SearchLayout,
        theme: UiTheme,
    ) {
        let [left, top, width, height] = layout.results;

        if search.results.is_empty() {
            let text = ui.text(
                left + 12.0,
                top + 12.0,
                width - 24.0,
                17.0,
                if self.available_consoles().is_empty() {
                    "Add a ROM folder in Settings to search your library."
                } else {
                    "No matching games. Try another name."
                },
                UI_MUTED,
            );

            text.max_height = height - 24.0;
            return;
        }

        let row_height = (54.0 * ui.text_scale).min(height);
        let visible = (height / row_height).floor().max(1.0) as usize;
        let start = search.selected.saturating_sub(visible.saturating_sub(1));

        let selected_y = self
            .menu_motion
            .position(Menu::Search, search.selected)
            .clamp(start as f32, (start + visible - 1) as f32)
            - start as f32;

        ui.rect(
            [
                left,
                top + selected_y * row_height + 2.0,
                width,
                row_height - 4.0,
            ],
            theme.soft,
            8.0,
        );

        if search.focus == SearchFocus::Results {
            ui.rect(
                [
                    left,
                    top + selected_y * row_height + 10.0,
                    3.0,
                    row_height - 20.0,
                ],
                theme.accent,
                1.5,
            );
        }

        let badge_width = (64.0 * ui.text_scale).min(width * 0.3);
        let badge_height = (26.0 * ui.text_scale).min(row_height - 12.0);
        let font_size = 16.0_f32.min((row_height - 8.0) / (1.3 * ui.text_scale));

        for (index, result) in search.results.iter().enumerate().skip(start).take(visible) {
            let y = top + (index - start) as f32 * row_height;
            let selected = index == search.selected;

            let text = ui.text(
                left + 12.0,
                y + 4.0,
                width - badge_width - 38.0,
                font_size,
                &result.title,
                if selected && search.focus == SearchFocus::Results {
                    theme.accent
                } else {
                    UI_INK
                },
            );

            text.max_height = row_height - 8.0;
            text.vertically_centered = true;

            if selected {
                text.font = FontFace::Semibold;
            }

            let badge = [
                left + width - badge_width - 12.0,
                y + (row_height - badge_height) * 0.5,
                badge_width,
                badge_height,
            ];

            ui.rect(
                badge,
                if selected {
                    theme.background
                } else {
                    theme.info
                },
                badge_height * 0.5,
            );

            let text = ui.text(
                badge[0],
                badge[1],
                badge[2],
                11.0,
                result.id.system.name(),
                UI_MUTED,
            );

            text.max_height = badge_height;
            text.align = TextAlign::Center;
            text.vertically_centered = true;
            text.font = FontFace::Semibold;
        }
    }

    fn search_keyboard_ui(
        &self,
        ui: &mut UiFrame,
        search: &SearchState,
        layout: &SearchLayout,
        theme: UiTheme,
    ) {
        let [left, top, width, height] = layout.keyboard;
        let gap = 5.0;
        let cell_width = (width - gap * (COLS - 1) as f32) / COLS as f32;
        let cell_height = (44.0 * ui.text_scale.sqrt()).min(height);

        let visible = ((height + gap) / (cell_height + gap))
            .floor()
            .clamp(1.0, 4.0) as usize;

        let row = search.keyboard_index / COLS;
        let start = row.saturating_sub(visible.saturating_sub(1));
        let active = search.focus == SearchFocus::Keyboard;

        let highlight = active.then(|| {
            let x = left
                + self.search_key_motion[0]
                    .position(Menu::Search, search.keyboard_index % COLS)
                    .clamp(0.0, (COLS - 1) as f32)
                    * (cell_width + gap);

            let y = top
                + (self.search_key_motion[1]
                    .position(Menu::Search, row)
                    .clamp(start as f32, (start + visible - 1) as f32)
                    - start as f32)
                    * (cell_height + gap);

            [x, y, cell_width, cell_height]
        });

        for (index, key) in KEYS
            .iter()
            .enumerate()
            .skip(start * COLS)
            .take(visible * COLS)
        {
            let x = left + (index % COLS) as f32 * (cell_width + gap);
            let y = top + (index / COLS - start) as f32 * (cell_height + gap);
            ui.rect([x, y, cell_width, cell_height], theme.info, 6.0);

            let label = match key.action {
                KeyboardAction::Insert(" ") => "␣",
                KeyboardAction::Backspace => "⌫",
                KeyboardAction::Clear => "×",
                KeyboardAction::Results => "↵",
                _ => key.label,
            };

            let font_size = 16.0_f32
                .min((cell_width - 6.0) / ui.text_scale)
                .min((cell_height - 6.0) / (1.3 * ui.text_scale));

            let text = ui.text(
                x,
                y,
                cell_width,
                font_size,
                label,
                if highlight.is_some_and(|[hx, hy, _, _]| {
                    (x - hx).abs() < cell_width * 0.5 && (y - hy).abs() < cell_height * 0.5
                }) {
                    [255; 4]
                } else {
                    UI_INK
                },
            );

            text.max_height = cell_height;
            text.align = TextAlign::Center;
            text.vertically_centered = true;
            text.font = FontFace::Semibold;
        }

        if let Some(highlight) = highlight {
            ui.rect(highlight, theme.accent, 6.0);
        }
    }
}

struct SearchLayout {
    panel: [f32; 4],
    query: [f32; 4],
    results: [f32; 4],
    keyboard: [f32; 4],
    padding: f32,
    heading_height: f32,
    summary_height: f32,
    footer_height: f32,
    wide: bool,
}

impl SearchLayout {
    fn new([width, height]: [f32; 2], scale: f32) -> Self {
        let small = width < 700.0 || height < 500.0;
        let margin = if small { 12.0 } else { 24.0 };
        let padding = if small { 12.0 } else { 24.0 };
        let panel_width = (width - margin * 2.0).min(1120.0 * scale.sqrt());
        let panel_height = (height - margin * 2.0).min(660.0 * scale.sqrt());
        let left = (width - panel_width) * 0.5;
        let top = (height - panel_height) * 0.5;
        let inner = panel_width - padding * 2.0;

        let heading_height = if height < 440.0 {
            0.0
        } else {
            32.0 * scale + 12.0
        };

        let query_height = 28.0 * scale + 16.0;
        let summary_height = 18.0 * scale + 8.0;
        let footer_height = 28.0 * scale.min(1.4) + 6.0;

        let query = [
            left + padding,
            top + padding + heading_height,
            inner,
            query_height,
        ];

        let body_top = query[1] + query_height + summary_height;
        let body_height =
            (top + panel_height - padding - footer_height - 14.0 - body_top).max(32.0);
        let wide = inner >= 940.0 * scale;
        let results_width = if wide { inner * 0.57 } else { inner };
        let results = [query[0], body_top, results_width, body_height];

        let keyboard = if wide {
            [
                query[0] + results_width + 24.0,
                body_top,
                inner - results_width - 24.0,
                body_height,
            ]
        } else {
            results
        };

        Self {
            panel: [left, top, panel_width, panel_height],
            query,
            results,
            keyboard,
            padding,
            heading_height,
            summary_height,
            footer_height,
            wide,
        }
    }
}
