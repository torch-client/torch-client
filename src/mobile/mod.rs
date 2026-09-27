pub mod look;
pub mod pad;
pub mod pointer;

use bevy::input::ButtonInput;
use bevy::input::mouse::MouseButton;
use bevy::input::touch::Touches;
use bevy::math::Vec2;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::gui::painter::Painter;
use crate::gui::render::effective_gui_scale;
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::renderer::input::{CameraAngles, apply_look};
use crate::renderer::systems::{Shared, WorldCamera};
use crate::session::{
    Gamemode, MOVE_BACK, MOVE_DESCEND, MOVE_FORWARD, MOVE_JUMP, MOVE_LEFT, MOVE_RIGHT, MOVE_SNEAK,
    MOVE_SPRINT,
};
use look::{LastTap, LookTrack, MobileConfig};
use pad::{Cell, Control, Layout, Movement, PadState, Rect, Stick};
use pointer::{Pointer, PointerId};

struct Owner {
    id: PointerId,
    control: Control,
    look: Option<LookTrack>,
    stick_at: Vec2,
    seen: bool,
}

#[derive(Default)]
pub struct MobileUi {
    pub cfg: MobileConfig,
    pad: PadState,
    pointers: Vec<Pointer>,
    owners: Vec<Owner>,
    pressed_cells: u16,
    use_pressed: bool,
    stick_at: Option<Vec2>,
    last_tap: Option<LastTap>,
    hold_hints: Vec<(Vec2, f32)>,
}

impl MobileUi {
    fn pressed(&self, i: usize) -> bool {
        self.pressed_cells & (1 << i) != 0
    }

    fn release_all(&mut self) {
        self.owners.clear();
        self.pressed_cells = 0;
        self.use_pressed = false;
        self.stick_at = None;
        self.last_tap = None;
        self.hold_hints.clear();
    }
}

pub fn register(app: &mut App) {
    app.add_systems(
        Update,
        mobile_input
            .before(crate::renderer::intent::publish)
            .after(crate::gui::render::collect_input)
            .before(crate::gui::render::draw_gui)
            .run_if(pad_is_drawn),
    );
}

fn pad_is_drawn(state: Res<GuiState>) -> bool {
    !state.in_menu()
}

#[allow(clippy::too_many_arguments)]
fn mobile_input(
    windows: Query<&Window, With<PrimaryWindow>>,
    touches: Res<Touches>,
    buttons: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    shared: Res<Shared>,
    mut gui: ResMut<GuiState>,
    mut gui_input: ResMut<crate::gui::render::GuiInput>,
    mut angles: ResMut<CameraAngles>,
    mut camera: Query<&mut Transform, With<WorldCamera>>,
    mut intent: ResMut<crate::renderer::intent::Intent>,
) {
    let Ok(window) = windows.single() else { return };
    let gui = &mut *gui;

    let (win_w, win_h) = (
        window.physical_width() as f32,
        window.physical_height() as f32,
    );
    let scale = effective_gui_scale(win_w, win_h, gui.options.gui_scale);
    let layout = Layout::new(
        win_w / scale,
        win_h / scale,
        gui.mobile.cfg.safe_inset,
        gui.options.touch_movement,
    );
    let now = time.elapsed_secs();

    let ui = &mut gui.mobile;
    let cfg = ui.cfg;
    pointer::gather(
        &mut ui.pointers,
        &touches,
        &buttons,
        window.physical_cursor_position(),
        scale,
        window.scale_factor(),
    );

    if gui.screen.is_open() {
        let closed = gui.screen != Screen::Death
            && ui
                .pointers
                .iter()
                .any(|p| p.pressed && layout.close_button.contains(p.pos));
        ui.release_all();
        if closed {
            let next = close_target(gui);
            gui.nav = Some(next);
            swallow_click(&mut gui_input);
        }
        return;
    }
    if gui.hide_gui {
        gui.mobile.release_all();
        let mut s = shared.0.lock().unwrap();
        s.session.attack_held = false;
        s.session.use_held = false;
        return;
    }

    let ui = &mut gui.mobile;
    let MobileUi {
        pad,
        pointers,
        owners,
        pressed_cells,
        use_pressed,
        stick_at,
        last_tap,
        hold_hints,
        ..
    } = ui;

    for owner in owners.iter_mut() {
        owner.seen = false;
    }

    let mut nav: Option<Screen> = None;
    let mut hotbar: Option<u8> = None;
    let mut attack_click = false;
    let mut use_click = false;
    let mut fly_toggle = false;
    let mut look_delta = Vec2::ZERO;
    let mut claimed = false;

    for p in pointers.iter() {
        if p.pressed {
            let control = layout.hit(p.pos);
            match control {
                Control::Pad(Cell::Inventory) => nav = Some(inventory_screen(&shared)),
                Control::Pad(Cell::Pause) => nav = Some(Screen::Pause),
                Control::Pad(Cell::Jump) => {
                    fly_toggle |= pad.jump_double_tap(now, cfg.double_tap_secs);
                }
                Control::Pad(cell) => pad.press(cell, now, cfg.double_tap_secs),
                Control::Hotbar(slot) => hotbar = Some(slot),
                Control::Use => use_click = true,
                Control::Chat => {
                    gui.chat.open("");
                    nav = Some(Screen::Chat);
                }
                Control::Stick | Control::Look => {}
            }
            claimed |= control != Control::Look;
            owners.retain(|o| o.id != p.id);
            owners.push(Owner {
                id: p.id,
                control,
                look: (control == Control::Look).then(|| LookTrack::new(p.pos, now)),
                stick_at: p.pos,
                seen: true,
            });
            continue;
        }

        let Some(owner) = owners.iter_mut().find(|o| o.id == p.id) else {
            continue;
        };
        owner.seen = !p.released;
        if owner.control == Control::Stick {
            owner.stick_at = p.pos;
        }
        if let Some(track) = owner.look.as_mut() {
            let moved = track.advance(p.pos);
            look_delta += moved;
            if p.released && track.is_tap(now, &cfg) {
                if last_tap.is_some_and(|t| t.doubles(track.start, now, &cfg)) {
                    use_click = true;
                    *last_tap = None;
                } else {
                    attack_click = true;
                    *last_tap = Some(LastTap {
                        pos: track.start,
                        time: now,
                    });
                }
            }
        }
    }
    owners.retain(|o| o.seen);

    let mut move_bits = 0u8;
    let mut attack_held = false;
    let mut use_held = false;
    *pressed_cells = 0;
    *use_pressed = false;
    *stick_at = None;
    hold_hints.clear();
    for owner in owners.iter() {
        match owner.control {
            Control::Pad(cell) => {
                *pressed_cells |= 1 << cell.index();
                move_bits |= match cell {
                    Cell::Forward => MOVE_FORWARD,
                    Cell::Back => MOVE_BACK,
                    Cell::Left => MOVE_LEFT,
                    Cell::Right => MOVE_RIGHT,
                    Cell::Jump => MOVE_JUMP,
                    _ => 0,
                };
            }
            Control::Stick => {
                let Some(stick) = layout.stick else { continue };
                *stick_at = Some(stick.knob(owner.stick_at));
                let Some((x, y)) = stick.direction(owner.stick_at) else {
                    continue;
                };
                if x > 0 {
                    move_bits |= MOVE_RIGHT;
                } else if x < 0 {
                    move_bits |= MOVE_LEFT;
                }
                if y < 0 {
                    move_bits |= MOVE_FORWARD;
                } else if y > 0 {
                    move_bits |= MOVE_BACK;
                }
            }
            Control::Use => {
                *use_pressed = true;
                use_held = true;
            }
            Control::Look => {
                let Some(track) = owner.look else { continue };
                if track.attacking(now, &cfg) {
                    attack_held = true;
                }
                hold_hints.push((track.last, track.progress(now, &cfg)));
            }
            Control::Hotbar(_) | Control::Chat => {}
        }
    }
    if pad.auto_walk {
        move_bits |= MOVE_FORWARD;
    }
    let (sneak, sprint) = (pad.sneak, pad.sprint);

    if look_delta != Vec2::ZERO {
        apply_look(
            look_delta.x * cfg.look_deg_per_unit,
            look_delta.y * cfg.look_deg_per_unit,
            &mut angles,
            &mut camera,
            &shared,
            false,
            time.delta_secs(),
        );
    }

    if claimed {
        swallow_click(&mut gui_input);
    }

    if let Some(next) = nav {
        gui.nav = Some(next);
    }

    let flying = {
        let mut s = shared.0.lock().unwrap();
        if fly_toggle && matches!(s.session.gamemode, Gamemode::Creative | Gamemode::Spectator) {
            s.session.fly_toggle = true;
        }
        if let Some(slot) = hotbar {
            s.session.hotbar_target = Some(slot);
        }
        s.session.flying
    };
    let descend = flying && sneak;
    intent.walk(
        move_bits
            | (sneak as u8) * MOVE_SNEAK
            | (descend as u8) * MOVE_DESCEND
            | (sprint as u8) * MOVE_SPRINT,
    );
    intent.hold(attack_held, use_held);
    intent.click(attack_click, use_click);
}

fn swallow_click(input: &mut crate::gui::render::GuiInput) {
    input.left_click = false;
    input.left_down = false;
    input.left_release = false;
    input.double_click = false;
    input.triple_click = false;
}

fn close_target(gui: &mut GuiState) -> Screen {
    match gui.screen {
        Screen::Options => gui.options_parent,
        Screen::VideoSettings | Screen::Controls | Screen::GameSettings => Screen::Options,
        #[cfg(feature = "shader_support")]
        Screen::ShaderPacks => Screen::VideoSettings,
        #[cfg(feature = "shader_support")]
        Screen::ShaderOptions if gui.shaderpacks.pop_screen() => Screen::ShaderOptions,
        #[cfg(feature = "shader_support")]
        Screen::ShaderOptions => Screen::ShaderPacks,
        Screen::Chat => {
            gui.chat.close();
            Screen::None
        }
        _ => Screen::None,
    }
}

fn inventory_screen(shared: &Shared) -> Screen {
    if shared.0.lock().unwrap().session.gamemode == Gamemode::Creative {
        Screen::Creative
    } else {
        Screen::Inventory
    }
}

const FACE: u32 = 0x5010_1014;
const FACE_PRESSED: u32 = 0xA0E0_E0E0;
const FACE_LATCHED: u32 = 0xE0_5CC8_4A;
const BORDER: u32 = 0x80FF_FFFF;
const BORDER_LATCHED: u32 = 0xFFD8_FFA0;
const LABEL: u32 = 0xFF_FFFF;
const LABEL_PRESSED: u32 = 0x20_2020;
const LINE_H: f32 = 8.0;
const GAP: f32 = 1.5;

pub fn draw(p: &mut Painter, ctx: &ScreenCtx, state: &GuiState) {
    if state.in_menu() {
        return;
    }
    let ui = &state.mobile;
    let layout = Layout::new(
        ctx.vw,
        ctx.vh,
        ui.cfg.safe_inset,
        state.options.touch_movement,
    );
    if state.screen.is_open() {
        if state.screen != Screen::Death {
            button(p, layout.close_button, "X", false, false);
        }
        return;
    }
    if state.hide_gui {
        return;
    }
    match layout.movement {
        Movement::Buttons => {
            for (i, cell) in Cell::ALL.iter().enumerate() {
                button(
                    p,
                    layout.cell_rect(i),
                    cell.label(),
                    ui.pressed(i),
                    ui.pad.latched(*cell),
                );
            }
        }
        Movement::PadStick => {
            for (i, cell) in Cell::ALL.iter().enumerate() {
                if *cell == Cell::Jump {
                    continue;
                }
                let pressable = matches!(
                    cell,
                    Cell::SneakToggle | Cell::SprintToggle | Cell::Inventory | Cell::Pause
                );
                button(
                    p,
                    layout.cell_rect(i),
                    cell.label(),
                    pressable && ui.pressed(i),
                    ui.pad.latched(*cell),
                );
            }
        }
        Movement::Joystick => {
            if let Some(round) = layout.round_cells {
                for (cell, at) in round {
                    round_button(
                        p,
                        at,
                        layout.cell * 0.5,
                        cell.label(),
                        ui.pressed(cell.index()),
                        ui.pad.latched(cell),
                    );
                }
            }
        }
    }
    if let Some(stick) = layout.stick {
        draw_stick(p, stick, ui.stick_at);
    }
    if let Some(jump) = layout.jump_button {
        round_button(
            p,
            jump.center(),
            jump.w * 0.5,
            "JUMP",
            ui.pressed(Cell::Jump.index()),
            false,
        );
    }
    button(p, layout.use_button, "USE", ui.use_pressed, false);
    button(p, layout.close_button, "CHAT", false, false);

    let radius = layout.cell * HINT_RADIUS;
    for (at, progress) in &ui.hold_hints {
        ring(p, *at, radius, HINT_EDGE, BORDER);
        disc(p, *at, (radius - HINT_EDGE) * progress, BORDER);
    }
}

fn draw_stick(p: &mut Painter, stick: Stick, at: Option<Vec2>) {
    disc(p, stick.center, stick.radius, FACE);
    ring(p, stick.center, stick.radius, STICK_EDGE, BORDER);
    let knob = at.unwrap_or(stick.center);
    let knob_r = (stick.radius * STICK_KNOB).max(STICK_EDGE);
    disc(
        p,
        knob,
        knob_r,
        if at.is_some() { FACE_PRESSED } else { FACE },
    );
    ring(p, knob, knob_r, STICK_EDGE, BORDER);
}

fn round_button(p: &mut Painter, c: Vec2, r: f32, label: &str, pressed: bool, latched: bool) {
    let face = if pressed {
        FACE_PRESSED
    } else if latched {
        FACE_LATCHED
    } else {
        FACE
    };
    let (edge, border) = if latched {
        (2.0, BORDER_LATCHED)
    } else {
        (1.0, BORDER)
    };
    disc(p, c, r, face);
    ring(p, c, r, edge, border);
    let text_w = p.atlas.font.width_str(label);
    p.text_str(
        label,
        (c.x - text_w / 2.0).round(),
        (c.y - LINE_H / 2.0).round(),
        if pressed || latched {
            LABEL_PRESSED
        } else {
            LABEL
        },
        !pressed && !latched,
    );
}

const STICK_EDGE: f32 = 1.0;
const STICK_KNOB: f32 = 0.45;

const HINT_RADIUS: f32 = 0.45;
const HINT_EDGE: f32 = 1.5;

fn disc(p: &mut Painter, c: Vec2, r: f32, argb: u32) {
    scan(c, r, |y, dx| p.fill(c.x - dx, y, dx * 2.0, 1.0, argb));
}

fn ring(p: &mut Painter, c: Vec2, r: f32, thickness: f32, argb: u32) {
    let inner = (r - thickness).max(0.0);
    scan(c, r, |y, dx| {
        let dy = y + 0.5 - c.y;
        let inner_dx = inner * inner - dy * dy;
        if inner_dx <= 0.0 {
            p.fill(c.x - dx, y, dx * 2.0, 1.0, argb);
            return;
        }
        let ix = inner_dx.sqrt();
        p.fill(c.x - dx, y, dx - ix, 1.0, argb);
        p.fill(c.x + ix, y, dx - ix, 1.0, argb);
    });
}

fn scan(c: Vec2, r: f32, mut row: impl FnMut(f32, f32)) {
    if r <= 0.0 {
        return;
    }
    let top = (c.y - r).floor();
    let rows = (r * 2.0).ceil() as i32;
    for i in 0..=rows {
        let y = top + i as f32;
        let dy = y + 0.5 - c.y;
        let dx2 = r * r - dy * dy;
        if dx2 <= 0.0 {
            continue;
        }
        row(y, dx2.sqrt());
    }
}

fn button(p: &mut Painter, r: Rect, label: &str, pressed: bool, latched: bool) {
    let (x, y, w, h) = (r.x + GAP, r.y + GAP, r.w - GAP * 2.0, r.h - GAP * 2.0);
    let face = if pressed {
        FACE_PRESSED
    } else if latched {
        FACE_LATCHED
    } else {
        FACE
    };
    p.fill(x, y, w, h, face);
    let (edge, border) = if latched {
        (2.0, BORDER_LATCHED)
    } else {
        (1.0, BORDER)
    };
    p.fill(x, y, w, edge, border);
    p.fill(x, y + h - edge, w, edge, border);
    p.fill(x, y, edge, h, border);
    p.fill(x + w - edge, y, edge, h, border);
    if latched {
        p.fill(
            x + edge,
            y + h - edge * 3.0,
            w - edge * 2.0,
            edge,
            BORDER_LATCHED,
        );
    }

    let text_w = p.atlas.font.width_str(label);
    p.text_str(
        label,
        (x + (w - text_w) / 2.0).round(),
        (y + (h - LINE_H) / 2.0).round(),
        if pressed || latched {
            LABEL_PRESSED
        } else {
            LABEL
        },
        !pressed && !latched,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanned_circle_stays_inside_its_radius() {
        for r in [1.0_f32, 2.5, 7.0, 17.3, 40.0] {
            let c = Vec2::new(100.25, 50.75);
            let mut rows = 0;
            scan(c, r, |y, dx| {
                rows += 1;
                assert!(dx <= r, "row at {y} is wider than the circle");
                assert!(y + 1.0 > c.y - r && y < c.y + r, "row at {y} is outside");
            });
            assert!(rows > 0, "radius {r} drew nothing");
            assert!(rows <= (r * 2.0).ceil() as i32 + 1);
        }
    }

    #[test]
    fn empty_circle_draws_nothing() {
        let mut rows = 0;
        scan(Vec2::ZERO, 0.0, |_, _| rows += 1);
        assert_eq!(rows, 0);
    }
}
