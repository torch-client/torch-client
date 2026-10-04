use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::AccumulatedMouseScroll;
#[cfg(feature = "mobile_ui")]
use bevy::input::touch::Touches;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::PrimaryWindow;

use crate::gui::atlas::GuiAtlas;
use crate::gui::painter::Painter;
use crate::gui::pool::{GuiFrames, GuiTextures};
use crate::gui::{GuiState, Screen, keybinds, screens};
use crate::session::SharedMutex;

pub fn clipboard_get() -> String {
    crate::platform::clipboard::get()
}

pub fn clipboard_set(text: &str) {
    crate::platform::clipboard::set(text)
}

#[derive(Resource)]
pub struct GuiAssets {
    pub atlas: Arc<GuiAtlas>,
    pub image: Handle<Image>,
    pub unihex_image: Handle<Image>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditKey {
    Char(char),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    Up,
    Down,
    Enter,
}

#[derive(Resource, Default, Clone)]
pub struct GuiInput {
    pub mouse: Option<Vec2>,
    pub size: Vec2,
    pub scale: f32,
    pub time: f32,
    pub device_scale: f32,
    pub left_click: bool,
    pub right_click: bool,
    pub middle_click: bool,
    pub left_down: bool,
    pub right_down: bool,
    pub middle_down: bool,
    pub left_release: bool,
    pub right_release: bool,
    pub scroll: f32,
    pub typed: Vec<char>,
    pub edits: Vec<EditKey>,
    pub backspace: bool,
    pub delete: bool,
    pub left_arrow: bool,
    pub right_arrow: bool,
    pub home: bool,
    pub end: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub hotbar_keys: [bool; 9],
    pub drop_key: bool,
    pub swap_key: bool,
    pub escape: bool,
    pub defocus: bool,
    pub enter: bool,
    pub select: bool,
    pub up_arrow: bool,
    pub down_arrow: bool,
    pub page_up: bool,
    pub page_down: bool,
    pub tab: bool,
    pub alt: bool,
    pub copy: bool,
    pub paste: bool,
    pub cut: bool,
    pub select_all: bool,
    #[cfg(feature = "hud_editor")]
    pub undo: bool,
    #[cfg(feature = "hud_editor")]
    pub hud_hide: bool,
    #[cfg(feature = "hud_editor")]
    pub hud_remove: bool,
    #[cfg(feature = "hud_editor")]
    pub hud_reset: bool,
    pub profile_down: bool,
    pub profile_up: bool,
    pub pressed_key: Option<KeyCode>,
    pub pressed_mouse: Option<MouseButton>,
    pub double_click: bool,
    pub triple_click: bool,
}

#[derive(Resource)]
pub struct GuiShared(pub Arc<SharedMutex>);

pub struct GuiPlugin {
    pub atlas: Arc<GuiAtlas>,
    pub shared: Arc<SharedMutex>,
    pub startup: StartupState,
}

#[derive(Clone, Default)]
pub struct StartupState {
    pub screen: crate::gui::Screen,
    pub first_run: bool,
    pub pending_connect: Option<String>,
    pub after_download: crate::gui::Screen,
}

#[derive(Resource, Clone, Copy)]
pub struct StartScreen(pub crate::gui::Screen);

#[derive(Resource, Clone, Copy)]
#[allow(
    dead_code,
    reason = "read only by gui::assetdownload, which is one feature's screen"
)]
pub struct AfterDownloadScreen(pub crate::gui::Screen);

impl Plugin for GuiPlugin {
    fn build(&self, app: &mut App) {
        let options = crate::gui::GuiOptions::default();
        #[cfg(feature = "audio")]
        crate::audio::set_volumes(&options.volumes);
        app.init_resource::<GuiFrames>().add_plugins(
            bevy::render::extract_resource::ExtractResourcePlugin::<GuiTextures>::default(),
        );
        crate::gui::draw::build(app);
        app.insert_resource(GuiShared(self.shared.clone()))
            .insert_resource(PendingGuiAtlas(self.atlas.clone()))
            .insert_resource(StartScreen(self.startup.screen))
            .insert_resource(AfterDownloadScreen(self.startup.after_download))
            .init_resource::<GuiInput>()
            .insert_resource(GuiState {
                screen: self.startup.screen,
                options_parent: self.startup.screen,
                profile: crate::gui::profile::ProfileState::starting(
                    self.startup.first_run,
                    self.startup.pending_connect.clone(),
                ),
                options,
                chat: crate::gui::chat::ChatState::with_saved_history(),
                hud_state: crate::gui::hud_layout::HudState::load(),
                ..Default::default()
            })
            .init_resource::<crate::gui::atlas_writes::AtlasWrites>()
            .add_systems(Startup, setup_gui)
            .add_systems(Update, upload_server_icons)
            .add_systems(
                Update,
                (collect_input, draw_gui, upload_banner_icons).chain(),
            );

        if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render_app
                .init_resource::<crate::gui::atlas_writes::AtlasWrites>()
                .add_systems(
                    bevy::render::ExtractSchedule,
                    crate::gui::atlas_writes::extract_atlas_writes,
                )
                .add_systems(
                    bevy::render::Render,
                    crate::gui::atlas_writes::upload_atlas_writes
                        .in_set(bevy::render::RenderSystems::PrepareResources),
                );
        }

        #[cfg(feature = "skins")]
        app.init_resource::<crate::gui::player_faces::PlayerFaces>()
            .add_systems(
                Update,
                crate::gui::player_faces::update_player_faces.before(draw_gui),
            );

        #[cfg(feature = "asset_download")]
        app.add_systems(
            Update,
            crate::gui::assetdownload::adopt_downloaded_assets.run_if(bevy::prelude::not(
                bevy::prelude::resource_exists::<crate::renderer::systems::AssetsReady>,
            )),
        );
    }

    fn finish(&self, app: &mut App) {
        crate::gui::draw::finish(app);
    }
}

#[derive(Resource)]
struct PendingGuiAtlas(Arc<GuiAtlas>);

pub(crate) fn atlas_textures(atlas: &GuiAtlas) -> (Image, Image) {
    let (width, height) = atlas.size();
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let pixels = atlas.take_pixels();
    if pixels.is_none() {
        crate::log_warn!(
            "gui",
            "atlas pixels already uploaded; binding a blank atlas"
        );
    }
    let (rgba, unihex) = match pixels {
        Some(p) => (Some(p.rgba.into_raw()), p.unihex),
        None => (None, Vec::new()),
    };

    let mut image = match rgba {
        Some(data) => Image::new(
            size,
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        ),
        None => Image::new_uninit(
            size,
            TextureDimension::D2,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        ),
    };
    image.sampler = ImageSampler::nearest();

    let (uw, uh) = atlas.font.unihex_dims;
    let has_unihex = uw > 0 && uh > 0 && unihex.len() == uw as usize * uh as usize;
    let (unihex_size, unihex_data) = if has_unihex {
        ((uw, uh), unihex)
    } else {
        ((1, 1), vec![0u8])
    };
    let mut unihex_image = Image::new(
        Extent3d {
            width: unihex_size.0,
            height: unihex_size.1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        unihex_data,
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    unihex_image.sampler = ImageSampler::nearest();

    (image, unihex_image)
}

fn setup_gui(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    pending: Res<PendingGuiAtlas>,
    mut writes: ResMut<crate::gui::atlas_writes::AtlasWrites>,
) {
    let atlas = pending.0.clone();

    let (image, unihex_image) = atlas_textures(&atlas);
    let image_handle = images.add(image);
    writes.image = Some(image_handle.clone());
    let unihex_image_handle = images.add(unihex_image);

    commands.insert_resource(GuiTextures {
        atlas: image_handle.clone(),
        unihex: unihex_image_handle.clone(),
    });
    commands.insert_resource(GuiAssets {
        atlas,
        image: image_handle,
        unihex_image: unihex_image_handle,
    });
    commands.remove_resource::<PendingGuiAtlas>();
}

fn upload_banner_icons(
    assets: Option<Res<GuiAssets>>,
    items: Option<Res<crate::renderer::item_assets::ItemAssets>>,
    mut writes: ResMut<crate::gui::atlas_writes::AtlasWrites>,
) {
    let (Some(assets), Some(items)) = (assets, items) else {
        return;
    };
    let (edge, pending) = crate::gui::banner_icons::take_pending();
    if pending.is_empty() {
        return;
    }

    for (slot, key) in pending {
        let Some(region) = assets.atlas.banner_icon(slot) else {
            continue;
        };
        let Some(icon) = items.bake_pattern_icon(&key, edge) else {
            continue;
        };
        writes.push(
            crate::gui::atlas::Region {
                x: region.x,
                y: region.y,
                w: edge,
                h: edge,
            },
            icon.as_raw().clone(),
        );
        crate::gui::banner_icons::mark_ready(slot);
    }
}

fn upload_server_icons(
    assets: Option<Res<GuiAssets>>,
    mut writes: ResMut<crate::gui::atlas_writes::AtlasWrites>,
) {
    let Some(assets) = assets else {
        return;
    };
    let pending = crate::gui::ping::take_pending_icons();
    if pending.is_empty() {
        return;
    }

    let icon = crate::gui::atlas::SERVER_ICON_PX;
    let mut written = 0usize;
    for (slot, pixels) in pending {
        let Some(region) = assets.atlas.server_icon(slot) else {
            continue;
        };
        if pixels.len() != (icon * icon * 4) as usize {
            continue;
        }
        writes.push(region, pixels);
        written += 1;
    }

    if written > 0 {
        crate::log_info!("gui", "wrote {written} server icon(s) into the atlas");
    }
}

pub fn auto_gui_scale(width: f32, height: f32) -> f32 {
    let mut scale = 1.0_f32;
    while scale < 8.0 && (scale + 1.0) <= width / 320.0 && (scale + 1.0) <= height / 240.0 {
        scale += 1.0;
    }
    if cfg!(feature = "mobile_ui") {
        scale = (scale + 1.0).min(8.0);
    }
    scale
}

pub fn effective_gui_scale(width: f32, height: f32, option: u32) -> f32 {
    let max = auto_gui_scale(width, height);
    match option {
        0 => max,
        n => (n as f32).clamp(1.0, max),
    }
}

#[cfg(feature = "mobile_ui")]
fn touch_pointer(touches: &Touches) -> Option<(Vec2, bool, bool, bool)> {
    if let Some(t) = touches.iter().next() {
        return Some((t.position(), touches.just_pressed(t.id()), true, false));
    }
    touches
        .iter_just_released()
        .next()
        .map(|t| (t.position(), false, false, true))
}

pub(crate) fn collect_input(
    mut input: ResMut<GuiInput>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cursor_opts: Query<&bevy::window::CursorOptions, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    #[cfg(feature = "mobile_ui")] touches: Res<Touches>,
    keys: Res<ButtonInput<KeyCode>>,
    mut key_events: MessageReader<KeyboardInput>,
    wheel: Res<AccumulatedMouseScroll>,
    state: Res<GuiState>,
    time: Res<Time>,
    mut last_left_press: Local<Option<f32>>,
    mut click_run: Local<u32>,
    mut wheel_acc: Local<(f64, f64)>,
    #[cfg(target_os = "android")] mut keyboard_seen: Local<bool>,
) {
    let Ok(window) = windows.single() else { return };
    let (w, h) = (
        window.physical_width() as f32,
        window.physical_height() as f32,
    );
    let scale = effective_gui_scale(w, h, state.options.gui_scale);
    input.scale = scale;
    input.time = time.elapsed_secs();
    input.size = Vec2::new(w / scale, h / scale);
    input.device_scale = scale;

    let grabbed = cursor_opts
        .single()
        .map(|o| o.grab_mode != bevy::window::CursorGrabMode::None)
        .unwrap_or(false);

    let elsewhere = crate::gui::keyboard_elsewhere();

    #[cfg(feature = "mobile_ui")]
    let touch = state
        .screen
        .is_open()
        .then(|| touch_pointer(&touches))
        .flatten();
    #[cfg(not(feature = "mobile_ui"))]
    let touch: Option<(Vec2, bool, bool, bool)> = None;

    input.mouse = if grabbed || !state.screen.is_open() {
        None
    } else if let Some((pos, ..)) = touch {
        Some(pos * window.scale_factor() / scale)
    } else {
        window.physical_cursor_position().map(|p| p / scale)
    };

    let (touch_click, touch_down, touch_release) =
        touch.map(|(_, c, d, r)| (c, d, r)).unwrap_or_default();

    input.left_click = buttons.just_pressed(MouseButton::Left) || touch_click;
    input.right_click = buttons.just_pressed(MouseButton::Right);
    input.middle_click = buttons.just_pressed(MouseButton::Middle);
    input.left_down = buttons.pressed(MouseButton::Left) || touch_down;
    input.right_down = buttons.pressed(MouseButton::Right);
    input.middle_down = buttons.pressed(MouseButton::Middle);
    input.left_release = buttons.just_released(MouseButton::Left) || touch_release;
    input.right_release = buttons.just_released(MouseButton::Right);
    let notches = crate::renderer::input::wheel_notches(&wheel);
    let (_, wy) = crate::renderer::input::accumulate_scroll(
        &mut wheel_acc,
        notches.x as f64,
        notches.y as f64,
    );
    input.scroll = wy as f32;

    input.ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    input.shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    input.alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    input.backspace = false;
    input.delete = false;
    input.left_arrow = false;
    input.right_arrow = false;
    input.home = false;
    input.end = false;
    input.enter = false;
    input.select = false;
    input.up_arrow = false;
    input.down_arrow = false;
    input.page_up = false;
    input.page_down = false;
    input.tab = false;
    input.typed.clear();
    input.edits.clear();
    input.defocus = false;
    input.escape = !elsewhere && keys.just_pressed(KeyCode::Escape);
    #[cfg(feature = "mobile_ui")]
    if input.left_click {
        crate::gui::widgets::touch_tap::note_press(input.mouse);
    }
    let binds = &state.keybinds;
    let anvil_typing = state.anvil_typing();
    let slot_key = |action: keybinds::Action| {
        !elsewhere
            && !(anvil_typing && !matches!(binds.bound(action), keybinds::Bound::Mouse(_)))
            && binds.just(action, &keys, &buttons)
    };
    input.drop_key = slot_key(keybinds::Action::Drop);
    input.swap_key = slot_key(keybinds::Action::SwapOffhand);
    input.pressed_key = if elsewhere {
        None
    } else {
        keys.get_just_pressed().next().copied()
    };
    input.pressed_mouse = buttons.get_just_pressed().next().copied();

    let shortcut = !elsewhere && input.ctrl && !input.shift && !input.alt;
    input.copy = shortcut && keys.just_pressed(KeyCode::KeyC);
    input.paste = shortcut && keys.just_pressed(KeyCode::KeyV);
    input.cut = shortcut && keys.just_pressed(KeyCode::KeyX);
    input.select_all = shortcut && keys.just_pressed(KeyCode::KeyA);
    input.profile_down = shortcut && keys.just_pressed(KeyCode::BracketLeft);
    input.profile_up = shortcut && keys.just_pressed(KeyCode::BracketRight);
    #[cfg(feature = "hud_editor")]
    {
        input.undo = shortcut && keys.just_pressed(KeyCode::KeyZ);
        let bare = !elsewhere && !input.ctrl && !input.alt;
        input.hud_hide = bare && keys.just_pressed(KeyCode::KeyH);
        input.hud_remove = bare && keys.just_pressed(KeyCode::Delete);
        input.hud_reset = bare && keys.just_pressed(KeyCode::KeyR);
    }

    input.double_click = false;
    input.triple_click = false;
    if input.left_click {
        let now = time.elapsed_secs();
        let chained = last_left_press.is_some_and(|prev| now - prev <= 0.25);
        *click_run = if chained { (*click_run % 3) + 1 } else { 1 };
        *last_left_press = Some(now);
        input.double_click = *click_run == 2;
        input.triple_click = *click_run == 3;
    }
    for (i, action) in keybinds::HOTBAR.into_iter().enumerate() {
        input.hotbar_keys[i] = slot_key(action);
    }

    for ev in key_events.read() {
        if !ev.state.is_pressed() || elsewhere {
            continue;
        }
        match ev.key_code {
            KeyCode::Enter | KeyCode::NumpadEnter => {
                input.enter = true;
                input.select = true;
                input.edits.push(EditKey::Enter);
                continue;
            }
            KeyCode::Tab => {
                input.tab = true;
                continue;
            }
            _ => {}
        }
        match &ev.logical_key {
            Key::Backspace => {
                input.backspace = true;
                input.edits.push(EditKey::Backspace);
            }
            Key::Delete => {
                input.delete = true;
                input.edits.push(EditKey::Delete);
            }
            Key::ArrowLeft => {
                input.left_arrow = true;
                input.edits.push(EditKey::Left);
            }
            Key::ArrowRight => {
                input.right_arrow = true;
                input.edits.push(EditKey::Right);
            }
            Key::ArrowUp => {
                input.up_arrow = true;
                input.edits.push(EditKey::Up);
            }
            Key::ArrowDown => {
                input.down_arrow = true;
                input.edits.push(EditKey::Down);
            }
            Key::Home => {
                input.home = true;
                input.edits.push(EditKey::Home);
            }
            Key::End => {
                input.end = true;
                input.edits.push(EditKey::End);
            }
            Key::PageUp => input.page_up = true,
            Key::PageDown => input.page_down = true,
            Key::Enter => {
                input.enter = true;
                input.select = true;
                input.edits.push(EditKey::Enter);
            }
            Key::Tab => input.tab = true,
            Key::Character(s) => {
                for c in s.chars() {
                    if !c.is_control() {
                        input.typed.push(c);
                        input.edits.push(EditKey::Char(c));
                    }
                }
            }
            Key::Space => {
                input.typed.push(' ');
                input.edits.push(EditKey::Char(' '));
                input.select = true;
            }
            #[cfg(target_os = "android")]
            Key::BrowserBack => {
                let keyboard_up = crate::platform::keyboard::has_focus();
                crate::log_info!(
                    "input",
                    "back key: {}",
                    if keyboard_up {
                        "dismissing the keyboard"
                    } else {
                        "escape"
                    }
                );
                if keyboard_up {
                    input.defocus = true;
                } else {
                    input.escape = true;
                }
            }
            _ => {}
        }
    }

    if let Some(shift) = crate::platform::keyboard::take_tab() {
        input.tab = true;
        input.shift |= shift;
    }

    if state.screen != Screen::Chat && crate::platform::keyboard::take_enter() {
        input.enter = true;
        input.select = true;
        input.edits.push(EditKey::Enter);
    }

    #[cfg(target_os = "android")]
    {
        if !crate::platform::keyboard::has_focus() {
            *keyboard_seen = false;
        } else if crate::platform::keyboard::inset_px() > 0.0 {
            *keyboard_seen = true;
        } else if std::mem::take(&mut *keyboard_seen) {
            crate::log_info!("input", "soft keyboard hid itself: dismissing the field");
            input.defocus = true;
        }
    }

    crate::platform::keyboard::resolve_taps();

    #[cfg(target_os = "android")]
    if input.enter && state.screen.is_menu() && crate::platform::keyboard::has_focus() {
        crate::log_info!("input", "enter key: dismissing the keyboard");
        input.enter = false;
        input.select = false;
        input.defocus = true;
    }

    if elsewhere || state.screen.is_menu() {
        input.escape |= crate::platform::keyboard::take_escape();
    }

    let nav = crate::platform::keyboard::take_nav();
    input.up_arrow |= nav.up;
    input.down_arrow |= nav.down;
    if nav.up {
        input.edits.push(EditKey::Up);
    }
    if nav.down {
        input.edits.push(EditKey::Down);
    }
    input.page_up |= nav.page_up;
    input.page_down |= nav.page_down;

    let creative_search_focused =
        state.screen == Screen::Creative && state.creative.search_focused();
    if binds.just(keybinds::Action::Inventory, &keys, &buttons)
        && matches!(
            state.screen,
            Screen::Creative | Screen::Inventory | Screen::None
        )
        && !creative_search_focused
    {
        let name = binds.bound(keybinds::Action::Inventory).display_name();
        let mut letters = name.chars();
        if let (Some(letter), None) = (letters.next(), letters.next()) {
            input.typed.retain(|c| !c.eq_ignore_ascii_case(&letter));
            input
                .edits
                .retain(|e| !matches!(e, EditKey::Char(c) if c.eq_ignore_ascii_case(&letter)));
        }
    }
}

pub(crate) fn draw_gui(
    assets: Option<Res<GuiAssets>>,
    shared: Res<GuiShared>,
    input: Res<GuiInput>,
    mut state: ResMut<GuiState>,
    primary: Query<Entity, With<PrimaryWindow>>,
    time: Res<Time>,
    frames: Res<GuiFrames>,
    mut scratch: Local<crate::gui::painter::PainterBuffers>,
    #[cfg(feature = "skins")] mut faces: ResMut<crate::gui::player_faces::PlayerFaces>,
) {
    #[cfg(feature = "budget")]
    let _t = crate::diag::budget::timed(crate::diag::budget::Slot::Gui);
    let Some(assets) = assets else { return };
    let Ok(window) = primary.single() else { return };
    let (vw, vh) = (input.size.x, input.size.y);
    if vw <= 0.0 || vh <= 0.0 {
        return;
    }

    let frame = (time.elapsed_secs() * 20.0) as u32;

    let bufs = std::mem::take(&mut *scratch);

    let mut painter = Painter::with_buffers(&assets.atlas, frame, bufs);
    painter.scale = input.device_scale;
    screens::draw(
        &mut painter,
        &mut state,
        &input,
        &shared.0,
        #[cfg(feature = "skins")]
        &mut faces,
    );
    let bufs = painter.into_buffers();

    frames.push(
        window,
        &bufs.positions,
        &bufs.uvs,
        &bufs.colors,
        &bufs.indices,
        vw,
        vh,
    );
    *scratch = bufs;
}

#[allow(
    dead_code,
    reason = "diagnostic, kept compiled while its call site is commented out"
)]
fn draw_geometry_probe(
    painter: &mut Painter,
    windows: &Query<&Window, With<PrimaryWindow>>,
    vw: f32,
    vh: f32,
) {
    let Ok(window) = windows.single() else { return };

    let scale = window.scale_factor();
    let lines = [
        format!(
            "physical  {} x {}",
            window.physical_width(),
            window.physical_height()
        ),
        format!("logical   {:.0} x {:.0}", window.width(), window.height()),
        format!("scale     {scale:.3}"),
        format!("fill      x{:.2}", scale * scale),
        format!("gui units {vw:.0} x {vh:.0}"),
    ];

    const LINE: f32 = 11.0;
    const PAD: f32 = 6.0;
    const ZOOM: f32 = 2.0;
    let text_w = lines
        .iter()
        .map(|l| painter.atlas.font.width_str(l))
        .fold(0.0f32, f32::max);
    let box_w = text_w + PAD * 2.0;
    let box_h = lines.len() as f32 * LINE + PAD * 2.0;

    let origin_x = (vw - box_w * ZOOM) / 2.0;
    let origin_y = (vh - box_h * ZOOM) / 2.0;

    painter.scaled(ZOOM, origin_x, origin_y, |p| {
        p.fill(0.0, 0.0, box_w, box_h, 0xE0_000000);
        p.outline(0.0, 0.0, box_w, box_h, 0xFF_FF0000);
        for (i, line) in lines.iter().enumerate() {
            let colour = if i == 2 || i == 3 { 0xFF5555 } else { 0xFFFF55 };
            p.text_plain(line, PAD, PAD + i as f32 * LINE, colour, true);
        }
    });
}

#[cfg(target_os = "android")]
#[allow(
    dead_code,
    reason = "diagnostic, kept compiled while its call site is commented out"
)]
fn draw_keyboard_probe(p: &mut Painter, input: &GuiInput, vw: f32, vh: f32) {
    const BAR: f32 = 3.0;
    const PAD: f32 = 4.0;
    const RED: u32 = 0xFF_FF0000;

    let px = crate::platform::keyboard::inset_px();
    let wanted = crate::platform::keyboard::has_focus();
    if px <= 0.0 && !wanted {
        return;
    }
    let gui = if input.scale > 0.0 {
        px / input.scale
    } else {
        0.0
    };

    if gui > 0.0 {
        let top = (vh - gui).max(BAR);
        p.fill(0.0, top - BAR, vw, BAR, RED);
    }

    let lines = [
        format!("ime {px:.0} px"),
        format!("gui {gui:.1} of {vh:.0}"),
        format!("keyboard {}", if wanted { "wanted" } else { "not wanted" }),
    ];
    let w = lines
        .iter()
        .map(|l| p.atlas.font.width_str(l))
        .fold(0.0f32, f32::max);
    let side = (w + PAD * 2.0).max(lines.len() as f32 * 10.0 + PAD * 2.0);
    p.fill(PAD, PAD, side, side, 0xC0_800000);
    p.outline(PAD, PAD, side, side, RED);
    for (i, line) in lines.iter().enumerate() {
        p.text_plain(line, PAD * 2.0, PAD * 2.0 + i as f32 * 10.0, 0xFFFFFF, true);
    }
}
