use std::sync::Arc;

use crate::gui::hud_layout::{ElementId, Hud};
use crate::gui::painter::Painter;
use crate::gui::render::GuiInput;
use crate::gui::slots::{self, CONTAINER_W, INV_H, INV_W, Layout, draw_stack};
use crate::gui::{
    GuiState, Screen, ScreenCtx, Snapshot, book, chat, container, creative, focus, health, hud,
    multiplayer, options, pause, profile, sign_edit, sleep, spectator_menu, tablist, title, toast,
    tooltip,
};
use crate::session::{ContainerKind, InvAction, SharedMutex, SlotStack};

pub fn draw(
    p: &mut Painter,
    state: &mut GuiState,
    input: &GuiInput,
    shared: &Arc<SharedMutex>,
    #[cfg(feature = "skins")] faces: &mut crate::gui::player_faces::PlayerFaces,
) {
    #[cfg(feature = "skins")]
    faces.begin_frame();

    let eye_height;
    let eye_pos;
    let mut snap = {
        let mut s = shared.lock().unwrap();
        let menu_slots = if state.screen.is_modal() {
            s.session.menu_slots.clone()
        } else {
            Vec::new()
        };
        eye_height = crate::renderer::systems::session_eye_height(&s.session) as f64;
        eye_pos = s.session.player_pos;

        Snapshot {
            action_bar: s.session.action_bar.take(),
            title_text: s.session.title_text.take(),
            subtitle_text: s.session.subtitle_text.take(),
            title_times: s.session.title_times.take(),
            titles_clear: s.session.titles_clear.take(),
            menu_slots,
            hotbar: s.session.hotbar.clone(),
            carried: s.session.carried.clone(),
            selected: s.session.hotbar_selected,
            gamemode: s.session.gamemode,
            health: s.session.health,
            food: s.session.food,
            saturation: s.session.saturation,
            air_supply: s.session.air_supply,
            health_display: s.session.health_display,
            hardcore: s.session.hardcore,
            health_epoch: s.session.health_epoch,
            jump_charge: s.session.jump_charge,
            eye_in_water: false,
            chat_incoming: std::mem::take(&mut s.session.chat_incoming),
            toast_incoming: std::mem::take(&mut s.session.toast_incoming),
            chat_error: s.session.chat_send_error.take(),
            enforces_secure_chat: s.session.enforces_secure_chat,
            chat_signing: s.chat_signing_allowed || s.session.chat_signing_session,
            attack_strength: s.session.attack_strength,
            attack_delay: s.session.attack_delay,
            targeted_entity: s.session.targeted_entity,
            container_title: if state.screen.is_container() {
                s.session.container_title.clone()
            } else {
                crate::session::empty_spans()
            },
            container_id: s.session.container_id,
            container_data: if state.screen.is_container() {
                s.session.container_data
            } else {
                Default::default()
            },
            enchant_clues: match state.screen {
                Screen::Container(ContainerKind::Enchantment) => s.session.enchant_clues.clone(),
                _ => Default::default(),
            },
            merchant: match state.screen {
                Screen::Container(ContainerKind::Merchant) => s.session.merchant.clone(),
                _ => None,
            },
            merchant_future_xp: s.session.merchant_future_xp,
            stonecutter_recipes: match state.screen {
                Screen::Container(ContainerKind::Stonecutter) => {
                    s.session.stonecutter_recipes.clone()
                }
                _ => Default::default(),
            },
            active_effects: if matches!(state.screen, Screen::Inventory | Screen::Creative) {
                s.session.active_effects.clone()
            } else {
                Vec::new()
            },
            xp_progress: s.session.xp_progress,
            xp_level: s.session.xp_level,
            xp_recent_gain: s.session.xp_seen
                && s.session.xp_display_tick.saturating_add(100)
                    > crate::client::tracking::GAME_TIME.load(std::sync::atomic::Ordering::Relaxed),
            waypoints: s.session.waypoints.clone(),
            eye_pos: [eye_pos[0], eye_pos[1] + eye_height as f32, eye_pos[2]],
            camera_yaw: s.camera_yaw,
            camera_pitch: s.camera_pitch,
            sleep_timer: s.session.sleep_timer,
            sleeping: s.session.sleeping,
            sidebar: s.session.sidebar.clone(),
            boss_bars: s.session.boss_bars.clone(),
            tab_list: if state.tab_list_held
                || (cfg!(feature = "mobile_ui") && state.screen == Screen::Pause)
            {
                s.session.tab_list.clone()
            } else {
                Default::default()
            },
        }
    };

    snap.eye_in_water = crate::util::block_model::eye_fluid_at(
        eye_pos[0] as f64,
        eye_pos[1] as f64 + eye_height,
        eye_pos[2] as f64,
    ) == Some(crate::util::block_model::EyeFluid::Water);

    #[cfg(feature = "mobile_ui")]
    let lift = {
        let inset = if input.scale > 0.0 {
            crate::platform::keyboard::inset_px() / input.scale
        } else {
            0.0
        };
        crate::gui::widgets::keyboard_lift::begin(
            state.screen.is_menu(),
            input.size.y,
            inset,
            input.left_down || input.left_release,
        )
    };
    #[cfg(feature = "mobile_ui")]
    let lifted_input;
    #[cfg(feature = "mobile_ui")]
    let input = if lift > 0.0 {
        let mut shifted = input.clone();
        shifted.mouse = shifted.mouse.map(|m| m + bevy::math::Vec2::new(0.0, lift));
        lifted_input = shifted;
        &lifted_input
    } else {
        input
    };

    let ctx = ScreenCtx {
        input,
        vw: input.size.x,
        vh: input.size.y,
    };

    state.hud_overlays.update(&mut snap, input.time);

    let toast_settings = toast::ToastSettings::from_state(state);
    state.toasts.ingest(
        std::mem::take(&mut snap.toast_incoming),
        &p.atlas.font,
        toast_settings,
    );

    let mut actions: Vec<InvAction> = Vec::new();

    let chat_incoming = std::mem::take(&mut snap.chat_incoming);
    let chat_error = snap.chat_error.take();
    let secure_chat = snap.enforces_secure_chat;
    let chat_signing = snap.chat_signing;

    let in_menu = state.in_menu();
    #[cfg(feature = "hud_editor")]
    let editing = state.screen == Screen::HudEditor;
    #[cfg(not(feature = "hud_editor"))]
    let editing = false;
    let hide_gui = state.hide_gui && !editing;
    let hud_shown = !in_menu || editing;
    #[cfg(feature = "hud_editor")]
    if editing {
        crate::gui::hud_editor::draw_under(p, &state.hud_state, &ctx);
    }
    let hud_visible = hud_shown && !hide_gui;

    state.hud_state.frames.begin(ctx.vw, ctx.vh);
    let mut elements = Hud::new(
        &state.hud_state.layout,
        &mut state.hud_state.frames,
        editing,
        input.device_scale,
    );

    if hud_visible {
        draw_hud(
            p,
            &ctx,
            &snap,
            state.options.main_hand_left,
            &mut state.hearts,
            &mut elements,
        );
        hud::draw_nametags(p, &ctx, &state.hud.nametags);
        let show_fps = state.options.fps_counter && state.hud.debug.is_none();
        hud::draw(
            p,
            &ctx,
            &state.hud,
            &snap.sidebar,
            state.screen == Screen::Chat,
            show_fps,
            &mut elements,
        );
        elements.draw(p, ctx.vw, ctx.vh, ElementId::BossBars, |p| {
            hud::draw_boss_bars(p, &ctx, &snap.boss_bars);
        });
        if snap.gamemode == crate::session::Gamemode::Spectator {
            elements.follow(p, ctx.vw, ctx.vh, ElementId::Hotbar, |p| {
                spectator_menu::draw(
                    p,
                    &ctx,
                    &mut state.spectator_menu,
                    &snap.tab_list,
                    #[cfg(feature = "skins")]
                    faces,
                );
            });
        }
    }

    if !hide_gui && hud_shown {
        elements.draw(p, ctx.vw, ctx.vh, ElementId::Sidebar, |p| {
            hud::draw_sidebar(p, &ctx, &snap.sidebar);
        });
        hud::draw_overlays(p, &ctx, &state.hud_overlays, snap.gamemode, &mut elements);
    }

    if !in_menu {
        let focused = state.screen == Screen::Chat;
        let placed = !focused && hud_visible && elements.shown(ElementId::Chat);
        let visible = hud_visible && (focused || placed);
        macro_rules! draw_chat {
            ($p:expr) => {
                chat::draw(
                    $p,
                    &mut state.chat,
                    &ctx,
                    focused,
                    visible,
                    chat_incoming,
                    chat_error,
                    secure_chat,
                    chat_signing,
                    state.advanced_tooltips,
                    shared,
                )
            };
        }
        if placed {
            elements.draw(p, ctx.vw, ctx.vh, ElementId::Chat, |p| draw_chat!(p));
        } else {
            draw_chat!(p);
        }
    }
    if in_menu && editing {
        elements.draw(p, ctx.vw, ctx.vh, ElementId::Chat, |_| {});
    }
    #[cfg(feature = "mobile_ui")]
    if state.screen != Screen::Pause {
        tablist::hide(&mut state.tablist);
    }

    #[cfg(feature = "mobile_ui")]
    if chat::texting() && state.screen != Screen::Chat {
        chat::set_texting(false);
    }
    #[cfg(not(feature = "mobile_ui"))]
    if hud_visible && state.tab_list_held {
        tablist::draw(
            p,
            &ctx,
            &snap.tab_list,
            &mut state.tablist,
            #[cfg(feature = "skins")]
            faces,
        );
    } else {
        tablist::hide(&mut state.tablist);
    }

    let debug_gate = !hide_gui || state.screen.is_open();
    if let Some(d) = &state.hud.debug
        && debug_gate
    {
        elements.draw(p, ctx.vw, ctx.vh, ElementId::Debug, |p| {
            hud::draw_debug(p, &ctx, &state.hud, d);
        });
    }
    #[cfg(feature = "mobile_ui")]
    if editing {
        let view = crate::mobile::View {
            ui: &state.mobile,
            movement: state.options.touch_movement,
            screen: state.screen,
            hide_gui,
            in_menu,
            editing: true,
        };
        crate::mobile::draw(p, &ctx, view, &mut elements);
    }

    #[cfg(feature = "hud_editor")]
    if state.hud.debug.is_none() && debug_gate && elements.editing() {
        elements.ghost(
            p,
            ctx.vw,
            ctx.vh,
            ElementId::Debug,
            "Debug (F3), off",
            |p| {
                hud::draw_debug(p, &ctx, &state.hud, &hud::DebugInfo::default());
            },
        );
    }

    focus::begin(
        state.screen.tab_navigates(),
        match (ctx.input.tab, ctx.input.shift) {
            (true, false) => 1,
            (true, true) => -1,
            _ => 0,
        },
    );

    #[cfg(feature = "mobile_ui")]
    if lift > 0.0 {
        p.set_lift(lift);
    }
    let hovered: Option<SlotStack> = match state.screen {
        Screen::None | Screen::Chat => None,
        Screen::Inventory => draw_inventory(p, state, &ctx, &snap, &mut actions),
        Screen::Creative => creative::draw(
            p,
            &mut state.creative,
            &mut state.slots,
            &ctx,
            &snap,
            &mut actions,
        ),
        Screen::Container(kind) => container::draw(p, state, &ctx, &snap, kind, &mut actions),
        Screen::Pause => {
            dim_background(p, ctx.vw, ctx.vh);
            pause::draw(p, state, &ctx);
            #[cfg(feature = "mobile_ui")]
            tablist::draw_side(
                p,
                &ctx,
                &snap.tab_list,
                &mut state.tablist,
                pause::side_strip_right(ctx.vw, ctx.vh),
                #[cfg(feature = "skins")]
                faces,
            );
            None
        }
        Screen::Options | Screen::VideoSettings | Screen::Controls | Screen::GameSettings => {
            if state.in_menu() {
                crate::gui::menu::background(p, ctx.vw, ctx.vh);
            } else {
                dim_background(p, ctx.vw, ctx.vh);
            }
            match state.screen {
                Screen::VideoSettings => options::draw_video(p, state, &ctx),
                Screen::Controls => options::draw_controls(p, state, &ctx),
                Screen::GameSettings => options::draw_game(p, state, &ctx),
                _ => options::draw(p, state, &ctx),
            }
            None
        }
        #[cfg(feature = "audio")]
        Screen::AudioSettings => {
            if state.in_menu() {
                crate::gui::menu::background(p, ctx.vw, ctx.vh);
            } else {
                dim_background(p, ctx.vw, ctx.vh);
            }
            options::draw_audio(p, state, &ctx);
            None
        }
        #[cfg(resource_packs)]
        Screen::ResourcePacks => {
            if state.in_menu() {
                crate::gui::menu::background(p, ctx.vw, ctx.vh);
            } else {
                dim_background(p, ctx.vw, ctx.vh);
            }
            crate::gui::resourcepacks::draw(p, state, &ctx);
            None
        }
        #[cfg(feature = "shader_support")]
        Screen::ShaderPacks | Screen::ShaderOptions => {
            if state.in_menu() {
                crate::gui::menu::background(p, ctx.vw, ctx.vh);
            } else {
                dim_background(p, ctx.vw, ctx.vh);
            }
            match state.screen {
                Screen::ShaderPacks => crate::gui::shaderpacks::draw_packs(p, state, &ctx),
                _ => crate::gui::shaderpacks::draw_options(p, state, &ctx),
            }
            None
        }
        Screen::Title => {
            title::draw(p, state, &ctx);
            None
        }
        #[cfg(feature = "asset_download")]
        Screen::AssetDownload => {
            crate::gui::assetdownload::draw(p, state, &ctx);
            None
        }
        Screen::Multiplayer => {
            multiplayer::draw(p, state, &ctx);
            None
        }
        Screen::ManageServer => {
            multiplayer::draw_manage(p, state, &ctx);
            None
        }
        Screen::DirectConnect => {
            multiplayer::draw_direct(p, state, &ctx);
            None
        }
        Screen::Accounts => {
            profile::draw(p, state, &ctx);
            None
        }
        Screen::EditProfile => {
            profile::draw_edit(p, state, &ctx);
            None
        }
        #[cfg(feature = "online_mode")]
        Screen::AddAccount => {
            profile::draw_add(p, state, &ctx);
            None
        }
        #[cfg(feature = "online_mode")]
        Screen::MicrosoftLogin => {
            profile::draw_microsoft(p, state, &ctx);
            None
        }
        Screen::Disconnected => {
            if let Some(next) = crate::gui::disconnected::draw(p, state, &ctx) {
                state.nav = Some(next);
            }
            None
        }
        #[cfg(feature = "click_gui")]
        Screen::ClickGui => {
            if state.clickgui_parent.is_menu() {
                crate::gui::menu::background(p, ctx.vw, ctx.vh);
            }
            crate::gui::clickgui::draw(p, &mut state.clickgui, &ctx);
            None
        }
        #[cfg(feature = "hud_editor")]
        Screen::HudEditor => {
            if let Some(next) =
                crate::gui::hud_editor::draw(p, &mut state.hud_state, &mut state.options, &ctx)
            {
                state.nav = Some(next);
            }
            None
        }
        Screen::Sleep => {
            sleep::draw(p, &ctx, shared);
            None
        }
        Screen::Death => {
            if let Some(next) = crate::gui::death::draw(p, &mut state.death, &ctx, shared) {
                state.disconnect = true;
                state.nav = Some(next);
            }
            None
        }
        Screen::SignedChatPrompt => {
            dim_background(p, ctx.vw, ctx.vh);
            if let Some(next) = crate::gui::signedchat::draw(p, &ctx, shared) {
                state.nav = Some(next);
            }
            None
        }
        Screen::Dialog => {
            if state.in_menu() {
                crate::gui::menu::background(p, ctx.vw, ctx.vh);
            } else if state.dialog.pauses() {
                dim_background(p, ctx.vw, ctx.vh);
            }
            let out = crate::gui::dialog::draw(p, &mut state.dialog, &ctx, shared);
            if out.disconnect {
                state.disconnect = true;
            }
            if let Some(command) = out.chat {
                state.chat.open(&command);
                state.nav = Some(Screen::Chat);
            } else if let Some(next) = out.nav {
                state.nav = Some(next);
            }
            None
        }
        Screen::CommandBlock => {
            dim_background(p, ctx.vw, ctx.vh);
            if let Some(next) =
                crate::gui::command_block::draw(p, &mut state.command_block, &ctx, shared)
            {
                state.nav = Some(next);
                clear_world_clicks(shared);
            }
            None
        }
        Screen::SignEdit => {
            dim_background(p, ctx.vw, ctx.vh);
            if let Some(next) = sign_edit::draw(p, &mut state.sign_edit, &ctx, shared) {
                state.nav = Some(next);
                clear_world_clicks(shared);
            }
            None
        }
        Screen::BookView => {
            if let Some(next) = book::draw_view(p, &mut state.book, &ctx) {
                state.nav = Some(next);
                clear_world_clicks(shared);
            }
            None
        }
        Screen::BookEdit => {
            if let Some(out) = book::draw_edit(p, &mut state.book, &ctx) {
                apply_book_out(out, state, shared);
            }
            None
        }
        Screen::BookSign => {
            if let Some(out) = book::draw_sign(p, &mut state.book, &ctx) {
                apply_book_out(out, state, shared);
            }
            None
        }
    };
    #[cfg(feature = "mobile_ui")]
    if lift > 0.0 {
        p.set_lift(0.0);
    }

    focus::end();

    let carried = state.slots.cursor(&snap);
    let shown = state.slots.display_cursor(&snap);
    if state.screen.is_open()
        && !shown.is_empty()
        && let Some(m) = ctx.mouse()
    {
        draw_stack(p, &shown, m.x - 8.0, m.y - 8.0);
    }

    if let Some(stack) = hovered
        && carried.is_empty()
        && let Some(m) = ctx.mouse()
    {
        tooltip::draw(p, &stack, m.x, m.y, ctx.vw, ctx.vh, state.advanced_tooltips);
    }

    toast::draw(p, &ctx, &mut state.toasts, toast_settings);

    #[cfg(feature = "mobile_ui")]
    {
        #[cfg(feature = "hud_editor")]
        let editing = state.screen == Screen::HudEditor;
        #[cfg(not(feature = "hud_editor"))]
        let editing = false;
        if !editing {
            let view = crate::mobile::View {
                ui: &state.mobile,
                movement: state.options.touch_movement,
                screen: state.screen,
                hide_gui: state.hide_gui,
                in_menu,
                editing: false,
            };
            let mut controls = Hud::new(
                &state.hud_state.layout,
                &mut state.hud_state.frames,
                false,
                input.device_scale,
            );
            crate::mobile::draw(p, &ctx, view, &mut controls);
        }
    }

    if !actions.is_empty() {
        let mut s = shared.lock().unwrap();
        for action in &actions {
            if let InvAction::CreativeSet { slot, stack } = action
                && let Some(dst) = s.session.menu_slots.get_mut(*slot as usize)
            {
                *dst = local_stack(stack);
            }
        }
        s.session.inv_actions.extend(actions);
    }
}

fn local_stack(stack: &SlotStack) -> SlotStack {
    if stack.is_empty() {
        return SlotStack::default();
    }
    let max_damage = crate::generated_items::item(stack.item)
        .map(|d| d.max_damage)
        .unwrap_or(0);
    SlotStack {
        item: stack.item,
        count: stack.count,
        max_damage,
        enchantments: stack.enchantments.clone(),
        potion: stack.potion.clone(),
        ..Default::default()
    }
}

#[derive(PartialEq, Debug)]
struct AttackIndicator {
    x: f32,
    y: f32,
    ready: bool,
    progress: Option<f32>,
}

impl AttackIndicator {
    fn new(vw: f32, vh: f32, snap: &Snapshot) -> Self {
        let x = (vw / 2.0).floor() - 8.0;
        let y = (vh / 2.0).floor() - 7.0 + 16.0;
        let charged = snap.attack_strength >= 1.0;
        let ready = snap.targeted_entity && charged && snap.attack_delay > 5.0;
        AttackIndicator {
            x,
            y,
            ready,
            progress: (!ready && !charged).then(|| (snap.attack_strength * 17.0) as i32 as f32),
        }
    }
}

fn draw_attack_indicator(p: &mut Painter, vw: f32, vh: f32, snap: &Snapshot) {
    let ind = AttackIndicator::new(vw, vh, snap);
    if ind.ready {
        p.sprite(
            "hud/crosshair_attack_indicator_full",
            ind.x,
            ind.y,
            16.0,
            16.0,
        );
        return;
    }
    let Some(progress) = ind.progress else { return };
    p.sprite(
        "hud/crosshair_attack_indicator_background",
        ind.x,
        ind.y,
        16.0,
        4.0,
    );
    if progress <= 0.0 {
        return;
    }
    let clip = p.push_clip(ind.x, ind.y, progress, 4.0);
    p.sprite(
        "hud/crosshair_attack_indicator_progress",
        ind.x,
        ind.y,
        16.0,
        4.0,
    );
    p.pop_clip(clip);
}

pub(crate) struct HotbarGeom {
    left: f32,
    top: f32,
    bottom: f32,
}

impl HotbarGeom {
    pub(crate) fn new(vw: f32, vh: f32) -> Self {
        let center = (vw / 2.0).floor();
        HotbarGeom {
            left: center - 91.0,
            top: vh.floor() - 22.0,
            bottom: vh.floor(),
        }
    }

    pub(crate) fn left(&self) -> f32 {
        self.left
    }

    pub(crate) fn top(&self) -> f32 {
        self.top
    }

    pub(crate) fn item_x(&self, i: usize) -> f32 {
        self.left + 3.0 + i as f32 * 20.0
    }

    fn item_y(&self) -> f32 {
        self.bottom - 19.0
    }

    #[cfg(feature = "mobile_ui")]
    pub(crate) fn slot_rect(&self, i: usize) -> (f32, f32, f32, f32) {
        (self.left + 1.0 + i as f32 * 20.0, self.top, 20.0, 22.0)
    }
}

fn draw_hud(
    p: &mut Painter,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    main_hand_left: bool,
    hearts: &mut health::HeartAnim,
    elements: &mut Hud,
) {
    let (vw, vh) = (ctx.vw, ctx.vh);

    health::draw_freeze_overlay(p, snap, vw, vh);

    p.sprite(
        "hud/crosshair",
        ((vw - 15.0) / 2.0).floor(),
        ((vh - 15.0) / 2.0).floor(),
        15.0,
        15.0,
    );

    draw_attack_indicator(p, vw, vh, snap);

    elements.draw(p, vw, vh, ElementId::Hotbar, |p| {
        draw_hotbar_cluster(p, vw, vh, snap, main_hand_left, hearts);
    });
    draw_sleep_overlay(p, vw, vh, snap);
}

fn draw_hotbar_cluster(
    p: &mut Painter,
    vw: f32,
    vh: f32,
    snap: &Snapshot,
    main_hand_left: bool,
    hearts: &mut health::HeartAnim,
) {
    let g = HotbarGeom::new(vw, vh);
    let (left, top) = (g.left, g.top);
    if snap.gamemode != crate::session::Gamemode::Spectator {
        p.sprite("hud/hotbar", left, top, 182.0, 22.0);
        p.sprite(
            "hud/hotbar_selection",
            left - 1.0 + snap.selected as f32 * 20.0,
            top - 1.0,
            24.0,
            23.0,
        );

        let offhand = snap.offhand();
        if !offhand.is_empty() {
            let (sprite, x) = if main_hand_left {
                ("hud/hotbar_offhand_right", left + 182.0)
            } else {
                ("hud/hotbar_offhand_left", left - 29.0)
            };
            p.sprite(sprite, x, g.bottom - 23.0, 29.0, 24.0);
        }

        let item_y = g.item_y();
        for i in 0..9 {
            let stack = snap.hotbar(i);
            if !stack.is_empty() {
                draw_stack(p, stack, g.item_x(i), item_y);
            }
        }
        if !offhand.is_empty() {
            let x = if main_hand_left {
                left + 182.0 + 10.0
            } else {
                left - 26.0
            };
            draw_stack(p, offhand, x, item_y);
        }
    }

    let row_y = g.bottom - 39.0;
    let x_left = left;
    let x_right = left + 182.0;

    let survival = matches!(
        snap.gamemode,
        crate::session::Gamemode::Survival | crate::session::Gamemode::Adventure
    );

    let tick = p.frame as u64;
    let (rows, blink) = hearts.frame(snap, tick);
    let mut rng = health::frame_random(tick);
    let vehicle_hearts = health::draw(
        p, snap, &rows, blink, tick, x_left, x_right, row_y, survival, &mut rng,
    );
    if survival {
        draw_air_bubbles(
            p,
            snap,
            x_right,
            health::air_row_y(row_y, vehicle_hearts),
            &mut rng,
        );
    }

    draw_contextual_bar(p, vw, vh, snap);
}

fn draw_sleep_overlay(p: &mut Painter, vw: f32, vh: f32, snap: &Snapshot) {
    if snap.sleep_timer == 0 {
        return;
    }
    let amount = if snap.sleep_timer <= 100 {
        snap.sleep_timer as f32 / 100.0
    } else {
        (1.0 - (snap.sleep_timer - 100) as f32 / 10.0).max(0.0)
    };
    let alpha = ((220.0 * amount).clamp(0.0, 255.0)) as u32;
    p.fill(0.0, 0.0, vw, vh, (alpha << 24) | 0x0010_1020);
}

fn draw_contextual_bar(p: &mut Painter, vw: f32, vh: f32, snap: &Snapshot) {
    let has_experience = matches!(
        snap.gamemode,
        crate::session::Gamemode::Survival | crate::session::Gamemode::Adventure
    );
    let has_locator = !snap.waypoints.is_empty();
    let charging = snap.jump_charge.is_some_and(|scale| scale > 0.0);

    if has_locator {
        if charging {
            draw_jump_bar(p, vw, vh, snap.jump_charge.unwrap_or(0.0));
        } else if has_experience && snap.xp_recent_gain {
            draw_experience_bar(p, vw, vh, snap);
        } else {
            draw_locator_bar(p, vw, vh, snap);
        }
    } else if snap.jump_charge.is_some() {
        draw_jump_bar(p, vw, vh, snap.jump_charge.unwrap_or(0.0));
    } else if has_experience {
        draw_experience_bar(p, vw, vh, snap);
    }

    if has_experience && snap.xp_level > 0 {
        draw_experience_level(p, vw, vh, snap.xp_level);
    }
}

fn contextual_bar_origin(vw: f32, vh: f32) -> (f32, f32) {
    (((vw - 182.0) / 2.0).floor(), vh - 24.0 - 5.0)
}

fn draw_experience_bar(p: &mut Painter, vw: f32, vh: f32, snap: &Snapshot) {
    let (left, top) = contextual_bar_origin(vw, vh);
    p.sprite("hud/experience_bar_background", left, top, 182.0, 5.0);
    let progress = (snap.xp_progress.clamp(0.0, 1.0) * 183.0) as i32;
    if progress > 0 {
        p.sprite_part(
            "hud/experience_bar_progress",
            0.0,
            0.0,
            progress as f32,
            5.0,
            left,
            top,
        );
    }
}

fn draw_jump_bar(p: &mut Painter, vw: f32, vh: f32, scale: f32) {
    let (left, top) = contextual_bar_origin(vw, vh);
    p.sprite("hud/jump_bar_background", left, top, 182.0, 5.0);
    let progress = (scale.clamp(0.0, 1.0) * 182.0) as i32;
    if progress > 0 {
        p.sprite_part(
            "hud/jump_bar_progress",
            0.0,
            0.0,
            progress as f32,
            5.0,
            left,
            top,
        );
    }
}

fn draw_experience_level(p: &mut Painter, vw: f32, vh: f32, level: u32) {
    let text = format!("{level}");
    let w = p.atlas.font.width_str(&text);
    let x = ((vw - w) / 2.0).floor();
    let y = vh - 24.0 - 9.0 - 2.0;
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        p.text_str(&text, x + dx, y + dy, 0x000000, false);
    }
    p.text_str(&text, x, y, 0x80FF20, false);
}

fn draw_locator_bar(p: &mut Painter, vw: f32, vh: f32, snap: &Snapshot) {
    let (left, top) = contextual_bar_origin(vw, vh);
    p.sprite("hud/locator_bar_background", left, top, 182.0, 5.0);

    let mc_yaw = wrap_deg(-snap.camera_yaw - 180.0);
    let screen_mid = ((vw - 9.0) / 2.0).ceil();

    for wp in snap.waypoints.iter() {
        let (dx, dz, horiz_dist, dy) = waypoint_delta(snap, wp);
        let target_yaw = (-dx).atan2(dz).to_degrees();
        let angle = wrap_deg(target_yaw - mc_yaw);
        if !(-60.0..=60.0).contains(&angle) {
            continue;
        }
        let dot_pos = (angle * 173.0 / 2.0 / 60.0).floor();
        let x = screen_mid + dot_pos;

        let sprite = if wp.style == "bowtie" {
            "hud/locator_bar_dot/bowtie".to_string()
        } else {
            format!(
                "hud/locator_bar_dot/default_{}",
                dot_style_index(horiz_dist)
            )
        };
        let color = wp
            .color
            .map(pack_color)
            .unwrap_or_else(|| identity_color(&wp.id));
        p.sprite_tinted(&sprite, x, top - 2.0, 9.0, 9.0, color);

        let vertical_angle = dy.atan2(horiz_dist as f64).to_degrees() as f32;
        let pitch_delta = vertical_angle - snap.camera_pitch;
        let arrow_x = x + 1.0;
        if pitch_delta > 8.0 {
            p.sprite_part(
                "hud/locator_bar_arrow_up",
                0.0,
                0.0,
                7.0,
                5.0,
                arrow_x,
                top - 6.0,
            );
        } else if pitch_delta < -8.0 {
            p.sprite_part(
                "hud/locator_bar_arrow_down",
                0.0,
                0.0,
                7.0,
                5.0,
                arrow_x,
                top + 6.0,
            );
        }
    }
}

fn pack_color([r, g, b]: [f32; 3]) -> u32 {
    0xFF00_0000 | ((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32
}

fn identity_color(id: &crate::session::WaypointKey) -> u32 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    let hash = hasher.finish() as u32;
    let (r, g, b) = ((hash >> 16) & 0xFF, (hash >> 8) & 0xFF, hash & 0xFF);
    let max = r.max(g).max(b);
    if max == 0 {
        return 0xFFE6_E6E6;
    }
    let scale = (0.9 * 255.0) / max as f32;
    let scaled = |c: u32| ((c as f32 * scale).round().clamp(0.0, 255.0)) as u32;
    0xFF00_0000 | scaled(r) << 16 | scaled(g) << 8 | scaled(b)
}

fn dot_style_index(distance: f32) -> u32 {
    const NEAR: f32 = 128.0;
    const FAR: f32 = 332.0;
    if distance < NEAR {
        0
    } else if distance >= FAR {
        3
    } else {
        let t = (distance - NEAR) / (FAR - NEAR);
        1 + (t * 2.0).floor() as u32
    }
}

fn waypoint_delta(snap: &Snapshot, wp: &crate::session::WaypointInfo) -> (f32, f32, f32, f64) {
    use crate::session::WaypointPos;
    match wp.pos {
        WaypointPos::Block([bx, by, bz]) => {
            let dx = bx as f64 + 0.5 - snap.eye_pos[0] as f64;
            let dy = by as f64 + 0.5 - snap.eye_pos[1] as f64;
            let dz = bz as f64 + 0.5 - snap.eye_pos[2] as f64;
            let horiz = (dx * dx + dz * dz).sqrt();
            (dx as f32, dz as f32, horiz as f32, dy)
        }
        WaypointPos::Chunk { x, z } => {
            let dx = (x * 16 + 8) as f64 - snap.eye_pos[0] as f64;
            let dz = (z * 16 + 8) as f64 - snap.eye_pos[2] as f64;
            let horiz = (dx * dx + dz * dz).sqrt();
            (dx as f32, dz as f32, horiz as f32, 0.0)
        }
        WaypointPos::Azimuth(angle_rad) => {
            let dx = -angle_rad.sin() * FAR_DISTANCE;
            let dz = angle_rad.cos() * FAR_DISTANCE;
            (dx, dz, FAR_DISTANCE, 0.0)
        }
    }
}

const FAR_DISTANCE: f32 = 10_000.0;

fn wrap_deg(mut deg: f32) -> f32 {
    deg %= 360.0;
    if deg >= 180.0 {
        deg -= 360.0;
    } else if deg < -180.0 {
        deg += 360.0;
    }
    deg
}

fn draw_air_bubbles(
    p: &mut Painter,
    snap: &Snapshot,
    x_right: f32,
    y: f32,
    rng: &mut crate::util::javarandom::JavaRandom,
) {
    let max = crate::session::MAX_AIR_SUPPLY;
    let current = snap.air_supply.clamp(0, max);
    if !snap.eye_in_water && current >= max {
        return;
    }
    let states = air_bubble_states(current, max, snap.eye_in_water);
    let wobbles = p.frame % 2 == 0 && empty_bubbles(current, max, snap.eye_in_water) == 10;
    for (i, state) in states.into_iter().enumerate() {
        let x = x_right - i as f32 * 8.0 - 9.0;
        let sprite = match state {
            AirBubble::Full => "hud/air",
            AirBubble::Popping => "hud/air_bursting",
            AirBubble::Empty => "hud/air_empty",
            AirBubble::Hidden => continue,
        };
        let y = if wobbles && state == AirBubble::Empty {
            y + rng.next_int(2) as f32
        } else {
            y
        };
        p.sprite(sprite, x, y, 9.0, 9.0);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AirBubble {
    Full,
    Popping,
    Empty,
    Hidden,
}

fn empty_bubbles(current: i32, max: i32, eye_in_water: bool) -> i32 {
    let delay = if current != 0 && eye_in_water { 1 } else { 0 };
    10 - ((((current + delay) * 10) as f32) / max as f32).ceil() as i32
}

fn air_bubble_states(current: i32, max: i32, eye_in_water: bool) -> [AirBubble; 10] {
    let bubble_count = |offset: i32| -> i32 {
        let scaled = (current + offset) * 10;
        (scaled as f32 / max as f32).ceil() as i32
    };
    let full = bubble_count(-2);
    let popping_pos = bubble_count(0);
    let empty = empty_bubbles(current, max, eye_in_water);
    let is_popping = full != popping_pos;

    std::array::from_fn(|i| {
        let bubble = i as i32 + 1;
        if bubble <= full {
            AirBubble::Full
        } else if is_popping && bubble == popping_pos && eye_in_water {
            AirBubble::Popping
        } else if bubble > 10 - empty {
            AirBubble::Empty
        } else {
            AirBubble::Hidden
        }
    })
}

fn draw_inventory(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let (left, top) = container::window_origin(Layout::PlayerMenu, ctx);
    dim_background(p, ctx.vw, ctx.vh);
    p.blit_sheet("container/inventory", 0.0, 0.0, INV_W, INV_H, left, top);
    let hovered = slots::panel(
        p,
        Layout::PlayerMenu,
        (left, top),
        ctx,
        snap,
        &mut state.slots,
        out,
    );
    draw_effects_panel(p, ctx, &snap.active_effects, left, top, INV_W);
    hovered
}

pub(crate) fn draw_effects_panel(
    p: &mut Painter,
    ctx: &ScreenCtx,
    effects: &[crate::play::mob_effects::MobEffectInstance],
    left: f32,
    top: f32,
    image_width: f32,
) {
    if effects.is_empty() {
        return;
    }
    let x0 = left + image_width + 2.0;
    let available = ctx.vw - x0;
    if available < 32.0 {
        return;
    }
    let max_width = if available >= 120.0 {
        available - 7.0
    } else {
        32.0
    };
    let y_step = if effects.len() > 5 {
        132.0 / (effects.len() - 1) as f32
    } else {
        33.0
    };

    let mut sorted: Vec<&crate::play::mob_effects::MobEffectInstance> = effects.iter().collect();
    sorted.sort_by(|a, b| crate::play::mob_effects::compare(a, b));

    let mut y0 = top;
    for effect in sorted {
        let name = crate::play::mob_effects::display_name(&effect.id, effect.amplifier);
        let duration = crate::play::mob_effects::format_duration(effect.duration);
        let name_w = 32.0 + p.atlas.font.width_str(&name) + 7.0;
        let duration_w = 32.0 + p.atlas.font.width_str(&duration) + 7.0;
        let texture_w = name_w.max(duration_w).min(max_width);

        let bg = if effect.ambient {
            "container/inventory/effect_background_ambient"
        } else {
            "container/inventory/effect_background"
        };
        p.sprite(bg, x0.floor(), y0.floor(), texture_w.floor(), 32.0);

        let icon = format!("mob_effect/{}", effect.id);
        p.sprite(&icon, (x0 + 7.0).floor(), (y0 + 7.0).floor(), 18.0, 18.0);

        let text_x = x0 + 32.0;
        let text_y = y0 + 7.0;
        if texture_w - 32.0 - 7.0 > 0.0 {
            p.text_str(&name, text_x.floor(), text_y.floor(), 0xFFFFFF, false);
            p.text_str(
                &duration,
                text_x.floor(),
                (text_y + 9.0).floor(),
                0x808080,
                false,
            );
        }

        y0 += y_step;
    }
}

fn clear_world_clicks(shared: &Arc<SharedMutex>) {
    let mut s = shared.lock().unwrap();
    s.session.attack_held = false;
    s.session.use_held = false;
    s.session.attack_clicked = false;
    s.session.use_clicked = false;
}

fn apply_book_out(out: book::BookOut, state: &mut GuiState, shared: &Arc<SharedMutex>) {
    match out {
        book::BookOut::Nav(next) => state.nav = Some(next),
        book::BookOut::Send(req) => {
            shared.lock().unwrap().session.edit_book = Some(req);
            state.nav = Some(Screen::None);
        }
    }
    clear_world_clicks(shared);
}

pub fn dim_background(p: &mut Painter, vw: f32, vh: f32) {
    p.gradient_v(0.0, 0.0, vw, vh, 0xC0101010, 0xD0101010);
}

#[cfg(test)]
mod tests {
    use super::{AttackIndicator, Snapshot};
    use crate::gui::painter::ITEM_SIZE;

    const SWORD_DELAY: f32 = 12.5;
    const HAND_DELAY: f32 = 5.0;

    fn snap(strength: f32, delay: f32, entity: bool) -> Snapshot {
        Snapshot {
            attack_strength: strength,
            attack_delay: delay,
            targeted_entity: entity,
            ..Default::default()
        }
    }

    #[test]
    fn attack_indicator_sits_under_the_crosshair() {
        let ind = AttackIndicator::new(427.0, 240.0, &snap(0.5, SWORD_DELAY, false));
        assert_eq!((ind.x, ind.y), (213.0 - 8.0, 120.0 - 7.0 + 16.0));
        assert_eq!(ind.x, ((427.0f32 - 15.0) / 2.0).floor() - 1.0);
    }

    #[test]
    fn attack_indicator_progress_truncates_like_vanilla() {
        for (strength, want) in [(0.0, 0.0), (0.05, 0.0), (0.5, 8.0), (0.99, 16.0)] {
            let ind = AttackIndicator::new(427.0, 240.0, &snap(strength, SWORD_DELAY, false));
            assert_eq!(ind.progress, Some(want), "strength {strength}");
            assert!(!ind.ready);
        }
    }

    #[test]
    fn attack_indicator_disappears_when_charged() {
        let ind = AttackIndicator::new(427.0, 240.0, &snap(1.0, SWORD_DELAY, false));
        assert_eq!(ind.progress, None);
        assert!(!ind.ready);
    }

    #[test]
    fn attack_indicator_ready_needs_target_charge_and_a_slow_weapon() {
        assert!(AttackIndicator::new(427.0, 240.0, &snap(1.0, SWORD_DELAY, true)).ready);
        assert!(!AttackIndicator::new(427.0, 240.0, &snap(1.0, HAND_DELAY, true)).ready);
        let mid = AttackIndicator::new(427.0, 240.0, &snap(0.5, SWORD_DELAY, true));
        assert!(!mid.ready && mid.progress == Some(8.0));
        assert!(!AttackIndicator::new(427.0, 240.0, &snap(1.0, SWORD_DELAY, false)).ready);
    }

    #[test]
    fn hotbar_geometry_matches_vanilla() {
        let (vw, vh) = (427.0_f32, 240.0_f32);
        let g = super::HotbarGeom::new(vw, vh);
        let center = 213.0_f32;
        assert_eq!(g.left, center - 91.0);
        assert_eq!(g.top, vh - 22.0);
        assert_eq!(g.left - 1.0 + 4.0 * 20.0, center - 91.0 - 1.0 + 4.0 * 20.0);
        assert_eq!(g.top - 1.0, vh - 23.0);
        for i in 0..9 {
            assert_eq!(g.item_x(i), center - 90.0 + i as f32 * 20.0 + 2.0);
        }
        assert_eq!(g.item_y(), vh - 16.0 - 3.0);
        assert_eq!(g.left - 29.0, center - 91.0 - 29.0);
        assert_eq!(g.left - 26.0, center - 91.0 - 26.0);
        assert_eq!(g.item_x(0) - (g.left + 1.0), 2.0);
        assert_eq!((g.left + 1.0 + 20.0) - (g.item_x(0) + ITEM_SIZE), 2.0);
        assert_eq!(g.item_y() - (g.top + 1.0), 2.0);
        assert_eq!((g.top + 21.0) - (g.item_y() + ITEM_SIZE), 2.0);
        assert_eq!(g.bottom - 39.0, vh - 39.0);
        assert_eq!(g.left + 182.0 - 9.0 + 9.0, center + 91.0);
    }

    use super::{AirBubble::*, air_bubble_states};

    #[test]
    fn full_air_underwater_shows_all_ten_full() {
        assert_eq!(air_bubble_states(300, 300, true), [Full; 10]);
    }

    #[test]
    fn full_air_on_land_has_no_bursting_bubble() {
        let states = air_bubble_states(300, 300, false);
        assert!(!states.contains(&Popping));
        assert_eq!(states, [Full; 10]);
    }

    #[test]
    fn no_air_shows_all_ten_empty() {
        assert_eq!(air_bubble_states(0, 300, true), [Empty; 10]);
    }

    #[test]
    fn draining_air_marks_the_next_bubble_popping() {
        let states = air_bubble_states(121, 300, true);
        assert_eq!(
            states,
            [
                Full, Full, Full, Full, Popping, Empty, Empty, Empty, Empty, Empty
            ]
        );
    }

    #[test]
    fn draining_air_never_pops_out_of_water() {
        let states = air_bubble_states(121, 300, false);
        assert!(!states.contains(&Popping));
    }

    #[test]
    fn empty_delay_leaves_a_gap_bubble_underwater() {
        assert_eq!(
            air_bubble_states(150, 300, true),
            [
                Full, Full, Full, Full, Full, Hidden, Empty, Empty, Empty, Empty
            ]
        );
    }

    #[test]
    fn empty_delay_only_applies_underwater() {
        assert_eq!(
            air_bubble_states(150, 300, false),
            [
                Full, Full, Full, Full, Full, Empty, Empty, Empty, Empty, Empty
            ]
        );
    }
}
