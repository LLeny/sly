use bevy::audio::{AudioSource, PlaybackSettings};
use bevy::ecs::query::QueryFilter;
#[cfg(target_os = "ios")]
use bevy::ecs::system::NonSendMarker;
use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui::widget::ViewportNode;
#[cfg(target_os = "ios")]
use bevy::window::{PrimaryWindow, RawHandleWrapper};

use crate::assets::{Fonts, GameTexture, Icons, PromptIcons};
use crate::camera::DetailCamera;
use crate::discovery::{Library, title_alphabet_index};
use crate::game::{FRAME_HEIGHT, FRAME_WIDTH};
use crate::gamedb::GameMeta;
use crate::launch::Blackout;
use crate::materials::{
    BackgroundPattern, BackgroundUiMaterial, FrostedBubbleMaterial, IrisBlackoutMaterial,
};
use crate::state::{LauncherState, ViewMode};
use crate::touch::{self, TouchControls, TouchTarget};

pub const BG_ROOT: Color = Color::srgb(0.955, 0.957, 0.962);
pub const TEXT_PRIMARY: Color = Color::srgb(0.125, 0.11, 0.15);
pub const TEXT_SECONDARY: Color = Color::srgb(0.21, 0.19, 0.25);
pub const TEXT_MUTED: Color = Color::srgb(0.33, 0.30, 0.38);
pub const ACCENT: Color = Color::srgb(0.34, 0.19, 0.52);
const ACCENT_SOFT: Color = Color::srgba(0.34, 0.19, 0.52, 0.14);
const CELL_ACTIVE: Color = Color::srgba(0.975, 0.965, 0.99, 0.95);
const CELL_LETTER: Color = Color::srgba(0.86, 0.83, 0.9, 0.72);
const CELL_LETTER_EMPTY: Color = Color::srgba(0.86, 0.83, 0.9, 0.26);
pub const HAIRLINE: Color = Color::srgba(0.42, 0.39, 0.48, 0.28);
pub const GAME_BLACK: Color = Color::srgb(0.008, 0.008, 0.012);

pub const BUBBLE_MARGIN_PX: f32 = 28.0;
pub const EDGE_MARGIN_PX: f32 = 16.0;
const ALPHABET_TOP_PX: f32 = EDGE_MARGIN_PX;
const ALPHABET_CELL_PX: f32 = 28.0;
const ALPHABET_GAP_PX: f32 = 2.0;
const ALPHABET_PAD_PX: f32 = 2.0;
const ALPHABET_BORDER_PX: f32 = 1.0;

const ALPHABET_BAR_PX: f32 = alphabet_bar(ALPHABET_CELL_PX);

const fn alphabet_bar(cell: f32) -> f32 {
    cell + 2.0 * ALPHABET_PAD_PX + 2.0 * ALPHABET_BORDER_PX
}
#[cfg(target_vendor = "apple")]
const ALPHABET_LETTER_TOP_PX: f32 = 0.0;
#[cfg(not(target_vendor = "apple"))]
const ALPHABET_LETTER_TOP_PX: f32 = 1.0;
pub const BUBBLE_VMARGIN_PX: f32 = ALPHABET_TOP_PX + ALPHABET_BAR_PX + ALPHABET_TOP_PX;
pub const BUBBLE_WIDTH_FRACTION: f32 = 0.38;
pub const BUBBLE_VIEWPORT_FRACTION: f32 = 0.52;
const BUBBLE_VIEWPORT_FRACTION_TOUCH: f32 = 0.44;
pub const VIEWPORT_Z_IDLE: i32 = 4;

pub const RIBBON_STRIP_LEFT_FRACTION: f32 = 0.20;
pub const RIBBON_STRIP_TOP_FRACTION: f32 = 0.68;
const RIBBON_STRIP_TOP_FRACTION_TOUCH: f32 = 0.63;
pub const RIBBON_STRIP_WIDTH_FRACTION: f32 = 0.60;
pub const RIBBON_STRIP_HEIGHT_FRACTION: f32 = 0.24;
pub const RIBBON_VIEWPORT_FRACTION: f32 = 0.30;

const HINT_KEY_PX: f32 = 30.0;
const KEY_SPRITE_BORDER_FRACTION: f32 = 16.0 / 128.0;
const HINT_BAR_BOTTOM_PX: f32 = EDGE_MARGIN_PX - HINT_KEY_PX * KEY_SPRITE_BORDER_FRACTION;
const GAME_HINT: Color = Color::srgba(1.0, 1.0, 1.0, 0.30);

const TITLE_BASE_PX: f32 = 31.0;
const TITLE_RIBBON_BASE_PX: f32 = 38.0;

const PILL_ROW_PX: f32 = 78.0;
const TITLE_SAFETY_PX: f32 = 12.0;

const TITLE_ADVANCE: f32 = 0.60;

const FACT_LABEL_PX: f32 = 15.0;
const FACT_ICON_PX: f32 = 16.0;
const FACT_VALUE_PX: f32 = 16.0;
const BUBBLE_PILL_PX: f32 = 12.0;

const UI_REFERENCE_SIZE: Vec2 = Vec2::new(1280.0, 720.0);
const UI_SCALE_MIN: f32 = 0.85;
const UI_SCALE_MIN_TOUCH: f32 = 0.72;
const UI_SCALE_MAX: f32 = 2.75;

fn layout_scale(window: Vec2, controls: TouchControls) -> f32 {
    let min = if controls.0 {
        UI_SCALE_MIN_TOUCH
    } else {
        UI_SCALE_MIN
    };
    (window / UI_REFERENCE_SIZE)
        .min_element()
        .clamp(min, UI_SCALE_MAX)
}

#[derive(Resource, Default, Clone, Copy)]
pub struct SafeArea(pub Vec4);

#[derive(Component, Default, Clone)]
pub struct SafeAreaRoot;

#[cfg(target_os = "ios")]
pub fn update_safe_area(
    _main_thread: NonSendMarker,
    ui_scale: Res<UiScale>,
    window: Single<&RawHandleWrapper, With<PrimaryWindow>>,
    mut safe_area: ResMut<SafeArea>,
    mut roots: Query<&mut Node, With<SafeAreaRoot>>,
) {
    let insets = chrome_insets(crate::ios::safe_area_insets(&window) / ui_scale.0);
    if safe_area.0 != insets {
        safe_area.0 = insets;
    }

    let edges = [px(insets.x), px(insets.y), px(insets.z), px(insets.w)];
    for mut node in &mut roots {
        if [node.left, node.top, node.right, node.bottom] != edges {
            [node.left, node.top, node.right, node.bottom] = edges;
        }
    }
}

#[cfg(target_os = "ios")]
fn chrome_insets(safe: Vec4) -> Vec4 {
    let inset = (safe - Vec4::splat(EDGE_MARGIN_PX)).max(Vec4::ZERO);
    let vertical = inset.y.max(inset.w);
    Vec4::new(inset.x, vertical, inset.z, vertical)
}

pub fn safe_area_root() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            right: px(0),
            bottom: px(0),
        }
        SafeAreaRoot
    }
}

pub fn update_ui_scale(
    window: Single<&Window>,
    controls: Res<TouchControls>,
    mut ui_scale: ResMut<UiScale>,
    bubbles: Query<&MaterialNode<FrostedBubbleMaterial>, With<DetailBubble>>,
    mut frosted_materials: ResMut<Assets<FrostedBubbleMaterial>>,
) {
    let scale = layout_scale(Vec2::new(window.width(), window.height()), *controls);
    if (ui_scale.0 - scale).abs() < 1e-3 {
        return;
    }
    ui_scale.0 = scale;

    for handle in &bubbles {
        if let Some(mut material) = frosted_materials.get_mut(handle) {
            material.parameters = crate::materials::frosted_bubble_parameters(scale);
        }
    }
}

#[derive(Component, Default, Clone)]
pub struct LauncherUi;

#[derive(Component, Default, Clone)]
pub struct GameUi;

#[derive(Component, Default, Clone)]
pub struct GameFrame;

#[derive(Component, Default, Clone)]
pub struct TitleText;

#[derive(Component, Default, Clone)]
pub struct StatusText;

#[derive(Component, Default, Clone)]
pub struct DetailViewport;

#[derive(Component, Default, Clone)]
pub struct DetailFacts;

#[derive(Component, Default, Clone)]
pub struct DetailBubble;

#[derive(Component, Default, Clone)]
pub struct AlphabetRail;

#[derive(Component, Default, Clone)]
pub struct AlphabetIndex;

#[derive(Component, Default, Clone)]
pub struct FactRow;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum FactKind {
    #[default]
    Released,
    Developer,
    Publisher,
    Genre,
    Players,
}

#[derive(Component, Default, Clone)]
pub struct FactValue(pub FactKind);

impl FactKind {
    fn value(&self, meta: &GameMeta) -> Option<String> {
        match self {
            FactKind::Released => meta.released_label(),
            FactKind::Developer => meta.developer.clone(),
            FactKind::Publisher => meta.publisher.clone(),
            FactKind::Genre => meta.genre.clone(),
            FactKind::Players => meta.players_label(),
        }
    }
}

#[derive(Component, Default, Clone)]
pub struct HintIcon;

#[derive(Component, Default, Clone)]
pub struct SwipeHintIcon;

#[derive(Component, Default, Clone)]
pub struct CornerPill;

#[derive(Component, Default, Clone)]
pub struct PillRow;

#[derive(Component, Default, Clone)]
pub struct TvPill;

#[derive(Component, Default, Clone)]
pub struct TvPillText;

#[derive(Component, Default, Clone)]
pub struct TintedPill;

#[derive(Component, Default, Clone)]
pub struct TintedPillText;

#[derive(Component, Default, Clone)]
pub struct AlphabetLetter(usize);

#[derive(Component, Default, Clone)]
pub struct AlphabetCell(usize);

impl AlphabetCell {
    const COUNT: usize = 26;
}

#[allow(clippy::too_many_arguments)]
pub fn launcher_ui(
    fonts: &Fonts,
    icons: &Icons,
    prompts: &PromptIcons,
    background_pattern: &BackgroundPattern,
    library_camera: Entity,
    detail_camera: Entity,
    frosted_bubble: Handle<FrostedBubbleMaterial>,
    controls: TouchControls,
) -> impl Scene {
    let pattern = background_pattern.material.clone();
    let mono = fonts.mono_medium.clone();
    let touch_hints = Vec::from_iter(
        controls
            .0
            .then(|| touch::browse_hints(icons, prompts, mono.clone())),
    );
    let key_hints = Vec::from_iter((!controls.0).then(|| hints_bar(mono.clone(), prompts)));
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
                safe_area_root()
                template_value(Pickable::IGNORE)
                Children [
                    (
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(EDGE_MARGIN_PX),
                            top: px(EDGE_MARGIN_PX),
                            padding: UiRect::axes(px(12), px(5)),
                            border_radius: BorderRadius::MAX,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                        }
                        BackgroundColor(ACCENT_SOFT)
                        GlobalZIndex(6)
                        TintedPill
                        CornerPill
                        Children [
                            (
                                Text("SNES")
                                Node { top: px(1) }
                                text_font(mono.clone(), 13.0)
                                TextColor(ACCENT)
                                TintedPillText
                            )
                        ]
                    ),
                    (
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(EDGE_MARGIN_PX),
                            bottom: px(EDGE_MARGIN_PX),
                            padding: UiRect::axes(px(12), px(5)),
                            border_radius: BorderRadius::MAX,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                        }
                        BackgroundColor(ACCENT_SOFT)
                        GlobalZIndex(6)
                        TintedPill
                        CornerPill
                        Children [
                            (
                                Text("Scanning ROM library")
                                Node { top: px(1) }
                                text_font(mono.clone(), 13.0)
                                TextColor(ACCENT)
                                StatusText
                                TintedPillText
                            )
                        ]
                    ),
                    alphabet_rail(mono.clone()),
                    {touch_hints},
                    {key_hints},
                    detail_bubble(fonts, icons, detail_camera, frosted_bubble, controls),
                ]
            ),
        ]
    }
}

fn hint_key(image: Handle<Image>, color: Color) -> impl Scene {
    bsn! {
        Node {
            width: px(HINT_KEY_PX),
            height: px(HINT_KEY_PX),
        }
        ImageNode { image: {image}, color: {color} }
    }
}

pub fn hint_label(label: &'static str, mono: Handle<Font>, color: Color) -> impl Scene {
    bsn! {
        Text(label)
        Node { top: px(1) }
        text_font(mono, 12.0)
        TextColor(color)
    }
}

pub fn tinted_hint_key(image: Handle<Image>, size: f32) -> impl Scene {
    bsn! {
        Node {
            width: {px(size)},
            height: {px(size)},
        }
        ImageNode { image: {image}, color: {TEXT_SECONDARY} }
        HintIcon
    }
}

fn hints_bar(mono: Handle<Font>, prompts: &PromptIcons) -> impl Scene {
    let arrow_left = prompts.arrow_left.clone();
    let arrow_right = prompts.arrow_right.clone();
    let key_f = prompts.key_f.clone();
    let key_tab = prompts.key_tab.clone();
    let key_enter = prompts.key_enter.clone();
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            right: px(EDGE_MARGIN_PX),
            bottom: px(HINT_BAR_BOTTOM_PX),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(18),
        }
        GlobalZIndex(6)
        template_value(Pickable::IGNORE)
        Children [
            hint_group(vec![arrow_left, arrow_right], "Scroll", mono.clone()),
            hint_group(vec![key_f], "Flip", mono.clone()),
            hint_group(vec![key_tab], "View", mono.clone()),
            hint_group(vec![key_enter], "Start", mono),
        ]
    }
}

fn hint_group(keys: Vec<Handle<Image>>, label: &'static str, mono: Handle<Font>) -> impl Scene {
    hint_group_sized(keys, label, HINT_KEY_PX, mono)
}

pub fn hint_group_sized(
    keys: Vec<Handle<Image>>,
    label: &'static str,
    size: f32,
    mono: Handle<Font>,
) -> impl Scene {
    let icons: Vec<_> = keys
        .into_iter()
        .map(|key| tinted_hint_key(key, size))
        .collect();
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(5),
        }
        Children [
            {icons},
            hint_label(label, mono, TEXT_MUTED),
        ]
    }
}

fn alphabet_rail(mono: Handle<Font>) -> impl Scene {
    let cells: Vec<_> = (0..AlphabetCell::COUNT)
        .map(|letter_index| alphabet_cell(letter_index, mono.clone()))
        .collect();
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(EDGE_MARGIN_PX),
            height: px(ALPHABET_BAR_PX),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        GlobalZIndex(5)
        AlphabetRail
        template_value(Pickable::IGNORE)
        Children [
            (
                Node {
                    height: px(ALPHABET_BAR_PX),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(px(ALPHABET_PAD_PX)),
                    column_gap: px(ALPHABET_GAP_PX),
                    border: UiRect::all(px(ALPHABET_BORDER_PX)),
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
            width: px(ALPHABET_CELL_PX),
            height: px(ALPHABET_CELL_PX),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::MAX,
        }
        BackgroundColor(Color::NONE)
        AlphabetCell(letter_index)
        template_value(TouchTarget::Letter(letter_index))
        template_value(Pickable::IGNORE)
        Children [
            (
                Text(letter)
                Node { top: px(ALPHABET_LETTER_TOP_PX) }
                text_font(mono, 13.0)
                TextColor(TEXT_MUTED)
                AlphabetLetter(letter_index)
            )
        ]
    }
}

fn detail_bubble(
    fonts: &Fonts,
    icons: &Icons,
    detail_camera: Entity,
    frosted_bubble: Handle<FrostedBubbleMaterial>,
    controls: TouchControls,
) -> impl Scene {
    let sans = fonts.sans.clone();
    let sans_semibold = fonts.sans_semibold.clone();
    let mono = fonts.mono_medium.clone();
    let details: Vec<_> = [
        ("Released", FactKind::Released, icons.calendar.clone()),
        ("Developer", FactKind::Developer, icons.developer.clone()),
        ("Publisher", FactKind::Publisher, icons.publisher.clone()),
        ("Genre", FactKind::Genre, icons.genre.clone()),
        ("Players", FactKind::Players, icons.players.clone()),
    ]
    .into_iter()
    .map(|(label, kind, icon)| fact_row(label, kind, icon, sans.clone(), mono.clone()))
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
        template_value(TouchTarget::Start)
        Children [
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(0),
                    top: percent(0),
                    width: percent(100),
                    height: {percent(viewport_fraction(controls) * 100.0)},
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
                    top: {percent(facts_top(controls))},
                    bottom: px(22),
                    padding: UiRect::new(px(2), px(2), px(18), px(0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(11),
                    border: UiRect::top(px(1)),
                }
                template_value(BorderColor::all(HAIRLINE))
                GlobalZIndex(5)
                DetailFacts
                Children [
                    (

                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: px(12),
                            overflow: Overflow::clip_x(),
                        }
                        Children [
                            (
                                Text("No game selected")
                                Node {
                                    flex_shrink: 1.0,
                                    min_width: px(0),
                                }
                                template_value(TextLayout {
                                    linebreak: LineBreak::NoWrap,
                                    ..default()
                                })
                                text_font(sans_semibold, TITLE_BASE_PX)
                                TextColor(TEXT_PRIMARY)
                                TitleText
                            ),
                            pill_row(mono.clone()),
                        ]
                    ),
                    {details},
                ]
            ),
        ]
    }
}

pub fn game_ui(
    game_texture: &GameTexture,
    fonts: &Fonts,
    icons: &Icons,
    prompts: &PromptIcons,
    controls: TouchControls,
) -> impl Scene {
    let frame = game_texture.0.clone();
    let mono = fonts.mono_medium.clone();
    let key_escape = prompts.key_escape.clone();
    let pad = Vec::from_iter(controls.0.then(|| touch::game_pad(fonts)));
    let pause = Vec::from_iter(controls.0.then(|| touch::pause_menu(icons, fonts)));
    let key_hint = Vec::from_iter((!controls.0).then(|| game_hint_bar(key_escape, mono)));
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
            ),
            {key_hint},
            {pad},
            {pause},
        ]
    }
}

fn game_hint_bar(key_escape: Handle<Image>, mono: Handle<Font>) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            right: px(EDGE_MARGIN_PX),
            bottom: px(HINT_BAR_BOTTOM_PX),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(5),
        }
        template_value(Pickable::IGNORE)
        Children [
            hint_key(key_escape, GAME_HINT),
            hint_label("Library", mono, GAME_HINT),
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

fn pill_row(mono: Handle<Font>) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
            flex_shrink: 0.0,
        }
        PillRow
        Children [
            (
                Node {
                    padding: UiRect::axes(px(11), px(4)),
                    border_radius: BorderRadius::MAX,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                }
                BackgroundColor(ACCENT)
                TvPill
                Children [
                    (
                        Text("NTSC")
                        Node { top: px(1) }
                        text_font(mono.clone(), BUBBLE_PILL_PX)
                        TextColor(Color::srgb(0.97, 0.96, 0.99))
                        TvPillText
                    )
                ]
            ),
        ]
    }
}

pub fn tint_pills(
    pattern: Res<BackgroundPattern>,
    mut applied: Local<Option<Vec3>>,
    mut tv_background: Query<&mut BackgroundColor, With<TvPill>>,
    mut tinted_backgrounds: Query<&mut BackgroundColor, (With<TintedPill>, Without<TvPill>)>,
    mut tinted_text: Query<&mut TextColor, With<TintedPillText>>,
    mut hint_icons: Query<&mut ImageNode, With<HintIcon>>,
) {
    let tint = pattern.tint;
    if *applied == Some(tint) {
        return;
    }
    *applied = Some(tint);

    let srgb = |v: Vec3| Color::srgb(v.x, v.y, v.z);
    let solid = tint * 0.72;
    let dark = tint * 0.48;
    for mut icon in &mut hint_icons {
        icon.color = srgb(dark);
    }
    if let Ok(mut background) = tv_background.single_mut() {
        background.0 = srgb(solid);
    }
    for mut background in &mut tinted_backgrounds {
        background.0 = Color::srgba(tint.x, tint.y, tint.z, 0.20);
    }
    for mut color in &mut tinted_text {
        color.0 = srgb(dark);
    }
}

fn fact_row(
    label: &'static str,
    kind: FactKind,
    icon: Handle<Image>,
    sans: Handle<Font>,
    mono: Handle<Font>,
) -> impl Scene {
    bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
        }
        FactRow
        Children [
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(8),
                }
                Children [
                    (
                        Node {
                            width: px(FACT_ICON_PX),
                            height: px(FACT_ICON_PX),
                        }
                        ImageNode { image: {icon}, color: {TEXT_MUTED} }
                    ),
                    (
                        Text(label)
                        text_font(sans, FACT_LABEL_PX)
                        TextColor(TEXT_MUTED)
                    ),
                ]
            ),
            (
                Text("-")
                text_font(mono, FACT_VALUE_PX)
                TextColor(TEXT_PRIMARY)
                FactValue(kind)
            ),
        ]
    }
}

pub fn text_font(font: Handle<Font>, size: f32) -> impl Scene {
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

pub fn display(visible: bool) -> Display {
    if visible {
        Display::Flex
    } else {
        Display::None
    }
}

pub fn show_game_screen(
    show_game: bool,
    launcher_ui: &mut Query<&mut Node, (With<LauncherUi>, Without<GameUi>)>,
    game_ui: &mut Query<&mut Node, (With<GameUi>, Without<LauncherUi>)>,
) {
    if let Ok(mut node) = launcher_ui.single_mut() {
        node.display = display(!show_game);
    }

    if let Ok(mut node) = game_ui.single_mut() {
        node.display = display(show_game);
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
    window: Single<Ref<Window>>,
    ui_scale: Res<UiScale>,
    mut texts: ParamSet<(
        Query<(&mut Text, &mut TextFont), With<TitleText>>,
        Query<&mut Text, With<StatusText>>,
        Query<&mut Text, With<TvPillText>>,
        Query<(&FactValue, &mut Text)>,
    )>,
) {
    if !state.is_changed()
        && !library.is_changed()
        && !window.is_changed()
        && !ui_scale.is_changed()
    {
        return;
    }

    set_text_if_changed(&mut texts.p1(), &state.status);

    let Some(entry) = library.entries.get(state.selected) else {
        return;
    };

    let window_size = Vec2::new(window.width(), window.height()) / ui_scale.0;
    if let Ok((mut text, mut font)) = texts.p0().single_mut() {
        let title = elide(&entry.title, title_max_chars(state.view_mode, window_size));
        if text.0 != title {
            text.0 = title;
        }

        let size = title_font_size(state.view_mode);
        if font.font_size != FontSize::Px(size) {
            font.font_size = FontSize::Px(size);
        }
    }
    set_text_if_changed(&mut texts.p2(), &entry.region.tv_system().to_string());

    let meta = &entry.meta;
    for (fact, mut text) in texts.p3().iter_mut() {
        let value = fact.0.value(meta).unwrap_or_else(|| "-".to_string());
        if text.0 != value {
            text.0 = value;
        }
    }
}

fn title_max_chars(mode: ViewMode, window: Vec2) -> usize {
    let facts = match mode {
        ViewMode::Row | ViewMode::Column => window.x * BUBBLE_WIDTH_FRACTION - 2.0 * 26.0,
        ViewMode::Ribbon => {
            window.x * RIBBON_STRIP_WIDTH_FRACTION * (1.0 - RIBBON_VIEWPORT_FRACTION) - 26.0
        }
    };
    let budget = facts - PILL_ROW_PX - TITLE_SAFETY_PX;

    (budget / (title_font_size(mode) * TITLE_ADVANCE)).max(6.0) as usize
}

fn elide(title: &str, max: usize) -> String {
    if title.chars().count() <= max {
        return title.to_string();
    }

    let kept: String = title.chars().take(max.saturating_sub(3)).collect();
    format!("{}...", kept.trim_end())
}

fn title_font_size(mode: ViewMode) -> f32 {
    if mode == ViewMode::Ribbon {
        TITLE_RIBBON_BASE_PX
    } else {
        TITLE_BASE_PX
    }
}

fn ribbon_top(controls: TouchControls) -> f32 {
    if controls.0 {
        RIBBON_STRIP_TOP_FRACTION_TOUCH
    } else {
        RIBBON_STRIP_TOP_FRACTION
    }
}

fn viewport_fraction(controls: TouchControls) -> f32 {
    if controls.0 {
        BUBBLE_VIEWPORT_FRACTION_TOUCH
    } else {
        BUBBLE_VIEWPORT_FRACTION
    }
}

fn facts_top(controls: TouchControls) -> f32 {
    viewport_fraction(controls) * 100.0 + 1.0
}

pub fn viewport_idle_rect(mode: ViewMode, controls: TouchControls, window: Vec2) -> (Vec2, Vec2) {
    match mode {
        ViewMode::Row | ViewMode::Column => {
            let width = window.x * BUBBLE_WIDTH_FRACTION;
            let origin = Vec2::new(window.x - BUBBLE_MARGIN_PX - width, BUBBLE_VMARGIN_PX);
            let size = Vec2::new(
                width,
                (window.y - 2.0 * BUBBLE_VMARGIN_PX).max(1.0) * viewport_fraction(controls),
            );
            (origin, size)
        }
        ViewMode::Ribbon => {
            let origin = Vec2::new(
                window.x * RIBBON_STRIP_LEFT_FRACTION,
                window.y * ribbon_top(controls),
            );
            let size = Vec2::new(
                window.x * RIBBON_STRIP_WIDTH_FRACTION * RIBBON_VIEWPORT_FRACTION,
                window.y * RIBBON_STRIP_HEIGHT_FRACTION,
            );
            (origin, size)
        }
    }
}

pub fn detail_camera_transform(mode: ViewMode) -> Transform {
    let focus = Vec3::new(0.0, 0.025, 0.0);
    match mode {
        ViewMode::Row | ViewMode::Column => {
            Transform::from_xyz(0.0, 0.035, 0.50).looking_at(focus, Vec3::Y)
        }
        ViewMode::Ribbon => Transform::from_xyz(0.0, 0.030, 0.50).looking_at(focus, Vec3::Y),
    }
}

pub fn style_viewport_idle(mode: ViewMode, controls: TouchControls, node: &mut Node) {
    node.left = percent(0);
    node.top = percent(0);

    match mode {
        ViewMode::Row | ViewMode::Column => {
            node.width = percent(100);
            node.height = percent(viewport_fraction(controls) * 100.0);
        }
        ViewMode::Ribbon => {
            node.width = percent(RIBBON_VIEWPORT_FRACTION * 100.0);
            node.height = percent(100);
        }
    }
}

fn style_alphabet_outer(mode: ViewMode, node: &mut Node) {
    match mode {
        ViewMode::Column => {
            node.left = px(EDGE_MARGIN_PX);
            node.right = Val::Auto;
            node.top = px(0);
            node.bottom = px(0);
            node.width = px(ALPHABET_BAR_PX);
            node.height = Val::Auto;
        }
        ViewMode::Row | ViewMode::Ribbon => {
            node.left = px(0);
            node.right = px(0);
            node.top = px(EDGE_MARGIN_PX);
            node.bottom = Val::Auto;
            node.width = Val::Auto;
            node.height = px(ALPHABET_BAR_PX);
        }
    }
}

fn style_alphabet_inner(mode: ViewMode, node: &mut Node) {
    match mode {
        ViewMode::Column => {
            node.width = px(ALPHABET_BAR_PX);
            node.height = Val::Auto;
            node.flex_direction = FlexDirection::Column;
            node.padding = UiRect::vertical(px(ALPHABET_PAD_PX));
            node.row_gap = px(ALPHABET_GAP_PX);
            node.column_gap = px(0);
        }
        ViewMode::Row | ViewMode::Ribbon => {
            node.width = Val::Auto;
            node.height = px(ALPHABET_BAR_PX);
            node.flex_direction = FlexDirection::Row;
            node.padding = UiRect::horizontal(px(ALPHABET_PAD_PX));
            node.row_gap = px(0);
            node.column_gap = px(ALPHABET_GAP_PX);
        }
    }
}

fn style_bubble(mode: ViewMode, controls: TouchControls, node: &mut Node) {
    match mode {
        ViewMode::Row | ViewMode::Column => {
            node.left = Val::Auto;
            node.right = px(BUBBLE_MARGIN_PX);
            node.top = px(BUBBLE_VMARGIN_PX);
            node.bottom = px(BUBBLE_VMARGIN_PX);
            node.width = percent(BUBBLE_WIDTH_FRACTION * 100.0);
            node.height = Val::Auto;
        }
        ViewMode::Ribbon => {
            node.left = percent(RIBBON_STRIP_LEFT_FRACTION * 100.0);
            node.right = Val::Auto;
            node.top = percent(ribbon_top(controls) * 100.0);
            node.bottom = Val::Auto;
            node.width = percent(RIBBON_STRIP_WIDTH_FRACTION * 100.0);
            node.height = percent(RIBBON_STRIP_HEIGHT_FRACTION * 100.0);
        }
    }
}

fn style_facts(mode: ViewMode, controls: TouchControls, node: &mut Node) {
    match mode {
        ViewMode::Row | ViewMode::Column => {
            node.left = px(26);
            node.right = px(26);
            node.top = percent(facts_top(controls));
            node.bottom = px(22);
            node.padding = UiRect::new(px(2), px(2), px(18), px(0));
            node.flex_direction = FlexDirection::Column;
            node.align_items = AlignItems::Default;
            node.row_gap = px(11);
            node.column_gap = px(0);
            node.border = UiRect::top(px(1));
        }
        ViewMode::Ribbon => {
            node.left = percent(RIBBON_VIEWPORT_FRACTION * 100.0 + 2.0);
            node.right = px(24);
            node.top = px(0);
            node.bottom = px(0);
            node.padding = UiRect::all(px(0));
            node.flex_direction = FlexDirection::Row;
            node.align_items = AlignItems::Center;
            node.row_gap = px(0);
            node.column_gap = px(14);
            node.border = UiRect::all(px(0));
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn apply_view_mode(
    state: Res<LauncherState>,
    controls: Res<TouchControls>,
    mut applied: Local<Option<ViewMode>>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<AlphabetRail>>,
        Query<&mut Node, With<AlphabetIndex>>,
        Query<&mut Node, With<DetailBubble>>,
        Query<&mut Node, With<DetailViewport>>,
        Query<&mut Node, With<DetailFacts>>,
        Query<&mut Node, With<FactRow>>,
        Query<&mut Node, With<CornerPill>>,
    )>,
    mut detail_camera: Query<&mut Transform, With<DetailCamera>>,
) {
    let mode = state.view_mode;
    if *applied == Some(mode) {
        return;
    }
    *applied = Some(mode);

    if let Ok(mut camera) = detail_camera.single_mut() {
        *camera = detail_camera_transform(mode);
    }

    if let Ok(mut outer) = nodes.p0().single_mut() {
        style_alphabet_outer(mode, &mut outer);
    }
    if let Ok(mut inner) = nodes.p1().single_mut() {
        style_alphabet_inner(mode, &mut inner);
    }
    if let Ok(mut bubble) = nodes.p2().single_mut() {
        style_bubble(mode, *controls, &mut bubble);
    }
    if let Ok(mut viewport) = nodes.p3().single_mut() {
        style_viewport_idle(mode, *controls, &mut viewport);
    }
    if let Ok(mut facts) = nodes.p4().single_mut() {
        style_facts(mode, *controls, &mut facts);
    }

    let pill_left = if mode == ViewMode::Column {
        px(EDGE_MARGIN_PX + ALPHABET_BAR_PX + 8.0)
    } else {
        px(EDGE_MARGIN_PX)
    };
    for mut node in nodes.p6().iter_mut() {
        node.left = pill_left;
    }

    let bubble_details = if mode == ViewMode::Ribbon {
        Display::None
    } else {
        Display::Flex
    };
    for mut node in nodes.p5().iter_mut() {
        node.display = bubble_details;
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn fit_alphabet_rail(
    window: Single<&Window>,
    ui_scale: Res<UiScale>,
    safe_area: Res<SafeArea>,
    state: Res<LauncherState>,
    mut applied: Local<Option<(ViewMode, f32)>>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<AlphabetCell>>,
        Query<&mut Node, With<AlphabetIndex>>,
        Query<&mut Node, With<AlphabetRail>>,
    )>,
    mut letters: Query<&mut TextFont, With<AlphabetLetter>>,
) {
    let vertical = state.view_mode.is_vertical();
    let cell = if vertical {
        let height = window.height() / ui_scale.0
            - safe_area.0.y
            - safe_area.0.w
            - 2.0 * EDGE_MARGIN_PX
            - 2.0 * ALPHABET_PAD_PX;
        let letters = AlphabetCell::COUNT as f32;
        ((height - (letters - 1.0) * ALPHABET_GAP_PX) / letters).clamp(12.0, ALPHABET_CELL_PX)
    } else {
        ALPHABET_CELL_PX
    };

    if *applied == Some((state.view_mode, cell)) {
        return;
    }
    *applied = Some((state.view_mode, cell));

    for mut node in nodes.p0().iter_mut() {
        node.width = px(cell);
        node.height = px(cell);
    }

    let bar = px(alphabet_bar(cell));
    let fit = |node: &mut Node| {
        if vertical {
            node.width = bar;
        } else {
            node.height = bar;
        }
    };
    for mut node in nodes.p1().iter_mut() {
        fit(&mut node);
    }
    for mut node in nodes.p2().iter_mut() {
        fit(&mut node);
    }

    for mut font in &mut letters {
        font.font_size = FontSize::Px((cell * 0.52).floor());
    }
}

pub fn update_alphabet_highlight(
    state: Res<LauncherState>,
    library: Res<Library>,
    mut populated: Local<[bool; AlphabetCell::COUNT]>,
    mut cells: Query<(&AlphabetCell, &mut BackgroundColor)>,
    mut letters: Query<(&AlphabetLetter, &mut TextColor)>,
) {
    let library_changed = library.is_changed();
    if !state.is_changed() && !library_changed {
        return;
    }
    let Some(entry) = library.entries.get(state.selected) else {
        return;
    };

    let active = title_alphabet_index(&entry.title);
    if library_changed {
        populated.fill(false);
        for entry in &library.entries {
            populated[title_alphabet_index(&entry.title)] = true;
        }
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
