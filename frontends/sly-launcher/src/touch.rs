use bevy::color::color_difference::EuclideanDistance;
use bevy::prelude::*;

use core_model::Button;

use crate::assets::{Fonts, Icons, Models, PromptIcons, UiSounds};
use crate::cartridge::DetailCart;
use crate::discovery::Library;
use crate::game::GameSession;
use crate::state::{LauncherMode, LauncherState, ModeTimers, Paused};
use crate::ui::{
    BUBBLE_MARGIN_PX, EDGE_MARGIN_PX, HAIRLINE, SwipeHintIcon, TEXT_SECONDARY, display, hint_label,
    safe_area_root, text_font, tinted_hint_key, ui_sound,
};
use crate::{input, launch};

const SWIPE_STEP_PX: f32 = 64.0;

const FLING_DECAY: f32 = 4.5;

const FLING_MIN_PX: f32 = 45.0;

const FLING_MAX_PX: f32 = 2200.0;
const TAP_MAX_TRAVEL_PX: f32 = 18.0;
const TAP_MAX_SECS: f32 = 0.45;
const TOUCH_SLOP_PX: f32 = 8.0;

const PAD_FILL: Color = Color::srgba(1.0, 1.0, 1.0, 0.11);
const PAD_BORDER: Color = Color::srgba(1.0, 1.0, 1.0, 0.20);
const FACE_A: Color = Color::srgba(0.87, 0.28, 0.30, 0.24);
const FACE_B: Color = Color::srgba(0.95, 0.76, 0.24, 0.24);
const FACE_X: Color = Color::srgba(0.31, 0.51, 0.91, 0.24);
const FACE_Y: Color = Color::srgba(0.37, 0.73, 0.44, 0.24);
const PAD_HELD_GAIN: f32 = 2.8;
const PAD_SQUEEZE: f32 = 0.06;
const PAD_LABEL: Color = Color::srgba(1.0, 1.0, 1.0, 0.62);
const PAD_LABEL_PX: f32 = 21.0;
const FACE_SIZE: f32 = 38.0;
const CHIP_HEIGHT: f32 = 9.0;
const CHIP_PITCH: f32 = CHIP_HEIGHT + 4.0;
const CHIP_TOP: f32 = 4.0;
const HINT_PILL_ICON_PX: f32 = 22.0;
const PAUSE_SCRIM: Color = Color::srgba(0.02, 0.02, 0.03, 0.72);
const PRESS_RATE: f32 = 22.0;
const PRESS_EPSILON: f32 = 0.002;
const PAD_RADIUS_PX: f32 = 14.0;

#[derive(Resource, Clone, Copy)]
pub struct TouchControls(pub bool);

pub fn touch_enabled(controls: Res<TouchControls>) -> bool {
    controls.0
}

#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TouchTarget {
    #[default]
    Start,
    Letter(usize),
    Flip,
    View,
}

#[derive(Component, Default, Clone)]
pub struct PadButtons(pub Vec<Button>);

#[derive(Component, Default, Clone, Copy, PartialEq, Eq)]
pub enum PadAction {
    #[default]
    Library,
    Pause,
    Resume,
    SaveState,
    LoadState,
}

#[derive(Component, Default, Clone)]
pub struct PauseMenu;

#[derive(Component, Default, Clone)]
pub struct PadRoot;

pub fn update_pause_menu(
    paused: Res<Paused>,
    mut menus: Query<&mut Node, (With<PauseMenu>, Without<PadRoot>)>,
    mut pads: Query<&mut Node, (With<PadRoot>, Without<PauseMenu>)>,
) {
    if !paused.is_changed() {
        return;
    }

    for mut node in &mut menus {
        node.display = display(paused.0);
    }
    for mut node in &mut pads {
        node.display = display(!paused.0);
    }
}

#[derive(Component, Clone)]
pub struct PadPress {
    pub held: bool,
    pub squeeze: f32,
    pub rest: Color,
}

impl Default for PadPress {
    fn default() -> Self {
        Self {
            held: false,
            squeeze: PAD_SQUEEZE,
            rest: PAD_FILL,
        }
    }
}

fn pad_surface(rest: Color, squeeze: f32) -> impl Scene {
    bsn! {
        BackgroundColor(rest)
        template_value(BorderColor::all(PAD_BORDER))
        template_value(UiTransform::IDENTITY)
        template_value(PadPress {
            rest,
            squeeze,
            ..default()
        })
    }
}

pub fn animate_pad(
    time: Res<Time>,
    mut controls: Query<(&PadPress, &mut BackgroundColor, &mut UiTransform)>,
) {
    let t = 1.0 - (-time.delta_secs() * PRESS_RATE).exp();
    for (press, mut background, mut transform) in &mut controls {
        let (fill, scale) = if press.held {
            (
                press.rest.with_alpha(press.rest.alpha() * PAD_HELD_GAIN),
                1.0 - press.squeeze,
            )
        } else {
            (press.rest, 1.0)
        };
        let scale = Vec2::splat(scale);

        let settled = background.0.to_linear().distance(&fill.to_linear()) < PRESS_EPSILON
            && transform.scale.distance(scale) < PRESS_EPSILON;
        if settled {
            background.set_if_neq(BackgroundColor(fill));
            if transform.scale != scale {
                transform.scale = scale;
            }
            continue;
        }

        background.0 = background.0.mix(&fill, t);
        transform.scale = transform.scale.lerp(scale, t);
    }
}

#[derive(Default)]
pub struct BrowseDrag {
    touch: Option<u64>,

    residual: f32,
    travel: f32,
    elapsed: f32,

    velocity: f32,

    fling: f32,
}

fn contains(node: &ComputedNode, transform: &UiGlobalTransform, point: Vec2) -> bool {
    if node.size() == Vec2::ZERO {
        return false;
    }

    let inverse_scale = node.inverse_scale_factor();
    let half = node.size() * inverse_scale * 0.5 + TOUCH_SLOP_PX;
    let center = transform.translation * inverse_scale;
    (point - center).abs().cmple(half).all()
}

#[allow(clippy::too_many_arguments)]
pub fn browse_touch(
    touches: Res<Touches>,
    time: Res<Time>,
    ui_scale: Res<UiScale>,
    mut commands: Commands,
    mut state: ResMut<LauncherState>,
    mut timers: ResMut<ModeTimers>,
    mut next_mode: ResMut<NextState<LauncherMode>>,
    library: Res<Library>,
    models: Res<Models>,
    ui_sounds: Res<UiSounds>,
    mut carts: Query<&mut DetailCart>,
    mut drag: Local<BrowseDrag>,
    targets: Query<(&TouchTarget, &ComputedNode, &UiGlobalTransform)>,
) {
    let scale = ui_scale.0;
    let dt = time.delta_secs().max(f32::EPSILON);

    if drag.touch.is_none()
        && let Some(touch) = touches.iter_just_pressed().next()
    {
        *drag = BrowseDrag {
            touch: Some(touch.id()),
            ..default()
        };
    }

    if drag.touch.is_none() && drag.fling != 0.0 {
        drag.residual += drag.fling * dt;
        drag.fling *= (-FLING_DECAY * dt).exp();
        if drag.fling.abs() < FLING_MIN_PX {
            drag.fling = 0.0;
        }
    }

    let stopped = spend_swipe(
        &mut drag,
        &mut commands,
        &mut state,
        &library,
        &mut carts,
        &models,
        &ui_sounds,
    );
    if stopped {
        drag.fling = 0.0;
        drag.residual = 0.0;
    }

    let Some(id) = drag.touch else {
        return;
    };

    if touches.just_canceled(id) {
        drag.touch = None;
        return;
    }

    if let Some(touch) = touches.get_pressed(id) {
        let delta = touch.delta() / scale;

        let along = if state.view_mode.is_vertical() {
            delta.y
        } else {
            delta.x
        };

        drag.residual += along;
        drag.velocity = drag.velocity.lerp(along / dt, 0.4);
        drag.travel += delta.length();
        drag.elapsed += time.delta_secs();
        return;
    }

    let Some(touch) = touches.get_released(id) else {
        drag.touch = None;
        return;
    };
    let released = touch.position() / scale;
    let tapped = drag.travel <= TAP_MAX_TRAVEL_PX && drag.elapsed <= TAP_MAX_SECS;
    drag.touch = None;
    drag.fling = if tapped {
        0.0
    } else {
        drag.velocity.clamp(-FLING_MAX_PX, FLING_MAX_PX)
    };
    if !tapped {
        return;
    }

    let hit = targets
        .iter()
        .filter(|(_, node, transform)| contains(node, transform, released))
        .min_by_key(|(_, node, _)| node.size().element_product() as u32)
        .map(|(target, _, _)| *target);

    match hit {
        Some(TouchTarget::Start) => launch::begin_launch(
            &mut commands,
            &mut state,
            &mut timers,
            &mut next_mode,
            &library,
            &mut carts,
            &ui_sounds,
        ),
        Some(TouchTarget::Letter(letter)) => {
            if let Some(index) = library.first_with_letter(letter) {
                input::select(
                    index,
                    &mut commands,
                    &mut state,
                    &library,
                    &mut carts,
                    &models,
                    &ui_sounds,
                );
            }
        }
        Some(TouchTarget::Flip) => state.box_flipped = !state.box_flipped,
        Some(TouchTarget::View) => {
            state.view_mode = state.view_mode.next();
            commands.spawn(ui_sound(ui_sounds.scroll.clone()));
        }
        None => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn spend_swipe(
    drag: &mut BrowseDrag,
    commands: &mut Commands,
    state: &mut LauncherState,
    library: &Library,
    carts: &mut Query<&mut DetailCart>,
    models: &Models,
    ui_sounds: &UiSounds,
) -> bool {
    let steps = (drag.residual / SWIPE_STEP_PX).trunc();
    if steps == 0.0 || library.entries.is_empty() {
        return false;
    }

    drag.residual -= steps * SWIPE_STEP_PX;
    let last = library.entries.len() as isize - 1;
    let wanted = state.selected as isize - steps as isize;
    let target = wanted.clamp(0, last);
    input::select(
        target as usize,
        commands,
        state,
        library,
        carts,
        models,
        ui_sounds,
    );

    wanted != target
}

#[allow(clippy::too_many_arguments)]
pub fn game_touch(
    touches: Res<Touches>,
    ui_scale: Res<UiScale>,
    mut next_mode: ResMut<NextState<LauncherMode>>,
    mut timers: ResMut<ModeTimers>,
    mut paused: ResMut<Paused>,
    mut zones: Query<(
        &PadButtons,
        &ComputedNode,
        &UiGlobalTransform,
        &mut PadPress,
    )>,
    mut actions: Query<
        (&PadAction, &ComputedNode, &UiGlobalTransform, &mut PadPress),
        Without<PadButtons>,
    >,
    session: Option<ResMut<GameSession>>,
) {
    let Some(mut session) = session else {
        return;
    };
    let scale = ui_scale.0;

    let mut held = [false; Button::ALL.len()];
    for (zone, node, transform, _) in &zones {
        let down = touches
            .iter()
            .any(|touch| contains(node, transform, touch.position() / scale));
        if down {
            for &button in &zone.0 {
                held[button as usize] = true;
            }
        }
    }

    for (zone, _, _, mut press) in &mut zones {
        press.held = !zone.0.is_empty() && zone.0.iter().all(|&button| held[button as usize]);
    }
    for (_, node, transform, mut press) in &mut actions {
        press.held = touches
            .iter()
            .any(|touch| contains(node, transform, touch.position() / scale));
    }

    for button in Button::ALL {
        session.core.set_button(0, button, held[button as usize]);
    }

    for touch in touches.iter_just_released() {
        let point = touch.position() / scale;
        let action = actions
            .iter()
            .find(|(_, node, transform, _)| contains(node, transform, point))
            .map(|(action, ..)| *action);

        match action {
            Some(PadAction::Library) => {
                paused.0 = false;
                launch::return_to_library(Some(&session), &mut next_mode, &mut timers);
            }
            Some(PadAction::Pause) => paused.0 = true,
            Some(PadAction::Resume) => paused.0 = false,
            Some(PadAction::SaveState) => session.save_state(),

            Some(PadAction::LoadState) => {
                session.load_state();
                paused.0 = false;
            }
            None => {}
        }
    }
}

pub fn pause_menu(icons: &Icons, fonts: &Fonts) -> impl Scene {
    let mono = fonts.mono_medium.clone();
    let entries = [
        ("RESUME", icons.play.clone(), PadAction::Resume),
        ("SAVE STATE", icons.save.clone(), PadAction::SaveState),
        ("LOAD STATE", icons.load.clone(), PadAction::LoadState),
        ("LIBRARY", icons.library.clone(), PadAction::Library),
    ];
    let entries: Vec<_> = entries
        .into_iter()
        .map(|(label, icon, action)| menu_entry(label, icon, action, mono.clone()))
        .collect();

    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            display: Display::None,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(12),
        }
        BackgroundColor(PAUSE_SCRIM)
        GlobalZIndex(22)
        PauseMenu
        Children [{entries}]
    }
}

fn menu_entry(
    label: &'static str,
    icon: Handle<Image>,
    action: PadAction,
    mono: Handle<Font>,
) -> impl Scene {
    bsn! {
        Node {
            width: px(240),
            padding: UiRect::axes(px(20), px(12)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(14)),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(12),
        }
        pad_surface(PAD_FILL, PAD_SQUEEZE)
        template_value(action)
        Children [
            (
                Node {
                    width: px(PAD_LABEL_PX),
                    height: px(PAD_LABEL_PX),
                }
                ImageNode { image: {icon}, color: {PAD_LABEL} }
            ),
            pad_label(label, mono),
        ]
    }
}

pub fn game_pad(fonts: &Fonts) -> impl Scene {
    let mono = fonts.mono_medium.clone();
    bsn! {
        safe_area_root()
        PadRoot
        GlobalZIndex(21)
        Children [
            direction_pad(),
            face_buttons(mono.clone()),
            (pad_chip("L", true, chip_top(0), mono.clone()) PadButtons(vec![Button::L])),
            (pad_chip("R", false, chip_top(0), mono.clone()) PadButtons(vec![Button::R])),
            (
                pad_chip("SELECT", false, chip_top(1), mono.clone())
                PadButtons(vec![Button::Select])
            ),
            (
                pad_chip("START", false, chip_top(2), mono.clone())
                PadButtons(vec![Button::Start])
            ),
            (pad_chip("PAUSE", true, chip_top(1), mono) template_value(PadAction::Pause)),
        ]
    }
}

fn pad_label(label: &'static str, mono: Handle<Font>) -> impl Scene {
    bsn! {
        Text(label)
        text_font(mono, PAD_LABEL_PX)
        TextColor(PAD_LABEL)
    }
}

fn direction_pad() -> impl Scene {
    let rows: Vec<_> = (0..3).map(direction_row).collect();

    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: percent(1),
            bottom: percent(5),

            height: percent(40),
            aspect_ratio: {Some(1.0)},
            flex_direction: FlexDirection::Column,
        }
        Children [{rows}]
    }
}

fn direction_row(row: usize) -> impl Scene {
    let cells: Vec<_> = (0..3).map(|column| direction_cell(row, column)).collect();
    bsn! {
        Node {
            width: percent(100),
            height: percent(33.34),
            flex_direction: FlexDirection::Row,
        }
        Children [{cells}]
    }
}

fn direction_cell(row: usize, column: usize) -> impl Scene {
    let vertical = [Some(Button::Up), None, Some(Button::Down)][row];
    let horizontal = [Some(Button::Left), None, Some(Button::Right)][column];
    let buttons: Vec<_> = [vertical, horizontal].into_iter().flatten().collect();

    let cross = row == 1 || column == 1;
    let fill = if cross { PAD_FILL } else { Color::NONE };

    let rounded = |vertical: usize, horizontal: usize| {
        let outward = (row, column) == (vertical, 1) || (row, column) == (1, horizontal);
        if outward { px(PAD_RADIUS_PX) } else { px(0) }
    };
    let radii = BorderRadius {
        top_left: rounded(0, 0),
        top_right: rounded(0, 2),
        bottom_left: rounded(2, 0),
        bottom_right: rounded(2, 2),
    };

    bsn! {
        Node {
            width: percent(33.34),
            height: percent(100),
            border_radius: {radii},
        }

        pad_surface(fill, 0.0)
        template_value(BorderColor::all(Color::NONE))
        PadButtons(buttons)
    }
}

fn face_buttons(mono: Handle<Font>) -> impl Scene {
    let mid = (100.0 - FACE_SIZE) / 2.0;
    let far = 100.0 - FACE_SIZE;
    let buttons = [
        ("X", Button::X, FACE_X, mid, 0.0),
        ("Y", Button::Y, FACE_Y, 0.0, mid),
        ("A", Button::A, FACE_A, far, mid),
        ("B", Button::B, FACE_B, mid, far),
    ];
    let buttons: Vec<_> = buttons
        .into_iter()
        .map(|(label, button, tint, left, top)| {
            face_button(label, button, tint, left, top, mono.clone())
        })
        .collect();

    bsn! {
        Node {
            position_type: PositionType::Absolute,
            right: percent(1),
            bottom: percent(5),
            height: percent(40),
            aspect_ratio: {Some(1.0)},
        }
        Children [{buttons}]
    }
}

fn face_button(
    label: &'static str,
    button: Button,
    tint: Color,
    left: f32,
    top: f32,
    mono: Handle<Font>,
) -> impl Scene {
    let ring = tint.with_alpha(tint.alpha() * 2.0);
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: {percent(left)},
            top: {percent(top)},
            width: {percent(FACE_SIZE)},
            height: {percent(FACE_SIZE)},
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::MAX,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        pad_surface(tint, PAD_SQUEEZE)
        template_value(BorderColor::all(ring))
        PadButtons(vec![button])
        Children [ pad_label(label, mono) ]
    }
}

fn chip_top(index: usize) -> f32 {
    CHIP_TOP + index as f32 * CHIP_PITCH
}

fn pad_chip(label: &'static str, left_edge: bool, top: f32, mono: Handle<Font>) -> impl Scene {
    let (left, right) = if left_edge {
        (percent(1), Val::Auto)
    } else {
        (Val::Auto, percent(1))
    };
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: {left},
            right: {right},
            top: {percent(top)},
            width: percent(13),
            height: {percent(CHIP_HEIGHT)},
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(12)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        pad_surface(PAD_FILL, PAD_SQUEEZE)
        Children [ pad_label(label, mono) ]
    }
}

pub fn browse_hints(icons: &Icons, prompts: &PromptIcons, mono: Handle<Font>) -> impl Scene {
    let swipe = prompts.swipe_horizontal.clone();
    let play = prompts.tap.clone();
    let flip = icons.flip.clone();
    let view = icons.view.clone();
    bsn! {
        Node {
            position_type: PositionType::Absolute,

            right: px(BUBBLE_MARGIN_PX),
            bottom: px(EDGE_MARGIN_PX),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(10),
        }
        GlobalZIndex(6)
        Children [
            hint_pill("Browse", swipe_key(swipe), mono.clone()),
            (
                hint_pill("Play", pill_key(play), mono.clone())
                template_value(TouchTarget::Start)
            ),
            (hint_pill("Flip", pill_key(flip), mono.clone()) template_value(TouchTarget::Flip)),
            (hint_pill("View", pill_key(view), mono) template_value(TouchTarget::View)),
        ]
    }
}

fn pill_key(image: Handle<Image>) -> impl Scene {
    tinted_hint_key(image, HINT_PILL_ICON_PX)
}

fn swipe_key(image: Handle<Image>) -> impl Scene {
    bsn! {
        pill_key(image)
        SwipeHintIcon
    }
}

pub fn update_swipe_hint(
    state: Res<LauncherState>,
    prompts: Res<PromptIcons>,
    mut icons: Query<&mut ImageNode, With<SwipeHintIcon>>,
) {
    if !state.is_changed() {
        return;
    }

    let wanted = if state.view_mode.is_vertical() {
        &prompts.swipe_vertical
    } else {
        &prompts.swipe_horizontal
    };
    for mut icon in &mut icons {
        if icon.image != *wanted {
            icon.image = wanted.clone();
        }
    }
}

fn hint_pill(label: &'static str, icon: impl Scene, mono: Handle<Font>) -> impl Scene {
    let icon = vec![icon];
    bsn! {
        Node {
            padding: UiRect::axes(px(12), px(4)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::MAX,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        BackgroundColor(Color::srgba(0.99, 0.98, 1.0, 0.55))
        template_value(BorderColor::all(HAIRLINE))
        Children [
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(6),
                }
                Children [
                    {icon},
                    hint_label(label, mono, TEXT_SECONDARY),
                ]
            )
        ]
    }
}
