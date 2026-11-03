use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU16, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, PoisonError};

use azalea::Client;
use mlua::{Error, Lua, Table, Value};

use super::api::{
    BUTTON, Ctx, DEATH, FILEREQUEST, JOB, JOIN, KEYDOWN, KEYUP, NONE, OUT, PACKET, PROFILE, SCROLL,
    SPAWN, UICLICK, UICLOSE, UIKEY, UIPROMPT, view_of,
};
use super::profile::Answer;
use super::{LIVE, QUEUED, WANTS};
use crate::client::tablist::Profile;
use crate::session::{
    Gamemode, MOVE_BACK, MOVE_FORWARD, MOVE_JUMP, MOVE_LEFT, MOVE_RIGHT, MOVE_SNEAK, MOVE_SPRINT,
    SharedState, SlotStack, WaypointInfo, WaypointKey, WaypointPos,
};

pub(super) fn gamemode_name(g: Gamemode) -> &'static str {
    match g {
        Gamemode::Survival => "survival",
        Gamemode::Creative => "creative",
        Gamemode::Adventure => "adventure",
        Gamemode::Spectator => "spectator",
    }
}

pub(super) fn uuid_text(u: u128) -> String {
    uuid::Uuid::from_u128(u).hyphenated().to_string()
}

pub(super) fn parse_uuid(s: &str, what: &str) -> mlua::Result<u128> {
    uuid::Uuid::parse_str(s.trim())
        .map(|u| u.as_u128())
        .map_err(|_| Error::runtime(format!("{what}: `{s}` is not a uuid")))
}

#[derive(Clone, Copy, Default)]
pub(super) struct Item {
    pub id: &'static str,
    pub count: u8,
    pub damage: u16,
    pub max_damage: u16,
}

impl Item {
    fn of(s: &SlotStack) -> Item {
        if s.is_empty() {
            return Item::default();
        }
        Item {
            id: s.item,
            count: s.count,
            damage: s.damage,
            max_damage: s.max_damage,
        }
    }
}

pub(super) struct Snapshot {
    pub pos: [f64; 3],
    pub slots: [Item; 36],
    pub offhand: Item,
    pub players: Vec<String>,
    pub nearby: Vec<(i32, String)>,
    pub waypoints: Arc<Vec<WaypointInfo>>,
    pub profiles: Arc<Vec<Profile>>,
    pub op_level: u8,
    pub creative: bool,
    pub dimension: String,
    pub address: Option<String>,
}

impl Default for Snapshot {
    fn default() -> Snapshot {
        Snapshot {
            pos: [0.0; 3],
            slots: [Item::default(); 36],
            offhand: Item::default(),
            players: Vec::new(),
            nearby: Vec::new(),
            waypoints: Arc::default(),
            profiles: Arc::default(),
            op_level: 0,
            creative: false,
            dimension: String::new(),
            address: None,
        }
    }
}

impl Snapshot {
    pub fn spare() -> Snapshot {
        SPARE
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .unwrap_or_default()
    }

    pub fn recycle(self) {
        *SPARE.lock().unwrap_or_else(PoisonError::into_inner) = Some(self);
    }

    pub fn read(&mut self, s: &SharedState) {
        let snap = self;
        snap.pos = s.session.player_pos;
        snap.address.clone_from(&s.session.current_address);
        snap.players.clone_from(&s.session.player_names);
        snap.waypoints = s.session.waypoints.clone();
        snap.profiles = s.session.player_profiles.clone();
        snap.op_level = s.session.op_level;
        snap.creative = s.session.gamemode == Gamemode::Creative;
        let others = &s.session.other_players;
        snap.nearby.truncate(others.len());
        for (i, p) in others.iter().enumerate() {
            match snap.nearby.get_mut(i) {
                Some((id, name)) => {
                    *id = p.id;
                    name.clone_from(&p.username);
                }
                None => snap.nearby.push((p.id, p.username.clone())),
            }
        }
        snap.slots = [Item::default(); 36];
        snap.offhand = Item::default();
        let hotbar = &s.session.hotbar;
        for (slot, stack) in snap.slots.iter_mut().zip(hotbar.iter().take(9)) {
            *slot = Item::of(stack);
        }
        if let Some(off) = hotbar.get(9) {
            snap.offhand = Item::of(off);
        }
        let menu = &s.session.menu_slots;
        let main = match menu.len() {
            46 => Some(&menu[9..36]),
            n if n >= 36 => Some(&menu[n - 36..n - 9]),
            _ => None,
        };
        if let Some(main) = main {
            for (slot, stack) in snap.slots[9..].iter_mut().zip(main) {
                *slot = Item::of(stack);
            }
        }
    }
}

pub(super) static SPARE: Mutex<Option<Snapshot>> = Mutex::new(None);

pub(super) type WorldLock = Arc<parking_lot::RwLock<azalea_world::World>>;

#[derive(Default)]
pub(super) struct Game {
    pub bot: Option<Client>,
    pub world: Option<WorldLock>,
    pub snap: Snapshot,
    pub drawing: Option<usize>,
    pub hud: Vec<ModHud>,
    hud_spare: Option<HudFrame>,
    pub panel: Option<u8>,
    pub canvas: Option<Layer>,
    pub pin: bool,
    pub drawn: (usize, usize),
    pub panels: Vec<(usize, &'static str, u8)>,
    pub hud_dirty: bool,
    pub moves: u8,
    pub releases: u8,
    pub placed: bool,
}

#[derive(Default, PartialEq)]
pub(super) struct Drawn {
    cmds: Vec<(Layer, Cmd)>,
    text: String,
}

impl Drawn {
    fn clear(&mut self) {
        self.cmds.clear();
        self.text.clear();
    }
}

#[derive(Default)]
pub(super) struct ModHud {
    now: Drawn,
    before: Drawn,
}

impl Game {
    pub fn clear_hud(&mut self, k: usize) {
        if let Some(h) = self.hud.get_mut(k)
            && !h.now.cmds.is_empty()
        {
            h.now.clear();
            self.hud_dirty = true;
        }
    }

    pub fn begin_hud(&mut self, k: usize) {
        if let Some(h) = self.hud.get_mut(k) {
            std::mem::swap(&mut h.now, &mut h.before);
            h.now.clear();
        }
    }

    pub fn end_hud(&mut self, k: usize) {
        if let Some(h) = self.hud.get(k)
            && h.now != h.before
        {
            self.hud_dirty = true;
        }
    }

    pub fn publish_hud(&mut self) {
        if !std::mem::take(&mut self.hud_dirty) {
            return;
        }
        let mut frame = self.hud_spare.take().unwrap_or_default();
        frame.fill(&self.hud);
        let frame = (!frame.cmds.is_empty()).then(|| Arc::new(frame));
        let old = std::mem::replace(
            &mut *HUD.lock().unwrap_or_else(PoisonError::into_inner),
            frame,
        );
        self.hud_spare = old.and_then(|a| Arc::try_unwrap(a).ok());
    }
}

const LAYERS: usize = crate::gui::hud_layout::MOD_PANELS + 3;

#[derive(Default)]
pub(super) struct HudFrame {
    cmds: Vec<Cmd>,
    ranges: [(u32, u32); LAYERS],
    text: String,
}

impl HudFrame {
    fn fill(&mut self, hud: &[ModHud]) {
        let mut counts = [0u32; LAYERS];
        for (layer, _) in hud.iter().flat_map(|h| &h.now.cmds) {
            if let Some(i) = layer.index() {
                counts[i] += 1;
            }
        }
        let mut at = 0;
        for (range, n) in self.ranges.iter_mut().zip(counts) {
            *range = (at, at);
            at += n;
        }
        self.cmds.clear();
        self.cmds.resize(
            at as usize,
            Cmd::Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
                argb: 0,
            },
        );
        self.text.clear();
        for h in hud {
            let base = self.text.len() as u32;
            self.text.push_str(&h.now.text);
            for &(layer, mut cmd) in &h.now.cmds {
                let Some(i) = layer.index() else { continue };
                if let Cmd::Text { text, .. } = &mut cmd {
                    text[0] += base;
                }
                let slot = &mut self.ranges[i].1;
                self.cmds[*slot as usize] = cmd;
                *slot += 1;
            }
        }
    }

    fn layer(&self, layer: Layer) -> &[Cmd] {
        layer.index().map_or(&[], |i| {
            &self.cmds[self.ranges[i].0 as usize..self.ranges[i].1 as usize]
        })
    }

    fn text(&self, [start, len]: [u32; 2]) -> &str {
        self.text
            .get(start as usize..(start + len) as usize)
            .unwrap_or_default()
    }
}

pub(super) enum Pending {
    Join,
    Spawn,
    Death(String),
    Key {
        key: &'static str,
        label: &'static str,
    },
    Packet(Box<super::packets::Packet>),
    UiClick {
        owner: usize,
        row: usize,
        id: Option<String>,
        button: UiButton,
        action: Option<String>,
    },
    UiClose {
        owner: usize,
    },
    Profile {
        owner: usize,
        uuid: u128,
        answer: Answer,
    },
    KeyUp {
        key: &'static str,
        label: &'static str,
    },
    Scroll {
        owner: Option<usize>,
        delta: f32,
    },
    Job {
        owner: usize,
        id: u64,
        generation: u64,
        result: Box<super::nbt::JobResult>,
    },
    UiPrompt {
        owner: usize,
        text: Option<String>,
    },
    FileRequest {
        owner: usize,
        file: Option<Box<Imported>>,
    },
    Button {
        owner: usize,
        key: &'static str,
    },
    CanvasClick {
        owner: usize,
        button: UiButton,
        at: [f64; 2],
        screen: [f32; 2],
    },
    UiKey {
        owner: usize,
        key: &'static str,
    },
}

impl Pending {
    pub fn event(&self) -> usize {
        match self {
            Pending::Join => JOIN,
            Pending::Spawn => SPAWN,
            Pending::Death(_) => DEATH,
            Pending::Key { .. } => KEYDOWN,
            Pending::Packet(_) => PACKET,
            Pending::UiClick { .. } => UICLICK,
            Pending::UiClose { .. } => UICLOSE,
            Pending::Profile { .. } => PROFILE,
            Pending::KeyUp { .. } => KEYUP,
            Pending::Scroll { .. } => SCROLL,
            Pending::Job { .. } => JOB,
            Pending::UiPrompt { .. } => UIPROMPT,
            Pending::FileRequest { .. } => FILEREQUEST,
            Pending::Button { .. } => BUTTON,
            Pending::CanvasClick { .. } => UICLICK,
            Pending::UiKey { .. } => UIKEY,
        }
    }

    pub fn owner(&self) -> Option<usize> {
        match self {
            Pending::UiClick { owner, .. }
            | Pending::UiClose { owner }
            | Pending::Profile { owner, .. }
            | Pending::Job { owner, .. }
            | Pending::UiPrompt { owner, .. }
            | Pending::FileRequest { owner, .. }
            | Pending::Button { owner, .. }
            | Pending::CanvasClick { owner, .. }
            | Pending::UiKey { owner, .. } => Some(*owner),
            Pending::Scroll { owner, .. } => *owner,
            _ => None,
        }
    }

    pub fn fill(&self, lua: &Lua, e: &Table) -> mlua::Result<()> {
        match self {
            Pending::Join | Pending::Spawn | Pending::UiClose { .. } => Ok(()),
            Pending::Death(text) => e.raw_set("text", text.as_str()),
            Pending::Key { key, label } | Pending::KeyUp { key, label } => {
                e.raw_set("key", *key)?;
                e.raw_set("label", *label)
            }
            Pending::Packet(p) => p.fill(lua, e),
            Pending::UiClick {
                row,
                id,
                button,
                action,
                ..
            } => {
                e.raw_set("row", *row + 1)?;
                e.raw_set("id", id.as_deref())?;
                e.raw_set("button", button.name())?;
                e.raw_set("action", action.as_deref())
            }
            Pending::Profile { uuid, answer, .. } => {
                e.raw_set("uuid", uuid_text(*uuid))?;
                answer.fill(e)
            }
            Pending::Scroll { owner, delta } => {
                e.raw_set("delta", *delta as f64)?;
                e.raw_set("claimed", owner.is_some())
            }
            Pending::Job { id, result, .. } => {
                e.raw_set("id", *id)?;
                result.fill(lua, e)
            }
            Pending::UiPrompt { text, .. } => {
                e.raw_set("text", text.as_deref())?;
                e.raw_set("cancelled", text.is_none())
            }
            Pending::FileRequest { file, .. } => {
                if let Some(f) = file {
                    e.raw_set("path", f.path.as_str())?;
                    e.raw_set("name", f.name.as_str())?;
                    e.raw_set("size", f.size)?;
                }
                e.raw_set("cancelled", file.is_none())
            }
            Pending::Button { key, .. } => e.raw_set("id", *key),
            Pending::CanvasClick {
                button, at, screen, ..
            } => {
                e.raw_set("button", button.name())?;
                e.raw_set("x", at[0])?;
                e.raw_set("y", at[1])?;
                e.raw_set("sx", screen[0] as f64)?;
                e.raw_set("sy", screen[1] as f64)
            }
            Pending::UiKey { key, .. } => e.raw_set("key", *key),
        }
    }
}

pub(super) fn waypoint_fields(
    t: &Table,
    key: &WaypointKey,
    info: Option<&WaypointInfo>,
) -> mlua::Result<()> {
    match key {
        WaypointKey::Uuid(u) => {
            t.raw_set("id", uuid_text(*u))?;
            t.raw_set("kind", "uuid")?;
        }
        WaypointKey::Name(n) => {
            t.raw_set("id", n.as_str())?;
            t.raw_set("kind", "name")?;
        }
    }
    let Some(info) = info else {
        return Ok(());
    };
    t.raw_set("style", info.style.as_str())?;
    if let Some([r, g, b]) = info.color {
        let c = |v: f32| (v * 255.0).round() as u32;
        t.raw_set("color", c(r) << 16 | c(g) << 8 | c(b))?;
    }
    match info.pos {
        WaypointPos::Block([x, y, z]) => {
            t.raw_set("type", "block")?;
            t.raw_set("x", x)?;
            t.raw_set("y", y)?;
            t.raw_set("z", z)?;
        }
        WaypointPos::Chunk { x, z } => {
            t.raw_set("type", "chunk")?;
            t.raw_set("chunkx", x)?;
            t.raw_set("chunkz", z)?;
        }
        WaypointPos::Azimuth(angle) => {
            t.raw_set("type", "azimuth")?;
            t.raw_set("yaw", (angle as f64).to_degrees())?;
        }
    }
    Ok(())
}

const PENDING_CAP: usize = 256;

static PENDING: Mutex<VecDeque<(u64, Pending)>> = Mutex::new(VecDeque::new());

pub(super) static LISTEN: AtomicU32 = AtomicU32::new(0);

pub(super) fn push(p: Pending) {
    if !LIVE.load(Relaxed) {
        return;
    }
    let epoch = crate::session::SESSION_EPOCH.load(Relaxed);
    let mut q = PENDING.lock().unwrap_or_else(PoisonError::into_inner);
    if q.len() >= PENDING_CAP {
        q.pop_front();
    }
    q.push_back((epoch, p));
    WANTS.fetch_or(QUEUED, Relaxed);
}

pub(super) fn take(epoch: u64, out: &mut Vec<Pending>) {
    let mut q = PENDING.lock().unwrap_or_else(PoisonError::into_inner);
    WANTS.fetch_and(!QUEUED, Relaxed);
    let generation = super::nbt::generation();
    out.extend(
        q.drain(..)
            .filter(|(e, p)| {
                *e == epoch && !matches!(p, Pending::Job { generation: g, .. } if *g != generation)
            })
            .map(|(_, p)| p),
    );
    drop(q);
    super::image::drain(out);
}

pub(super) fn drop_owned() {
    PENDING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|(_, p)| p.owner().is_none());
}

pub(super) fn clear() {
    PENDING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    WANTS.fetch_and(!QUEUED, Relaxed);
}

pub(super) fn listens(ev: usize) -> bool {
    ev < 32 && LISTEN.load(Relaxed) >> ev & 1 != 0
}

pub(super) static TYPED: Mutex<Vec<String>> = Mutex::new(Vec::new());
pub(super) static TYPED_ANY: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Cmd {
    Text {
        x: f64,
        y: f64,
        text: [u32; 2],
        rgb: u32,
        shadow: bool,
        scale: f32,
        align: Align,
    },
    Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        argb: u32,
    },
    Item {
        x: f64,
        y: f64,
        id: &'static str,
        count: u32,
    },
    Image {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        key: u64,
        uv: [f32; 4],
    },
    Pin {
        x: f64,
        y: f64,
    },
    Unpin,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Layer {
    Screen,
    Panel(u8),
    Canvas,
    CanvasOverlay,
}

impl Layer {
    fn index(self) -> Option<usize> {
        match self {
            Layer::Screen => Some(0),
            Layer::Panel(s) => {
                (usize::from(s) < crate::gui::hud_layout::MOD_PANELS).then_some(1 + usize::from(s))
            }
            Layer::Canvas => Some(LAYERS - 2),
            Layer::CanvasOverlay => Some(LAYERS - 1),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Align {
    Left,
    Center,
    Right,
}

const TEXT_SCALE: std::ops::RangeInclusive<f64> = 0.25..=8.0;

pub(super) static HUD: Mutex<Option<Arc<HudFrame>>> = Mutex::new(None);

pub(super) static SCREEN: AtomicU64 = AtomicU64::new(0);

const HUD_CAP: usize = 256;
const CANVAS_CAP: usize = 4096;

const HUD_TEXT: usize = 256;

pub(super) fn screen() -> (f32, f32) {
    let v = SCREEN.load(Relaxed);
    (f32::from_bits((v >> 32) as u32), f32::from_bits(v as u32))
}

pub fn draw_hud(p: &mut crate::gui::painter::Painter, vw: f32, vh: f32) {
    note_screen(vw, vh);
    paint_cmds(p, Layer::Screen, None);
}

fn note_screen(vw: f32, vh: f32) {
    SCREEN.store(
        u64::from(vw.to_bits()) << 32 | u64::from(vh.to_bits()),
        Relaxed,
    );
}

pub fn draw_panel(p: &mut crate::gui::painter::Painter, slot: usize) {
    if let Ok(slot) = u8::try_from(slot) {
        paint_cmds(p, Layer::Panel(slot), None);
    }
}

pub fn draw_canvas(p: &mut crate::gui::painter::Painter, cam: Camera, vw: f32, vh: f32) {
    note_screen(vw, vh);
    paint_cmds(p, Layer::Canvas, Some((cam, vw, vh)));
}

pub fn draw_canvas_overlay(p: &mut crate::gui::painter::Painter) {
    paint_cmds(p, Layer::CanvasOverlay, None);
}

fn paint_cmds(p: &mut crate::gui::painter::Painter, layer: Layer, cam: Option<(Camera, f32, f32)>) {
    let Some(frame) = HUD.lock().unwrap_or_else(PoisonError::into_inner).clone() else {
        return;
    };
    let through = move |x: f64, y: f64| -> (f32, f32) {
        match cam {
            Some((c, vw, vh)) => (
                (f64::from(vw) * 0.5 + (x - c.x) * c.scale) as f32,
                (f64::from(vh) * 0.5 + (y - c.y) * c.scale) as f32,
            ),
            None => (x as f32, y as f32),
        }
    };
    let scale = cam.map_or(1.0, |(c, ..)| c.scale);
    let mut pin: Option<(f32, f32)> = None;
    for c in frame.layer(layer) {
        let at = move |x: f64, y: f64| match pin {
            Some((px, py)) => (px + x as f32, py + y as f32),
            None => through(x, y),
        };
        let size = move |w: f64, h: f64| {
            let s = if pin.is_some() { 1.0 } else { scale };
            ((w * s) as f32, (h * s) as f32)
        };
        match c {
            Cmd::Text {
                x,
                y,
                text,
                rgb,
                shadow,
                scale,
                align,
            } => {
                let text = frame.text(*text);
                let (x, y) = at(*x, *y);
                let dx = match align {
                    Align::Left => 0.0,
                    Align::Center => -p.atlas.font.width_str(text) * 0.5,
                    Align::Right => -p.atlas.font.width_str(text),
                };
                if *scale == 1.0 {
                    p.text_plain(text, x + dx, y, *rgb, *shadow);
                } else {
                    p.scaled_in(*scale, x, y, |p| p.text_plain(text, dx, 0.0, *rgb, *shadow));
                }
            }
            Cmd::Rect { x, y, w, h, argb } => {
                let ((x, y), (w, h)) = (at(*x, *y), size(*w, *h));
                p.fill(x, y, w, h, *argb);
            }
            Cmd::Item { x, y, id, count } => {
                let (x, y) = at(*x, *y);
                item_with_count(p, id, x, y, *count);
            }
            Cmd::Image {
                x,
                y,
                w,
                h,
                key,
                uv,
            } => {
                let ((x, y), (w, h)) = (at(*x, *y), size(*w, *h));
                p.image(*key, x, y, w, h, *uv, 0xFFFF_FFFF);
            }
            Cmd::Pin { x, y } => pin = Some(through(*x, *y)),
            Cmd::Unpin => pin = None,
        }
    }
}

fn item_with_count(p: &mut crate::gui::painter::Painter, id: &str, x: f32, y: f32, count: u32) {
    p.item_icon(id, x, y);
    if count > 1 {
        let n = count.to_string();
        let w = p.atlas.font.width_str(&n);
        p.text_plain(&n, x + 17.0 - w, y + 9.0, 0xFF_FFFF, true);
    }
}

fn item_kind(id: &str, what: &str) -> mlua::Result<azalea_registry::builtin::ItemKind> {
    use std::str::FromStr;
    let bare = id.strip_prefix("minecraft:").unwrap_or(id);
    azalea_registry::builtin::ItemKind::from_str(bare)
        .map_err(|_| Error::runtime(format!("{what}: `{id}` is not an item")))
}

fn item_id(id: &str, what: &str) -> mlua::Result<&'static str> {
    item_kind(id, what).map(|k| k.to_str().trim_start_matches("minecraft:"))
}

pub(super) fn clear_hud() {
    *HUD.lock().unwrap_or_else(PoisonError::into_inner) = None;
}

#[derive(Clone, Debug)]
pub struct UiRow {
    pub text: String,
    pub rgb: u32,
    pub detail: String,
    pub id: Option<String>,
    pub item: Option<&'static str>,
    pub buttons: Vec<String>,
}

#[derive(Debug)]
pub struct UiList {
    pub serial: u64,
    pub(super) owner: usize,
    pub title: String,
    pub footer: String,
    pub rows: Vec<UiRow>,
    pub form: UiForm,
}

#[derive(Debug)]
pub enum UiForm {
    Rows,
    Prompt(String),
    File(FileAsk),
    Canvas(Option<OverPrompt>),
}

#[derive(Clone, Debug)]
pub struct OverPrompt {
    pub title: String,
    pub default: String,
}

pub(super) fn reshow_canvas(list: &UiList, prompt: Option<OverPrompt>) -> Arc<UiList> {
    Arc::new(UiList {
        serial: UI_SERIAL.fetch_add(1, Relaxed),
        owner: list.owner,
        title: list.title.clone(),
        footer: String::new(),
        rows: Vec::new(),
        form: UiForm::Canvas(prompt),
    })
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub x: f64,
    pub y: f64,
    pub scale: f64,
    pub vw: f32,
    pub vh: f32,
    pub mouse: Option<[f32; 2]>,
}

impl Camera {
    const DEFAULT: Camera = Camera {
        x: 0.0,
        y: 0.0,
        scale: 1.0,
        vw: 0.0,
        vh: 0.0,
        mouse: None,
    };
    pub const SCALE: std::ops::RangeInclusive<f64> = 0.0625..=64.0;

    pub fn canvas_at(&self, sx: f32, sy: f32) -> [f64; 2] {
        [
            self.x + (f64::from(sx) - f64::from(self.vw) * 0.5) / self.scale,
            self.y + (f64::from(sy) - f64::from(self.vh) * 0.5) / self.scale,
        ]
    }
}

pub static CAMERA: Mutex<Camera> = Mutex::new(Camera::DEFAULT);

#[derive(Debug)]
pub struct FileAsk {
    pub mod_id: &'static str,
    pub(super) into: String,
    pub accept: Vec<String>,
}

#[derive(Debug)]
pub struct Imported {
    pub path: String,
    pub name: String,
    pub size: u64,
}

pub(super) fn show_ui(owner: usize, title: String, footer: String, rows: Vec<UiRow>, form: UiForm) {
    let serial = UI_SERIAL.fetch_add(1, Relaxed);
    *UI.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(UiList {
        serial,
        owner,
        title,
        footer,
        rows,
        form,
    }));
}

pub(super) static UI: Mutex<Option<Arc<UiList>>> = Mutex::new(None);

static UI_SERIAL: AtomicU64 = AtomicU64::new(1);

const UI_ROWS: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiButton {
    Left,
    Right,
    Middle,
}

impl UiButton {
    fn name(self) -> &'static str {
        match self {
            UiButton::Left => "left",
            UiButton::Right => "right",
            UiButton::Middle => "middle",
        }
    }
}

pub(super) fn close_ui_of(owner: usize) -> bool {
    let mut ui = UI.lock().unwrap_or_else(PoisonError::into_inner);
    if ui.as_ref().is_some_and(|l| l.owner == owner) {
        *ui = None;
        return true;
    }
    false
}

pub(super) fn clear_ui() {
    *UI.lock().unwrap_or_else(PoisonError::into_inner) = None;
}

pub(super) static MOVES: AtomicU8 = AtomicU8::new(0);

pub(super) static RELEASES: AtomicU8 = AtomicU8::new(0);

pub(super) static INPUT: AtomicU16 = AtomicU16::new(0);
pub(super) const INPUT_ATTACK: u16 = 1 << 8;
pub(super) const INPUT_USE: u16 = 1 << 9;

const ACTIONS: [(&str, u16); 9] = [
    ("forward", MOVE_FORWARD as u16),
    ("back", MOVE_BACK as u16),
    ("left", MOVE_LEFT as u16),
    ("right", MOVE_RIGHT as u16),
    ("jump", MOVE_JUMP as u16),
    ("sneak", MOVE_SNEAK as u16),
    ("sprint", MOVE_SPRINT as u16),
    ("attack", INPUT_ATTACK),
    ("use", INPUT_USE),
];

pub(super) static HELD: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

pub(super) static CLAIMS: Mutex<Vec<(usize, &'static str)>> = Mutex::new(Vec::new());

pub(super) fn release_scroll(owner: usize) {
    CLAIMS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|(k, _)| *k != owner);
}

pub(super) static QUERIES: Mutex<Vec<(u32, usize, u64)>> = Mutex::new(Vec::new());

const HOLD_KEYS: [(&str, u8); 7] = [
    ("forward", MOVE_FORWARD),
    ("back", MOVE_BACK),
    ("left", MOVE_LEFT),
    ("right", MOVE_RIGHT),
    ("jump", MOVE_JUMP),
    ("sneak", MOVE_SNEAK),
    ("sprint", MOVE_SPRINT),
];

fn move_bits(what: &str, keys: &Table) -> mlua::Result<u8> {
    let mut bits = 0;
    for pair in keys.pairs::<Value, Value>() {
        let (k, v) = pair?;
        let Value::String(k) = k else {
            return Err(Error::runtime(format!(
                "{what} takes {{ forward = true, ... }}"
            )));
        };
        let k = k.to_string_lossy();
        let Some(&(_, bit)) = HOLD_KEYS.iter().find(|(n, _)| *n == k) else {
            let names: Vec<&str> = HOLD_KEYS.iter().map(|(n, _)| *n).collect();
            return Err(Error::runtime(format!(
                "{what}: unknown key `{k}`{}; one of {}",
                super::api::suggest(&k, &names),
                names.join(", ")
            )));
        };
        if v.as_boolean() == Some(true) {
            bits |= bit;
        }
    }
    Ok(bits)
}

pub(super) struct Tables {
    pub server: Table,
    pub server_backing: Table,
    pub world: Table,
    pub inventory: Table,
    pub inventory_backing: Table,
    pub game: Table,
    pub input: Table,
    pub hud: Table,
    pub util: Table,
    pub clipboard: Table,
    pub ui: Table,
    pub profile: Table,
}

pub(super) fn game_fn<A, R>(
    lua: &Lua,
    ctx: &Arc<Ctx>,
    what: &'static str,
    f: impl Fn(&Lua, &Ctx, A) -> mlua::Result<R> + Send + 'static,
) -> mlua::Result<mlua::Function>
where
    A: mlua::FromLuaMulti,
    R: mlua::IntoLuaMulti,
{
    let ctx = ctx.clone();
    lua.create_function(move |lua, args: A| {
        ctx.at_load(what)?;
        f(lua, &ctx, args)
    })
}

fn whole(v: f64, what: &str) -> mlua::Result<i64> {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e15 {
        Ok(v as i64)
    } else {
        Err(Error::runtime(format!(
            "{what} takes whole numbers, not {v}"
        )))
    }
}

fn item(lua: &Lua, it: Item) -> mlua::Result<Value> {
    if it.id.is_empty() {
        return Ok(Value::Nil);
    }
    let t = lua.create_table()?;
    t.raw_set("id", format!("minecraft:{}", it.id))?;
    t.raw_set("count", it.count)?;
    t.raw_set("damage", it.damage)?;
    t.raw_set("maxdamage", it.max_damage)?;
    if let Some(b) = crate::items::placed::placed_block(it.id) {
        t.raw_set("block", b.to_str())?;
    }
    Ok(Value::Table(t))
}

fn color(v: Option<f64>, what: &str) -> mlua::Result<u32> {
    let Some(v) = v else {
        return Ok(0xFFFF_FFFF);
    };
    if !(0.0..=f64::from(u32::MAX)).contains(&v) || v.fract() != 0.0 {
        return Err(Error::runtime(format!(
            "{what}: a colour is 0xRRGGBB or 0xAARRGGBB, not {v}"
        )));
    }
    let c = v as u32;
    Ok(if c <= 0xFF_FFFF { 0xFF00_0000 | c } else { c })
}

pub(super) fn tables(lua: &Lua, ctx: &Arc<Ctx>) -> mlua::Result<Tables> {
    let server_backing = lua.create_table()?;
    server_backing.raw_set(
        "players",
        game_fn(lua, ctx, "torch.server.players", |lua, ctx, ()| {
            let game = ctx.game.lock().unwrap_or_else(PoisonError::into_inner);
            lua.create_sequence_from(game.snap.players.iter().map(String::as_str))
        })?,
    )?;
    server_backing.raw_set(
        "profiles",
        game_fn(lua, ctx, "torch.server.profiles", |lua, ctx, ()| {
            let profiles = ctx
                .game
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .snap
                .profiles
                .clone();
            let out = lua.create_table_with_capacity(profiles.len(), 0)?;
            for (i, p) in profiles.iter().enumerate() {
                out.raw_set(i + 1, profile(lua, p)?)?;
            }
            Ok(out)
        })?,
    )?;
    server_backing.raw_set(
        "profile",
        game_fn(lua, ctx, "torch.server.profile", |lua, ctx, id: String| {
            let uuid = parse_uuid(&id, "torch.server.profile")?;
            let profiles = ctx
                .game
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .snap
                .profiles
                .clone();
            match profiles.iter().find(|p| p.uuid == uuid) {
                Some(p) => profile(lua, p).map(Value::Table),
                None => Ok(Value::Nil),
            }
        })?,
    )?;
    let server = view_of(lua, &server_backing)?;

    let world = lua.create_table()?;
    world.raw_set(
        "block",
        game_fn(
            lua,
            ctx,
            "torch.world.block",
            |_, ctx, (x, y, z): (f64, f64, f64)| {
                let what = "torch.world.block";
                let pos = block_pos(x, y, z, what)?;
                let Some(world) = ctx.world(what)? else {
                    return Ok(None);
                };
                let state = world.read().get_block_state(pos);
                Ok(state.map(|s| s.as_block_kind().to_str()))
            },
        )?,
    )?;
    world.raw_set(
        "entities",
        game_fn(
            lua,
            ctx,
            "torch.world.entities",
            |lua, ctx, arg: Option<Table>| entities(lua, ctx, arg),
        )?,
    )?;
    world.raw_set(
        "waypoints",
        game_fn(lua, ctx, "torch.world.waypoints", |lua, ctx, ()| {
            waypoints(lua, ctx)
        })?,
    )?;
    world.raw_set(
        "target",
        game_fn(lua, ctx, "torch.world.target", |lua, ctx, ()| {
            target(lua, ctx)
        })?,
    )?;
    world.raw_set(
        "blockentity",
        game_fn(
            lua,
            ctx,
            "torch.world.blockentity",
            |lua, _, (x, y, z): (f64, f64, f64)| {
                let pos = block_pos(x, y, z, "torch.world.blockentity")?;
                let Some((kind, tag)) = crate::blockentities::feed::raw_at([pos.x, pos.y, pos.z])
                else {
                    return Ok(Value::Nil);
                };
                let t = super::nbt::compound_to_lua(lua, tag.as_ref())?;
                t.raw_set(
                    "id",
                    if kind.contains(':') {
                        kind
                    } else {
                        format!("minecraft:{kind}")
                    },
                )?;
                t.raw_set("x", pos.x)?;
                t.raw_set("y", pos.y)?;
                t.raw_set("z", pos.z)?;
                Ok(Value::Table(t))
            },
        )?,
    )?;
    world.raw_set(
        "querynbt",
        game_fn(lua, ctx, "torch.world.querynbt", |_, ctx, (x, y, z): (f64, f64, f64)| {
            use azalea_protocol::packets::game::s_block_entity_tag_query::ServerboundBlockEntityTagQuery;
            static TXN: AtomicU32 = AtomicU32::new(1 << 30);
            let what = "torch.world.querynbt";
            let pos = block_pos(x, y, z, what)?;
            let (k, _) = ctx.caller(what)?;
            if ctx.game.lock().unwrap_or_else(PoisonError::into_inner).snap.op_level < 2 {
                return Ok((None, Some("needs operator permission level 2")));
            }
            let bot = ctx.bot(what)?;
            let txn = TXN.fetch_add(1, Relaxed);
            let id = ctx.jobs.fetch_add(1, Relaxed) + 1;
            {
                let mut q = QUERIES.lock().unwrap_or_else(PoisonError::into_inner);
                if q.len() >= QUERY_CAP {
                    return Ok((None, Some("too many queries waiting")));
                }
                q.push((txn, k, id));
            }
            bot.write_packet(ServerboundBlockEntityTagQuery { transaction_id: txn, pos });
            Ok((Some(id), None))
        })?,
    )?;
    super::blocks::add_world(lua, ctx, &world)?;
    super::chunk::add_world(lua, ctx, &world)?;
    world.set_readonly(true);

    let inventory_backing = lua.create_table()?;
    inventory_backing.raw_set("selected", 1)?;
    inventory_backing.raw_set(
        "slot",
        game_fn(lua, ctx, "torch.inventory.slot", |lua, ctx, i: f64| {
            let i = whole(i, "torch.inventory.slot")?;
            if !(1..=36).contains(&i) {
                return Err(Error::runtime(format!(
                    "torch.inventory.slot({i}): slots are 1 to 36 (1 to 9 the hotbar)"
                )));
            }
            let it = ctx
                .game
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .snap
                .slots[i as usize - 1];
            item(lua, it)
        })?,
    )?;
    inventory_backing.raw_set(
        "offhand",
        game_fn(lua, ctx, "torch.inventory.offhand", |lua, ctx, ()| {
            let it = ctx
                .game
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .snap
                .offhand;
            item(lua, it)
        })?,
    )?;
    inventory_backing.raw_set(
        "select",
        game_fn(lua, ctx, "torch.inventory.select", |_, _, i: f64| {
            let i = whole(i, "torch.inventory.select")?;
            if !(1..=9).contains(&i) {
                return Err(Error::runtime(format!(
                    "torch.inventory.select({i}): hotbar slots are 1 to 9"
                )));
            }
            OUT.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .act(|a| a.select = Some(i as u8 - 1));
            Ok(())
        })?,
    )?;
    inventory_backing.raw_set(
        "swap",
        game_fn(
            lua,
            ctx,
            "torch.inventory.swap",
            |_, ctx, (slot, hotbar): (f64, f64)| {
                use azalea::entity::inventory::Inventory;
                use azalea::inventory::ContainerClickEvent;
                use azalea_inventory::operations::SwapClick;
                let what = "torch.inventory.swap";
                let source = menu_slot(slot, what)?;
                let hotbar = whole(hotbar, what)?;
                if !(1..=9).contains(&hotbar) {
                    return Err(Error::runtime(format!(
                        "{what}: hotbar slots are 1 to 9, not {hotbar}"
                    )));
                }
                let bot = ctx.bot(what)?;
                let window_id = match bot.component::<Inventory>() {
                    Ok(inv) if inv.container_menu.is_none() => inv.id,
                    _ => return Ok(false),
                };
                bot.ecs.write().trigger(ContainerClickEvent {
                    entity: bot.entity,
                    window_id,
                    operation: SwapClick {
                        source_slot: source,
                        target_slot: hotbar as u8 - 1,
                    }
                    .into(),
                });
                Ok(true)
            },
        )?,
    )?;
    inventory_backing.raw_set(
        "creative",
        game_fn(
            lua,
            ctx,
            "torch.inventory.creative",
            |_, ctx, (slot, id, count, components): (Value, Option<String>, Option<f64>, Option<Table>)| {
                use azalea_inventory::ItemStack;
                let what = "torch.inventory.creative";
                let slot = super::items::Slot::parse(&slot, what)?.player_menu_index();
                let bot = ctx.bot(what)?;
                let stack = match id {
                    None => ItemStack::Empty,
                    Some(id) => {
                        let kind = item_kind(&id, what)?;
                        let count = whole(count.unwrap_or(1.0), what)?;
                        if !(1..=99).contains(&count) {
                            return Err(Error::runtime(format!("{what}: a count is 1 to 99, not {count}")));
                        }
                        let stack = ItemStack::new(kind, count as i32);
                        match &components {
                            Some(spec) => {
                                super::items::with_registries(&bot, |r| super::items::apply(stack, spec, what, r))?
                            }
                            None => stack,
                        }
                    }
                };
                if !ctx.game.lock().unwrap_or_else(PoisonError::into_inner).snap.creative {
                    return Ok(false);
                }
                crate::play::inventory_bridge::set_creative_slot(&bot, slot, stack);
                Ok(true)
            },
        )?,
    )?;
    inventory_backing.raw_set(
        "components",
        game_fn(
            lua,
            ctx,
            "torch.inventory.components",
            |lua, ctx, (slot, opts): (Value, Option<Table>)| {
                let what = "torch.inventory.components";
                let slot = super::items::Slot::parse(&slot, what)?;
                let raw = match &opts {
                    Some(o) => o.raw_get::<Option<bool>>("raw")?.unwrap_or(false),
                    None => false,
                };
                super::items::read(lua, &ctx.bot(what)?, slot, raw)
            },
        )?,
    )?;
    let inventory = view_of(lua, &inventory_backing)?;

    let game = lua.create_table()?;
    game.raw_set(
        "attack",
        game_fn(lua, ctx, "torch.game.attack", |_, _, id: Option<f64>| {
            let what = "torch.game.attack";
            let id = id.map(|n| whole(n, what)).transpose()?;
            let id = id
                .map(|n| {
                    i32::try_from(n)
                        .map_err(|_| Error::runtime(format!("{what}: {n} is not an entity id")))
                })
                .transpose()?;
            OUT.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .act(|a| match id {
                    Some(id) => a.attack_id = Some(id),
                    None => a.attack = true,
                });
            Ok(())
        })?,
    )?;
    for (name, what) in [("use", "torch.game.use"), ("respawn", "torch.game.respawn")] {
        game.raw_set(
            name,
            game_fn(lua, ctx, what, move |_, _, ()| {
                OUT.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .act(|a| match name {
                        "use" => a.use_item = true,
                        _ => a.respawn = true,
                    });
                Ok(())
            })?,
        )?;
    }
    game.raw_set(
        "place",
        game_fn(lua, ctx, "torch.game.place", |_, ctx, spec: Table| {
            place(ctx, spec)
        })?,
    )?;
    game.raw_set(
        "command",
        game_fn(lua, ctx, "torch.game.command", |_, ctx, text: String| {
            let what = "torch.game.command";
            let line = text.strip_prefix('/').unwrap_or(&text);
            if line.is_empty() || line.chars().count() > 256 {
                return Err(Error::runtime(format!(
                    "{what} takes a command of 1 to 256 characters"
                )));
            }
            let op = ctx
                .game
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .snap
                .op_level
                >= 1;
            Ok(OUT
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .command(format!("/{line}"), op))
        })?,
    )?;
    game.set_readonly(true);

    let input = lua.create_table()?;
    input.raw_set(
        "look",
        game_fn(
            lua,
            ctx,
            "torch.input.look",
            |_, _, (yaw, pitch): (f64, f64)| {
                if !yaw.is_finite() || !pitch.is_finite() {
                    return Err(Error::runtime("torch.input.look takes finite degrees"));
                }
                let pitch = pitch.clamp(-90.0, 90.0) as f32;
                OUT.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .act(|a| a.look = Some((yaw as f32, pitch)));
                Ok(())
            },
        )?,
    )?;
    input.raw_set(
        "hold",
        game_fn(lua, ctx, "torch.input.hold", |_, ctx, keys: Table| {
            let bits = move_bits("torch.input.hold", &keys)?;
            ctx.game
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .moves |= bits;
            Ok(())
        })?,
    )?;
    input.raw_set(
        "release",
        game_fn(lua, ctx, "torch.input.release", |_, ctx, keys: Table| {
            let bits = move_bits("torch.input.release", &keys)?;
            ctx.game
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .releases |= bits;
            Ok(())
        })?,
    )?;
    input.raw_set(
        "action",
        game_fn(lua, ctx, "torch.input.action", |_, _, name: String| {
            let Some(&(_, bit)) = ACTIONS.iter().find(|(n, _)| *n == name) else {
                let names: Vec<&str> = ACTIONS.iter().map(|(n, _)| *n).collect();
                return Err(Error::runtime(format!(
                    "torch.input.action: unknown action `{name}`{}; one of {}",
                    super::api::suggest(&name, &names),
                    names.join(", ")
                )));
            };
            Ok(INPUT.load(Relaxed) & bit != 0)
        })?,
    )?;
    input.raw_set(
        "down",
        game_fn(lua, ctx, "torch.input.down", |_, _, key: String| {
            let key = bind_name(&key, "torch.input.down")?;
            Ok(HELD
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains(&key))
        })?,
    )?;
    input.raw_set(
        "claimscroll",
        game_fn(
            lua,
            ctx,
            "torch.input.claimscroll",
            |_, ctx, key: Option<String>| {
                let what = "torch.input.claimscroll";
                let (k, _) = ctx.caller(what)?;
                let unbound = crate::gui::keybinds::Bound::Unbound.serialized_name();
                let key = key
                    .map(|key| bind_name(&key, what))
                    .transpose()?
                    .filter(|k| *k != unbound);
                let mut claims = CLAIMS.lock().unwrap_or_else(PoisonError::into_inner);
                claims.retain(|(o, _)| *o != k);
                if let Some(key) = key {
                    claims.push((k, key));
                }
                Ok(())
            },
        )?,
    )?;
    input.set_readonly(true);

    let hud = lua.create_table()?;
    hud.raw_set(
        "text",
        game_fn(
            lua,
            ctx,
            "torch.hud.text",
            |_,
             ctx,
             (x, y, text, c, shadow, scale, align): (
                f64,
                f64,
                mlua::String,
                Option<f64>,
                Option<bool>,
                Option<f64>,
                Option<mlua::String>,
            )| {
                let what = "torch.hud.text";
                let rgb = color(c, what)? & 0xFF_FFFF;
                let text = text.to_str()?;
                let text = text
                    .char_indices()
                    .nth(HUD_TEXT)
                    .map_or(&*text, |(cut, _)| &text[..cut]);
                let scale = scale.unwrap_or(1.0);
                if !TEXT_SCALE.contains(&scale) {
                    return Err(Error::runtime(format!(
                        "{what}: scale {scale} is outside {} to {}",
                        TEXT_SCALE.start(),
                        TEXT_SCALE.end()
                    )));
                }
                let align = match align.as_ref().map(|a| a.as_bytes()).as_deref() {
                    None | Some(b"left") => Align::Left,
                    Some(b"center") => Align::Center,
                    Some(b"right") => Align::Right,
                    Some(other) => {
                        let other = String::from_utf8_lossy(other);
                        return Err(Error::runtime(format!(
                            "{what}: align `{other}` is one of left, center, right"
                        )));
                    }
                };
                let cmd = Cmd::Text {
                    x,
                    y,
                    text: [0, 0],
                    rgb,
                    shadow: shadow.unwrap_or(true),
                    scale: scale as f32,
                    align,
                };
                draw_text(ctx, what, cmd, text)
            },
        )?,
    )?;
    hud.raw_set(
        "rect",
        game_fn(
            lua,
            ctx,
            "torch.hud.rect",
            |_, ctx, (x, y, w, h, c): (f64, f64, f64, f64, Option<f64>)| {
                let argb = color(c, "torch.hud.rect")?;
                draw(ctx, "torch.hud.rect", Cmd::Rect { x, y, w, h, argb })
            },
        )?,
    )?;
    hud.raw_set(
        "item",
        game_fn(
            lua,
            ctx,
            "torch.hud.item",
            |_, ctx, (x, y, id, count): (f64, f64, String, Option<f64>)| {
                let what = "torch.hud.item";
                let id = item_id(&id, what)?;
                let count = whole(count.unwrap_or(1.0), what)?.clamp(0, i64::from(u32::MAX)) as u32;
                draw(ctx, what, Cmd::Item { x, y, id, count })
            },
        )?,
    )?;
    hud.raw_set(
        "image",
        game_fn(
            lua,
            ctx,
            "torch.hud.image",
            |_,
             ctx,
             (img, x, y, w, h, u, v, uw, vh): (
                mlua::AnyUserData,
                f64,
                f64,
                f64,
                f64,
                Option<f64>,
                Option<f64>,
                Option<f64>,
                Option<f64>,
            )| {
                let what = "torch.hud.image";
                let Some((key, iw, ih)) = super::image::of(&img, what)? else {
                    return Ok(());
                };
                let (iw, ih) = (f64::from(iw), f64::from(ih));
                let (u, v) = (u.unwrap_or(0.0), v.unwrap_or(0.0));
                let (uw, vh) = (uw.unwrap_or(iw - u), vh.unwrap_or(ih - v));
                let finite = [x, y, w, h, u, v, uw, vh].iter().all(|n| n.is_finite());
                if !finite || w < 0.0 || h < 0.0 {
                    return Err(Error::runtime(format!(
                        "{what} takes finite numbers and a size of at least 0"
                    )));
                }
                let uv = [
                    (u / iw) as f32,
                    (v / ih) as f32,
                    ((u + uw) / iw) as f32,
                    ((v + vh) / ih) as f32,
                ];
                draw(
                    ctx,
                    what,
                    Cmd::Image {
                        x,
                        y,
                        w,
                        h,
                        key,
                        uv,
                    },
                )
            },
        )?,
    )?;
    hud.raw_set(
        "panel",
        game_fn(
            lua,
            ctx,
            "torch.hud.panel",
            |_, ctx, (name, f): (String, mlua::Function)| hud_panel(ctx, name, f),
        )?,
    )?;
    hud.set_readonly(true);

    let util = lua.create_table()?;
    util.raw_set(
        "uuid",
        lua.create_function(|_, s: String| {
            Ok(uuid::Uuid::parse_str(s.trim())
                .ok()
                .map(|u| uuid_text(u.as_u128())))
        })?,
    )?;
    util.raw_set(
        "uuid_version",
        lua.create_function(|_, s: String| {
            Ok(uuid::Uuid::parse_str(s.trim())
                .ok()
                .map(|u| u.get_version_num()))
        })?,
    )?;
    util.raw_set(
        "offline_uuid",
        lua.create_function(|_, name: String| {
            if name.is_empty() || name.chars().count() > OFFLINE_NAME {
                return Err(Error::runtime(format!(
                    "torch.util.offline_uuid takes a name of 1 to {OFFLINE_NAME} characters"
                )));
            }
            Ok(uuid_text(
                azalea_crypto::offline::generate_uuid(&name).as_u128(),
            ))
        })?,
    )?;
    util.set_readonly(true);

    let clipboard = lua.create_table()?;
    clipboard.raw_set(
        "set",
        game_fn(lua, ctx, "torch.clipboard.set", |_, _, text: String| {
            if text.len() > CLIPBOARD_MAX {
                return Err(Error::runtime(format!(
                    "torch.clipboard.set: {} bytes; at most {CLIPBOARD_MAX}",
                    text.len()
                )));
            }
            OUT.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .copy(text);
            Ok(())
        })?,
    )?;
    clipboard.set_readonly(true);

    let ui = lua.create_table()?;
    ui.raw_set(
        "list",
        game_fn(lua, ctx, "torch.ui.list", |_, ctx, spec: Table| {
            ui_list(ctx, spec)
        })?,
    )?;
    ui.raw_set(
        "prompt",
        game_fn(lua, ctx, "torch.ui.prompt", |_, ctx, spec: Table| {
            ui_prompt(ctx, spec)
        })?,
    )?;
    ui.raw_set(
        "close",
        game_fn(lua, ctx, "torch.ui.close", |_, ctx, ()| {
            let k = ctx.current.load(Relaxed);
            Ok(k != NONE && close_ui_of(k))
        })?,
    )?;
    ui.raw_set(
        "isopen",
        game_fn(lua, ctx, "torch.ui.isopen", |_, ctx, ()| {
            let k = ctx.current.load(Relaxed);
            Ok(UI
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref()
                .is_some_and(|l| l.owner == k))
        })?,
    )?;
    ui.raw_set(
        "canvas",
        game_fn(
            lua,
            ctx,
            "torch.ui.canvas",
            |_, ctx, spec: Option<Table>| ui_canvas(ctx, spec),
        )?,
    )?;
    ui.raw_set(
        "draw",
        game_fn(lua, ctx, "torch.ui.draw", |_, ctx, f: mlua::Function| {
            canvas_layer(ctx, f, Layer::Canvas)
        })?,
    )?;
    ui.raw_set(
        "overlay",
        game_fn(lua, ctx, "torch.ui.overlay", |_, ctx, f: mlua::Function| {
            canvas_layer(ctx, f, Layer::CanvasOverlay)
        })?,
    )?;
    ui.raw_set(
        "pin",
        game_fn(
            lua,
            ctx,
            "torch.ui.pin",
            |_, ctx, (x, y, f): (f64, f64, mlua::Function)| ui_pin(ctx, x, y, f),
        )?,
    )?;
    ui.raw_set(
        "mouse",
        game_fn(lua, ctx, "torch.ui.mouse", |lua, ctx, ()| {
            let (k, _) = ctx.caller("torch.ui.mouse")?;
            if !canvas_open(k) {
                return Ok(Value::Nil);
            }
            let c = *CAMERA.lock().unwrap_or_else(PoisonError::into_inner);
            let Some([sx, sy]) = c.mouse else {
                return Ok(Value::Nil);
            };
            let [x, y] = c.canvas_at(sx, sy);
            let t = lua.create_table()?;
            for (key, v) in [
                ("sx", f64::from(sx)),
                ("sy", f64::from(sy)),
                ("x", x),
                ("y", y),
            ] {
                t.raw_set(key, v)?;
            }
            Ok(Value::Table(t))
        })?,
    )?;
    ui.raw_set(
        "view",
        game_fn(lua, ctx, "torch.ui.view", |lua, ctx, ()| {
            let (k, _) = ctx.caller("torch.ui.view")?;
            if !canvas_open(k) {
                return Ok(Value::Nil);
            }
            let c = *CAMERA.lock().unwrap_or_else(PoisonError::into_inner);
            let [left, top] = c.canvas_at(0.0, 0.0);
            let [right, bottom] = c.canvas_at(c.vw, c.vh);
            let t = lua.create_table()?;
            for (key, v) in [
                ("x", c.x),
                ("y", c.y),
                ("scale", c.scale),
                ("left", left),
                ("top", top),
                ("right", right),
                ("bottom", bottom),
            ] {
                t.raw_set(key, v)?;
            }
            Ok(Value::Table(t))
        })?,
    )?;
    ui.set_readonly(true);

    let lookup = super::profile::table(lua, ctx)?;

    Ok(Tables {
        server,
        server_backing,
        world,
        inventory,
        inventory_backing,
        game,
        input,
        hud,
        util,
        clipboard,
        ui,
        profile: lookup,
    })
}

const OFFLINE_NAME: usize = 64;

const CLIPBOARD_MAX: usize = 32 * 1024;

pub(super) const UI_TEXT: usize = 256;

fn ui_list(ctx: &Ctx, spec: Table) -> mlua::Result<()> {
    let what = "torch.ui.list";
    let k = ctx.current.load(Relaxed);
    if k == NONE {
        return Err(Error::runtime(format!(
            "{what} works inside a module's handlers"
        )));
    }
    let text =
        |v: Option<String>| -> String { v.unwrap_or_default().chars().take(UI_TEXT).collect() };
    for pair in spec.pairs::<String, Value>() {
        let (key, _) = pair?;
        if !["title", "footer", "rows"].contains(&key.as_str()) {
            return Err(Error::runtime(format!(
                "{what}: takes {{ title = ..., footer = ..., rows = {{ ... }} }}; `{key}` is not one of them{}",
                super::api::suggest(&key, &["title", "footer", "rows"])
            )));
        }
    }
    let title = text(spec.raw_get("title")?);
    let footer = text(spec.raw_get("footer")?);
    let mut rows = Vec::new();
    if let Some(list) = spec.raw_get::<Option<Table>>("rows")? {
        for (i, v) in list.sequence_values::<Value>().enumerate() {
            if i >= UI_ROWS {
                return Err(Error::runtime(format!("{what}: more than {UI_ROWS} rows")));
            }
            rows.push(match v? {
                Value::String(s) => UiRow {
                    text: text(Some(s.to_string_lossy())),
                    rgb: 0xFF_FFFF,
                    detail: String::new(),
                    id: None,
                    item: None,
                    buttons: Vec::new(),
                },
                Value::Table(t) => UiRow {
                    text: text(t.raw_get("text")?),
                    rgb: color(t.raw_get("color")?, what)? & 0xFF_FFFF,
                    detail: text(t.raw_get("detail")?),
                    id: t.raw_get::<Option<String>>("id")?,
                    item: t.raw_get::<Option<String>>("item")?.map(|id| item_id(&id, what)).transpose()?,
                    buttons: row_buttons(&t, what)?,
                },
                other => {
                    return Err(Error::runtime(format!(
                        "{what}: row {} is a {}; a row is a string or {{ text = ..., detail = ..., color = ..., id = ... }}",
                        i + 1,
                        other.type_name()
                    )));
                }
            });
        }
    }
    show_ui(k, title, footer, rows, UiForm::Rows);
    Ok(())
}

fn row_buttons(t: &Table, what: &str) -> mlua::Result<Vec<String>> {
    let Some(list) = t.raw_get::<Option<Table>>("buttons")? else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for label in list.sequence_values::<String>() {
        if out.len() == UI_BUTTONS {
            return Err(Error::runtime(format!(
                "{what}: at most {UI_BUTTONS} buttons a row"
            )));
        }
        out.push(label?.chars().take(UI_BUTTON_TEXT).collect());
    }
    Ok(out)
}

fn ui_prompt(ctx: &Ctx, spec: Table) -> mlua::Result<()> {
    let what = "torch.ui.prompt";
    const KEYS: [&str; 3] = ["title", "default", "over"];
    let (k, _) = ctx.caller(what)?;
    for pair in spec.pairs::<String, Value>() {
        let (key, _) = pair?;
        if !KEYS.contains(&key.as_str()) {
            return Err(Error::runtime(format!(
                "{what}: takes {{ title = ..., default = ..., over = ... }}; `{key}` is not one of them{}",
                super::api::suggest(&key, &KEYS)
            )));
        }
    }
    let text =
        |v: Option<String>| -> String { v.unwrap_or_default().chars().take(UI_TEXT).collect() };
    let title = text(spec.raw_get("title")?);
    let default = text(spec.raw_get("default")?);
    if spec.raw_get::<Option<bool>>("over")?.unwrap_or(false) {
        let mut ui = UI.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(list) = ui
            .as_ref()
            .filter(|l| l.owner == k && matches!(l.form, UiForm::Canvas(_)))
        {
            *ui = Some(reshow_canvas(list, Some(OverPrompt { title, default })));
            return Ok(());
        }
    }
    show_ui(k, title, String::new(), Vec::new(), UiForm::Prompt(default));
    Ok(())
}

pub(super) fn time_table(lua: &Lua) -> mlua::Result<Table> {
    use crate::platform::time::{SystemTime, UNIX_EPOCH};
    let t = lua.create_table()?;
    t.raw_set(
        "now",
        lua.create_function(|_, ()| {
            Ok(SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0.0, |d| d.as_secs_f64()))
        })?,
    )?;
    t.set_readonly(true);
    Ok(t)
}

pub(super) fn canvas_open(k: usize) -> bool {
    UI.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .is_some_and(|l| l.owner == k && matches!(l.form, UiForm::Canvas(_)))
}

fn ui_canvas(ctx: &Ctx, spec: Option<Table>) -> mlua::Result<()> {
    let what = "torch.ui.canvas";
    const KEYS: [&str; 4] = ["title", "x", "y", "scale"];
    let (k, _) = ctx.caller(what)?;
    let mut title = String::new();
    let mut set: [Option<f64>; 3] = [None; 3];
    if let Some(spec) = &spec {
        for pair in spec.pairs::<String, Value>() {
            let (key, _) = pair?;
            if !KEYS.contains(&key.as_str()) {
                return Err(Error::runtime(format!(
                    "{what}: takes {{ title, x, y, scale }}; `{key}` is not one of them{}",
                    super::api::suggest(&key, &KEYS)
                )));
            }
        }
        title = spec
            .raw_get::<Option<String>>("title")?
            .unwrap_or_default()
            .chars()
            .take(UI_TEXT)
            .collect();
        for (slot, key) in set.iter_mut().zip(["x", "y", "scale"]) {
            if let Some(v) = spec.raw_get::<Option<f64>>(key)? {
                if !v.is_finite() {
                    return Err(Error::runtime(format!(
                        "{what}: `{key}` is a finite number"
                    )));
                }
                *slot = Some(v);
            }
        }
    }
    if let Some(s) = set[2]
        && !Camera::SCALE.contains(&s)
    {
        return Err(Error::runtime(format!(
            "{what}: scale {s} is outside {} to {}",
            Camera::SCALE.start(),
            Camera::SCALE.end()
        )));
    }
    let open = {
        let ui = UI.lock().unwrap_or_else(PoisonError::into_inner);
        ui.as_ref().and_then(|l| match &l.form {
            UiForm::Canvas(over) if l.owner == k => Some((l.title == title, over.clone())),
            _ => None,
        })
    };
    {
        let mut cam = CAMERA.lock().unwrap_or_else(PoisonError::into_inner);
        if open.is_none() {
            *cam = Camera {
                vw: cam.vw,
                vh: cam.vh,
                mouse: cam.mouse,
                ..Camera::DEFAULT
            };
        }
        if let Some(x) = set[0] {
            cam.x = x;
        }
        if let Some(y) = set[1] {
            cam.y = y;
        }
        if let Some(s) = set[2] {
            cam.scale = s;
        }
    }
    match open {
        Some((true, _)) => {}
        Some((false, over)) => show_ui(k, title, String::new(), Vec::new(), UiForm::Canvas(over)),
        None => show_ui(k, title, String::new(), Vec::new(), UiForm::Canvas(None)),
    }
    Ok(())
}

fn canvas_layer(ctx: &Ctx, f: mlua::Function, layer: Layer) -> mlua::Result<bool> {
    let what = if layer == Layer::Canvas {
        "torch.ui.draw"
    } else {
        "torch.ui.overlay"
    };
    {
        let mut game = ctx.game.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(k) = game.drawing else {
            return Err(Error::runtime(format!(
                "{what} only works inside onrender2d"
            )));
        };
        if game.panel.is_some() || game.canvas.is_some() {
            return Err(Error::runtime(format!(
                "{what} cannot run inside torch.hud.panel, torch.ui.draw or torch.ui.overlay"
            )));
        }
        if !canvas_open(k) {
            return Ok(false);
        }
        game.canvas = Some(layer);
    }
    let r = f.call::<()>(());
    ctx.game
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .canvas = None;
    r.map(|()| true)
}

fn ui_pin(ctx: &Ctx, x: f64, y: f64, f: mlua::Function) -> mlua::Result<()> {
    let what = "torch.ui.pin";
    if !(x.is_finite() && y.is_finite()) {
        return Err(Error::runtime(format!("{what}: x and y must be finite")));
    }
    {
        let mut game = ctx.game.lock().unwrap_or_else(PoisonError::into_inner);
        if game.canvas != Some(Layer::Canvas) {
            return Err(Error::runtime(format!(
                "{what} only works inside torch.ui.draw"
            )));
        }
        if game.pin {
            return Err(Error::runtime(format!(
                "{what} cannot run inside another torch.ui.pin"
            )));
        }
        game.pin = true;
    }
    let r = draw(ctx, what, Cmd::Pin { x, y }).map(|()| f.call::<()>(()));
    let mut game = ctx.game.lock().unwrap_or_else(PoisonError::into_inner);
    game.pin = false;
    let r = r?;
    let Game {
        hud,
        drawn,
        drawing,
        ..
    } = &mut *game;
    if let Some(cmds) = (*drawing)
        .and_then(|k| hud.get_mut(k))
        .map(|h| &mut h.now.cmds)
    {
        if matches!(cmds.last(), Some((_, Cmd::Pin { .. }))) {
            cmds.pop();
            drawn.1 -= 1;
        } else {
            cmds.push((Layer::Canvas, Cmd::Unpin));
        }
    }
    r
}

const QUERY_CAP: usize = 64;

const UI_BUTTONS: usize = 4;
const UI_BUTTON_TEXT: usize = 24;

pub(super) fn block_pos(
    x: f64,
    y: f64,
    z: f64,
    what: &str,
) -> mlua::Result<azalea_core::position::BlockPos> {
    Ok(azalea_core::position::BlockPos::new(
        whole(x.floor(), what)? as i32,
        whole(y.floor(), what)? as i32,
        whole(z.floor(), what)? as i32,
    ))
}

fn menu_slot(slot: f64, what: &str) -> mlua::Result<u16> {
    let slot = whole(slot, what)?;
    match slot {
        1..=9 => Ok(36 + slot as u16 - 1),
        10..=36 => Ok(slot as u16 - 1),
        _ => Err(Error::runtime(format!(
            "{what}: slots are 1 to 36 (1 to 9 the hotbar), not {slot}"
        ))),
    }
}

fn bind_name(key: &str, what: &str) -> mlua::Result<&'static str> {
    super::api::bound(key)
        .map(|b| b.serialized_name())
        .map_err(|e| Error::runtime(format!("{what}: {e}")))
}

const FACES: [(&str, azalea_core::direction::Direction); 6] = {
    use azalea_core::direction::Direction as D;
    [
        ("down", D::Down),
        ("up", D::Up),
        ("north", D::North),
        ("south", D::South),
        ("west", D::West),
        ("east", D::East),
    ]
};

fn place(ctx: &Ctx, spec: Table) -> mlua::Result<bool> {
    use azalea_core::hit_result::BlockHitResult;
    use azalea_core::position::Vec3;
    let what = "torch.game.place";
    const FIELDS: [&str; 8] = ["x", "y", "z", "face", "hit", "hand", "slot", "sneak"];
    for pair in spec.pairs::<String, Value>() {
        let (key, _) = pair?;
        if !FIELDS.contains(&key.as_str()) {
            return Err(Error::runtime(format!(
                "{what}: `{key}` is not one of {}{}",
                FIELDS.join(", "),
                super::api::suggest(&key, &FIELDS)
            )));
        }
    }
    let num = |t: &Table, k: &str| -> mlua::Result<f64> {
        t.raw_get::<Option<f64>>(k)?
            .filter(|v| v.is_finite())
            .ok_or_else(|| Error::runtime(format!("{what}: `{k}` is a finite number")))
    };
    let pos = block_pos(num(&spec, "x")?, num(&spec, "y")?, num(&spec, "z")?, what)?;
    let face: String = spec
        .raw_get::<Option<String>>("face")?
        .unwrap_or_else(|| "up".to_string());
    let Some(&(_, direction)) = FACES.iter().find(|(n, _)| *n == face) else {
        let names: Vec<&str> = FACES.iter().map(|(n, _)| *n).collect();
        return Err(Error::runtime(format!(
            "{what}: face `{face}` is not one of {}{}",
            names.join(", "),
            super::api::suggest(&face, &names)
        )));
    };
    let location = match spec.raw_get::<Option<Table>>("hit")? {
        Some(h) => Vec3::new(num(&h, "x")?, num(&h, "y")?, num(&h, "z")?),
        None => {
            let n = direction.normal();
            Vec3::new(
                pos.x as f64 + 0.5 + n.x as f64 * 0.5,
                pos.y as f64 + 0.5 + n.y as f64 * 0.5,
                pos.z as f64 + 0.5 + n.z as f64 * 0.5,
            )
        }
    };
    let offhand = match spec.raw_get::<Option<String>>("hand")?.as_deref() {
        None | Some("main") => false,
        Some("off") => true,
        Some(other) => {
            return Err(Error::runtime(format!(
                "{what}: hand is \"main\" or \"off\", not `{other}`"
            )));
        }
    };
    let slot = match spec.raw_get::<Option<f64>>("slot")? {
        None => None,
        Some(s) => {
            let s = whole(s, what)?;
            if !(1..=9).contains(&s) {
                return Err(Error::runtime(format!(
                    "{what}: slot is a hotbar slot, 1 to 9, not {s}"
                )));
            }
            Some(s as u8 - 1)
        }
    };
    let sneak = spec.raw_get::<Option<bool>>("sneak")?.unwrap_or(false);

    let bot = ctx.bot(what)?;
    let me = {
        let mut g = ctx.game.lock().unwrap_or_else(PoisonError::into_inner);
        if std::mem::replace(&mut g.placed, true) {
            return Ok(false);
        }
        g.snap.pos
    };
    let range = bot
        .component::<azalea::entity::Attributes>()
        .map_or(4.5, |a| a.block_interaction_range.calculate());
    let eye = [me[0], me[1] + 1.62, me[2]];
    let gap = |e: f64, lo: i32| (lo as f64 - e).max(e - (lo as f64 + 1.0)).max(0.0);
    let (dx, dy, dz) = (gap(eye[0], pos.x), gap(eye[1], pos.y), gap(eye[2], pos.z));
    if (dx * dx + dy * dy + dz * dz).sqrt() > range + 1.0 {
        return Ok(false);
    }
    let hit = BlockHitResult {
        location,
        miss: false,
        direction,
        block_pos: pos,
        inside: false,
        world_border: false,
    };
    crate::play::interaction::place_at(&bot, &hit, offhand, slot, sneak);
    bot.write_packet(azalea_protocol::packets::game::s_swing::ServerboundSwing {
        hand: if offhand {
            azalea_protocol::packets::game::s_interact::InteractionHand::OffHand
        } else {
            azalea_protocol::packets::game::s_interact::InteractionHand::MainHand
        },
    });
    OUT.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .act(|a| a.swing = Some(offhand));
    Ok(true)
}

fn profile(lua: &Lua, p: &Profile) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.raw_set("uuid", uuid_text(p.uuid))?;
    if !p.name.is_empty() {
        t.raw_set("name", p.name.as_str())?;
    }
    t.raw_set("listed", p.listed)?;
    t.raw_set("gamemode", gamemode_name(p.gamemode))?;
    if let Some(d) = &p.display {
        t.raw_set("displayname", d.as_str())?;
    }
    Ok(t)
}

fn waypoints(lua: &Lua, ctx: &Ctx) -> mlua::Result<Table> {
    let (list, profiles, me) = {
        let game = ctx.game.lock().unwrap_or_else(PoisonError::into_inner);
        (
            game.snap.waypoints.clone(),
            game.snap.profiles.clone(),
            game.snap.pos,
        )
    };
    let out = lua.create_table_with_capacity(list.len(), 0)?;
    for (i, wp) in list.iter().enumerate() {
        let t = lua.create_table()?;
        waypoint_fields(&t, &wp.id, Some(wp))?;
        let toward = match wp.pos {
            WaypointPos::Block([x, y, z]) => {
                let d = [
                    x as f64 + 0.5 - me[0],
                    y as f64 + 0.5 - me[1],
                    z as f64 + 0.5 - me[2],
                ];
                Some((d[0], d[2], (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()))
            }
            WaypointPos::Chunk { x, z } => {
                let (dx, dz) = (x as f64 * 16.0 + 8.0 - me[0], z as f64 * 16.0 + 8.0 - me[2]);
                Some((dx, dz, (dx * dx + dz * dz).sqrt()))
            }
            WaypointPos::Azimuth(_) => None,
        };
        if let Some((dx, dz, d)) = toward {
            t.raw_set("yaw", (-dx).atan2(dz).to_degrees())?;
            t.raw_set("distance", d)?;
        }
        if let WaypointKey::Uuid(u) = &wp.id
            && let Some(p) = profiles.iter().find(|p| p.uuid == *u)
        {
            if !p.name.is_empty() {
                t.raw_set("name", p.name.as_str())?;
            }
            t.raw_set("listed", p.listed)?;
        }
        out.raw_set(i + 1, t)?;
    }
    Ok(out)
}

fn draw(ctx: &Ctx, what: &str, cmd: Cmd) -> mlua::Result<()> {
    draw_text(ctx, what, cmd, "")
}

fn draw_text(ctx: &Ctx, what: &str, mut cmd: Cmd, text: &str) -> mlua::Result<()> {
    if let Cmd::Text { x, y, .. }
    | Cmd::Rect { x, y, .. }
    | Cmd::Item { x, y, .. }
    | Cmd::Image { x, y, .. } = &cmd
        && !(x.is_finite() && y.is_finite())
    {
        return Err(Error::runtime(format!("{what}: x and y must be finite")));
    }
    let mut game = ctx.game.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(k) = game.drawing else {
        return Err(Error::runtime(format!(
            "{what} only works inside onrender2d"
        )));
    };
    let layer = match (game.canvas, game.panel) {
        (Some(layer), _) => layer,
        (None, Some(slot)) => Layer::Panel(slot),
        (None, None) => Layer::Screen,
    };
    let canvas = game.canvas.is_some();
    let (cap, count) = if canvas {
        (CANVAS_CAP, game.drawn.1)
    } else {
        (HUD_CAP, game.drawn.0)
    };
    if count >= cap {
        return Err(Error::runtime(format!(
            "{what}: more than {cap} draws in one onrender2d"
        )));
    }
    if canvas {
        game.drawn.1 += 1;
    } else {
        game.drawn.0 += 1;
    }
    let drawn = &mut game.hud[k].now;
    if let Cmd::Text { text: span, .. } = &mut cmd {
        *span = [drawn.text.len() as u32, text.len() as u32];
        drawn.text.push_str(text);
    }
    drawn.cmds.push((layer, cmd));
    Ok(())
}

pub(super) const NO_SLOT: u8 = u8::MAX;

fn hud_panel(ctx: &Ctx, name: String, f: mlua::Function) -> mlua::Result<()> {
    let what = "torch.hud.panel";
    let slot = {
        let mut game = ctx.game.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(k) = game.drawing else {
            return Err(Error::runtime(format!(
                "{what} only works inside onrender2d"
            )));
        };
        if game.panel.is_some() {
            return Err(Error::runtime(format!(
                "{what} cannot run inside another torch.hud.panel"
            )));
        }
        if game.canvas.is_some() {
            return Err(Error::runtime(format!(
                "{what} cannot run inside torch.ui.draw or torch.ui.overlay"
            )));
        }
        let mine: Vec<&'static str> = game
            .panels
            .iter()
            .filter(|(o, ..)| *o == k)
            .map(|(_, n, _)| *n)
            .collect();
        let found = game
            .panels
            .iter()
            .find(|(o, n, _)| *o == k && *n == name)
            .map(|(.., s)| *s);
        match found {
            Some(NO_SLOT) => return Ok(()),
            Some(slot) => {
                game.panel = Some(slot);
                slot
            }
            None => {
                return Err(Error::runtime(format!(
                    "{what}: this module has no panel `{name}`; declare it in torch.module {{ panels = ... }}{}",
                    super::api::suggest(&name, &mine)
                )));
            }
        }
    };
    let r = f.call::<()>(());
    let mut game = ctx.game.lock().unwrap_or_else(PoisonError::into_inner);
    if game.panel == Some(slot) {
        game.panel = None;
    }
    r
}

fn entities(lua: &Lua, ctx: &Ctx, arg: Option<Table>) -> mlua::Result<Table> {
    let what = "torch.world.entities";
    let (mut radius, mut names): (f64, Vec<String>) = (16.0, Vec::new());
    if let Some(arg) = &arg {
        for pair in arg.pairs::<String, Value>() {
            let (k, v) = pair?;
            match (k.as_str(), v) {
                ("radius", Value::Integer(n)) => radius = n as f64,
                ("radius", Value::Number(n)) => radius = n,
                ("type", Value::String(s)) => names.push(s.to_string_lossy()),
                ("type", Value::Table(t)) => {
                    for s in t.sequence_values::<String>() {
                        names.push(s?);
                    }
                }
                (k, _) => {
                    return Err(Error::runtime(format!(
                        "{what}: takes {{ type = \"zombie\" or {{ ... }}, radius = 16 }}; `{k}` is not one of them{}",
                        super::api::suggest(k, &["type", "radius"])
                    )));
                }
            }
        }
    }
    if !(radius.is_finite() && radius > 0.0) {
        return Err(Error::runtime(format!("{what}: radius must be above 0")));
    }
    let radius = radius.min(128.0);
    let filtered = !names.is_empty();
    let kinds: Vec<azalea_registry::builtin::EntityKind> = (names.iter())
        .filter_map(|n| std::str::FromStr::from_str(n.strip_prefix("minecraft:").unwrap_or(n)).ok())
        .collect();
    let bot = ctx.bot(what)?;
    let me = ctx
        .game
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .snap
        .pos;
    let mut found: Vec<(f64, i32, _, [f64; 3], Option<f32>, Option<u128>)> = Vec::new();
    super::super::nearby::each(&bot, |seen| {
        let b = &seen.physics.bounding_box;
        let pos = [
            (b.min.x + b.max.x) * 0.5,
            b.min.y,
            (b.min.z + b.max.z) * 0.5,
        ];
        let d =
            ((pos[0] - me[0]).powi(2) + (pos[1] - me[1]).powi(2) + (pos[2] - me[2]).powi(2)).sqrt();
        if d > radius {
            return;
        }
        if filtered && !kinds.contains(&seen.kind) {
            return;
        }
        let uuid = seen
            .world
            .get::<azalea::entity::EntityUuid>(seen.entity)
            .map(|u| u.as_u128());
        found.push((d, seen.id, seen.kind, pos, seen.health, uuid));
    });
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.truncate(256);
    let names: Vec<Option<String>> = {
        let game = ctx.game.lock().unwrap_or_else(PoisonError::into_inner);
        (found.iter())
            .map(|f| {
                game.snap
                    .nearby
                    .iter()
                    .find(|(pid, _)| *pid == f.1)
                    .map(|(_, name)| name.clone())
            })
            .collect()
    };
    let out = lua.create_table()?;
    for (i, ((d, id, kind, pos, health, uuid), name)) in found.into_iter().zip(names).enumerate() {
        out.raw_set(
            i + 1,
            entity_row(lua, id, kind, pos, d, health, name, uuid)?,
        )?;
    }
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn entity_row(
    lua: &Lua,
    id: i32,
    kind: azalea_registry::builtin::EntityKind,
    pos: [f64; 3],
    distance: f64,
    health: Option<f32>,
    name: Option<String>,
    uuid: Option<u128>,
) -> mlua::Result<Table> {
    let e = lua.create_table()?;
    e.raw_set("id", id)?;
    e.raw_set("type", kind.to_str())?;
    e.raw_set("x", pos[0])?;
    e.raw_set("y", pos[1])?;
    e.raw_set("z", pos[2])?;
    e.raw_set("distance", distance)?;
    if let Some(h) = health {
        e.raw_set("health", h as f64)?;
    }
    if let Some(name) = name {
        e.raw_set("name", name)?;
    }
    if let Some(uuid) = uuid {
        e.raw_set("uuid", uuid_text(uuid))?;
    }
    Ok(e)
}

fn target(lua: &Lua, ctx: &Ctx) -> mlua::Result<Value> {
    use azalea_core::hit_result::HitResult;
    let what = "torch.world.target";
    let bot = ctx.bot(what)?;
    let Ok(hit) = bot.hit_result() else {
        return Ok(Value::Nil);
    };
    let point = |v: azalea_core::position::Vec3| -> mlua::Result<Table> {
        let p = lua.create_table()?;
        p.raw_set("x", v.x)?;
        p.raw_set("y", v.y)?;
        p.raw_set("z", v.z)?;
        Ok(p)
    };
    let t = lua.create_table()?;
    match hit {
        HitResult::Block(b) => {
            if b.miss {
                return Ok(Value::Nil);
            }
            let state = ctx
                .world(what)?
                .and_then(|w| w.read().get_block_state(b.block_pos));
            let block = state.map(|s| s.as_block_kind().to_str());
            let face = FACES
                .iter()
                .find(|(_, d)| *d == b.direction)
                .map_or("up", |(n, _)| n);
            t.raw_set("kind", "block")?;
            t.raw_set("block", block)?;
            t.raw_set("x", b.block_pos.x)?;
            t.raw_set("y", b.block_pos.y)?;
            t.raw_set("z", b.block_pos.z)?;
            t.raw_set("face", face)?;
            t.raw_set("hit", point(b.location)?)?;
            t.raw_set("inside", b.inside)?;
        }
        HitResult::Entity(e) => {
            use azalea::entity::{EntityKindComponent, EntityUuid, Position, metadata::Health};
            use azalea_core::entity_id::MinecraftEntityId;
            let id = bot
                .entity_component::<MinecraftEntityId>(e.entity)
                .ok()
                .map(|c| c.0);
            let kind = bot
                .entity_component::<EntityKindComponent>(e.entity)
                .ok()
                .map(|c| c.0);
            let pos = bot
                .entity_component::<Position>(e.entity)
                .ok()
                .map(|p| [p.x, p.y, p.z]);
            let (Some(id), Some(kind), Some(pos)) = (id, kind, pos) else {
                return Ok(Value::Nil);
            };
            let health = bot.entity_component::<Health>(e.entity).ok().map(|h| h.0);
            let uuid = bot
                .entity_component::<EntityUuid>(e.entity)
                .ok()
                .map(|u| u.as_u128());
            let (me, name) = {
                let game = ctx.game.lock().unwrap_or_else(PoisonError::into_inner);
                let name = game
                    .snap
                    .nearby
                    .iter()
                    .find(|(pid, _)| *pid == id)
                    .map(|(_, n)| n.clone());
                (game.snap.pos, name)
            };
            let d =
                ((pos[0] - me[0]).powi(2) + (pos[1] - me[1]).powi(2) + (pos[2] - me[2]).powi(2))
                    .sqrt();
            t.raw_set("kind", "entity")?;
            t.raw_set(
                "entity",
                entity_row(lua, id, kind, pos, d, health, name, uuid)?,
            )?;
            t.raw_set("hit", point(e.location)?)?;
        }
    }
    Ok(Value::Table(t))
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;

    #[test]
    fn pending_stays_small() {
        let entry = size_of::<(u64, super::Pending)>();
        let unboxed = [
            ("Packet", size_of::<super::super::packets::Packet>()),
            ("JobResult", size_of::<super::super::nbt::JobResult>()),
            ("Imported", size_of::<super::Imported>()),
        ];
        eprintln!("(u64, Pending) = {entry} bytes; payloads it boxes: {unboxed:?}");
        assert!(entry <= 96, "(u64, Pending) is {entry} bytes");
    }

    #[test]
    fn a_frame_groups_layers_and_rebases_text() {
        use super::{Align, Cmd, Drawn, HudFrame, Layer, ModHud};
        let text = |x: f64| Cmd::Text {
            x,
            y: 0.0,
            text: [0, 0],
            rgb: 0,
            shadow: false,
            scale: 1.0,
            align: Align::Left,
        };
        let rect = |x: f64| Cmd::Rect {
            x,
            y: 0.0,
            w: 1.0,
            h: 1.0,
            argb: 0,
        };
        let module = |items: &[(Layer, Cmd, &str)]| {
            let mut now = Drawn::default();
            for &(layer, mut cmd, s) in items {
                if let Cmd::Text { text, .. } = &mut cmd {
                    *text = [now.text.len() as u32, s.len() as u32];
                    now.text.push_str(s);
                }
                now.cmds.push((layer, cmd));
            }
            ModHud {
                now,
                before: Drawn::default(),
            }
        };
        let hud = [
            module(&[
                (Layer::Screen, text(1.0), "ab"),
                (Layer::Panel(1), rect(2.0), ""),
                (Layer::Screen, text(3.0), "c"),
            ]),
            module(&[
                (Layer::Canvas, rect(4.0), ""),
                (Layer::CanvasOverlay, rect(6.0), ""),
                (Layer::Screen, text(5.0), "de"),
            ]),
        ];
        let mut frame = HudFrame::default();
        frame.fill(&hud);
        let texts: Vec<(f64, &str)> = (frame.layer(Layer::Screen).iter())
            .map(|c| match c {
                Cmd::Text { x, text, .. } => (*x, frame.text(*text)),
                _ => panic!("a rect on the screen layer"),
            })
            .collect();
        assert_eq!(texts, [(1.0, "ab"), (3.0, "c"), (5.0, "de")]);
        assert!(matches!(frame.layer(Layer::Panel(1)), [Cmd::Rect { x, .. }] if *x == 2.0));
        assert!(frame.layer(Layer::Panel(0)).is_empty());
        assert!(matches!(frame.layer(Layer::Canvas), [Cmd::Rect { x, .. }] if *x == 4.0));
        assert!(matches!(frame.layer(Layer::CanvasOverlay), [Cmd::Rect { x, .. }] if *x == 6.0));
        frame.fill(&hud[1..]);
        assert_eq!(frame.layer(Layer::Screen).len(), 1);
        assert!(frame.layer(Layer::Panel(1)).is_empty());
    }
}
