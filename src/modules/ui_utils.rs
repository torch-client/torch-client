use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

use azalea::Client;
use azalea::entity::inventory::Inventory;
use azalea_inventory::operations::ClickType;
use azalea_protocol::packets::Packet;
use azalea_protocol::packets::game::ServerboundGamePacket;
use azalea_protocol::packets::game::s_container_button_click::ServerboundContainerButtonClick;
use azalea_protocol::packets::game::s_container_click::{HashedStack, ServerboundContainerClick};
use azalea_protocol::packets::game::s_container_close::ServerboundContainerClose;
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use parking_lot::Mutex;

use super::chat_math::{GREEN, RED, show_local_colored};
use super::registry::{Id, ui_utils as setting};
use super::{Edge, Phase, store};
use crate::gui::keybinds::Bound;
use crate::gui::painter::Painter;
use crate::gui::render::GuiInput;
use crate::gui::widgets::{Button, TextBox, WIDGET_HEIGHT};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::lang::{Key, tr, trf};
use crate::session::{SESSION_EPOCH, SessionState, SharedMutex};

static SEND: AtomicBool = AtomicBool::new(true);
static DELAY: AtomicBool = AtomicBool::new(false);
static RESTORE: AtomicBool = AtomicBool::new(false);
static HIDDEN: AtomicBool = AtomicBool::new(false);

struct Stash {
    epoch: u64,
    packets: Vec<ServerboundGamePacket>,
}

static STASH: Mutex<Stash> = Mutex::new(Stash {
    epoch: 0,
    packets: Vec::new(),
});

struct Saved {
    epoch: u64,
    id: i32,
    menu: azalea_inventory::Menu,
    title: Option<azalea::FormattedText>,
    state_id: u32,
    crafter_disabled_slots: u16,
}

static SAVED: Mutex<Option<Saved>> = Mutex::new(None);

static EDGE: Edge = Edge::new(Id::UiUtils);

fn epoch() -> u64 {
    SESSION_EPOCH.load(Relaxed)
}

pub fn install() {
    let _ = azalea::packet::game::set_outgoing_game_filter(filter);
}

fn filter(packet: &ServerboundGamePacket) -> bool {
    let send = SEND.load(Relaxed);
    let delay = DELAY.load(Relaxed);
    if send && !delay {
        return true;
    }
    if !matches!(
        packet,
        ServerboundGamePacket::ContainerClick(_) | ServerboundGamePacket::ContainerButtonClick(_)
    ) {
        return true;
    }
    if !send {
        return false;
    }
    let mut stash = STASH.lock();
    let now = epoch();
    if stash.epoch != now {
        stash.epoch = now;
        stash.packets.clear();
    }
    stash.packets.push(packet.clone());
    false
}

fn send_bypass(bot: &Client, packet: ServerboundGamePacket) {
    bot.ecs
        .write()
        .trigger(azalea::packet::game::SendGamePacketBypassEvent {
            sent_by: bot.entity,
            packet,
        });
}

pub enum Action {
    CloseSilently,
    ForgetHidden,
    Desync,
    EndDelay,
    Save,
    Disconnect,
    DisconnectAndSend,
    WakeUp,
    Forge(Forged),
}

pub enum Forged {
    Click {
        sync_id: i32,
        revision: u32,
        slot: i16,
        button: u8,
        kind: ClickType,
        times: u32,
        delay: bool,
    },
    Button {
        sync_id: i32,
        button_id: u32,
        times: u32,
        delay: bool,
    },
}

#[derive(Default)]
pub struct SessionUi {
    pub actions: Vec<Action>,
    pub sync_id: i32,
    pub state_id: u32,
    pub title_json: Option<String>,
    title_for: i32,
    pub reopen: bool,
}

pub fn tick(bot: &Client, shared: &SharedMutex) {
    match EDGE.poll(store()) {
        Phase::Stopped => {
            if HIDDEN.swap(false, Relaxed) {
                forget_hidden(bot);
            }
            let (actions, horse) = take_actions(shared);
            for action in actions {
                apply(bot, shared, action, horse);
            }
            DELAY.store(false, Relaxed);
            flush(bot, shared);
            SEND.store(true, Relaxed);
            RESTORE.store(false, Relaxed);
            return;
        }
        Phase::Off => return,
        Phase::Started | Phase::Running => {}
    }
    let (actions, horse) = take_actions(shared);
    for action in actions {
        apply(bot, shared, action, horse);
    }
    if RESTORE.swap(false, Relaxed) {
        restore(bot, shared);
    }
    let horse = shared
        .lock()
        .unwrap()
        .session
        .horse
        .as_ref()
        .map(|h| (h.container_id, h.state_id));
    publish(bot, shared, horse);
}

fn take_actions(shared: &SharedMutex) -> (Vec<Action>, Option<(i32, u32)>) {
    let mut s = shared.lock().unwrap();
    let actions = std::mem::take(&mut s.session.ui_utils.actions);
    let horse = s
        .session
        .horse
        .as_ref()
        .map(|h| (h.container_id, h.state_id));
    (actions, horse)
}

fn say(shared: &SharedMutex, text: &str, color: u32) {
    show_local_colored(&mut shared.lock().unwrap().session, text, color);
}

fn apply(bot: &Client, shared: &SharedMutex, action: Action, horse: Option<(i32, u32)>) {
    match action {
        Action::CloseSilently => {
            if horse.is_some() {
                shared.lock().unwrap().session.horse = None;
            }
        }
        Action::ForgetHidden => forget_hidden(bot),
        Action::Desync => {
            let container_id = match horse {
                Some((id, _)) => id,
                None => bot.component::<Inventory>().map(|i| i.id).unwrap_or(0),
            };
            bot.write_packet(ServerboundContainerClose { container_id });
        }
        Action::EndDelay => {
            DELAY.store(false, Relaxed);
            flush(bot, shared);
        }
        Action::Save => save(bot, shared, horse.is_some()),
        Action::Disconnect => disconnect(shared),
        Action::DisconnectAndSend => {
            DELAY.store(false, Relaxed);
            flush(bot, shared);
            disconnect(shared);
        }
        Action::WakeUp => wake_up(bot),
        Action::Forge(forged) => forge(bot, forged),
    }
}

fn flush(bot: &Client, shared: &SharedMutex) {
    let packets = {
        let mut stash = STASH.lock();
        let packets = std::mem::take(&mut stash.packets);
        if stash.epoch != epoch() {
            Vec::new()
        } else {
            packets
        }
    };
    if packets.is_empty() {
        return;
    }
    let n = packets.len();
    if SEND.load(Relaxed) {
        for packet in packets {
            send_bypass(bot, packet);
        }
        say(shared, &trf!("modmsg.ui_utils.sent", n), GREEN);
    } else {
        say(shared, &trf!("modmsg.ui_utils.dropped", n), RED);
    }
}

fn disconnect(shared: &SharedMutex) {
    let mut s = shared.lock().unwrap();
    let status = tr!("modmsg.ui_utils.status_disconnected").to_string();
    let reason = crate::text::parse_formatted(tr!("modmsg.ui_utils.disconnect_reason"));
    s.end_session(status, Some(reason));
    s.disconnect_requested = true;
}

fn save(bot: &Client, shared: &SharedMutex, horse: bool) {
    if horse {
        say(shared, tr!("modmsg.ui_utils.mount_no_save"), RED);
        return;
    }
    let saved = bot.component::<Inventory>().ok().and_then(|inv| {
        Some(Saved {
            epoch: epoch(),
            id: inv.id,
            menu: inv.container_menu.clone()?,
            title: inv.container_menu_title.clone(),
            state_id: inv.container_state_id,
            crafter_disabled_slots: inv.crafter_disabled_slots,
        })
    });
    match saved {
        Some(saved) => {
            *SAVED.lock() = Some(saved);
            say(shared, tr!("modmsg.ui_utils.saved"), GREEN);
        }
        None => say(shared, tr!("modmsg.ui_utils.nothing_to_save"), RED),
    }
}

fn forget_hidden(bot: &Client) {
    bot.ecs
        .write()
        .trigger(azalea::inventory::ClientsideCloseContainerEvent { entity: bot.entity });
}

fn restore(bot: &Client, shared: &SharedMutex) {
    let saved = {
        let guard = SAVED.lock();
        guard.as_ref().filter(|s| s.epoch == epoch()).map(|s| {
            (
                s.id,
                s.menu.clone(),
                s.title.clone(),
                s.state_id,
                s.crafter_disabled_slots,
            )
        })
    };
    let Some((id, menu, title, state_id, crafter)) = saved else {
        say(shared, tr!("modmsg.ui_utils.no_saved"), RED);
        return;
    };
    let live = bot
        .component::<Inventory>()
        .is_ok_and(|inv| inv.id == id && inv.container_menu.is_some());
    if live {
        shared.lock().unwrap().session.ui_utils.reopen = true;
        return;
    }
    let mut ecs = bot.ecs.write();
    let Some(mut inv) = ecs.get_mut::<Inventory>(bot.entity) else {
        return;
    };
    inv.sync_player_slots(true);
    inv.id = id;
    inv.container_menu = Some(menu);
    inv.container_menu_title = title;
    inv.container_state_id = state_id;
    inv.crafter_disabled_slots = crafter;
    inv.sync_player_slots(false);
}

fn forge(bot: &Client, forged: Forged) {
    let (packet, times, delay): (ServerboundGamePacket, u32, bool) = match forged {
        Forged::Click {
            sync_id,
            revision,
            slot,
            button,
            kind,
            times,
            delay,
        } => (
            ServerboundContainerClick {
                container_id: sync_id,
                state_id: revision,
                slot_num: slot,
                button_num: button,
                click_type: kind,
                changed_slots: Default::default(),
                carried_item: HashedStack(None),
            }
            .into_variant(),
            times,
            delay,
        ),
        Forged::Button {
            sync_id,
            button_id,
            times,
            delay,
        } => (
            ServerboundContainerButtonClick {
                container_id: sync_id,
                button_id,
            }
            .into_variant(),
            times,
            delay,
        ),
    };
    for _ in 0..times {
        if delay {
            bot.write_packet(packet.clone());
        } else {
            send_bypass(bot, packet.clone());
        }
    }
}

fn wake_up(bot: &Client) {
    use crate::util::dismount::{Step, find_stand_up_position};
    use azalea::entity::metadata::SleepingPos;
    use azalea::entity::{LookDirection, Physics, Pose, Position};

    let Some(bed) = bot.component::<SleepingPos>().ok().and_then(|c| c.0) else {
        return;
    };
    let yaw = bot
        .component::<LookDirection>()
        .map(|l| l.y_rot())
        .unwrap_or(0.0);
    let border = bot
        .component::<azalea::world::WorldBorder>()
        .map(|b| *b)
        .unwrap_or_default();
    let stand = bot.world().ok().and_then(|world| {
        let guard = world.read();
        let state = guard.get_block_state(bed)?;
        let block: Box<dyn azalea::block::BlockTrait> = state.into();
        if !block.id().ends_with("_bed") {
            return None;
        }
        let forward = Step::from_name(block.get_property("facing")?)?;
        Some(
            find_stand_up_position(&guard, &border, bed, forward, yaw).unwrap_or(azalea::Vec3 {
                x: bed.x as f64 + 0.5,
                y: bed.y as f64 + 1.0 + 0.1,
                z: bed.z as f64 + 0.5,
            }),
        )
    });
    let mut ecs = bot.ecs.write();
    let entity = bot.entity;
    if let Some(stand) = stand {
        let (dx, dz) = (bed.x as f64 + 0.5 - stand.x, bed.z as f64 + 0.5 - stand.z);
        let dy = bed.y as f64 - stand.y;
        let len = (dx * dx + dy * dy + dz * dz).sqrt();
        let (nx, nz) = if len < 1.0e-5 {
            (0.0, 0.0)
        } else {
            (dx / len, dz / len)
        };
        let yaw =
            crate::util::mth::wrap_degrees_f64(nz.atan2(nx) * 57.295_776_367_187_5 - 90.0) as f32;
        if let Some(mut pos) = ecs.get_mut::<Position>(entity) {
            *pos = Position::new(stand);
        }
        if let Some(mut physics) = ecs.get_mut::<Physics>(entity) {
            physics.set_old_pos(Position::new(stand));
        }
        if let Some(mut look) = ecs.get_mut::<LookDirection>(entity) {
            *look = LookDirection::new(yaw, 0.0);
        }
        ecs.entity_mut(entity)
            .insert(azalea::movement::ServerSetLook);
    }
    if let Some(mut pose) = ecs.get_mut::<Pose>(entity) {
        *pose = Pose::Standing;
    }
    if let Some(mut sleeping) = ecs.get_mut::<SleepingPos>(entity) {
        sleeping.0 = None;
    }
}

fn publish(bot: &Client, shared: &SharedMutex, horse: Option<(i32, u32)>) {
    let title_for = shared.lock().unwrap().session.ui_utils.title_for;
    let (sync_id, state_id, title) = match horse {
        Some((id, state_id)) => (id, state_id, None),
        None => {
            let Ok(inv) = bot.component::<Inventory>() else {
                return;
            };
            let title = (inv.id != title_for).then(|| {
                inv.container_menu_title
                    .as_ref()
                    .and_then(|t| serde_json::to_string(t).ok())
            });
            (inv.id, inv.menu_state_id(), title)
        }
    };
    let mut s = shared.lock().unwrap();
    let ui = &mut s.session.ui_utils;
    ui.sync_id = sync_id;
    ui.state_id = state_id;
    if let Some(title) = title {
        ui.title_json = title;
        ui.title_for = sync_id;
    } else if horse.is_some() {
        ui.title_json = None;
        ui.title_for = sync_id;
    }
}

pub fn poll_restore(keys: &ButtonInput<KeyCode>, buttons: &ButtonInput<MouseButton>) {
    if !store().enabled(Id::UiUtils) {
        return;
    }
    let pressed = match store().key(setting::RESTORE_KEY) {
        Bound::Unbound => false,
        Bound::Key(code) => keys.just_pressed(code),
        Bound::Mouse(button) => buttons.just_pressed(button),
    };
    if pressed {
        RESTORE.store(true, Relaxed);
    }
}

pub fn bypass_pack(required: bool) -> bool {
    store().enabled(Id::UiUtils)
        && store().flag(setting::BYPASS_PACK)
        && (required || store().flag(setting::FORCE_DENY))
}

#[derive(Clone, Copy, PartialEq)]
enum Set {
    Full,
    Sign,
    Sleep,
}

fn set_for(screen: Screen) -> Option<Set> {
    if screen.is_modal() || matches!(screen, Screen::BookView | Screen::BookEdit) {
        Some(Set::Full)
    } else if screen == Screen::SignEdit {
        Some(Set::Sign)
    } else if screen == Screen::Sleep {
        Some(Set::Sleep)
    } else {
        None
    }
}

const COL_X: f32 = 5.0;
const COL_W: f32 = 115.0;
const ROW: f32 = 30.0;
const WIDE_W: f32 = 160.0;
const CHAT_Y: f32 = 245.0;
const READOUT_X: f32 = 200.0;

fn row_y(i: usize) -> f32 {
    5.0 + ROW * i as f32
}

fn rects(set: Set) -> &'static [(f32, f32, f32, f32)] {
    const FULL: [(f32, f32, f32, f32); 9] = [
        (COL_X, 5.0, COL_W, WIDGET_HEIGHT),
        (COL_X, 35.0, COL_W, WIDGET_HEIGHT),
        (COL_X, 65.0, COL_W, WIDGET_HEIGHT),
        (COL_X, 95.0, COL_W, WIDGET_HEIGHT),
        (COL_X, 125.0, COL_W, WIDGET_HEIGHT),
        (COL_X, 155.0, WIDE_W, WIDGET_HEIGHT),
        (COL_X, 185.0, COL_W, WIDGET_HEIGHT),
        (COL_X, 215.0, COL_W, WIDGET_HEIGHT),
        (COL_X, CHAT_Y, WIDE_W, WIDGET_HEIGHT),
    ];
    const SIGN: [(f32, f32, f32, f32); 2] = [
        (COL_X, 5.0, COL_W, WIDGET_HEIGHT),
        (COL_X, 35.0, COL_W, WIDGET_HEIGHT),
    ];
    const SLEEP: [(f32, f32, f32, f32); 1] = [(COL_X, 5.0, COL_W, WIDGET_HEIGHT)];
    match set {
        Set::Full => &FULL,
        Set::Sign => &SIGN,
        Set::Sleep => &SLEEP,
    }
}

#[derive(Default)]
pub struct Overlay {
    chat: Option<TextBox>,
    chat_lang: u32,
    form: Option<Form>,
    skip_close_packet: bool,
    skip_sign_update: bool,
    press_on_overlay: bool,
}

impl Overlay {
    pub fn take_skips(&mut self) -> (bool, bool) {
        (
            std::mem::take(&mut self.skip_close_packet),
            std::mem::take(&mut self.skip_sign_update),
        )
    }

    pub fn take_escape(&mut self) -> bool {
        self.form.take().is_some()
    }

    pub fn holds_keyboard(&self) -> bool {
        self.form.is_some() || self.chat.as_ref().is_some_and(|c| c.focused)
    }
}

pub fn mask(state: &mut GuiState, input: &GuiInput) -> Option<GuiInput> {
    if !store().enabled(Id::UiUtils) {
        state.ui_utils.press_on_overlay = false;
        return None;
    }
    let Some(set) = set_for(state.screen) else {
        state.ui_utils.press_on_overlay = false;
        return None;
    };
    let form = state.ui_utils.form.is_some();
    let over = input.mouse.is_some_and(|m| {
        rects(set)
            .iter()
            .any(|&(x, y, w, h)| m.x >= x && m.x < x + w && m.y >= y && m.y < y + h)
    });
    let keys = state.ui_utils.holds_keyboard();
    if over && (input.left_click || input.right_click) {
        state.ui_utils.press_on_overlay = true;
    }
    let held = state.ui_utils.press_on_overlay;
    if input.left_release || input.right_release {
        state.ui_utils.press_on_overlay = false;
    }
    if !form && !over && !keys && !held {
        return None;
    }
    let mut masked = input.clone();
    if held {
        masked.left_release = false;
        masked.right_release = false;
    }
    if form || over {
        masked.left_click = false;
        masked.right_click = false;
        masked.middle_click = false;
        masked.double_click = false;
        if form || !state.slots.drag.active() {
            masked.left_release = false;
            masked.right_release = false;
        }
        masked.scroll = 0.0;
        masked.scroll_x = 0.0;
    }
    if form {
        masked.mouse = None;
    }
    if form || keys {
        strip_keys(&mut masked);
    }
    Some(masked)
}

fn strip_keys(i: &mut GuiInput) {
    i.typed.clear();
    i.edits.clear();
    i.backspace = false;
    i.delete = false;
    i.left_arrow = false;
    i.right_arrow = false;
    i.up_arrow = false;
    i.down_arrow = false;
    i.home = false;
    i.end = false;
    i.page_up = false;
    i.page_down = false;
    i.enter = false;
    i.select = false;
    i.tab = false;
    i.hotbar_keys = [false; 9];
    i.drop_key = false;
    i.swap_key = false;
    i.copy = false;
    i.paste = false;
    i.cut = false;
    i.select_all = false;
    i.undo = false;
    i.redo = false;
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx, shared: &SharedMutex) {
    let set = if store().enabled(Id::UiUtils) {
        set_for(state.screen)
    } else {
        None
    };
    let Some(set) = set else {
        state.ui_utils.form = None;
        if let Some(chat) = &mut state.ui_utils.chat {
            chat.focused = false;
        }
        return;
    };
    if state.screen.is_container() {
        HIDDEN.store(false, Relaxed);
    } else if matches!(state.screen, Screen::Inventory | Screen::Creative)
        && HIDDEN.swap(false, Relaxed)
    {
        push(shared, Action::ForgetHidden);
    }
    match set {
        Set::Full => draw_full(p, state, ctx, shared),
        Set::Sign => {
            let close = Button::new(
                COL_X,
                row_y(0),
                COL_W,
                WIDGET_HEIGHT,
                tr!("modmsg.ui_utils.close_without_packet"),
            );
            if close.draw(p, ctx) {
                state.ui_utils.skip_sign_update = true;
                state.nav = Some(Screen::None);
            }
            let disconnect = Button::new(
                COL_X,
                row_y(1),
                COL_W,
                WIDGET_HEIGHT,
                tr!("modmsg.ui_utils.disconnect"),
            );
            if disconnect.draw(p, ctx) {
                push(shared, Action::Disconnect);
            }
        }
        Set::Sleep => {
            let wake = Button::new(
                COL_X,
                row_y(0),
                COL_W,
                WIDGET_HEIGHT,
                tr!("modmsg.ui_utils.wake_up"),
            );
            if wake.draw(p, ctx) {
                push(shared, Action::WakeUp);
            }
        }
    }
}

fn push(shared: &SharedMutex, action: Action) {
    shared.lock().unwrap().session.ui_utils.actions.push(action);
}

fn draw_full(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx, shared: &SharedMutex) {
    let inert = GuiInput::default();
    let form_open = state.ui_utils.form.is_some();
    let bctx = if form_open {
        ctx.with_input(&inert)
    } else {
        ctx.with_input(ctx.input)
    };
    let screen = state.screen;

    let (sync_id, state_id, in_world) = {
        let s = shared.lock().unwrap();
        (
            s.session.ui_utils.sync_id,
            s.session.ui_utils.state_id,
            s.in_world,
        )
    };

    let close = Button::new(
        COL_X,
        row_y(0),
        COL_W,
        WIDGET_HEIGHT,
        tr!("modmsg.ui_utils.close_without_packet"),
    );
    if close.draw(p, &bctx) {
        if screen.is_modal() {
            state.ui_utils.skip_close_packet = true;
        }
        if screen.is_container() {
            HIDDEN.store(true, Relaxed);
            push(shared, Action::CloseSilently);
        }
        state.nav = Some(Screen::None);
    }
    if Button::new(
        COL_X,
        row_y(1),
        COL_W,
        WIDGET_HEIGHT,
        tr!("modmsg.ui_utils.desync"),
    )
    .draw(p, &bctx)
    {
        push(shared, Action::Desync);
    }
    let send = SEND.load(Relaxed);
    let label = if send {
        tr!("modmsg.ui_utils.send_packets_on")
    } else {
        tr!("modmsg.ui_utils.send_packets_off")
    };
    if Button::new(COL_X, row_y(2), COL_W, WIDGET_HEIGHT, label).draw(p, &bctx) {
        SEND.store(!send, Relaxed);
    }
    let delay = DELAY.load(Relaxed);
    let label = if delay {
        tr!("modmsg.ui_utils.delay_packets_on")
    } else {
        tr!("modmsg.ui_utils.delay_packets_off")
    };
    if Button::new(COL_X, row_y(3), COL_W, WIDGET_HEIGHT, label).draw(p, &bctx) {
        if delay {
            push(shared, Action::EndDelay);
        } else {
            DELAY.store(true, Relaxed);
        }
    }
    if Button::new(
        COL_X,
        row_y(4),
        COL_W,
        WIDGET_HEIGHT,
        tr!("modmsg.ui_utils.save_gui"),
    )
    .draw(p, &bctx)
    {
        push(shared, Action::Save);
    }
    let label = tr!("modmsg.ui_utils.disconnect_and_send");
    if Button::new(COL_X, row_y(5), WIDE_W, WIDGET_HEIGHT, label).draw(p, &bctx) {
        push(shared, Action::DisconnectAndSend);
    }
    if Button::new(
        COL_X,
        row_y(6),
        COL_W,
        WIDGET_HEIGHT,
        tr!("modmsg.ui_utils.fabricate"),
    )
    .draw(p, &bctx)
    {
        state.ui_utils.form = Some(Form::choose());
    }
    if Button::new(
        COL_X,
        row_y(7),
        COL_W,
        WIDGET_HEIGHT,
        tr!("modmsg.ui_utils.copy_title"),
    )
    .draw(p, &bctx)
    {
        let json = title_json(screen, &shared.lock().unwrap().session);
        crate::platform::clipboard::set(&json);
    }

    if screen.is_modal() {
        p.text_plain(
            &trf!("modmsg.ui_utils.readout.sync_id", sync_id),
            READOUT_X,
            5.0,
            0xFFFFFF,
            false,
        );
        p.text_plain(
            &trf!("modmsg.ui_utils.readout.revision", state_id),
            READOUT_X,
            35.0,
            0xFFFFFF,
            false,
        );
    }

    draw_chat(p, &mut state.ui_utils, &bctx, shared);

    if let Some(form) = &mut state.ui_utils.form {
        if !form.draw(p, ctx, shared, in_world, sync_id, state_id) {
            state.ui_utils.form = None;
        }
    }
}

fn title_json(screen: Screen, session: &SessionState) -> String {
    match screen {
        Screen::Inventory => r#"{"translate":"container.crafting"}"#.to_string(),
        Screen::BookView => r#"{"translate":"book.view.title"}"#.to_string(),
        Screen::BookEdit => r#"{"translate":"book.edit.title"}"#.to_string(),
        _ if screen.is_container() => session.ui_utils.title_json.clone().unwrap_or_else(|| {
            let plain: String = session
                .container_title
                .iter()
                .map(|s| s.text.as_str())
                .collect();
            serde_json::to_string(&plain).unwrap_or_default()
        }),
        _ => r#""""#.to_string(),
    }
}

fn draw_chat(p: &mut Painter, ui: &mut Overlay, ctx: &ScreenCtx, shared: &SharedMutex) {
    let lang = crate::lang::epoch();
    if ui.chat_lang != lang {
        if let Some(old) = ui.chat.take() {
            let mut chat = chat_box();
            chat.set_text(&old.text);
            ui.chat = Some(chat);
        }
        ui.chat_lang = lang;
    }
    let chat = ui.chat.get_or_insert_with(chat_box);
    if ctx.input.left_click && !ctx.hovering(COL_X, CHAT_Y, WIDE_W, WIDGET_HEIGHT) && chat.focused {
        chat.focused = false;
        crate::gui::focus::clear();
    }
    if chat.focused {
        crate::platform::keyboard::enter_is_text();
    }
    let enter = chat.focused && ctx.input.enter;
    chat.update(p, ctx, COL_X, CHAT_Y, WIDE_W, WIDGET_HEIGHT);
    if enter && !chat.text.is_empty() {
        let line = std::mem::take(&mut chat.text);
        chat.clear();
        crate::renderer::input::send_typed(shared, line);
    }
}

fn chat_box() -> TextBox {
    TextBox::bordered(256, tr!("modmsg.ui_utils.chat_hint"))
}

const CLICK_KINDS: [(&str, ClickType); 7] = [
    ("PICKUP", ClickType::Pickup),
    ("QUICK_MOVE", ClickType::QuickMove),
    ("SWAP", ClickType::Swap),
    ("CLONE", ClickType::Clone),
    ("THROW", ClickType::Throw),
    ("QUICK_CRAFT", ClickType::QuickCraft),
    ("PICKUP_ALL", ClickType::PickupAll),
];

#[derive(Clone, Copy, PartialEq)]
enum FormKind {
    Choose,
    Click,
    Button,
}

struct Form {
    kind: FormKind,
    fields: Vec<(Key, TextBox)>,
    click_kind: usize,
    delay: bool,
    status: Option<(Key, u32, f32)>,
}

const STATUS_SECS: f32 = 1.5;
const FORM_W: f32 = 250.0;
const LABEL_W: f32 = 100.0;
const FIELD_W: f32 = 100.0;
const LINE: f32 = 24.0;

fn field(text: &str) -> TextBox {
    let mut b = TextBox::bordered(11, "");
    b.set_text(text);
    b
}

impl Form {
    fn choose() -> Form {
        Form {
            kind: FormKind::Choose,
            fields: Vec::new(),
            click_kind: 0,
            delay: false,
            status: None,
        }
    }

    fn open(kind: FormKind, sync_id: i32, state_id: u32) -> Form {
        let fields = match kind {
            FormKind::Click => vec![
                (
                    const { Key::of("modmsg.ui_utils.field.sync_id") },
                    field(&sync_id.to_string()),
                ),
                (
                    const { Key::of("modmsg.ui_utils.field.revision") },
                    field(&state_id.to_string()),
                ),
                (const { Key::of("modmsg.ui_utils.field.slot") }, field("")),
                (const { Key::of("modmsg.ui_utils.field.button") }, field("")),
                (const { Key::of("modmsg.ui_utils.field.times") }, field("1")),
            ],
            FormKind::Button => vec![
                (
                    const { Key::of("modmsg.ui_utils.field.sync_id") },
                    field(&sync_id.to_string()),
                ),
                (
                    const { Key::of("modmsg.ui_utils.field.button_id") },
                    field(""),
                ),
                (const { Key::of("modmsg.ui_utils.field.times") }, field("1")),
            ],
            FormKind::Choose => Vec::new(),
        };
        Form {
            kind,
            fields,
            click_kind: 0,
            delay: false,
            status: None,
        }
    }

    fn value(&self, i: usize) -> Option<i32> {
        self.fields.get(i)?.1.text.trim().parse().ok()
    }

    fn draw(
        &mut self,
        p: &mut Painter,
        ctx: &ScreenCtx,
        shared: &SharedMutex,
        in_world: bool,
        sync_id: i32,
        state_id: u32,
    ) -> bool {
        let rows = match self.kind {
            FormKind::Choose => 1,
            FormKind::Click => self.fields.len() + 1,
            FormKind::Button => self.fields.len(),
        };
        let h = 30.0 + rows as f32 * LINE + 30.0 + 16.0;
        let x = ((ctx.vw - FORM_W) / 2.0).floor();
        let y = ((ctx.vh - h) / 2.0).floor();
        p.fill(x, y, FORM_W, h, 0xE010_1010);
        p.outline(x, y, FORM_W, h, 0xFFA0_A0A0);
        let title = match self.kind {
            FormKind::Choose => tr!("modmsg.ui_utils.form.choose"),
            FormKind::Click => tr!("modmsg.ui_utils.form.click"),
            FormKind::Button => tr!("modmsg.ui_utils.form.button"),
        };
        p.text_centered(title, x + FORM_W / 2.0, y + 8.0, 0xFFFFFF, true);

        let label = tr!("modmsg.ui_utils.form.close");
        let close = Button::new(
            x + FORM_W - 60.0,
            y + h - 26.0 - 16.0,
            50.0,
            WIDGET_HEIGHT,
            label,
        );
        if close.draw(p, ctx) {
            return false;
        }

        if self.kind == FormKind::Choose {
            let click = Button::new(
                x + 15.0,
                y + 30.0,
                105.0,
                WIDGET_HEIGHT,
                tr!("modmsg.ui_utils.form.click_slot"),
            );
            let button = Button::new(
                x + 130.0,
                y + 30.0,
                105.0,
                WIDGET_HEIGHT,
                tr!("modmsg.ui_utils.form.button_click"),
            );
            if click.draw(p, ctx) {
                *self = Form::open(FormKind::Click, sync_id, state_id);
            } else if button.draw(p, ctx) {
                *self = Form::open(FormKind::Button, sync_id, state_id);
            }
            return true;
        }

        if ctx.input.left_click {
            for (_, b) in &mut self.fields {
                b.focused = false;
            }
        }
        let mut row_at = y + 30.0;
        let action_row = (self.kind == FormKind::Click).then_some(4);
        let mut fields = self.fields.iter_mut().enumerate();
        for row in 0..rows {
            if action_row == Some(row) {
                p.text_plain(
                    tr!("modmsg.ui_utils.form.action"),
                    x + 10.0,
                    row_at + 6.0,
                    0xFFFFFF,
                    false,
                );
                let name = CLICK_KINDS[self.click_kind].0;
                if Button::new(x + 10.0 + LABEL_W, row_at, FIELD_W, WIDGET_HEIGHT, name)
                    .draw(p, ctx)
                {
                    self.click_kind = (self.click_kind + 1) % CLICK_KINDS.len();
                }
            } else if let Some((_, (label, b))) = fields.next() {
                p.text_plain(label.get(), x + 10.0, row_at + 6.0, 0xFFFFFF, false);
                b.update(p, ctx, x + 10.0 + LABEL_W, row_at, FIELD_W, WIDGET_HEIGHT);
            }
            row_at += LINE;
        }

        let buttons_y = y + h - 26.0 - 16.0;
        let delay_label = if self.delay {
            tr!("modmsg.ui_utils.form.delay_on")
        } else {
            tr!("modmsg.ui_utils.form.delay_off")
        };
        if Button::new(x + 70.0, buttons_y, 75.0, WIDGET_HEIGHT, delay_label).draw(p, ctx) {
            self.delay = !self.delay;
        }
        if Button::new(
            x + 10.0,
            buttons_y,
            55.0,
            WIDGET_HEIGHT,
            tr!("modmsg.ui_utils.form.send"),
        )
        .draw(p, ctx)
        {
            self.status = Some(self.send(shared, in_world, ctx.input.time));
        }
        if let Some((text, color, until)) = self.status {
            if ctx.input.time >= until {
                self.status = None;
            } else {
                p.text_plain(text.get(), x + 10.0, y + h - 14.0, color, false);
            }
        }
        true
    }

    fn send(&self, shared: &SharedMutex, in_world: bool, now: f32) -> (Key, u32, f32) {
        let until = now + STATUS_SECS;
        const BAD: u32 = 0xAA_0000;
        const GOOD: u32 = 0x00AA_00;
        let times_at = self.fields.len() - 1;
        let times = self.value(times_at).filter(|t| (0..=10_000).contains(t));
        let forged = match self.kind {
            FormKind::Click => (|| {
                let slot = i16::try_from(self.value(2)?).ok()?;
                let button = self.value(3).filter(|b| (-128..=255).contains(b))? as u8;
                Some(Forged::Click {
                    sync_id: self.value(0)?,
                    revision: self.value(1)? as u32,
                    slot,
                    button,
                    kind: CLICK_KINDS[self.click_kind].1,
                    times: times? as u32,
                    delay: self.delay,
                })
            })(),
            FormKind::Button => (|| {
                Some(Forged::Button {
                    sync_id: self.value(0)?,
                    button_id: self.value(1)? as u32,
                    times: times? as u32,
                    delay: self.delay,
                })
            })(),
            FormKind::Choose => None,
        };
        let Some(forged) = forged else {
            return (
                const { Key::of("modmsg.ui_utils.form.invalid") },
                BAD,
                until,
            );
        };
        if !in_world {
            return (
                const { Key::of("modmsg.ui_utils.form.not_connected") },
                BAD,
                until,
            );
        }
        push(shared, Action::Forge(forged));
        (const { Key::of("modmsg.ui_utils.form.sent") }, GOOD, until)
    }
}
