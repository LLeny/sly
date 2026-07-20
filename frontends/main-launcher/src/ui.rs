use bevy::audio::{AudioSource, PlaybackSettings};
use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui::widget::ViewportNode;

use crate::assets::{Fonts, GameTexture};
use crate::discovery::Library;
use crate::game::{FRAME_HEIGHT, FRAME_WIDTH};
use crate::launch::Blackout;
use crate::materials::{
    BackgroundPattern, BackgroundUiMaterial, FrostedBubbleMaterial, IrisBlackoutMaterial,
};
use crate::state::LauncherState;

pub const BG_ROOT: Color = Color::srgb(0.955, 0.957, 0.962);
pub const TEXT_PRIMARY: Color = Color::srgb(0.125, 0.11, 0.15);
pub const TEXT_SECONDARY: Color = Color::srgb(0.21, 0.19, 0.25);
pub const TEXT_MUTED: Color = Color::srgb(0.33, 0.30, 0.38);
pub const ACCENT: Color = Color::srgb(0.34, 0.19, 0.52);
const CELL_ACTIVE: Color = Color::srgba(0.975, 0.965, 0.99, 0.95);
const CELL_LETTER: Color = Color::srgba(0.86, 0.83, 0.9, 0.72);
const CELL_LETTER_EMPTY: Color = Color::srgba(0.86, 0.83, 0.9, 0.26);
pub const GAME_BLACK: Color = Color::srgb(0.008, 0.008, 0.012);

pub const BUBBLE_MARGIN_PX: f32 = 28.0;
pub const BUBBLE_WIDTH_FRACTION: f32 = 0.38;
pub const BUBBLE_VIEWPORT_FRACTION: f32 = 0.52;
pub const VIEWPORT_Z_IDLE: i32 = 4;

#[derive(Component, Default, Clone)]
pub struct LauncherUi;

#[derive(Component, Default, Clone)]
pub struct GameUi;

#[derive(Component, Default, Clone)]
pub struct GameFrame;

#[derive(Component, Default, Clone)]
pub struct TitleText;

#[derive(Component, Default, Clone)]
pub struct RegionText;

#[derive(Component, Default, Clone)]
pub struct CountText;

#[derive(Component, Default, Clone)]
pub struct StatusText;

#[derive(Component, Default, Clone)]
pub struct DetailViewport;

#[derive(Component, Default, Clone)]
pub struct DetailFacts;

#[derive(Component, Default, Clone)]
pub struct DetailBubble;

#[derive(Component, Default, Clone)]
pub struct AlphabetIndex;

#[derive(Component, Default, Clone)]
pub struct AlphabetLetter(usize);

#[derive(Component, Default, Clone)]
pub struct AlphabetCell(usize);

pub fn launcher_ui(
    fonts: &Fonts,
    background_pattern: &BackgroundPattern,
    library_camera: Entity,
    detail_camera: Entity,
    frosted_bubble: Handle<FrostedBubbleMaterial>,
) -> impl Scene {
    let pattern = background_pattern.material.clone();
    let mono = fonts.mono_medium.clone();
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Row,
        }
        BackgroundColor(BG_ROOT)
        LauncherUi
        Children [
            (
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                }
                MaterialNode<BackgroundUiMaterial>({pattern})
                GlobalZIndex(0)
                template_value(Pickable::IGNORE)
            ),
            (
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                }
                template(move |_| Ok(ViewportNode::new(library_camera)))
                GlobalZIndex(1)
                template_value(Pickable::IGNORE)
            ),
            (
                Text("LIBRARY / SNES")
                text_font(mono.clone(), 12.0)
                TextColor(TEXT_SECONDARY)
                Node {
                    position_type: PositionType::Absolute,
                    left: px(28),
                    top: px(24),
                }
                GlobalZIndex(6)
            ),
            (
                Text("Scanning ROM library")
                text_font(mono.clone(), 11.0)
                TextColor(TEXT_MUTED)
                Node {
                    position_type: PositionType::Absolute,
                    left: px(28),
                    bottom: px(24),
                }
                GlobalZIndex(6)
                StatusText
            ),
            alphabet_rail(mono.clone()),
            detail_bubble(fonts, detail_camera, frosted_bubble),
        ]
    }
}

fn alphabet_rail(mono: Handle<Font>) -> impl Scene {
    let cells: Vec<_> = (0..26)
        .map(|letter_index| alphabet_cell(letter_index, mono.clone()))
        .collect();
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(18),
            height: px(30),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        GlobalZIndex(5)
        template_value(Pickable::IGNORE)
        Children [
            (
                Node {
                    height: px(30),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(px(4)),
                    column_gap: px(2),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::MAX,
                }
                BackgroundColor(Color::srgba(0.028, 0.024, 0.036, 0.78))
                template_value(BorderColor::all(Color::srgba(0.94, 0.91, 0.98, 0.12)))
                template_value(BoxShadow::new(
                    Color::srgba(0.04, 0.025, 0.06, 0.28),
                    px(0),
                    px(4),
                    px(-2),
                    px(10),
                ))
                AlphabetIndex
                template_value(Pickable::IGNORE)
                Children [{cells}]
            )
        ]
    }
}

fn alphabet_cell(letter_index: usize, mono: Handle<Font>) -> impl Scene {
    let letter = char::from(b'A' + letter_index as u8).to_string();
    bsn! {
        Node {
            width: px(20),
            height: px(20),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::MAX,
        }
        BackgroundColor(Color::NONE)
        AlphabetCell(letter_index)
        template_value(Pickable::IGNORE)
        Children [
            (
                Text(letter)
                Node { top: px(1) }
                text_font(mono, 11.0)
                TextColor(TEXT_MUTED)
                AlphabetLetter(letter_index)
            )
        ]
    }
}

fn detail_bubble(
    fonts: &Fonts,
    detail_camera: Entity,
    frosted_bubble: Handle<FrostedBubbleMaterial>,
) -> impl Scene {
    let sans = fonts.sans.clone();
    let sans_semibold = fonts.sans_semibold.clone();
    let mono = fonts.mono_medium.clone();
    let details: Vec<_> = [
        ("Released", "21 Nov 1994"),
        ("Developer", "Nintendo R&D4"),
        ("Publisher", "Nintendo"),
        ("Genre", "Action / Adventure"),
        ("Players", "1-2"),
    ]
    .into_iter()
    .map(|(label, value)| fact_row(label, value, sans.clone(), mono.clone()))
    .collect();

    bsn! {
        Node {
            position_type: PositionType::Absolute,
            right: px(BUBBLE_MARGIN_PX),
            top: px(BUBBLE_MARGIN_PX),
            bottom: px(BUBBLE_MARGIN_PX),
            width: percent(BUBBLE_WIDTH_FRACTION * 100.0),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(26)),
        }
        MaterialNode<FrostedBubbleMaterial>({frosted_bubble})
        template_value(BorderColor::all(Color::srgba(0.58, 0.55, 0.64, 0.30)))
        template_value(BoxShadow::new(
            Color::srgba(0.10, 0.08, 0.14, 0.18),
            px(0),
            px(10),
            px(-4),
            px(26),
        ))
        GlobalZIndex(3)
        DetailBubble
        Children [
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(0),
                    top: percent(0),
                    width: percent(100),
                    height: percent(BUBBLE_VIEWPORT_FRACTION * 100.0),
                }
                template(move |_| Ok(ViewportNode::new(detail_camera)))
                GlobalZIndex(VIEWPORT_Z_IDLE)
                DetailViewport
                template_value(Pickable::IGNORE)
            ),
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: px(26),
                    right: px(26),
                    top: percent(53),
                    bottom: px(22),
                    padding: UiRect::new(px(2), px(2), px(18), px(0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(9),
                    border: UiRect::top(px(1)),
                }
                template_value(BorderColor::all(Color::srgba(0.42, 0.39, 0.48, 0.28)))
                GlobalZIndex(5)
                DetailFacts
                Children [
                    (
                        Text("No game selected")
                        text_font(sans_semibold, 27.0)
                        TextColor(TEXT_PRIMARY)
                        TitleText
                    ),
                    (
                        Text("Waiting for library")
                        text_font(mono.clone(), 13.0)
                        TextColor(ACCENT)
                        RegionText
                    ),
                    {details},
                    (
                        Text("00 / 00  -  arrows or A/D browse  -  F flip  -  Enter launch")
                        text_font(mono.clone(), 12.0)
                        TextColor(TEXT_MUTED)
                        Node { margin: UiRect::top(px(8)) }
                        CountText
                    ),
                ]
            ),
        ]
    }
}

pub fn game_ui(game_texture: &GameTexture) -> impl Scene {
    let frame = game_texture.0.clone();
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            display: Display::None,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        BackgroundColor(GAME_BLACK)
        GlobalZIndex(20)
        GameUi
        Children [
            (
                Node {
                    width: px(FRAME_WIDTH as f32),
                    height: px(FRAME_HEIGHT as f32),
                }
                ImageNode { image: {frame} }
                GameFrame
            )
        ]
    }
}

pub fn blackout_ui(material: Handle<IrisBlackoutMaterial>) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
        }
        MaterialNode<IrisBlackoutMaterial>({material})
        GlobalZIndex(100)
        Blackout
        template_value(Pickable::IGNORE)
    }
}

fn fact_row(
    label: &'static str,
    value: &'static str,
    sans: Handle<Font>,
    mono: Handle<Font>,
) -> impl Scene {
    bsn! {
        Node {
            width: percent(100),
            justify_content: JustifyContent::SpaceBetween,
        }
        Children [
            (
                Text(label)
                text_font(sans, 13.0)
                TextColor(TEXT_MUTED)
            ),
            (
                Text(value)
                text_font(mono, 14.0)
                TextColor(TEXT_PRIMARY)
            ),
        ]
    }
}

fn text_font(font: Handle<Font>, size: f32) -> impl Scene {
    bsn! {
        TextFont {
            font: FontSourceTemplate::Handle(font),
            font_size: FontSize::Px(size),
        }
    }
}

pub fn ui_sound(source: Handle<AudioSource>) -> impl Bundle {
    (AudioPlayer::new(source), PlaybackSettings::DESPAWN)
}

pub fn show_game_screen(
    show_game: bool,
    launcher_ui: &mut Query<&mut Node, (With<LauncherUi>, Without<GameUi>)>,
    game_ui: &mut Query<&mut Node, (With<GameUi>, Without<LauncherUi>)>,
) {
    if let Ok(mut node) = launcher_ui.single_mut() {
        node.display = if show_game {
            Display::None
        } else {
            Display::Flex
        };
    }

    if let Ok(mut node) = game_ui.single_mut() {
        node.display = if show_game {
            Display::Flex
        } else {
            Display::None
        };
    }
}

fn set_text_if_changed<F: QueryFilter>(query: &mut Query<&mut Text, F>, value: &str) {
    if let Ok(mut text) = query.single_mut()
        && text.0 != value
    {
        value.clone_into(&mut text.0);
    }
}

#[allow(clippy::type_complexity)]
pub fn update_ui(
    state: Res<LauncherState>,
    library: Res<Library>,
    mut texts: ParamSet<(
        Query<&mut Text, With<TitleText>>,
        Query<&mut Text, With<RegionText>>,
        Query<&mut Text, With<CountText>>,
        Query<&mut Text, With<StatusText>>,
    )>,
) {
    if !state.is_changed() && !library.is_changed() {
        return;
    }

    set_text_if_changed(&mut texts.p3(), &state.status);

    let Some(entry) = library.entries.get(state.selected) else {
        return;
    };

    set_text_if_changed(&mut texts.p0(), &entry.title);
    set_text_if_changed(&mut texts.p1(), &entry.region.to_string());
    set_text_if_changed(
        &mut texts.p2(),
        &format!(
            "{:02} / {:02}  -  arrows or A/D browse  -  F flip  -  Enter launch",
            state.selected + 1,
            library.entries.len()
        ),
    );
}

fn title_alphabet_index(title: &str) -> usize {
    title
        .chars()
        .find(|character| character.is_ascii_alphabetic())
        .map(|character| (character.to_ascii_uppercase() as u8 - b'A') as usize)
        .unwrap_or(0)
}

pub fn update_alphabet_highlight(
    state: Res<LauncherState>,
    library: Res<Library>,
    mut cells: Query<(&AlphabetCell, &mut BackgroundColor)>,
    mut letters: Query<(&AlphabetLetter, &mut TextColor)>,
) {
    if !state.is_changed() && !library.is_changed() {
        return;
    }
    let Some(entry) = library.entries.get(state.selected) else {
        return;
    };

    let active = title_alphabet_index(&entry.title);
    let mut populated = [false; 26];
    for entry in &library.entries {
        populated[title_alphabet_index(&entry.title)] = true;
    }

    for (cell, mut background) in &mut cells {
        let target = if cell.0 == active {
            CELL_ACTIVE
        } else {
            Color::NONE
        };
        background.set_if_neq(BackgroundColor(target));
    }

    for (letter, mut color) in &mut letters {
        let target = if letter.0 == active {
            TEXT_PRIMARY
        } else if populated[letter.0] {
            CELL_LETTER
        } else {
            CELL_LETTER_EMPTY
        };
        color.set_if_neq(TextColor(target));
    }
}
