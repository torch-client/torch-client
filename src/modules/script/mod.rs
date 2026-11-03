use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Mutex, PoisonError};

use azalea::Client;

use super::registry::Mode;
use super::store::ScriptPart;
use super::{Store, registry, store};
use crate::session::SharedMutex;
use crate::text::Span;

mod api;
mod author;
mod blocks;
mod capture;
mod chunk;
mod fs;
mod game;
pub mod ghost;
mod image;
mod install;
mod items;
mod json;
mod kinds;
mod load;
mod nbt;
mod packets;
mod perm;
mod profile;
mod render;
mod run;
mod state;

pub use author::{
    SOURCE_MAX, SyntaxError, TEMPLATE, mod_id, open as open_source, save as save_source, syntax,
};
pub use chunk::{arrived as chunk_arrived, changed as block_changed};
pub use game::{
    CAMERA as CANVAS_CAMERA, Camera, FileAsk, OverPrompt, UiButton, UiForm, UiList, UiRow,
    draw_canvas, draw_canvas_overlay, draw_hud, draw_panel,
};
pub use install::uninstall;
pub use install::{
    Candidate, Verdict, check as check_drop, clear_note as clear_install_note, install,
    note as install_note, set_note as set_install_note,
};
pub use perm::notice as permission_notice;

pub const EXPLORE_URL: &str = "https://torchclient.com/mods";

pub const MOD_DATA: &str = "mod-data";

static WANTS: AtomicU32 = AtomicU32::new(0);

const RUNNING: u32 = 1 << 0;
const RESCAN: u32 = 1 << 1;
const CHAT: u32 = 1 << 2;
const SETTINGS: u32 = 1 << 3;
const OUTBOX: u32 = 1 << 4;
const QUEUED: u32 = 1 << 5;
const COMMAND: u32 = 1 << 6;

const TICK_SITE: u32 = RUNNING | RESCAN | OUTBOX | QUEUED | COMMAND;

static COMMANDS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

const COMMAND_QUEUE: usize = 16;

static RUNTIME: Mutex<Option<run::Runtime>> = Mutex::new(None);

static LIVE: AtomicBool = AtomicBool::new(false);

static BUSY: AtomicBool = AtomicBool::new(false);

static LEAVE_PENDING: AtomicBool = AtomicBool::new(false);

static INFO: Mutex<Vec<ModInfo>> = Mutex::new(Vec::new());

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    Folder,
    Zip,
    File,
}

impl Shape {
    pub fn name(self) -> &'static str {
        match self {
            Shape::Folder => "folder",
            Shape::Zip => "zip",
            Shape::File => "single file",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ModInfo {
    pub id: String,
    pub file: String,
    pub shape: Shape,
    pub name: Option<String>,
    pub version: Option<String>,
    pub license: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub source: Option<String>,
    pub modules: Vec<ModuleInfo>,
    pub error: Option<String>,
    pub last_error: Option<String>,
    pub disabled: bool,
    pub perms: Vec<Grant>,
}

#[derive(Clone, Debug)]
pub struct Grant {
    pub perm: &'static str,
    pub granted: bool,
    pub allow: &'static str,
    pub block: &'static str,
}

impl Grant {
    fn of(perm: usize, id: &str) -> Grant {
        let p = &perm::PERMS[perm];
        Grant {
            perm: p.name,
            granted: perm::granted(perm, id),
            allow: p.allow,
            block: p.block,
        }
    }
}

impl ModInfo {
    fn of(id: String, file: String, shape: Shape) -> ModInfo {
        ModInfo {
            id,
            file,
            shape,
            name: None,
            version: None,
            license: None,
            description: None,
            author: None,
            source: None,
            modules: Vec::new(),
            error: None,
            last_error: None,
            disabled: false,
            perms: Vec::new(),
        }
    }
}

pub fn set_granted(id: &str, perm: &str, on: bool) {
    let Some(ix) = perm::index(perm) else {
        return;
    };
    perm::set_granted(ix, id, on);
    let now = perm::granted(ix, id);
    let mut info = INFO.lock().unwrap_or_else(PoisonError::into_inner);
    for g in info
        .iter_mut()
        .filter(|r| r.id == id)
        .flat_map(|r| r.perms.iter_mut())
        .filter(|g| g.perm == perm)
    {
        g.granted = now;
    }
}

fn lists_permission(id: &str, perm: &str) -> bool {
    INFO.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .any(|r| r.id == id && r.perms.iter().any(|g| g.perm == perm))
}

fn disabled() -> std::collections::BTreeSet<String> {
    perm::id_set(&crate::platform::storage::MODS_DISABLED)
}

fn store_disabled(ids: &std::collections::BTreeSet<String>) {
    perm::store_id_set(&crate::platform::storage::MODS_DISABLED, ids);
}

pub fn set_mod_enabled(id: String, on: bool) {
    on_worker(move || {
        let mut ids = disabled();
        let changed = if on {
            ids.remove(&id)
        } else {
            ids.insert(id.clone())
        };
        if changed {
            store_disabled(&ids);
            crate::log_info!("mods", "{} {id}", if on { "enabled" } else { "disabled" });
        }
        let world = in_world();
        load(world, world);
    });
}

fn enable_ids<'a>(installed: impl IntoIterator<Item = &'a str>) {
    let mut ids = disabled();
    let before = ids.len();
    for id in installed {
        ids.remove(id);
    }
    if ids.len() != before {
        store_disabled(&ids);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ModuleInfo {
    pub name: &'static str,
    pub mode: Mode,
}

pub fn folder() -> PathBuf {
    crate::platform::packfs::root().join("mods")
}

pub fn init() {
    #[cfg(target_arch = "wasm32")]
    bevy::reflect::TypeRegistry::empty().register_derived_types();
    load(false, true);
}

fn in_world() -> bool {
    crate::SHARED
        .get()
        .is_some_and(|s| s.lock().unwrap().in_world)
}

pub fn busy() -> bool {
    BUSY.load(Relaxed)
}

fn on_worker(f: impl FnOnce() + Send + 'static) {
    struct Done;
    impl Drop for Done {
        fn drop(&mut self) {
            BUSY.store(false, Relaxed);
        }
    }
    if BUSY.swap(true, Relaxed) {
        return;
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _done = Done;
        f();
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let spawned = std::thread::Builder::new()
            .name("lua-mods".to_string())
            .spawn(move || {
                let _done = Done;
                f();
            });
        if let Err(e) = spawned {
            BUSY.store(false, Relaxed);
            crate::log_error!("mods", "could not start the mods worker thread: {e}");
        }
    }
}

pub fn reload() {
    on_worker(|| {
        let world = in_world();
        load(world, world);
    });
}

pub fn reset(id: String) {
    on_worker(move || {
        let world = in_world();
        let mut rt = RUNTIME.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(rt) = rt.as_mut() {
            rt.reset(&id, world);
        }
        state::delete(&id);
        if let Some(row) = INFO
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter_mut()
            .find(|m| m.id == id)
        {
            row.last_error = None;
        }
    });
}

pub fn leave() {
    match runtime_briefly() {
        Some(mut rt) => {
            if let Some(rt) = rt.as_mut() {
                rt.leave();
            }
        }
        None => {
            LEAVE_PENDING.store(true, Relaxed);
            crate::log_warn!(
                "mods",
                "a mod is still running on the bot thread; ondisable waits for it"
            );
        }
    }
    api::drop_sends();
    game::clear();
    game::QUERIES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    game::CLAIMS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    render::clear();
    ghost::clear();
    image::clear();
    chunk::clear();
    nbt::forget();
    game::TYPED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    game::TYPED_ANY.store(false, Relaxed);
    COMMANDS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    WANTS.fetch_and(!COMMAND, Relaxed);
    game::clear_hud();
    game::clear_ui();
    game::MOVES.store(0, Relaxed);
    game::RELEASES.store(0, Relaxed);
    profile::forget_requests();
}

pub fn with_mods<R>(f: impl FnOnce(&[ModInfo]) -> R) -> R {
    f(&INFO.lock().unwrap_or_else(PoisonError::into_inner))
}

fn note_error(id: &str, error: String) {
    if let Some(row) = INFO
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter_mut()
        .find(|m| m.id == id)
    {
        row.last_error = Some(error);
    }
}

fn load(in_world: bool, chat: bool) {
    let loaded = load::all(&folder(), &disabled());
    let mut rt = RUNTIME.lock().unwrap_or_else(PoisonError::into_inner);
    let s = store();
    if let Some(old) = rt.as_mut() {
        old.stop_all(in_world);
        state::save(s, &old.mods);
    }
    *rt = None;
    LIVE.store(false, Relaxed);
    WANTS.fetch_and(OUTBOX | QUEUED | COMMAND, Relaxed);
    game::LISTEN.store(0, Relaxed);
    packets::store_mask(packets::PacketSet::default());
    game::clear_hud();
    game::MOVES.store(0, Relaxed);
    game::RELEASES.store(0, Relaxed);
    game::drop_owned();
    game::clear_ui();
    game::CLAIMS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    game::QUERIES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    profile::forget_requests();
    fs::forget();
    nbt::forget();
    render::clear();
    ghost::clear();
    image::clear();
    chunk::clear();

    registry::install(loaded.extras);
    let clashes = super::commands::install_scripts(loaded.commands);
    s.install_scripts(ScriptPart::from_registry());
    state::load(s, &loaded.mods);
    state::save(s, &loaded.mods);

    for e in &loaded.errors {
        crate::log_error!("mods", "{e}");
        if chat {
            api::print(e);
        }
    }
    for c in &clashes {
        crate::log_warn!("mods", "{c}");
        if chat {
            api::print(c);
        }
    }
    *INFO.lock().unwrap_or_else(PoisonError::into_inner) = loaded.info;

    if let Some(mut new) = loaded.runtime {
        new.mods = loaded.mods;
        *rt = Some(new);
        LIVE.store(true, Relaxed);
        WANTS.fetch_or(RESCAN | SETTINGS, Relaxed);
    }
}

pub(super) fn note_toggle() {
    if LIVE.load(Relaxed) {
        WANTS.fetch_or(RESCAN, Relaxed);
    }
}

pub(super) fn note_setting() {
    WANTS.fetch_or(SETTINGS, Relaxed);
}

pub(super) fn save_state(s: &Store) {
    match runtime_briefly() {
        Some(rt) => {
            if let Some(rt) = rt.as_ref() {
                state::save(s, &rt.mods);
            }
        }
        None => crate::log_warn!(
            "mods",
            "a mod is still running on the bot thread; mod-data was not saved this time"
        ),
    }
}

fn runtime_briefly() -> Option<std::sync::MutexGuard<'static, Option<run::Runtime>>> {
    #[cfg(target_arch = "wasm32")]
    const TRIES: usize = 1;
    #[cfg(not(target_arch = "wasm32"))]
    const TRIES: usize = 50;
    for _ in 0..TRIES {
        match RUNTIME.try_lock() {
            Ok(rt) => return Some(rt),
            Err(std::sync::TryLockError::Poisoned(p)) => return Some(p.into_inner()),
            #[cfg(target_arch = "wasm32")]
            Err(std::sync::TryLockError::WouldBlock) => {
                crate::log_error!(
                    "mods",
                    "the mods runtime is held on the only thread; giving up on it"
                );
            }
            #[cfg(not(target_arch = "wasm32"))]
            Err(std::sync::TryLockError::WouldBlock) => {
                std::thread::sleep(std::time::Duration::from_millis(5))
            }
        }
    }
    None
}

pub fn tick(bot: &Client, shared: &Arc<SharedMutex>) -> Option<(f32, f32)> {
    if WANTS.load(Relaxed) & TICK_SITE == 0 {
        return None;
    }
    let (on_ground, mut body) = bot.component::<azalea::entity::Physics>().map_or_else(
        |_| (false, run::Body::default()),
        |p| {
            let (min, max) = (p.bounding_box.min, p.bounding_box.max);
            let body = run::Body {
                velocity: [p.velocity.x, p.velocity.y, p.velocity.z],
                fall_distance: p.fall_distance,
                horizontal_collision: p.horizontal_collision,
                vertical_collision: p.vertical_collision,
                bbox: [min.x, min.y, min.z, max.x, max.y, max.z],
                ..run::Body::default()
            };
            (p.on_ground(), body)
        },
    );
    let (yaw, pitch) = bot
        .component::<azalea::entity::LookDirection>()
        .map_or((0.0, 0.0), |l| (l.y_rot(), l.x_rot()));
    let mut snap = game::Snapshot::spare();
    snap.dimension.clear();
    if let Ok(name) = bot.component::<azalea::world::WorldName>() {
        use std::fmt::Write as _;
        let _ = write!(snap.dimension, "{}", name.0);
    }
    let read = {
        let s = shared.lock().unwrap();
        body.crouching = s.session.crouching;
        body.flying = s.session.flying;
        s.in_world.then(|| {
            snap.read(&s);
            run::World {
                epoch: s.session_epoch,
                pos: s.session.player_pos,
                health: s.session.health,
                food: s.session.food,
                screen_open: s.screen_open,
                yaw,
                pitch,
                camera: [-s.camera_yaw - 180.0, -s.camera_pitch],
                on_ground,
                physics: body,
                gamemode: game::gamemode_name(s.session.gamemode),
                xp: s.session.xp_level,
                dead: s.session.dead,
                op_level: s.session.op_level,
                selected: s.session.hotbar_selected,
            }
        })
    };
    let Some(world) = read else {
        snap.recycle();
        return None;
    };
    let commands = if WANTS.fetch_and(!COMMAND, Relaxed) & COMMAND != 0 {
        std::mem::take(&mut *COMMANDS.lock().unwrap_or_else(PoisonError::into_inner))
    } else {
        Vec::new()
    };
    if let Some(rt) = RUNTIME
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
    {
        if LEAVE_PENDING.swap(false, Relaxed) {
            rt.leave();
        }
        rt.tick(&world, snap, commands, bot);
    }
    flush(bot, shared)
}

pub fn command_typed(name: String, raw: String) {
    let mut q = COMMANDS.lock().unwrap_or_else(PoisonError::into_inner);
    if q.len() >= COMMAND_QUEUE {
        return;
    }
    q.push((name, raw));
    WANTS.fetch_or(COMMAND, Relaxed);
}

pub fn chat(bot: &Client, shared: &Arc<SharedMutex>, spans: &[Span]) -> bool {
    if WANTS.load(Relaxed) & CHAT == 0 || LEAVE_PENDING.load(Relaxed) {
        return false;
    }
    if !shared.lock().unwrap().in_world {
        return false;
    }
    let text: String = spans.iter().map(|s| s.text.as_str()).collect();
    match RUNTIME
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
    {
        Some(rt) => rt.chat(&text, bot),
        None => false,
    }
}

pub fn typed(line: String) {
    game::TYPED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(line);
    game::TYPED_ANY.store(true, Relaxed);
}

pub fn chatsend(bot: &Client, shared: &Arc<SharedMutex>, rest: Vec<String>) -> Vec<String> {
    if !game::TYPED_ANY.swap(false, Relaxed) {
        return rest;
    }
    let typed = std::mem::take(&mut *game::TYPED.lock().unwrap_or_else(PoisonError::into_inner));
    let listening = game::listens(api::CHATSEND) && !LEAVE_PENDING.load(Relaxed);
    let mut lines = if listening && shared.lock().unwrap().in_world {
        match RUNTIME
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            Some(rt) => rt.chatsend(typed, bot),
            None => typed,
        }
    } else {
        typed
    };
    lines.extend(rest);
    lines
}

pub fn attack(
    bot: &Client,
    shared: &Arc<SharedMutex>,
    entity: azalea::ecs::entity::Entity,
) -> bool {
    if !game::listens(api::ATTACK)
        || LEAVE_PENDING.load(Relaxed)
        || !shared.lock().unwrap().in_world
    {
        return false;
    }
    let id = bot
        .get_entity_component::<azalea_core::entity_id::MinecraftEntityId>(entity)
        .map_or(-1, |id| id.0);
    let kind = bot
        .get_entity_component::<azalea::entity::EntityKindComponent>(entity)
        .map(|k| k.0.to_str());
    match RUNTIME
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
    {
        Some(rt) => rt.attack(id, kind.unwrap_or("minecraft:"), bot),
        None => false,
    }
}

pub fn use_item(bot: &Client, shared: &Arc<SharedMutex>) -> bool {
    if !game::listens(api::USE) || LEAVE_PENDING.load(Relaxed) || !shared.lock().unwrap().in_world {
        return false;
    }
    match RUNTIME
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
    {
        Some(rt) => rt.use_item(bot),
        None => false,
    }
}

pub fn poll_keys(
    keys: &bevy::input::ButtonInput<bevy::input::keyboard::KeyCode>,
    buttons: &bevy::input::ButtonInput<bevy::input::mouse::MouseButton>,
    screen_open: bool,
) {
    use crate::gui::keybinds::Bound;
    static SCREEN_WAS: AtomicBool = AtomicBool::new(false);
    if !LIVE.load(Relaxed) {
        return;
    }
    let unbound = Bound::Unbound.serialized_name();
    if screen_open
        && game::listens(api::UIKEY)
        && let Some(owner) = canvas_owner()
    {
        for &k in keys
            .get_just_pressed()
            .filter(|&&k| k != bevy::input::keyboard::KeyCode::Escape)
        {
            let key = Bound::Key(k).serialized_name();
            if key != unbound {
                game::push(game::Pending::UiKey { owner, key });
            }
        }
    }
    if !screen_open && game::listens(api::KEYUP) {
        let released = (keys.get_just_released().map(|&k| Bound::Key(k)))
            .chain(buttons.get_just_released().map(|&b| Bound::Mouse(b)));
        for bound in released {
            let key = bound.serialized_name();
            if key != unbound {
                game::push(game::Pending::KeyUp {
                    key,
                    label: bound.display_name(),
                });
            }
        }
    }
    let moved = SCREEN_WAS.swap(screen_open, Relaxed) != screen_open
        || keys.get_just_pressed().next().is_some()
        || keys.get_just_released().next().is_some()
        || buttons.get_just_pressed().next().is_some()
        || buttons.get_just_released().next().is_some();
    if !moved {
        return;
    }
    let mut held = game::HELD.lock().unwrap_or_else(PoisonError::into_inner);
    held.clear();
    if !screen_open {
        let down = (keys.get_pressed().map(|&k| Bound::Key(k)))
            .chain(buttons.get_pressed().map(|&b| Bound::Mouse(b)));
        held.extend(down.map(|b| b.serialized_name()).filter(|k| *k != unbound));
    }
}

pub fn scroll(delta: f32) -> bool {
    if !LIVE.load(Relaxed) {
        return false;
    }
    let claimed = claimant();
    if claimed.is_some() || game::listens(api::SCROLL) {
        game::push(game::Pending::Scroll {
            owner: claimed,
            delta,
        });
    }
    claimed.is_some()
}

pub fn scroll_claimed() -> bool {
    LIVE.load(Relaxed) && claimant().is_some()
}

fn claimant() -> Option<usize> {
    let held = game::HELD.lock().unwrap_or_else(PoisonError::into_inner);
    let claims = game::CLAIMS.lock().unwrap_or_else(PoisonError::into_inner);
    claims
        .iter()
        .find(|(_, key)| held.contains(key))
        .map(|(k, _)| *k)
}

pub fn tag_query(packet: &azalea_protocol::packets::game::c_tag_query::ClientboundTagQuery) {
    let found = {
        let mut q = game::QUERIES.lock().unwrap_or_else(PoisonError::into_inner);
        let at = q.iter().position(|(t, ..)| *t == packet.transaction_id);
        at.map(|i| q.swap_remove(i))
    };
    if let Some((_, owner, id)) = found {
        game::push(game::Pending::Job {
            owner,
            id,
            generation: nbt::generation(),
            result: Box::new(nbt::JobResult::Tag(packet.tag.clone())),
        });
    }
}

pub fn joined() {
    game::push(game::Pending::Join);
}

pub fn spawned() {
    game::push(game::Pending::Spawn);
}

pub fn died(text: String) {
    game::push(game::Pending::Death(text));
}

pub fn wants_keys() -> bool {
    game::listens(api::KEYDOWN)
}

pub fn key_down(key: &'static str, label: &'static str) {
    game::push(game::Pending::Key { key, label });
}

pub fn packet(packet: &azalea_protocol::packets::game::ClientboundGamePacket) {
    if let Some(converted) = packets::convert(packet) {
        game::push(game::Pending::Packet(Box::new(converted)));
    }
}

pub fn ui() -> Option<Arc<UiList>> {
    game::UI
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

pub fn ui_click(serial: u64, row: usize, button: UiButton, action: Option<usize>) {
    let (owner, id, action) = {
        let ui = game::UI.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(list) = ui.as_ref().filter(|l| l.serial == serial) else {
            return;
        };
        let Some(r) = list.rows.get(row) else {
            return;
        };
        (
            list.owner,
            r.id.clone(),
            action.and_then(|a| r.buttons.get(a).cloned()),
        )
    };
    game::push(game::Pending::UiClick {
        owner,
        row,
        id,
        button,
        action,
    });
}

fn canvas_owner() -> Option<usize> {
    let ui = game::UI.lock().unwrap_or_else(PoisonError::into_inner);
    ui.as_ref()
        .filter(|l| matches!(l.form, UiForm::Canvas(None)))
        .map(|l| l.owner)
}

pub fn ui_canvas_click(serial: u64, button: UiButton, sx: f32, sy: f32) {
    let owner = {
        let ui = game::UI.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(list) = ui
            .as_ref()
            .filter(|l| l.serial == serial && matches!(l.form, UiForm::Canvas(None)))
        else {
            return;
        };
        list.owner
    };
    let at = game::CAMERA
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .canvas_at(sx, sy);
    game::push(game::Pending::CanvasClick {
        owner,
        button,
        at,
        screen: [sx, sy],
    });
}

pub fn setting_button(ix: registry::ModuleIx, key: &'static str) {
    let Some(owner) = (ix.0 as usize).checked_sub(registry::COUNT) else {
        return;
    };
    game::push(game::Pending::Button { owner, key });
}

pub fn ui_closed(serial: u64) {
    let Some(list) = take_ui(serial) else {
        return;
    };
    let owner = list.owner;
    game::push(match &list.form {
        UiForm::Rows => game::Pending::UiClose { owner },
        UiForm::Prompt(_) => game::Pending::UiPrompt { owner, text: None },
        UiForm::File(_) => game::Pending::FileRequest { owner, file: None },
        UiForm::Canvas(over) => {
            if over.is_some() {
                game::push(game::Pending::UiPrompt { owner, text: None });
            }
            game::Pending::UiClose { owner }
        }
    });
}

pub fn ui_prompt(serial: u64, text: Option<String>) {
    let mut ui = game::UI.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(list) = ui.as_ref().filter(|l| l.serial == serial) else {
        return;
    };
    let owner = list.owner;
    let next = match &list.form {
        UiForm::Prompt(_) => None,
        UiForm::Canvas(Some(_)) => Some(game::reshow_canvas(list, None)),
        _ => return,
    };
    *ui = next;
    game::push(game::Pending::UiPrompt { owner, text });
}

pub fn ui_file(serial: u64, name: &str, bytes: &[u8]) -> Result<(), String> {
    let Some(list) = ui().filter(|l| l.serial == serial) else {
        return Ok(());
    };
    let UiForm::File(ask) = &list.form else {
        return Ok(());
    };
    let file = fs::import(ask, name, bytes)?;
    if take_ui(serial).is_some() {
        game::push(game::Pending::FileRequest {
            owner: list.owner,
            file: Some(Box::new(file)),
        });
    }
    Ok(())
}

pub const FILE_MAX: u64 = fs::QUOTA;

fn take_ui(serial: u64) -> Option<Arc<UiList>> {
    let mut ui = game::UI.lock().unwrap_or_else(PoisonError::into_inner);
    ui.as_ref().filter(|l| l.serial == serial)?;
    ui.take()
}

pub fn moves() -> u8 {
    game::MOVES.load(Relaxed)
}

pub fn releases() -> u8 {
    game::RELEASES.load(Relaxed)
}

pub fn note_input(moves: u8, attack: bool, use_item: bool) {
    let attack = if attack { game::INPUT_ATTACK } else { 0 };
    let use_item = if use_item { game::INPUT_USE } else { 0 };
    game::INPUT.store(u16::from(moves) | attack | use_item, Relaxed);
}

fn flush(bot: &Client, shared: &Arc<SharedMutex>) -> Option<(f32, f32)> {
    if WANTS.load(Relaxed) & OUTBOX == 0 {
        return None;
    }
    let (sends, prints, act, copy) = {
        let mut out = api::OUT.lock().unwrap_or_else(PoisonError::into_inner);
        let mut sends = out.sendable(crate::platform::time::Instant::now());
        sends.extend(out.commands_now());
        let prints = std::mem::take(&mut out.prints);
        let act = std::mem::replace(&mut out.actions, api::Actions::NONE);
        let copy = out.copy.take();
        if !out.waiting() {
            WANTS.fetch_and(!OUTBOX, Relaxed);
        }
        (sends, prints, act, copy)
    };
    if let Some(push) = act.velocity {
        let wrote = bot.query_self::<&mut azalea::entity::Physics, _>(|mut p| {
            let v = push.apply([p.velocity.x, p.velocity.y, p.velocity.z]);
            p.velocity = azalea_core::position::Vec3::new(v[0], v[1], v[2]);
        });
        if let Err(e) = wrote {
            crate::log_warn!("mods", "torch.player velocity not written: {e}");
        }
    }
    if let Some(text) = copy {
        crate::platform::clipboard::set(&text);
    }
    if let Some(left) = act.swing {
        crate::client::tick::swing_local_player(left, shared);
    }
    if let Some(id) = act.attack_id {
        let mut target = None;
        crate::modules::nearby::each(bot, |seen| {
            if seen.id == id {
                target = Some(seen.entity);
            }
        });
        if let Some(entity) = target {
            bot.attack(entity);
            crate::client::tick::swing_local_player(false, shared);
        }
    }
    if sends.is_empty() && prints.is_empty() && act == api::Actions::NONE {
        return None;
    }
    let mut s = shared.lock().unwrap();
    s.session.outgoing_chat.extend(sends);
    for line in &prints {
        super::chat_math::show_local(&mut s.session, line);
    }
    s.session.attack_clicked |= act.attack;
    s.session.use_clicked |= act.use_item;
    s.respawn_requested |= act.respawn;
    if let Some(slot) = act.select {
        s.session.hotbar_target = Some(slot);
    }
    if let Some((yaw, pitch)) = act.look {
        s.pending_look = Some((-yaw - 180.0, -pitch));
    }
    act.look
}
