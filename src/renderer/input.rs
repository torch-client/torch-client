use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowFocused};

use super::systems::{Shared, WorldCamera, enter_screen};
use crate::gui::keybinds::Action;
use crate::session::{
    DropRequest, GAME_MASTER_LEVEL, Gamemode, InvAction, MOVE_BACK, MOVE_DESCEND, MOVE_FORWARD,
    MOVE_JUMP, MOVE_LEFT, MOVE_RIGHT, MOVE_SNEAK, MOVE_SPRINT,
};

#[derive(Resource, Default)]
pub(crate) struct CameraAngles {
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
}

impl CameraAngles {
    pub(crate) fn rotation(&self) -> Quat {
        Quat::from_rotation_y(self.yaw.to_radians())
            * Quat::from_rotation_x(self.pitch.to_radians())
    }
}

#[derive(Resource, Default)]
pub struct CameraView {
    perspective: Perspective,
    free: bool,
    free_eye: bevy::math::DVec3,
}

impl CameraView {
    pub fn free(&self) -> bool {
        self.free
    }

    pub fn third_person(&self) -> bool {
        self.perspective != Perspective::FirstPerson && !self.free
    }

    pub fn mirrored(&self) -> bool {
        self.perspective == Perspective::ThirdPersonFront
    }

    pub fn detached(&self) -> bool {
        self.perspective != Perspective::FirstPerson || self.free
    }

    pub fn first_person(&self) -> bool {
        !self.detached()
    }

    pub fn free_eye(&self) -> bevy::math::DVec3 {
        self.free_eye
    }
}

#[derive(Resource, Default)]
pub struct FreecamTouch {
    pub dir: Vec2,
    pub up: bool,
    pub down: bool,
    pub sprint: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Perspective {
    #[default]
    FirstPerson,
    ThirdPersonBack,
    ThirdPersonFront,
}

impl Perspective {
    fn cycle(self) -> Self {
        match self {
            Self::FirstPerson => Self::ThirdPersonBack,
            Self::ThirdPersonBack => Self::ThirdPersonFront,
            Self::ThirdPersonFront => Self::FirstPerson,
        }
    }
}

pub(crate) fn run_exit_tasks() {
    save_on_session_end();
    crate::client::bot::shutdown_connection();
}

#[cfg(target_arch = "wasm32")]
pub(super) fn run_exit_tasks_on_exit(mut exit: bevy::ecs::message::MessageReader<AppExit>) {
    if exit.read().next().is_some() {
        run_exit_tasks();
    }
}

#[inline]
fn save_on_session_end() {
    #[cfg(lua_mods)]
    crate::modules::script::leave();
    crate::modules::pathfinder::reset();
    crate::modules::save();
    crate::gui::command_history::save();
}

fn reset_session_gui(
    state: &mut crate::gui::GuiState,
    #[cfg(feature = "skins")] faces: &mut crate::gui::player_faces::PlayerFaces,
) {
    save_on_session_end();
    state.chat.reset_for_session();
    state.hud = Default::default();
    state.hud_overlays = Default::default();
    state.toasts.clear();
    state.advancements = Default::default();
    #[cfg(feature = "skins")]
    faces.reset();
}

struct NavPoll {
    ended: Option<Option<Vec<crate::text::Span>>>,
    container_id: i32,
    container_kind: crate::session::ContainerKind,
    sign_editor_open: Option<crate::session::SignEditRequest>,
    signing_prompt: bool,
    server_pack_prompt: bool,
    book_open: Option<crate::session::InteractionHand>,
    book_edit_open: Option<crate::session::BookEditRequest>,
    command_block_open: Option<crate::session::CommandBlockOpen>,
    game_master_open: Option<crate::session::GameMasterOpen>,
    dialog_show: Option<std::sync::Arc<crate::play::dialog::Dialog>>,
    dialog_clear: bool,
    win_game: bool,
    code_of_conduct: Option<String>,
    reconfiguring: bool,
    sleeping: bool,
    dead: bool,
    death_opening: Option<(Option<Vec<crate::text::Span>>, i32, bool)>,
}

pub(super) fn apply_gui_nav(
    shared: Res<Shared>,
    mut state: ResMut<crate::gui::GuiState>,
    mut windows: Query<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>,
    mut exit: bevy::ecs::message::MessageWriter<AppExit>,
    mut last_container_id: Local<i32>,
    #[cfg(feature = "skins")] mut faces: ResMut<crate::gui::player_faces::PlayerFaces>,
) {
    if std::mem::take(&mut state.quit) {
        shared.0.lock().unwrap().quit_requested = true;
        #[cfg(target_os = "android")]
        if let Some(app) = bevy::android::ANDROID_APP.get() {
            run_exit_tasks();
            crate::step("quit pressed, finishing the activity");
            crate::finish_and_exit(app);
        }
        exit.write(AppExit::Success);
        return;
    }
    if std::mem::take(&mut state.disconnect) {
        crate::client::bot::request_disconnect();
        reset_session_gui(
            &mut state,
            #[cfg(feature = "skins")]
            &mut faces,
        );
    }
    #[cfg(feature = "online_mode")]
    crate::gui::bannotice::poll(&mut state);
    use crate::gui::Screen;
    let poll = {
        let mut s = shared.0.lock().unwrap();
        let ended = std::mem::replace(&mut s.disconnected_pending, false)
            .then(|| s.disconnect_reason.take());
        let dead = s.session.dead;
        let death_opening = dead
            && (state.screen == Screen::None
                || state.screen.is_modal()
                || matches!(
                    state.screen,
                    Screen::Advancements | Screen::Statistics | Screen::GameModeSwitcher
                ))
            && s.session.show_death_screen;
        NavPoll {
            ended,
            container_id: s.session.container_id,
            container_kind: s.session.container_kind,
            sign_editor_open: s.session.sign_editor_open.take(),
            signing_prompt: std::mem::take(&mut s.session.chat_signing_prompt),
            server_pack_prompt: std::mem::take(&mut s.session.server_packs.open_prompt),
            book_open: s.session.book_open.take(),
            book_edit_open: s.session.book_edit_open.take(),
            command_block_open: s.session.command_block_open.take(),
            game_master_open: s.session.game_master_open.take(),
            dialog_show: s.session.dialog_show.take(),
            dialog_clear: std::mem::take(&mut s.session.dialog_clear),
            win_game: std::mem::take(&mut s.session.win_game),
            code_of_conduct: s.session.code_of_conduct.take(),
            reconfiguring: s.session.reconfiguring,
            sleeping: s.session.sleeping,
            dead,
            death_opening: death_opening.then(|| {
                (
                    s.session.death_cause.clone(),
                    s.session.death_score,
                    s.session.hardcore,
                )
            }),
        }
    };
    if let Some(reason) = poll.ended {
        reset_session_gui(
            &mut state,
            #[cfg(feature = "skins")]
            &mut faces,
        );
        state.nav = Some(crate::gui::Screen::Disconnected);
        state.disconnect_reason = reason;
    }
    if let Some(address) = state.connect.take() {
        crate::client::bot::start_bot(address);
        let mut s = shared.0.lock().unwrap();
        s.render_distance_sent = None;
        s.skin_prefs_sent = None;
    }
    let (container_id, kind) = (poll.container_id, poll.container_kind);
    if container_id != *last_container_id && container_id != 0 {
        if kind.is_some() {
            crate::renderer::systems::reset_menu_state(&mut state, &shared);
            state.nav = Some(Screen::Container(kind));
        } else if state.screen.is_container() {
            crate::renderer::systems::reset_menu_state(&mut state, &shared);
            state.nav = Some(Screen::None);
        }
    } else if container_id == 0 && state.screen.is_container() {
        state.nav = Some(Screen::None);
    }
    *last_container_id = container_id;

    let sign_editor_open = poll.sign_editor_open;
    let signing_prompt = poll.signing_prompt;
    let book_open = poll.book_open;
    let book_edit_open = poll.book_edit_open;
    let command_block_open = poll.command_block_open;
    if let Some(req) = command_block_open {
        state.command_block.open(req.pos, req.mode, req.conditional);
        state.nav = Some(Screen::CommandBlock);
    }
    if let Some(req) = poll.game_master_open {
        state.nav = Some(state.op_blocks.open(req, &shared.0));
    }
    if let Some(req) = sign_editor_open {
        let pos = req.pos;
        if state.screen == Screen::SignEdit {
            let mut s = shared.0.lock().unwrap();
            s.session.sign_update = Some(state.sign_edit.close(None));
        }
        state.sign_edit.open(req);
        state.nav = Some(Screen::SignEdit);
        shared.0.lock().unwrap().session.sign_edit_open_pos = Some(pos);
    }

    if book_open.is_some() || book_edit_open.is_some() {
        let (view, edit) = {
            let s = shared.0.lock().unwrap();
            let selected = s.session.hotbar_selected;
            let book_of = |hand: crate::session::InteractionHand| {
                s.session
                    .hotbar
                    .get(hand.hotbar_index(selected))
                    .and_then(|slot| slot.extra.book.as_deref())
                    .cloned()
            };
            (
                book_open.and_then(&book_of),
                book_edit_open.and_then(|req| book_of(req.hand).map(|book| (req, book, selected))),
            )
        };
        if let Some(book) = view {
            state.book.open_view(&book);
            state.nav = Some(Screen::BookView);
        } else if let Some((req, book, selected)) = edit {
            state.book.open_edit(&book, req.hand, selected, &req.author);
            state.nav = Some(Screen::BookEdit);
        }
    }

    if let Some(dialog) = poll.dialog_show {
        let current = state.screen;
        state.dialog.show(dialog, current);
        state.nav = Some(Screen::Dialog);
    }
    if poll.dialog_clear && state.screen == Screen::Dialog {
        state.nav = Some(state.dialog.clear());
    }

    if signing_prompt && state.screen != Screen::SignedChatPrompt {
        state.nav = Some(Screen::SignedChatPrompt);
    }

    if poll.server_pack_prompt
        && state.screen != Screen::ServerPackPrompt
        && state.nav != Some(Screen::Disconnected)
    {
        state.server_pack_parent = state.nav.unwrap_or(state.screen);
        state.nav = Some(Screen::ServerPackPrompt);
    }

    if state.nav != Some(Screen::Disconnected) {
        if poll.win_game {
            state.win.open();
            state.nav = Some(Screen::WinGame);
        }
        if let Some(text) = poll.code_of_conduct {
            let parent = state.nav.unwrap_or(state.screen);
            state.config_phase.open_code_of_conduct(&text, parent);
            state.nav = Some(Screen::CodeOfConduct);
        }
    }
    if state.nav.is_none() {
        if poll.reconfiguring
            && !state.screen_is_menu()
            && !matches!(
                state.screen,
                Screen::Reconfiguring
                    | Screen::CodeOfConduct
                    | Screen::Dialog
                    | Screen::ServerPackPrompt
            )
        {
            state.config_phase.open_reconfig();
            state.nav = Some(Screen::Reconfiguring);
        } else if !poll.reconfiguring && state.screen == Screen::Reconfiguring {
            state.nav = Some(Screen::None);
        }
    }

    if state.nav.is_none() {
        let (sleeping, dead) = (poll.sleeping, poll.dead);
        if let Some((cause, score, hardcore)) = poll.death_opening {
            state.death.open(cause, score, hardcore);
            state.nav = Some(Screen::Death);
        } else if !dead && state.screen == Screen::Death {
            state.nav = Some(Screen::None);
        } else if sleeping && state.screen == Screen::None {
            state.chat.open("");
            state.nav = Some(Screen::Sleep);
        } else if !sleeping && state.screen == Screen::Sleep {
            if state.chat.has_draft() {
                state.nav = Some(Screen::Chat);
            } else {
                state.chat.close();
                state.nav = Some(Screen::None);
            }
        }
    }

    #[cfg(lua_mods)]
    crate::gui::lua_list::poll(&mut state);

    let Some(next) = state.nav.take() else { return };
    if next == state.screen {
        return;
    }
    let is_container_screen = |s: Screen| s.is_modal();
    if is_container_screen(state.screen) && !is_container_screen(next) {
        shared
            .0
            .lock()
            .unwrap()
            .session
            .inv_actions
            .push(InvAction::Close);
    }
    if state.screen == Screen::Dialog && next != Screen::Dialog {
        state.dialog.clear();
    }
    if state.screen == Screen::CommandBlock && next != Screen::CommandBlock {
        let mut s = shared.0.lock().unwrap();
        s.session.command_block_pos = None;
        s.session.command_block_data = None;
    }
    if state.screen == Screen::TestInstanceBlock && next != Screen::TestInstanceBlock {
        let mut s = shared.0.lock().unwrap();
        s.session.test_instance_pos = None;
        s.session.test_instance_status.clear();
    }
    if state.screen == Screen::SignEdit && next != Screen::SignEdit {
        let mut s = shared.0.lock().unwrap();
        let rev = s
            .session
            .block_entities
            .get(&state.sign_edit.key())
            .map(|info| info.rev);
        s.session.sign_update = Some(state.sign_edit.close(rev));
        s.session.sign_edit_open_pos = None;
    }
    enter_screen(next, &mut state, &shared, &mut windows);
}

pub(super) fn mouse_look(
    motion: Res<AccumulatedMouseMotion>,
    #[cfg_attr(feature = "mobile_ui", allow(unused_mut))] mut angles: ResMut<CameraAngles>,
    shared: Res<Shared>,
    camera: Res<CameraView>,
    state: Res<crate::gui::GuiState>,
    time: Res<Time>,
) {
    #[cfg(feature = "mobile_ui")]
    {
        let _ = (motion, angles, shared, camera, state, time);
        return;
    }
    #[cfg(not(feature = "mobile_ui"))]
    {
        let hand = if state.screen.is_open() || !crate::platform::pointer::is_locked() {
            Vec2::ZERO
        } else {
            motion.delta * LOOK_SCALE
        };
        if hand == Vec2::ZERO && !crate::modules::hooks::drives_look() {
            return;
        }
        apply_look(
            hand.x,
            hand.y,
            &mut angles,
            &shared,
            camera.free(),
            time.delta_secs(),
        );
    }
}

pub(crate) const LOOK_SCALE: f32 = 0.15;

pub(super) fn apply_server_look(
    mut angles: ResMut<CameraAngles>,
    shared: Res<Shared>,
    camera: Res<CameraView>,
) {
    let Some((yaw, pitch)) = shared.0.lock().unwrap().pending_look.take() else {
        return;
    };
    if camera.free() {
        return;
    }
    angles.yaw = yaw;
    angles.pitch = pitch.clamp(-89., 89.);
}

#[derive(Resource, Default)]
pub(super) struct VehicleTurnSpread {
    seq: Option<u32>,
    turn: f32,
    applied: f32,
}

pub(super) fn apply_vehicle_look(
    mut angles: ResMut<CameraAngles>,
    mut spread: ResMut<VehicleTurnSpread>,
    shared: Res<Shared>,
    camera: Res<CameraView>,
) {
    let mut s = shared.0.lock().unwrap();
    let Some(look) = s.session.vehicle_look else {
        *spread = VehicleTurnSpread::default();
        return;
    };
    if camera.free() {
        return;
    }
    if spread.seq != Some(look.seq) {
        angles.yaw += spread.turn - spread.applied;
        *spread = VehicleTurnSpread {
            seq: Some(look.seq),
            turn: look.turn,
            applied: 0.0,
        };
    }
    let partial = super::systems::partial_ticks(&s);
    let target = look.turn * partial;
    angles.yaw += target - spread.applied;
    spread.applied = target;
    clamp_to_hull(&mut angles, look, partial);
    s.camera_yaw = angles.yaw;
}

fn clamp_to_hull(angles: &mut CameraAngles, look: crate::session::VehicleLook, partial: f32) {
    let hull = look.boat_yaw - look.turn * (1.0 - partial);
    let delta = crate::util::mth::wrap_degrees(angles.yaw - hull);
    let limit = crate::play::riding::RIDER_YAW_LIMIT;
    angles.yaw += delta.clamp(-limit, limit) - delta;
}

pub(crate) fn apply_look(
    dx: f32,
    dy: f32,
    angles: &mut CameraAngles,
    shared: &Shared,
    freecam_active: bool,
    dt: f32,
) {
    let (dx, dy) = if !freecam_active && crate::modules::hooks::shapes_look() {
        let s = shared.0.lock().unwrap();
        crate::modules::hooks::look_delta(&s, angles.yaw, angles.pitch, dt, (dx, dy))
    } else {
        (dx, dy)
    };

    angles.yaw -= dx;
    angles.pitch = (angles.pitch - dy).clamp(-89., 89.);
    if freecam_active {
        return;
    }
    let mut s = shared.0.lock().unwrap();
    if let Some(look) = s.session.vehicle_look {
        clamp_to_hull(angles, look, super::systems::partial_ticks(&s));
    }
    s.camera_yaw = angles.yaw;
    s.camera_pitch = angles.pitch;
}

pub(crate) fn bot_movement_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    camera: Res<CameraView>,
    time: Res<Time>,
    shared: Res<Shared>,
    state: Res<crate::gui::GuiState>,
    mut intent: ResMut<crate::renderer::intent::Intent>,
    mut last_forward_tap: Local<Option<f32>>,
    mut sprint_latch: Local<bool>,
) {
    let toggle_mode = state.options.toggle_sprint;
    if camera.free() || state.screen.is_open() || crate::gui::keyboard_elsewhere() {
        *last_forward_tap = None;
        if !toggle_mode {
            *sprint_latch = false;
        }
        return;
    }
    let binds = &state.keybinds;
    let down = |action| binds.down(action, &keys, &buttons);
    let fwd = down(Action::Forward);
    let back = down(Action::Back);
    let left = down(Action::Left);
    let right = down(Action::Right);
    let jump = down(Action::Jump);
    let shift = down(Action::Sneak);
    let sprint_key = down(Action::Sprint);

    let (flying, blind) = {
        let mut s = shared.0.lock().unwrap();
        s.session.auto_jump = state.options.auto_jump;
        let blind = crate::play::mob_effects::has(&s.session.active_effects, "blindness");
        (s.session.flying, blind)
    };
    let now = time.elapsed_secs();
    if binds.just(Action::Forward, &keys, &buttons) {
        if last_forward_tap.is_some_and(|t| now - t <= FLY_DOUBLE_TAP_SECS) {
            *sprint_latch = !toggle_mode || !*sprint_latch;
            *last_forward_tap = None;
        } else {
            *last_forward_tap = Some(now);
        }
    }
    let sprint = if toggle_mode {
        if binds.just(Action::Sprint, &keys, &buttons) {
            *sprint_latch = !*sprint_latch;
        }
        *sprint_latch && !shift
    } else {
        if !fwd || shift || blind {
            *sprint_latch = false;
        }
        sprint_key || *sprint_latch
    };
    let descend = flying && shift;

    intent.walk(
        (fwd as u8) * MOVE_FORWARD
            | (back as u8) * MOVE_BACK
            | (left as u8) * MOVE_LEFT
            | (right as u8) * MOVE_RIGHT
            | (jump as u8) * MOVE_JUMP
            | (sprint as u8) * MOVE_SPRINT
            | (descend as u8) * MOVE_DESCEND
            | (shift as u8) * MOVE_SNEAK,
    );
}

pub(super) const FLY_DOUBLE_TAP_SECS: f32 = 7.0 / 20.0;

pub(super) fn flight_toggle_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    camera: Res<CameraView>,
    time: Res<Time>,
    shared: Res<Shared>,
    mut last_tap: Local<Option<f32>>,
    state: Res<crate::gui::GuiState>,
) {
    if !state.keybinds.just(Action::Jump, &keys, &buttons)
        || camera.free()
        || state.screen.is_open()
    {
        return;
    }
    let now = time.elapsed_secs();
    let doubled = last_tap.is_some_and(|t| now - t <= FLY_DOUBLE_TAP_SECS);
    if !doubled {
        *last_tap = Some(now);
        return;
    }
    *last_tap = None;

    let mut s = shared.0.lock().unwrap();
    if matches!(s.session.gamemode, Gamemode::Creative | Gamemode::Spectator) {
        s.session.fly_toggle = true;
    }
}

pub(super) fn window_focus_cursor(
    mut events: bevy::ecs::message::MessageReader<WindowFocused>,
    mut cursor_opts: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    for ev in events.read() {
        if !ev.focused {
            if let Ok(opts) = cursor_opts.single_mut() {
                super::systems::apply_cursor(opts, CursorGrabMode::None, true);
            }
        }
    }
}

pub(super) fn pause_on_lost_focus(
    time: Res<Time>,
    view: Res<super::FrameView>,
    shared: Res<Shared>,
    mut state: ResMut<crate::gui::GuiState>,
    mut windows: Query<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>,
    mut last_active: Local<f32>,
) {
    if cfg!(feature = "mobile_ui") {
        return;
    }
    let now = time.elapsed_secs();
    let focused = windows.single().map_or(true, |(window, _)| {
        crate::platform::keyboard::window_active(window.focused)
    });
    if focused || !view.in_world {
        *last_active = now;
        return;
    }
    if now - *last_active > 0.5 && !state.screen.is_open() {
        crate::log_info!("input", "pausing: window lost focus");
        enter_screen(crate::gui::Screen::Pause, &mut state, &shared, &mut windows);
    }
}

pub(super) fn track_lifecycle(
    mut lifecycle: bevy::ecs::message::MessageReader<bevy::window::AppLifecycle>,
) {
    for event in lifecycle.read() {
        match event {
            bevy::window::AppLifecycle::Suspended => crate::platform::notify::suspended(),
            bevy::window::AppLifecycle::Running => crate::platform::notify::resumed(),
            _ => {}
        }
    }
}

pub(super) fn web_pointer_lock(
    buttons: Res<ButtonInput<MouseButton>>,
    shared: Res<Shared>,
    #[cfg_attr(feature = "mobile_ui", allow(unused_mut))] mut state: ResMut<crate::gui::GuiState>,
    #[cfg_attr(feature = "mobile_ui", allow(unused_mut))] mut windows: Query<
        (&mut Window, &mut CursorOptions),
        With<PrimaryWindow>,
    >,
    mut held_frames: Local<u32>,
    time: Res<Time>,
    mut screen_seen: Local<f32>,
) {
    #[cfg(feature = "mobile_ui")]
    {
        let _ = (
            buttons,
            shared,
            state,
            windows,
            held_frames,
            time,
            screen_seen,
        );
        return;
    }
    #[cfg(not(feature = "mobile_ui"))]
    {
        if !cfg!(target_arch = "wasm32") {
            return;
        }
        let claimed = crate::platform::pointer::take_claimed();
        let now = time.elapsed_secs();
        if state.screen.is_open() {
            *screen_seen = now;
            if crate::platform::pointer::is_locked() {
                crate::platform::pointer::unlock();
            }
            *held_frames = 0;
            return;
        }
        let pressed =
            buttons.just_pressed(MouseButton::Left) || buttons.just_pressed(MouseButton::Right);
        if crate::platform::pointer::is_locked() {
            if claimed && pressed {
                state.suppress_click = true;
            }
            *held_frames = held_frames.saturating_add(1);
            return;
        }
        const SETTLED: u32 = 3;
        const CLOSE_GRACE: f32 = 0.6;
        if std::mem::take(&mut *held_frames) >= SETTLED && now - *screen_seen > CLOSE_GRACE {
            crate::log_info!("input", "pausing: pointer lock lost");
            enter_screen(crate::gui::Screen::Pause, &mut state, &shared, &mut windows);
            return;
        }
        if pressed {
            crate::platform::pointer::lock();
            state.suppress_click = true;
        }
    }
}

pub(super) fn fullscreen_key(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    state: Res<crate::gui::GuiState>,
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))] mut windows: Query<
        &mut Window,
        With<bevy::window::PrimaryWindow>,
    >,
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))] time: Res<
        Time<bevy::time::Real>,
    >,
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))] mut last_toggle: Local<
        Option<f64>,
    >,
    #[cfg(target_arch = "wasm32")] mut captured: Local<bool>,
) {
    let pressed = state.keybinds.just(Action::Fullscreen, &keys, &buttons);

    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    {
        const ECHO_SECS: f64 = 0.5;
        let now = time.elapsed_secs_f64();
        let echo = last_toggle.is_some_and(|at| now - at < ECHO_SECS);
        if pressed
            && !echo
            && let Ok(mut window) = windows.single_mut()
        {
            use bevy::window::{MonitorSelection, WindowMode};
            window.mode = match window.mode {
                WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
                _ => WindowMode::Windowed,
            };
            *last_toggle = Some(now);
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        if pressed {
            crate::platform::fullscreen::toggle();
        }
        let fullscreen = crate::platform::fullscreen::is_fullscreen();
        if fullscreen != *captured {
            *captured = fullscreen;
            crate::platform::fullscreen::capture_escape(fullscreen);
        }
    }

    #[cfg(target_os = "android")]
    let _ = pressed;
}

pub(super) fn poll_module_binds(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    state: Res<crate::gui::GuiState>,
) {
    use crate::gui::keybinds::Bound;
    use crate::modules::store;

    #[cfg(lua_mods)]
    crate::modules::script::poll_keys(&keys, &buttons, state.screen.is_open());
    if state.screen.is_open() {
        return;
    }
    #[cfg(lua_mods)]
    if crate::modules::script::wants_keys() {
        let pressed = (keys.get_just_pressed().map(|&k| Bound::Key(k)))
            .chain(buttons.get_just_pressed().map(|&b| Bound::Mouse(b)));
        for bound in pressed {
            let key = bound.serialized_name();
            if key != Bound::Unbound.serialized_name() {
                crate::modules::script::key_down(key, bound.display_name());
            }
        }
    }
    for m in crate::modules::registry::modules() {
        let id = m.id;
        let pressed = match store().bind(id) {
            Bound::Unbound => false,
            Bound::Key(code) => keys.just_pressed(code),
            Bound::Mouse(button) => buttons.just_pressed(button),
        };
        if pressed {
            store().toggle(id);
        }
    }
}

pub(super) fn toggle_third_person(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut camera: ResMut<CameraView>,
    state: Res<crate::gui::GuiState>,
) {
    if state.screen.is_open() {
        return;
    }
    if state
        .keybinds
        .just(Action::TogglePerspective, &keys, &buttons)
    {
        camera.perspective = camera.perspective.cycle();
    }
}

pub(super) fn toggle_freecam(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut view: ResMut<CameraView>,
    mut angles: ResMut<CameraAngles>,
    shared: Res<Shared>,
    camera: Query<&Transform, With<WorldCamera>>,
    origin: Res<super::RenderOrigin>,
    state: Res<crate::gui::GuiState>,
) {
    if !state.screen.is_open() && state.keybinds.just(Action::Freecam, &keys, &buttons) {
        crate::modules::freecam::toggle();
    }
    let want = crate::modules::freecam::active();
    if want == view.free {
        return;
    }
    view.free = want;
    if want {
        if let Ok(t) = camera.single() {
            view.free_eye = origin.to_world(t.translation);
        }
    } else {
        let s = shared.0.lock().unwrap();
        angles.yaw = s.camera_yaw;
        angles.pitch = s.camera_pitch;
    }
}

pub(crate) fn freecam_movement(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut view: ResMut<CameraView>,
    angles: Res<CameraAngles>,
    time: Res<Time>,
    state: Res<crate::gui::GuiState>,
    mut touch: ResMut<FreecamTouch>,
) {
    let touch = std::mem::take(&mut *touch);
    if !view.free || state.screen.is_open() {
        return;
    }
    let binds = &state.keybinds;
    let down = |action| binds.down(action, &keys, &buttons);
    let speed = crate::modules::freecam::speed(down(Action::Sprint) || touch.sprint);
    let dt = time.delta_secs();
    let eye = &mut view.free_eye;
    let step = |dir: Vec3| (dir * speed * dt).as_dvec3();

    let yaw_rad = angles.yaw.to_radians();
    let forward = Vec3::new(-yaw_rad.sin(), 0.0, -yaw_rad.cos());
    let right = Vec3::new(yaw_rad.cos(), 0.0, -yaw_rad.sin());

    if down(Action::Forward) {
        *eye += step(forward);
    }
    if down(Action::Back) {
        *eye -= step(forward);
    }
    if down(Action::Right) {
        *eye += step(right);
    }
    if down(Action::Left) {
        *eye -= step(right);
    }
    if down(Action::Jump) || touch.up {
        *eye += step(Vec3::Y);
    }
    if down(Action::Sneak) || touch.down {
        *eye -= step(Vec3::Y);
    }
    if touch.dir != Vec2::ZERO {
        *eye += step(forward * touch.dir.y + right * touch.dir.x);
    }
}

pub(crate) fn handle_mouse_input(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    shared: Res<Shared>,
    #[cfg_attr(feature = "mobile_ui", allow(unused_mut))] mut state: ResMut<crate::gui::GuiState>,
    #[cfg_attr(feature = "mobile_ui", allow(unused_mut))] mut intent: ResMut<
        crate::renderer::intent::Intent,
    >,
) {
    #[cfg(feature = "mobile_ui")]
    {
        let _ = (buttons, keys, shared, state, intent);
        return;
    }
    #[cfg(not(feature = "mobile_ui"))]
    handle_mouse_input_desktop(buttons, keys, shared, state, intent);
}

#[cfg(not(feature = "mobile_ui"))]
fn handle_mouse_input_desktop(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    shared: Res<Shared>,
    mut state: ResMut<crate::gui::GuiState>,
    mut intent: ResMut<crate::renderer::intent::Intent>,
) {
    if state.screen.is_open() {
        shared.0.lock().unwrap().session.clear_clicks();
        return;
    }
    if std::mem::take(&mut state.suppress_click) {
        shared.0.lock().unwrap().session.clear_clicks();
        return;
    }
    let binds = &state.keybinds;
    intent.hold(
        binds.down(Action::Attack, &keys, &buttons),
        binds.down(Action::Use, &keys, &buttons),
    );
    intent.click(
        binds.just(Action::Attack, &keys, &buttons),
        binds.just(Action::Use, &keys, &buttons),
    );
    if binds.just(Action::PickItem, &keys, &buttons) {
        intent.pick(keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight));
    }
}

pub(super) fn handle_hotbar_keys(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    shared: Res<Shared>,
    mut state: ResMut<crate::gui::GuiState>,
) {
    if state.screen.is_open() {
        return;
    }
    for (i, action) in crate::gui::keybinds::HOTBAR.into_iter().enumerate() {
        if state.keybinds.just(action, &keys, &buttons) {
            let mut s = shared.0.lock().unwrap();
            if s.session.gamemode == Gamemode::Spectator {
                let tab_list = s.session.tab_list.clone();
                drop(s);
                if let Some(uuid) =
                    state
                        .spectator_menu
                        .on_hotbar_selected(i as u8, &tab_list, time.elapsed_secs())
                {
                    shared.0.lock().unwrap().session.spectator_teleport_target = Some(uuid);
                }
            } else {
                s.session.hotbar_target = Some(i as u8);
            }
            break;
        }
    }
}

pub(super) fn handle_spectator_action_key(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    shared: Res<Shared>,
    mut state: ResMut<crate::gui::GuiState>,
) {
    if state.screen.is_open() {
        return;
    }
    if !state
        .keybinds
        .just(Action::SpectatorAction, &keys, &buttons)
    {
        return;
    }
    let s = shared.0.lock().unwrap();
    if s.session.gamemode != Gamemode::Spectator {
        return;
    }
    let tab_list = s.session.tab_list.clone();
    drop(s);
    if let Some(uuid) = state
        .spectator_menu
        .on_action_key(&tab_list, time.elapsed_secs())
    {
        shared.0.lock().unwrap().session.spectator_teleport_target = Some(uuid);
    }
}

pub(crate) fn wheel_notches(wheel: &AccumulatedMouseScroll) -> Vec2 {
    const PIXELS_PER_NOTCH: f32 = 100.0;
    #[cfg(target_arch = "wasm32")]
    const LINES_PER_NOTCH: f32 = 3.0;
    #[cfg(not(target_arch = "wasm32"))]
    const LINES_PER_NOTCH: f32 = 1.0;

    let per = match wheel.unit {
        MouseScrollUnit::Line => LINES_PER_NOTCH,
        MouseScrollUnit::Pixel => PIXELS_PER_NOTCH,
    };
    wheel.delta / per
}

pub(crate) fn accumulate_scroll(acc: &mut (f64, f64), dx: f64, dy: f64) -> (i32, i32) {
    if acc.0 != 0.0 && dx.signum() != acc.0.signum() {
        acc.0 = 0.0;
    }
    if acc.1 != 0.0 && dy.signum() != acc.1.signum() {
        acc.1 = 0.0;
    }
    acc.0 += dx;
    acc.1 += dy;
    let (wx, wy) = (acc.0.trunc() as i32, acc.1.trunc() as i32);
    if wx == 0 && wy == 0 {
        return (0, 0);
    }
    acc.0 -= wx as f64;
    acc.1 -= wy as f64;
    (wx, wy)
}

pub(crate) fn next_scroll_selection(wheel: f64, current: i32, limit: i32) -> i32 {
    let step = if wheel > 0.0 {
        1
    } else if wheel < 0.0 {
        -1
    } else {
        0
    };
    let mut selected = (current - step).max(-1);
    while selected < 0 {
        selected += limit;
    }
    while selected >= limit {
        selected -= limit;
    }
    selected
}

pub(super) fn handle_hotbar_scroll(
    wheel: Res<AccumulatedMouseScroll>,
    shared: Res<Shared>,
    state: Res<crate::gui::GuiState>,
    mut acc: Local<(f64, f64)>,
) {
    if state.screen.is_open() {
        return;
    }
    let notches = wheel_notches(&wheel);
    let (dx, dy) = (notches.x as f64, notches.y as f64);
    if dx == 0.0 && dy == 0.0 {
        return;
    }
    #[cfg(lua_mods)]
    if crate::modules::script::scroll(if dy == 0.0 { -dx as f32 } else { dy as f32 }) {
        return;
    }
    let mut s = shared.0.lock().unwrap();
    if s.session.gamemode == Gamemode::Spectator {
        return;
    }
    let (wx, wy) = accumulate_scroll(&mut acc, dx, dy);
    if wx == 0 && wy == 0 {
        return;
    }
    let notches = if wy == 0 { -wx } else { wy };

    let current = s.session.hotbar_target.unwrap_or(s.session.hotbar_selected) as i32;
    let next = next_scroll_selection(notches as f64, current, HOTBAR_SLOTS);
    if next != current {
        s.session.hotbar_target = Some(next as u8);
    }
}

pub(super) const HOTBAR_SLOTS: i32 = 9;

pub(super) fn handle_spectator_fly_speed_scroll(
    wheel: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    shared: Res<Shared>,
    mut state: ResMut<crate::gui::GuiState>,
    mut acc: Local<(f64, f64)>,
) {
    if state.screen.is_open() {
        return;
    }
    let dy = wheel_notches(&wheel).y;
    if dy == 0.0 {
        return;
    }
    #[cfg(lua_mods)]
    if crate::modules::script::scroll_claimed() {
        return;
    }
    let mut s = shared.0.lock().unwrap();
    if s.session.gamemode != Gamemode::Spectator {
        return;
    }
    if !state.spectator_menu.is_open() {
        s.session.fly_speed_scroll += dy * 0.005;
        return;
    }
    let tab_list = s.session.tab_list.clone();
    drop(s);
    let (_, notches) = accumulate_scroll(&mut acc, 0.0, dy as f64);
    if notches != 0 {
        state
            .spectator_menu
            .on_scroll(notches, &tab_list, time.elapsed_secs());
    }
}

fn send_typed(shared: &Shared, line: String) {
    let Some(line) = crate::modules::commands::intercept(&shared.0, line) else {
        return;
    };
    #[cfg(target_arch = "wasm32")]
    crate::client::chat_trace::queued(&line);
    #[cfg(lua_mods)]
    {
        let _ = shared;
        crate::modules::script::typed(line);
    }
    #[cfg(not(lua_mods))]
    shared.0.lock().unwrap().session.outgoing_chat.push(line);
}

pub(super) fn toggle_screen(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    shared: Res<Shared>,
    mut state: ResMut<crate::gui::GuiState>,
    mut windows: Query<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>,
    gui_input: Res<crate::gui::GuiInput>,
) {
    use crate::gui::Screen;

    if crate::gui::keyboard_elsewhere() {
        return;
    }

    let field_escape = !state.screen_is_menu() && crate::platform::keyboard::take_escape();

    let guard = std::mem::take(&mut state.keybinds.escape_guard);
    if state.keybinds.capturing.is_some() || (guard && keys.just_pressed(KeyCode::Escape)) {
        return;
    }

    #[cfg(feature = "click_gui")]
    {
        let click_gui = state.keybinds.just(Action::ClickGui, &keys, &buttons);
        if state.screen == Screen::ClickGui {
            #[cfg(lua_mods)]
            let click_gui = click_gui && !state.clickgui.typing();
            let escape =
                (keys.just_pressed(KeyCode::Escape) || field_escape) && !state.clickgui.escape();
            if click_gui || escape {
                let back = state.clickgui_parent;
                enter_screen(back, &mut state, &shared, &mut windows);
            }
            return;
        }
        if click_gui && (state.screen == Screen::None || state.screen_is_menu()) {
            state.clickgui_parent = state.screen;
            enter_screen(Screen::ClickGui, &mut state, &shared, &mut windows);
            return;
        }
    }

    #[cfg(feature = "hud_editor")]
    {
        let hud_key = state.keybinds.just(Action::HudEditor, &keys, &buttons);
        if state.screen == Screen::HudEditor {
            let escape =
                (keys.just_pressed(KeyCode::Escape) || field_escape) && !state.hud_state.escape();
            if hud_key || escape {
                let back = state.hud_state.parent;
                enter_screen(back, &mut state, &shared, &mut windows);
            }
            return;
        }
        if hud_key && (state.screen == Screen::None || state.screen_is_menu()) {
            state.hud_state.parent = state.screen;
            state.hud_state.over_menu = state.screen_is_menu();
            enter_screen(Screen::HudEditor, &mut state, &shared, &mut windows);
            return;
        }
    }

    let creative_search_focused =
        state.screen == Screen::Creative && state.creative.search_focused();
    let toggle_key = state.keybinds.just(Action::Inventory, &keys, &buttons)
        && !state.screen.chat_open()
        && state.screen != Screen::SignEdit
        && state.screen != Screen::CommandBlock
        && !state.screen.is_op_block_editor()
        && !matches!(
            state.screen,
            Screen::BookView | Screen::BookEdit | Screen::BookSign
        )
        && !state.screen.is_local_overlay()
        && !creative_search_focused
        && !state.anvil_typing();
    let close_key = keys.just_pressed(KeyCode::Escape) || field_escape;
    #[cfg(lua_mods)]
    let close_key = close_key
        || (state.screen == Screen::LuaList
            && keys.just_pressed(KeyCode::ShiftRight)
            && state.lua_list.windowed());
    #[cfg(target_arch = "wasm32")]
    let pause_key = state.keybinds.just(Action::Pause, &keys, &buttons);
    #[cfg(not(target_arch = "wasm32"))]
    let pause_key = false;
    let enter_from_field = state.screen.chat_open() && crate::platform::keyboard::take_enter();
    let submit_key = state.screen.chat_open()
        && (keys.just_pressed(KeyCode::Enter)
            || keys.just_pressed(KeyCode::NumpadEnter)
            || enter_from_field);

    if state.screen_is_menu() {
        return;
    }

    #[cfg(lua_mods)]
    let lua_escape = close_key && state.screen == Screen::LuaList && state.lua_list.escape();
    #[cfg(not(lua_mods))]
    let lua_escape = false;

    let next = if state.screen == Screen::Pause || state.screen.is_options() {
        if close_key || (pause_key && state.screen == Screen::Pause) {
            #[cfg(feature = "shader_support")]
            let stepped_within = (state.screen == Screen::ShaderOptions
                && state.shaderpacks.pop_screen())
                || (state.screen == Screen::ShaderPacks && state.shaderpacks.cancel_prompt());
            #[cfg(not(feature = "shader_support"))]
            let stepped_within = false;
            #[cfg(resource_packs)]
            let stepped_within = stepped_within
                || (state.screen == Screen::ResourcePacks && state.resourcepacks.cancel_prompt());
            #[cfg(resource_packs)]
            if state.screen == Screen::ResourcePacks && !stepped_within {
                state.resourcepacks.apply();
            }

            if stepped_within {
                None
            } else {
                Some(match state.screen {
                    Screen::Options => state.options_parent,
                    #[cfg(feature = "shader_support")]
                    Screen::ShaderPacks => Screen::VideoSettings,
                    #[cfg(feature = "shader_support")]
                    Screen::ShaderOptions => Screen::ShaderPacks,
                    #[cfg(explore_modrinth)]
                    Screen::ExplorePacks => Screen::ResourcePacks,
                    s if s.is_options() => Screen::Options,
                    _ => Screen::None,
                })
            }
        } else {
            None
        }
    } else if state.screen == Screen::Chat {
        if (close_key || submit_key) && state.chat.dismiss_link_prompt() {
            None
        } else if close_key && state.chat.hide_suggestions() {
            None
        } else if close_key || submit_key {
            if submit_key && let Some(line) = state.chat.submit() {
                send_typed(&shared, line);
            }
            if submit_key && crate::gui::chat::texting() {
                None
            } else {
                state.chat.close();
                Some(Screen::None)
            }
        } else {
            None
        }
    } else if state.screen == Screen::CommandBlock {
        if close_key && state.command_block.hide_suggestions() {
            None
        } else if close_key {
            Some(Screen::None)
        } else {
            None
        }
    } else if state.screen == Screen::Dialog {
        if close_key {
            let out = state.dialog.on_escape(&shared.0);
            if let Some(command) = out.chat {
                state.chat.open(&command);
                Some(Screen::Chat)
            } else {
                out.nav
            }
        } else {
            None
        }
    } else if state.screen == Screen::Sleep {
        let taken = ((close_key || submit_key) && state.chat.dismiss_link_prompt())
            || (close_key && state.chat.hide_suggestions());
        if !taken && submit_key {
            if let Some(line) = state.chat.submit() {
                send_typed(&shared, line);
            }
        } else if !taken && close_key {
            shared.0.lock().unwrap().leave_bed_requested = true;
        }
        None
    } else if matches!(
        state.screen,
        Screen::WinGame | Screen::CodeOfConduct | Screen::Reconfiguring | Screen::ServerPackPrompt
    ) {
        None
    } else if state.screen == Screen::Death {
        None
    } else if state.screen == Screen::GameModeSwitcher {
        if state.keybinds.just(Action::SwitchGameMode, &keys, &buttons) {
            state.gamemode_switcher.select_next();
        }
        if state
            .keybinds
            .released(Action::DebugModifier, &keys, &buttons)
        {
            switch_gamemode(state.gamemode_switcher.hovered(), &shared);
            Some(Screen::None)
        } else if close_key {
            Some(Screen::None)
        } else {
            None
        }
    } else if state.screen == Screen::Statistics {
        close_key.then_some(Screen::Pause)
    } else if state.screen == Screen::Credits {
        None
    } else if state.screen == Screen::Advancements {
        if close_key {
            Some(state.advancements.parent)
        } else if state.keybinds.just(Action::Advancements, &keys, &buttons) {
            Some(Screen::None)
        } else {
            None
        }
    } else if state.screen.is_open() {
        if lua_escape {
            None
        } else if (toggle_key || close_key)
            && crate::gui::recipe_book_ghost::key_before_close(
                &state,
                &shared.0,
                close_key,
                gui_input.size.x,
            )
        {
            None
        } else if toggle_key || close_key {
            Some(Screen::None)
        } else {
            None
        }
    } else if state.keybinds.down(Action::DebugModifier, &keys, &buttons)
        && !(close_key || pause_key)
    {
        if state.keybinds.just(Action::SwitchGameMode, &keys, &buttons) {
            open_gamemode_switcher(&mut state, &shared)
        } else {
            None
        }
    } else if toggle_key {
        let creative = shared.0.lock().unwrap().session.gamemode == Gamemode::Creative;
        if creative {
            state.creative.open();
        }
        Some(if creative {
            Screen::Creative
        } else {
            Screen::Inventory
        })
    } else if state.keybinds.just(Action::Advancements, &keys, &buttons) {
        state.advancements.parent = Screen::None;
        Some(Screen::Advancements)
    } else if state.keybinds.just(Action::Chat, &keys, &buttons) {
        state.chat.open("");
        Some(Screen::Chat)
    } else if state.keybinds.just(Action::Command, &keys, &buttons) {
        state.chat.open("/");
        Some(Screen::Chat)
    } else if let Some((_, prefix)) = keys
        .get_just_pressed()
        .next()
        .and_then(|_| crate::modules::commands::open_key())
        .filter(|(key, _)| keys.just_pressed(*key))
    {
        state.chat.open(&prefix);
        Some(Screen::Chat)
    } else if close_key || pause_key {
        Some(Screen::Pause)
    } else {
        None
    };

    let Some(next) = next else { return };
    if next == Screen::None
        && !matches!(state.screen, Screen::Chat | Screen::Pause)
        && !state.screen.is_options()
        && !state.screen.is_local_overlay()
    {
        shared
            .0
            .lock()
            .unwrap()
            .session
            .inv_actions
            .push(InvAction::Close);
    }
    enter_screen(next, &mut state, &shared, &mut windows);
}

fn debug_combo(
    action: Action,
    keys: &ButtonInput<KeyCode>,
    buttons: &ButtonInput<MouseButton>,
    state: &crate::gui::GuiState,
) -> bool {
    !state.screen.is_open()
        && state.keybinds.down(Action::DebugModifier, keys, buttons)
        && state.keybinds.just(action, keys, buttons)
}

fn debug_feedback(session: &mut crate::session::SessionState, key: &str) {
    use crate::text::{Span, Style};
    let prefix = Style {
        bold: true,
        ..Style::colored(0xFFFF55)
    };
    let spans = vec![
        Span {
            text: format!("{} ", crate::gui::tooltip::translate("debug.prefix", &[])),
            style: prefix,
        },
        Span::colored(crate::gui::tooltip::translate(key, &[]), 0xFF_FFFF),
    ];
    session.chat_incoming.push(crate::session::ChatEntry {
        spans,
        events: None,
        tag: crate::session::ChatTag::System,
        original: None,
    });
}

fn open_gamemode_switcher(
    state: &mut crate::gui::GuiState,
    shared: &Shared,
) -> Option<crate::gui::Screen> {
    let mut s = shared.0.lock().unwrap();
    if !s.in_world {
        return None;
    }
    if s.session.op_level < GAME_MASTER_LEVEL {
        debug_feedback(&mut s.session, "debug.gamemodes.error");
        return None;
    }
    state
        .gamemode_switcher
        .open(s.session.gamemode, s.session.previous_gamemode);
    Some(crate::gui::Screen::GameModeSwitcher)
}

fn switch_gamemode(to: Gamemode, shared: &Shared) {
    let mut s = shared.0.lock().unwrap();
    if s.in_world && to != s.session.gamemode && s.session.op_level >= GAME_MASTER_LEVEL {
        s.session.gamemode_request = Some(to);
    }
}

pub(super) fn debug_spectate(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    state: Res<crate::gui::GuiState>,
    shared: Res<Shared>,
) {
    if !debug_combo(Action::DebugSpectate, &keys, &buttons, &state) {
        return;
    }
    let mut s = shared.0.lock().unwrap();
    if !s.in_world || s.session.op_level < GAME_MASTER_LEVEL {
        debug_feedback(&mut s.session, "debug.creative_spectator.error");
        return;
    }
    s.session.gamemode_request = Some(if s.session.gamemode != Gamemode::Spectator {
        Gamemode::Spectator
    } else {
        s.session.previous_gamemode.unwrap_or(Gamemode::Creative)
    });
}

pub(super) fn toggle_advanced_tooltips(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut state: ResMut<crate::gui::GuiState>,
    shared: Res<Shared>,
) {
    if debug_combo(Action::AdvancedTooltips, &keys, &buttons, &state) {
        state.advanced_tooltips = !state.advanced_tooltips;
        shared.0.lock().unwrap().session.advanced_tooltips = state.advanced_tooltips;
    }
}

pub(super) fn toggle_chunk_borders(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut state: ResMut<crate::gui::GuiState>,
) {
    if debug_combo(Action::ChunkBorders, &keys, &buttons, &state) {
        state.chunk_borders = !state.chunk_borders;
    }
}

#[cfg(resource_packs)]
pub(super) fn reload_resource_packs(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    state: Res<crate::gui::GuiState>,
    shared: Res<Shared>,
) {
    if !debug_combo(Action::ReloadResourcePacks, &keys, &buttons, &state) {
        return;
    }
    crate::renderer::reload::request();
    debug_feedback(
        &mut shared.0.lock().unwrap().session,
        "debug.reload_resourcepacks.message",
    );
}

pub(super) fn reload_all_chunks(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    state: Res<crate::gui::GuiState>,
    shared: Res<Shared>,
) {
    if debug_combo(Action::ReloadChunks, &keys, &buttons, &state) {
        shared.0.lock().unwrap().reload_chunks_requested = true;
    }
}

pub(super) fn handle_keybinds(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut key_events: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mut state: ResMut<crate::gui::GuiState>,
    shared: Res<Shared>,
    mut commands: Commands,
) {
    let drop_clicks = state
        .keybinds
        .clicks(Action::Drop, key_events.read(), &buttons);
    let drop_clicks = if crate::gui::keyboard_elsewhere() {
        0
    } else {
        drop_clicks
    };

    let held = state.keybinds.down(Action::PlayerList, &keys, &buttons) && !state.screen.is_open();
    if state.tab_list_held != held {
        state.tab_list_held = held;
    }

    if state.screen.is_open() {
        return;
    }

    if state.keybinds.just(Action::ToggleGui, &keys, &buttons) {
        state.hide_gui = !state.hide_gui;
    }

    if state.keybinds.just(Action::Screenshot, &keys, &buttons) {
        super::screenshot::request(&mut commands);
    }

    if state.keybinds.just(Action::SwapOffhand, &keys, &buttons) {
        shared.0.lock().unwrap().session.swap_hands = true;
    }

    if drop_clicks > 0 {
        let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        let drop = if ctrl {
            DropRequest::HeldStack
        } else {
            DropRequest::HeldItem
        };
        let mut s = shared.0.lock().unwrap();
        s.session
            .pending_drops
            .extend(std::iter::repeat_n(drop, drop_clicks as usize));
    }
}

#[cfg(test)]
mod tests {
    use super::{HOTBAR_SLOTS, accumulate_scroll, next_scroll_selection};

    #[test]
    fn scroll_direction_matches_vanilla() {
        assert_eq!(next_scroll_selection(1.0, 4, HOTBAR_SLOTS), 3);
        assert_eq!(next_scroll_selection(-1.0, 4, HOTBAR_SLOTS), 5);
        assert_eq!(next_scroll_selection(0.0, 4, HOTBAR_SLOTS), 4);
    }

    #[test]
    fn scroll_wraps_at_both_ends() {
        assert_eq!(next_scroll_selection(1.0, 0, HOTBAR_SLOTS), 8);
        assert_eq!(next_scroll_selection(-1.0, 8, HOTBAR_SLOTS), 0);
    }

    #[test]
    fn multi_notch_scroll_moves_one_slot() {
        assert_eq!(next_scroll_selection(3.0, 4, HOTBAR_SLOTS), 3);
        assert_eq!(next_scroll_selection(-7.0, 4, HOTBAR_SLOTS), 5);
        assert_eq!(next_scroll_selection(9.0, 0, HOTBAR_SLOTS), 8);
    }

    #[test]
    fn scroll_accumulator_banks_fractions() {
        let mut acc = (0.0, 0.0);
        assert_eq!(accumulate_scroll(&mut acc, 0.0, 0.4), (0, 0));
        assert_eq!(accumulate_scroll(&mut acc, 0.0, 0.4), (0, 0));
        assert_eq!(accumulate_scroll(&mut acc, 0.0, 0.4), (0, 1));
        assert_eq!(accumulate_scroll(&mut acc, 0.0, 0.9), (0, 1));
    }

    #[test]
    fn scroll_accumulator_resets_on_reversal() {
        let mut acc = (0.0, 0.0);
        assert_eq!(accumulate_scroll(&mut acc, 0.0, 0.9), (0, 0));
        assert_eq!(accumulate_scroll(&mut acc, 0.0, -0.9), (0, 0));
        assert_eq!(accumulate_scroll(&mut acc, 0.0, -0.2), (0, -1));
    }

    #[test]
    fn scroll_accumulator_passes_whole_notches() {
        let mut acc = (0.0, 0.0);
        assert_eq!(accumulate_scroll(&mut acc, 0.0, 1.0), (0, 1));
        assert_eq!(accumulate_scroll(&mut acc, 0.0, -1.0), (0, -1));
        assert_eq!(accumulate_scroll(&mut acc, 0.0, 2.0), (0, 2));
        assert_eq!(acc.1, 0.0);
    }

    #[test]
    fn repeated_notches_walk_the_hotbar() {
        let mut slot = 1;
        for _ in 0..3 {
            slot = next_scroll_selection(1.0, slot, HOTBAR_SLOTS);
        }
        assert_eq!(slot, 7);
        for _ in 0..3 {
            slot = next_scroll_selection(-1.0, slot, HOTBAR_SLOTS);
        }
        assert_eq!(slot, 1);
    }
}
