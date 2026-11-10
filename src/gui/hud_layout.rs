#![cfg_attr(not(feature = "hud_editor"), allow(dead_code))]

use std::ops::Deref;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::{Mutex, PoisonError};

use bevy::math::Vec2;

use crate::gui::painter::Painter;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    pub fn contains(&self, m: Vec2) -> bool {
        m.x >= self.x && m.x < self.x + self.w && m.y >= self.y && m.y < self.y + self.h
    }

    pub fn intersect(&self, o: &Rect) -> Option<Rect> {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = (self.x + self.w).min(o.x + o.w);
        let y1 = (self.y + self.h).min(o.y + o.h);
        (x1 > x0 && y1 > y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
    }

    pub fn pos(&self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    pub fn size(&self) -> Vec2 {
        Vec2::new(self.w, self.h)
    }
}

macro_rules! elements {
    ($($(#[$m:meta])* $id:ident = $key:literal, $label:literal, $layer:ident;)*) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        #[repr(u8)]
        pub enum ElementId { $($(#[$m])* $id),* }

        pub const COUNT: usize = [$(ElementId::$id),*].len();

        impl ElementId {
            pub const ALL: [ElementId; COUNT] = [$(ElementId::$id),*];

            pub const fn index(self) -> usize {
                self as usize
            }

            pub const fn key(self) -> &'static str {
                match self { $(ElementId::$id => $key),* }
            }

            pub const fn label(self) -> &'static str {
                match self { $(ElementId::$id => $label),* }
            }

            pub const fn layer(self) -> Layer {
                match self { $(ElementId::$id => Layer::$layer),* }
            }
        }
    };
}

elements! {
    BossBars = "boss_bars", "Boss bars", Base;
    Effects = "effects", "Effects", Base;
    Hotbar = "hotbar", "Hotbar", Base;
    Chat = "chat", "Chat", Base;
    Sidebar = "sidebar", "Scoreboard", Base;
    Fps = "fps", "FPS", Extras;
    Coords = "coords", "Coordinates", Extras;
    Direction = "direction", "Direction", Extras;
    Gamemode = "gamemode", "Gamemode", Extras;
    Flying = "flying", "Flying", Extras;
    Status = "status", "Status", Extras;
    Ram = "ram", "RAM", Extras;
    Freecam = "freecam", "Freecam", Extras;
    Pad = "pad", "Movement", Controls;
    UseButton = "use_button", "Use button", Controls;
    JumpButton = "jump_button", "Jump button", Controls;
    ChatButton = "chat_button", "Chat button", Controls;
    ModButton = "mod_button", "Modules button", Controls;
    Debug = "debug", "Debug (F3)", Debug;
    ModPanel0 = "mod_panel_0", "Mod panel", Extras;
    ModPanel1 = "mod_panel_1", "Mod panel", Extras;
    ModPanel2 = "mod_panel_2", "Mod panel", Extras;
    ModPanel3 = "mod_panel_3", "Mod panel", Extras;
    ModPanel4 = "mod_panel_4", "Mod panel", Extras;
    ModPanel5 = "mod_panel_5", "Mod panel", Extras;
    ModPanel6 = "mod_panel_6", "Mod panel", Extras;
    ModPanel7 = "mod_panel_7", "Mod panel", Extras;
    ModPanel8 = "mod_panel_8", "Mod panel", Extras;
    ModPanel9 = "mod_panel_9", "Mod panel", Extras;
    ModPanel10 = "mod_panel_10", "Mod panel", Extras;
    ModPanel11 = "mod_panel_11", "Mod panel", Extras;
    ModPanel12 = "mod_panel_12", "Mod panel", Extras;
    ModPanel13 = "mod_panel_13", "Mod panel", Extras;
    ModPanel14 = "mod_panel_14", "Mod panel", Extras;
    ModPanel15 = "mod_panel_15", "Mod panel", Extras;
}

pub const MOD_PANELS: usize = 16;
const _: () =
    assert!(ElementId::ModPanel15 as usize + 1 == ElementId::ModPanel0 as usize + MOD_PANELS);

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ModPanel {
    pub key: &'static str,
    pub label: &'static str,
    pub w: f32,
    pub h: f32,
    pub anchor: Anchor,
    pub offset: Vec2,
}

static MOD_PANEL_DEFS: Mutex<[Option<ModPanel>; MOD_PANELS]> = Mutex::new([None; MOD_PANELS]);
static MOD_PANEL_GEN: AtomicU32 = AtomicU32::new(1);

#[cfg_attr(not(lua_mods), allow(dead_code))]
pub const LUA_FIRST: usize = crate::modules::hud_panels::COUNT;

pub fn set_mod_panels(panels: &[ModPanel]) -> usize {
    let mut defs = MOD_PANEL_DEFS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    *defs = [None; MOD_PANELS];
    for (slot, p) in defs[LUA_FIRST..].iter_mut().zip(panels) {
        *slot = Some(*p);
    }
    drop(defs);
    MOD_PANEL_GEN.fetch_add(1, Relaxed);
    panels.len().saturating_sub(MOD_PANELS - LUA_FIRST)
}

pub fn mod_panel(i: usize) -> Option<ModPanel> {
    crate::modules::hud_panels::def(i).or_else(|| {
        MOD_PANEL_DEFS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(i)
            .copied()
            .flatten()
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Layer {
    Base,
    #[default]
    Extras,
    #[cfg(feature = "mobile_ui")]
    Areas,
    Controls,
    Debug,
}

impl Layer {
    #[cfg(feature = "mobile_ui")]
    pub const ALL: [Layer; 5] = [
        Layer::Base,
        Layer::Extras,
        Layer::Areas,
        Layer::Controls,
        Layer::Debug,
    ];
    #[cfg(not(feature = "mobile_ui"))]
    pub const ALL: [Layer; 3] = [Layer::Base, Layer::Extras, Layer::Debug];

    pub const fn label(self) -> &'static str {
        match self {
            Layer::Base => "Base game",
            Layer::Extras => "Extras",
            #[cfg(feature = "mobile_ui")]
            Layer::Areas => "Areas",
            Layer::Controls => "Controls",
            Layer::Debug => "Debug",
        }
    }
}

const _: () = assert!(COUNT <= 64);

impl ElementId {
    pub const fn mod_panel_index(self) -> Option<usize> {
        let (i, first) = (self as usize, ElementId::ModPanel0 as usize);
        if i >= first && i < first + MOD_PANELS {
            Some(i - first)
        } else {
            None
        }
    }

    pub const fn mod_panel_slot(i: usize) -> ElementId {
        ElementId::ALL[ElementId::ModPanel0 as usize + i]
    }

    fn panel(self) -> Option<ModPanel> {
        self.mod_panel_index().and_then(mod_panel)
    }

    pub fn available(self) -> bool {
        self.mod_panel_index().is_none() || self.panel().is_some()
    }

    pub fn name(self) -> &'static str {
        self.panel().map_or(self.label(), |p| p.label)
    }

    pub const fn default_visibility(self) -> Visibility {
        if matches!(self, ElementId::Gamemode | ElementId::Flying) {
            return Visibility::Removed;
        }
        let hidden = if cfg!(feature = "mobile_ui") {
            matches!(self, ElementId::Coords | ElementId::Direction)
        } else {
            matches!(self, ElementId::Ram)
        };
        if hidden {
            Visibility::Hidden
        } else {
            Visibility::Shown
        }
    }

    pub const fn stackable(self) -> bool {
        (matches!(self.layer(), Layer::Extras) && self.mod_panel_index().is_none())
            || matches!(self, ElementId::Effects)
    }

    pub const fn required(self) -> bool {
        matches!(self.layer(), Layer::Controls)
    }

    #[cfg(feature = "mobile_ui")]
    pub const fn home(self) -> Option<(Home, u16)> {
        match self {
            ElementId::Flying => Some((Home::Left, 0)),
            ElementId::Status => Some((Home::Left, 1)),
            ElementId::Effects => Some((Home::Right, 0)),
            ElementId::Freecam => Some((Home::Right, 1)),
            ElementId::Direction => Some((Home::Corner, 0)),
            ElementId::Coords => Some((Home::Corner, 1)),
            _ => None,
        }
    }

    #[cfg(not(feature = "mobile_ui"))]
    pub const fn home(self) -> Option<(Home, u16)> {
        match self {
            ElementId::Coords => Some((Home::Left, 0)),
            ElementId::Direction => Some((Home::Left, 1)),
            ElementId::Flying => Some((Home::Left, 2)),
            ElementId::Status => Some((Home::Left, 3)),
            ElementId::Fps => Some((Home::Left, 4)),
            ElementId::Ram => Some((Home::Left, 5)),
            ElementId::Effects => Some((Home::Right, 0)),
            ElementId::Freecam => Some((Home::Right, 1)),
            _ => None,
        }
    }

    #[cfg(feature = "mobile_ui")]
    fn default_free(self, _vw: f32, vh: f32) -> Option<(Anchor, Vec2, f32)> {
        if let Some(p) = self.panel() {
            return Some((p.anchor, p.offset, 1.0));
        }
        match self {
            ElementId::Chat => {
                let (left, top) = mobile_chat_offset(vh);
                Some((Anchor::TopLeft, Vec2::new(left, top), MOBILE_CHAT_SCALE))
            }
            _ => None,
        }
    }

    #[cfg(not(feature = "mobile_ui"))]
    fn default_free(self, _vw: f32, _vh: f32) -> Option<(Anchor, Vec2, f32)> {
        self.panel().map(|p| (p.anchor, p.offset, 1.0))
    }

    const fn default_scale(self) -> f32 {
        #[cfg(feature = "mobile_ui")]
        if matches!(self, ElementId::Chat) {
            return MOBILE_CHAT_SCALE;
        }
        1.0
    }

    pub fn placeholder(self, vw: f32, vh: f32) -> Rect {
        if let Some(p) = self.panel() {
            return Rect::new(0.0, 0.0, p.w, p.h);
        }
        let center = (vw / 2.0).floor();
        match self {
            ElementId::Effects => Rect::new(vw - 50.0, 1.0, 49.0, 24.0),
            ElementId::BossBars => Rect::new(center - 91.0, 3.0, 182.0, 14.0),
            ElementId::Sidebar => Rect::new(vw - 62.0, (vh / 2.0).floor() - 30.0, 61.0, 60.0),
            ElementId::Debug => Rect::new(vw - 132.0, 2.0, 130.0, 80.0),
            ElementId::Hotbar => Rect::new(center - 91.0, vh.floor() - 49.0, 182.0, 49.0),
            #[cfg(not(feature = "mobile_ui"))]
            ElementId::Chat => Rect::new(0.0, vh.floor() - 40.0 - 90.0, 328.0, 90.0),
            #[cfg(feature = "mobile_ui")]
            ElementId::Chat => crate::gui::chat::hud_box(vh),
            ElementId::Pad => Rect::new(8.0, vh - 110.0, 90.0, 90.0),
            ElementId::UseButton => Rect::new(vw - 60.0, vh - 80.0, 50.0, 50.0),
            ElementId::JumpButton => Rect::new(vw - 100.0, vh - 60.0, 30.0, 30.0),
            ElementId::ChatButton => Rect::new(vw - 30.0, 8.0, 24.0, 24.0),
            ElementId::ModButton => Rect::new(vw - 60.0, 8.0, 24.0, 24.0),
            _ => Rect::new(2.0, 2.0, 60.0, 9.0),
        }
    }
}

#[cfg(feature = "mobile_ui")]
pub const MOBILE_CHAT_SCALE: f32 = 0.9;

#[cfg(feature = "mobile_ui")]
pub fn mobile_chat_offset(vh: f32) -> (f32, f32) {
    const CHAT_LIFT: f32 = 6.0;
    (0.0, crate::gui::chat::hud_box(vh).y - CHAT_LIFT)
}

#[derive(Clone, Copy)]
pub struct Transforms {
    xf: [Option<(f32, f32, f32)>; COUNT],
    drawn: u64,
}

impl Default for Transforms {
    fn default() -> Self {
        Transforms {
            xf: [None; COUNT],
            drawn: 0,
        }
    }
}

impl Transforms {
    #[cfg_attr(not(feature = "mobile_ui"), allow(dead_code))]
    pub const IDENTITY: Transforms = Transforms {
        xf: [None; COUNT],
        drawn: u64::MAX,
    };

    #[cfg_attr(not(feature = "mobile_ui"), allow(dead_code))]
    pub fn to_local(&self, id: ElementId, p: Vec2) -> Vec2 {
        match self.xf[id.index()] {
            Some((s, ox, oy)) if s > 0.0 => (p - Vec2::new(ox, oy)) / s,
            _ => p,
        }
    }

    #[cfg_attr(not(feature = "mobile_ui"), allow(dead_code))]
    pub fn drawn(&self, id: ElementId) -> bool {
        self.drawn & (1u64 << id.index()) != 0
    }

    fn record(&mut self, id: ElementId, xf: Option<(f32, f32, f32)>) {
        self.xf[id.index()] = xf;
        self.drawn |= 1u64 << id.index();
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl Anchor {
    const ALL: [Anchor; 9] = [
        Anchor::TopLeft,
        Anchor::Top,
        Anchor::TopRight,
        Anchor::Left,
        Anchor::Center,
        Anchor::Right,
        Anchor::BottomLeft,
        Anchor::Bottom,
        Anchor::BottomRight,
    ];

    pub fn col(self) -> usize {
        self as usize % 3
    }

    fn row(self) -> usize {
        self as usize / 3
    }

    fn frac(i: usize) -> f32 {
        i as f32 * 0.5
    }

    fn sign(i: usize) -> f32 {
        if i == 2 { -1.0 } else { 1.0 }
    }

    pub fn resolve(self, vw: f32, vh: f32, size: Vec2, offset: Vec2) -> Vec2 {
        let (c, r) = (self.col(), self.row());
        Vec2::new(
            Self::frac(c) * (vw - size.x) + Self::sign(c) * offset.x,
            Self::frac(r) * (vh - size.y) + Self::sign(r) * offset.y,
        )
    }

    pub fn offset_for(self, vw: f32, vh: f32, size: Vec2, pos: Vec2) -> Vec2 {
        let base = self.resolve(vw, vh, size, Vec2::ZERO);
        let (c, r) = (self.col(), self.row());
        Vec2::new(
            (pos.x - base.x) * Self::sign(c),
            (pos.y - base.y) * Self::sign(r),
        )
    }

    pub fn nearest(rect: Rect, vw: f32, vh: f32) -> Anchor {
        let third = |c: f32, extent: f32| {
            if c < extent / 3.0 {
                0
            } else if c > extent * 2.0 / 3.0 {
                2
            } else {
                1
            }
        };
        let c = third(rect.x + rect.w / 2.0, vw);
        let r = third(rect.y + rect.h / 2.0, vh);
        Anchor::ALL[r * 3 + c]
    }

    fn key(self) -> &'static str {
        match self {
            Anchor::TopLeft => "top_left",
            Anchor::Top => "top",
            Anchor::TopRight => "top_right",
            Anchor::Left => "left",
            Anchor::Center => "center",
            Anchor::Right => "right",
            Anchor::BottomLeft => "bottom_left",
            Anchor::Bottom => "bottom",
            Anchor::BottomRight => "bottom_right",
        }
    }

    fn from_key(key: &str) -> Option<Anchor> {
        Anchor::ALL.into_iter().find(|a| a.key() == key)
    }

    pub fn parse(key: &str) -> Option<Anchor> {
        Anchor::from_key(key)
    }

    pub fn names() -> [&'static str; 9] {
        Anchor::ALL.map(Anchor::key)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Visibility {
    Shown,
    Hidden,
    Removed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StackId(u32);

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Place {
    Fixed,
    Free {
        anchor: Anchor,
        offset: Vec2,
        scale: f32,
    },
    Stacked {
        stack: StackId,
        order: u16,
    },
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Element {
    pub visibility: Visibility,
    pub place: Place,
}

impl Element {
    pub fn toggle_hidden(&mut self) {
        self.visibility = match self.visibility {
            Visibility::Shown => Visibility::Hidden,
            Visibility::Hidden => Visibility::Shown,
            Visibility::Removed => Visibility::Removed,
        };
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Home {
    Left,
    Right,
    #[cfg_attr(not(feature = "mobile_ui"), allow(dead_code))]
    Corner,
}

#[cfg(feature = "mobile_ui")]
pub const HOMES: [Home; 3] = [Home::Left, Home::Right, Home::Corner];
#[cfg(not(feature = "mobile_ui"))]
pub const HOMES: [Home; 2] = [Home::Left, Home::Right];

impl Home {
    fn key(self) -> &'static str {
        match self {
            Home::Left => "left",
            Home::Right => "right",
            Home::Corner => "corner",
        }
    }

    fn from_key(key: &str) -> Option<Home> {
        HOMES.into_iter().find(|h| h.key() == key)
    }

    pub fn label(self) -> &'static str {
        match self {
            Home::Left => "Stack upwards (left)",
            Home::Right => "Stack upwards (right)",
            Home::Corner => "Stack upwards (corner)",
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Stack {
    pub id: StackId,
    pub anchor: Anchor,
    pub offset: Vec2,
    pub scale: f32,
    pub home: Option<Home>,
    pub moved: bool,
}

#[derive(Clone, Copy)]
pub struct Members {
    ids: [ElementId; COUNT],
    len: usize,
}

impl Members {
    fn remove(&mut self, id: ElementId) {
        if let Some(i) = self.iter().position(|m| *m == id) {
            self.ids.copy_within(i + 1..self.len, i);
            self.len -= 1;
        }
    }

    fn insert(&mut self, at: usize, id: ElementId) {
        let at = at.min(self.len);
        self.ids.copy_within(at..self.len, at + 1);
        self.ids[at] = id;
        self.len += 1;
    }
}

impl Deref for Members {
    type Target = [ElementId];

    fn deref(&self) -> &[ElementId] {
        &self.ids[..self.len]
    }
}

const INACTIVE_RGB: f32 = 0.5;
const INACTIVE_ALPHA: f32 = 0.25;

pub const MIN_SCALE: f32 = 0.5;
pub const MAX_SCALE: f32 = 3.0;

pub const GRIDS: [f32; 4] = [0.0, 4.0, 8.0, 16.0];
pub const DEFAULT_GRID: usize = 2;

#[derive(Clone, PartialEq, Debug)]
pub struct HudLayout {
    pub elements: [Element; COUNT],
    pub stacks: Vec<Stack>,
    next_stack: u32,
    pub grid: usize,
    #[cfg(feature = "mobile_ui")]
    pub areas: Vec<Area>,
    #[cfg(feature = "hud_editor")]
    pub panel: Option<Panel>,
    #[cfg(feature = "hud_editor")]
    pub hints: bool,
    mod_saved: Vec<(String, serde_json::Value)>,
    mod_keys: [Option<&'static str>; MOD_PANELS],
    mod_gen: u32,
}

#[cfg(feature = "hud_editor")]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Panel {
    pub centre: Vec2,
    pub width: f32,
}

#[cfg(feature = "mobile_ui")]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Area {
    pub anchor: Anchor,
    pub offset: Vec2,
    pub size: Vec2,
    pub zoom: bool,
}

#[cfg(feature = "mobile_ui")]
impl Area {
    pub const MIN: f32 = 12.0;

    pub fn at(r: Rect, vw: f32, vh: f32) -> Area {
        let mut area = Area {
            anchor: Anchor::TopLeft,
            offset: Vec2::ZERO,
            size: Vec2::ZERO,
            zoom: false,
        };
        area.place(r, vw, vh);
        area
    }

    pub fn place(&mut self, r: Rect, vw: f32, vh: f32) {
        self.anchor = Anchor::nearest(r, vw, vh);
        self.offset = self.anchor.offset_for(vw, vh, r.size(), r.pos());
        self.size = r.size();
    }

    pub fn rect(&self, vw: f32, vh: f32) -> Rect {
        let size = self.size.min(Vec2::new(vw, vh));
        let pos = self.anchor.resolve(vw, vh, size, self.offset);
        Rect::new(
            pos.x.min(vw - size.x).max(0.0),
            pos.y.min(vh - size.y).max(0.0),
            size.x,
            size.y,
        )
    }
}

#[cfg(feature = "mobile_ui")]
pub fn default_areas() -> Vec<Area> {
    let thumb = |anchor| Area {
        anchor,
        offset: Vec2::ZERO,
        size: Vec2::new(128.0, 180.0),
        zoom: false,
    };
    vec![
        thumb(Anchor::BottomLeft),
        thumb(Anchor::BottomRight),
        ZOOM_AREA,
    ]
}

#[cfg(feature = "mobile_ui")]
const ZOOM_AREA: Area = Area {
    anchor: Anchor::Center,
    offset: Vec2::ZERO,
    size: Vec2::new(40.0, 40.0),
    zoom: true,
};

impl Default for HudLayout {
    fn default() -> Self {
        let mut layout = HudLayout::bare();
        for home in HOMES {
            layout.push_home(home);
        }
        for id in ElementId::ALL {
            layout.reset(id);
        }
        layout
    }
}

impl HudLayout {
    fn bare() -> HudLayout {
        HudLayout {
            elements: ElementId::ALL.map(|id| Element {
                visibility: id.default_visibility(),
                place: Place::Fixed,
            }),
            stacks: Vec::new(),
            next_stack: 0,
            grid: DEFAULT_GRID,
            #[cfg(feature = "mobile_ui")]
            areas: default_areas(),
            #[cfg(feature = "hud_editor")]
            panel: None,
            #[cfg(feature = "hud_editor")]
            hints: true,
            mod_saved: Vec::new(),
            mod_keys: [None; MOD_PANELS],
            mod_gen: 0,
        }
    }

    pub fn sync_mod_panels(&mut self) {
        let generation = MOD_PANEL_GEN.load(Relaxed);
        if generation == self.mod_gen {
            return;
        }
        self.mod_gen = generation;
        for i in 0..MOD_PANELS {
            let id = ElementId::mod_panel_slot(i);
            let new = mod_panel(i).map(|p| p.key);
            let old = self.mod_keys[i];
            if new == old {
                continue;
            }
            if let Some(old) = old {
                let saved = self.element_json(id);
                self.keep_mod_entry(old, saved);
            }
            *self.get_mut(id) = Element {
                visibility: Visibility::Shown,
                place: Place::Fixed,
            };
            if let Some(key) = new
                && let Some((_, e)) = self.mod_saved.iter().find(|(k, _)| k == key)
            {
                let e = e.clone();
                self.apply_element_json(id, &e);
            }
            self.mod_keys[i] = new;
        }
    }

    fn keep_mod_entry(&mut self, key: &str, e: serde_json::Value) {
        match self.mod_saved.iter_mut().find(|(k, _)| k == key) {
            Some((_, old)) => *old = e,
            None => self.mod_saved.push((key.to_string(), e)),
        }
    }

    fn element_json(&self, id: ElementId) -> serde_json::Value {
        let element = self.get(id);
        let mut e = serde_json::Map::new();
        e.insert(
            "enabled".into(),
            (element.visibility == Visibility::Shown).into(),
        );
        e.insert(
            "removed".into(),
            (element.visibility == Visibility::Removed).into(),
        );
        if let Place::Free {
            anchor,
            offset,
            scale,
        } = element.place
        {
            e.insert("anchor".into(), anchor.key().into());
            e.insert("x".into(), offset.x.into());
            e.insert("y".into(), offset.y.into());
            e.insert("scale".into(), scale.into());
        }
        e.into()
    }

    fn apply_element_json(&mut self, id: ElementId, e: &serde_json::Value) {
        let num = |k: &str| e.get(k).and_then(|v| v.as_f64()).map(|v| v as f32);
        let element = self.get_mut(id);
        if let Some(enabled) = e.get("enabled").and_then(|v| v.as_bool()) {
            element.visibility = if enabled {
                Visibility::Shown
            } else {
                Visibility::Hidden
            };
        }
        let removed = e.get("removed").and_then(|v| v.as_bool());
        if removed.unwrap_or(id.default_visibility() == Visibility::Removed) {
            element.visibility = Visibility::Removed;
        }
        let anchor = e
            .get("anchor")
            .and_then(|v| v.as_str())
            .and_then(Anchor::from_key);
        if let (Some(anchor), Some(x), Some(y)) = (anchor, num("x"), num("y")) {
            element.place = Place::Free {
                anchor,
                offset: Vec2::new(x, y),
                scale: num("scale").unwrap_or(1.0).clamp(MIN_SCALE, MAX_SCALE),
            };
        }
    }

    pub fn get(&self, id: ElementId) -> &Element {
        &self.elements[id.index()]
    }

    pub fn get_mut(&mut self, id: ElementId) -> &mut Element {
        &mut self.elements[id.index()]
    }

    pub fn stack(&self, id: StackId) -> Option<&Stack> {
        self.stacks.iter().find(|s| s.id == id)
    }

    pub fn stack_mut(&mut self, id: StackId) -> Option<&mut Stack> {
        self.stacks.iter_mut().find(|s| s.id == id)
    }

    pub fn stack_of(&self, id: ElementId) -> Option<StackId> {
        match self.get(id).place {
            Place::Stacked { stack, .. } if self.stack(stack).is_some() => Some(stack),
            _ => None,
        }
    }

    pub fn members(&self, stack: StackId) -> Members {
        let mut sorted = [(0u16, ElementId::Coords); COUNT];
        let mut len = 0;
        for id in ElementId::ALL {
            if let Place::Stacked { stack: s, order } = self.get(id).place
                && s == stack
            {
                sorted[len] = (order, id);
                len += 1;
            }
        }
        sorted[..len].sort_unstable_by_key(|(order, _)| *order);
        let mut members = Members {
            ids: [ElementId::Coords; COUNT],
            len,
        };
        for (slot, (_, id)) in members.ids.iter_mut().zip(&sorted[..len]) {
            *slot = *id;
        }
        members
    }

    pub fn scale_of(&self, id: ElementId) -> f32 {
        match self.get(id).place {
            Place::Stacked { stack, .. } => self.stack(stack).map_or(1.0, |s| s.scale),
            Place::Free { scale, .. } => scale,
            Place::Fixed => id.default_scale(),
        }
    }

    fn home_stack(&self, home: Home) -> Option<StackId> {
        self.stacks
            .iter()
            .find(|s| s.home == Some(home))
            .map(|s| s.id)
    }

    pub fn removed_homes(&self) -> impl Iterator<Item = Home> + '_ {
        HOMES.into_iter().filter(|h| self.home_stack(*h).is_none())
    }

    fn push_stack(
        &mut self,
        anchor: Anchor,
        offset: Vec2,
        scale: f32,
        home: Option<Home>,
        moved: bool,
    ) -> StackId {
        let id = StackId(self.next_stack);
        self.next_stack += 1;
        self.stacks.push(Stack {
            id,
            anchor,
            offset,
            scale,
            home,
            moved,
        });
        id
    }

    fn push_home(&mut self, home: Home) -> StackId {
        let (anchor, offset) = home_default(home);
        self.push_stack(anchor, offset, 1.0, Some(home), false)
    }

    fn set_members(&mut self, stack: StackId, ids: &[ElementId]) {
        for (order, id) in ids.iter().enumerate() {
            self.get_mut(*id).place = Place::Stacked {
                stack,
                order: order as u16,
            };
        }
    }

    fn prune(&mut self) {
        let elements = &self.elements;
        self.stacks.retain(|s| {
            s.home.is_some()
                || elements
                    .iter()
                    .any(|e| matches!(e.place, Place::Stacked { stack, .. } if stack == s.id))
        });
    }

    pub fn reset(&mut self, id: ElementId) {
        match id.home() {
            Some((home, order)) => {
                let stack = self
                    .home_stack(home)
                    .unwrap_or_else(|| self.push_home(home));
                let mut members = self.members(stack);
                members.remove(id);
                let at = members
                    .iter()
                    .position(|m| matches!(m.home(), Some((h, o)) if h == home && o > order))
                    .unwrap_or(members.len());
                members.insert(at, id);
                self.set_members(stack, &members);
            }
            None => self.get_mut(id).place = Place::Fixed,
        }
        self.prune();
    }
}

fn home_default(home: Home) -> (Anchor, Vec2) {
    match home {
        Home::Left => (Anchor::TopLeft, Vec2::new(2.0, 2.0)),
        Home::Right => (Anchor::TopRight, Vec2::new(1.0, 1.0)),
        Home::Corner => (Anchor::BottomRight, Vec2::new(2.0, 60.0)),
    }
}

#[cfg(feature = "hud_editor")]
impl HudLayout {
    pub fn set_free(&mut self, id: ElementId, anchor: Anchor, offset: Vec2, scale: f32) {
        self.get_mut(id).place = Place::Free {
            anchor,
            offset,
            scale,
        };
        self.prune();
    }

    pub fn insert(&mut self, id: ElementId, stack: StackId, index: usize) {
        if !id.stackable() || self.stack(stack).is_none() {
            return;
        }
        let mut members = self.members(stack);
        let mut index = index;
        if let Some(old) = members.iter().position(|m| *m == id)
            && old < index
        {
            index -= 1;
        }
        members.remove(id);
        members.insert(index, id);
        self.set_members(stack, &members);
        self.prune();
    }

    pub fn new_stack(&mut self, anchor: Anchor, offset: Vec2, scale: f32, members: &[ElementId]) {
        let stack = self.push_stack(anchor, offset, scale, None, true);
        self.set_members(stack, members);
        self.prune();
    }

    pub fn split(&mut self, stack: StackId, rects: &[Option<Rect>; COUNT], vw: f32, vh: f32) {
        let Some(scale) = self.stack(stack).map(|s| s.scale) else {
            return;
        };
        for id in self.members(stack).iter().copied() {
            if let Some(r) = rects[id.index()] {
                let anchor = Anchor::nearest(r, vw, vh);
                let offset = anchor.offset_for(vw, vh, r.size(), r.pos());
                self.get_mut(id).place = Place::Free {
                    anchor,
                    offset,
                    scale,
                };
            }
        }
        self.prune();
    }

    pub fn delete_stack(
        &mut self,
        stack: StackId,
        rects: &[Option<Rect>; COUNT],
        vw: f32,
        vh: f32,
    ) {
        let Some(scale) = self.stack(stack).map(|s| s.scale) else {
            return;
        };
        for id in self.members(stack).iter().copied() {
            self.get_mut(id).place = match rects[id.index()] {
                Some(r) => {
                    let anchor = Anchor::nearest(r, vw, vh);
                    Place::Free {
                        anchor,
                        offset: anchor.offset_for(vw, vh, r.size(), r.pos()),
                        scale,
                    }
                }
                None => Place::Fixed,
            };
        }
        self.stacks.retain(|s| s.id != stack);
    }

    pub fn remove(&mut self, id: ElementId) {
        self.get_mut(id).visibility = Visibility::Removed;
    }

    pub fn restore(&mut self, id: ElementId) {
        self.get_mut(id).visibility = Visibility::Shown;
        self.reset(id);
    }

    pub fn restore_home(&mut self, home: Home) {
        if self.home_stack(home).is_none() {
            self.push_home(home);
        }
        for id in ElementId::ALL {
            if matches!(id.home(), Some((h, _)) if h == home)
                && self.get(id).visibility != Visibility::Removed
            {
                self.reset(id);
            }
        }
    }

    pub fn reset_layer(&mut self, layer: Layer) {
        #[cfg(feature = "mobile_ui")]
        if layer == Layer::Areas {
            self.areas = default_areas();
            return;
        }
        for id in ElementId::ALL.into_iter().filter(|id| id.layer() == layer) {
            self.get_mut(id).visibility = id.default_visibility();
            self.reset(id);
        }
        if layer == Layer::Extras {
            let homes: Vec<StackId> = self
                .stacks
                .iter()
                .filter(|s| s.home.is_some())
                .map(|s| s.id)
                .collect();
            for si in homes {
                self.reset_stack_position(si);
            }
        }
    }

    pub fn reset_stack_position(&mut self, stack: StackId) {
        let Some(s) = self.stack_mut(stack) else {
            return;
        };
        let Some(home) = s.home else {
            return;
        };
        (s.anchor, s.offset) = home_default(home);
        s.scale = 1.0;
        s.moved = false;
    }

    pub fn load() -> HudLayout {
        let Some(text) = crate::platform::storage::HUD.load() else {
            return HudLayout::default();
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
            return HudLayout::default();
        };
        let mut layout = HudLayout::bare();
        if let Some(grid) = json.get("grid").and_then(|g| g.as_u64()) {
            layout.grid = (grid as usize).min(GRIDS.len() - 1);
        }
        if let Some(hints) = json.get("hints").and_then(|h| h.as_bool()) {
            layout.hints = hints;
        }
        let num =
            |e: &serde_json::Value, k: &str| e.get(k).and_then(|v| v.as_f64()).map(|v| v as f32);
        let anchor_of = |e: &serde_json::Value| {
            e.get("anchor")
                .and_then(|v| v.as_str())
                .and_then(Anchor::from_key)
        };
        if let Some(elements) = json.get("elements").and_then(|e| e.as_object()) {
            for id in ElementId::ALL
                .into_iter()
                .filter(|id| id.mod_panel_index().is_none())
            {
                if let Some(e) = elements.get(id.key()) {
                    layout.apply_element_json(id, e);
                }
            }
            layout.mod_saved = elements
                .iter()
                .filter(|(k, _)| k.starts_with("mod:"))
                .map(|(k, e)| (k.clone(), e.clone()))
                .collect();
        }
        let has_stacks = if let Some(stacks) = json.get("stacks").and_then(|s| s.as_array()) {
            for s in stacks {
                let Some(anchor) = anchor_of(s) else {
                    continue;
                };
                let home = match s.get("home").and_then(|v| v.as_str()) {
                    Some(key) => match Home::from_key(key) {
                        Some(h) if layout.home_stack(h).is_none() => Some(h),
                        _ => continue,
                    },
                    None => None,
                };
                let offset = Vec2::new(num(s, "x").unwrap_or(0.0), num(s, "y").unwrap_or(0.0));
                let scale = num(s, "scale").unwrap_or(1.0).clamp(MIN_SCALE, MAX_SCALE);
                let moved = s.get("moved").and_then(|v| v.as_bool()).unwrap_or(true);
                let stack = layout.push_stack(anchor, offset, scale, home, moved);
                let mut members = layout.members(stack);
                let keys = s.get("members").and_then(|m| m.as_array());
                for id in keys
                    .into_iter()
                    .flatten()
                    .filter_map(|k| k.as_str().and_then(ElementId::from_key))
                {
                    if id.stackable()
                        && layout.get(id).place == Place::Fixed
                        && !members.contains(&id)
                    {
                        members.insert(members.len(), id);
                    }
                }
                layout.set_members(stack, &members);
            }
            true
        } else {
            false
        };
        layout.panel = json.get("panel").and_then(|p| {
            Some(Panel {
                centre: Vec2::new(num(p, "x")?, num(p, "y")?),
                width: num(p, "w")?,
            })
        });
        let deleted: Vec<Home> = json
            .get("removed_stacks")
            .and_then(|r| r.as_array())
            .into_iter()
            .flatten()
            .filter_map(|k| k.as_str().and_then(Home::from_key))
            .filter(|_| has_stacks)
            .collect();
        for home in HOMES {
            if layout.home_stack(home).is_none() && !deleted.contains(&home) {
                layout.push_home(home);
            }
        }
        for id in ElementId::ALL {
            let element = layout.get(id);
            if element.place == Place::Fixed
                && id.home().is_some()
                && element.visibility != Visibility::Removed
            {
                layout.reset(id);
            }
        }
        #[cfg(feature = "mobile_ui")]
        if let Some(areas) = json.get("areas").and_then(|a| a.as_array()) {
            layout.areas = areas
                .iter()
                .filter_map(|a| {
                    Some(Area {
                        anchor: anchor_of(a)?,
                        offset: Vec2::new(num(a, "x")?, num(a, "y")?),
                        size: Vec2::new(num(a, "w")?, num(a, "h")?).max(Vec2::splat(Area::MIN)),
                        zoom: a.get("zoom").and_then(|z| z.as_bool()).unwrap_or(false),
                    })
                })
                .collect();
            if !layout.areas.iter().any(|a| a.zoom) {
                layout.areas.push(ZOOM_AREA);
            }
        }
        layout.prune();
        layout
    }

    pub fn save(&self) {
        let mut elements = serde_json::Map::new();
        for id in ElementId::ALL {
            let key = match id.mod_panel_index() {
                Some(i) => match self.mod_keys[i] {
                    Some(key) => key,
                    None => continue,
                },
                None => id.key(),
            };
            elements.insert(key.into(), self.element_json(id));
        }
        for (key, e) in &self.mod_saved {
            if !elements.contains_key(key) {
                elements.insert(key.clone(), e.clone());
            }
        }
        let stacks: Vec<serde_json::Value> = self
            .stacks
            .iter()
            .map(|s| {
                serde_json::json!({
                    "home": s.home.map(Home::key),
                    "anchor": s.anchor.key(),
                    "x": s.offset.x,
                    "y": s.offset.y,
                    "scale": s.scale,
                    "members": self.members(s.id).iter().map(|m| m.key()).collect::<Vec<_>>(),
                    "moved": s.moved,
                })
            })
            .collect();
        let mut json = serde_json::json!({
            "version": 2,
            "grid": self.grid,
            "hints": self.hints,
            "elements": elements,
            "stacks": stacks,
            "removed_stacks": self.removed_homes().map(Home::key).collect::<Vec<_>>(),
        });
        if let Some(p) = self.panel {
            json["panel"] = serde_json::json!({ "x": p.centre.x, "y": p.centre.y, "w": p.width });
        }
        #[cfg(feature = "mobile_ui")]
        {
            let areas: Vec<serde_json::Value> = self
                .areas
                .iter()
                .map(|a| {
                    serde_json::json!({
                        "anchor": a.anchor.key(),
                        "x": a.offset.x,
                        "y": a.offset.y,
                        "w": a.size.x,
                        "h": a.size.y,
                        "zoom": a.zoom,
                    })
                })
                .collect();
            json["areas"] = areas.into();
        }
        crate::platform::storage::HUD.store(&json.to_string());
    }
}

#[cfg(feature = "hud_editor")]
impl ElementId {
    fn from_key(key: &str) -> Option<ElementId> {
        ElementId::ALL.into_iter().find(|id| id.key() == key)
    }
}

#[cfg(not(feature = "hud_editor"))]
#[derive(Default)]
pub struct HudState {
    pub layout: HudLayout,
    pub frames: Frames,
}

#[cfg(not(feature = "hud_editor"))]
impl HudState {
    pub fn load() -> Self {
        Self::default()
    }
}

#[cfg(feature = "hud_editor")]
pub use crate::gui::hud_editor::EditorState as HudState;

pub struct Frames {
    pub screen: [Option<Rect>; COUNT],
    pub stacks: Vec<(StackId, Rect)>,
    pub placeholder: [bool; COUNT],
    natural: [Option<Rect>; COUNT],
    pub indices: [Option<(usize, usize)>; COUNT],
    size: (f32, f32),
    pub transforms: Transforms,
    pub layer: Layer,
}

impl Default for Frames {
    fn default() -> Self {
        Frames {
            screen: [None; COUNT],
            stacks: Vec::new(),
            placeholder: [false; COUNT],
            natural: [None; COUNT],
            indices: [None; COUNT],
            size: (0.0, 0.0),
            transforms: Transforms::default(),
            layer: Layer::default(),
        }
    }
}

impl Frames {
    pub fn begin(&mut self, vw: f32, vh: f32) {
        self.screen = [None; COUNT];
        self.indices = [None; COUNT];
        self.placeholder = [false; COUNT];
        self.stacks.clear();
        self.transforms.drawn = 0;
        if self.size != (vw, vh) {
            self.size = (vw, vh);
            self.natural = [None; COUNT];
        }
    }

    pub fn stack_rect(&self, stack: StackId) -> Option<Rect> {
        self.stacks
            .iter()
            .find(|(s, _)| *s == stack)
            .map(|(_, r)| *r)
    }
}

pub fn snap_scale(scale: f32, device_scale: f32) -> f32 {
    if device_scale <= 0.0 {
        return scale;
    }
    (scale * device_scale).round().max(1.0) / device_scale
}

fn snap_px(v: f32, device_scale: f32) -> f32 {
    if device_scale <= 0.0 {
        return v.floor();
    }
    (v * device_scale).round() / device_scale
}

fn place_box(anchor: Anchor, offset: Vec2, size: Vec2, vw: f32, vh: f32, device: f32) -> Vec2 {
    let pos = anchor.resolve(vw, vh, size, offset);
    Vec2::new(
        snap_px(pos.x.clamp(0.0, (vw - size.x).max(0.0)), device),
        snap_px(pos.y.clamp(0.0, (vh - size.y).max(0.0)), device),
    )
}

fn bbox(p: &Painter, start: usize) -> Option<Rect> {
    let verts = p.positions.get(start..)?;
    let first = verts.first()?;
    let (mut x0, mut y0, mut x1, mut y1) = (first[0], first[1], first[0], first[1]);
    for v in &verts[1..] {
        x0 = x0.min(v[0]);
        y0 = y0.min(v[1]);
        x1 = x1.max(v[0]);
        y1 = y1.max(v[1]);
    }
    Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
}

fn truncate(p: &mut Painter, start: usize, index_start: usize) {
    p.positions.truncate(start);
    p.uvs.truncate(start);
    p.colors.truncate(start);
    p.indices.truncate(index_start);
}

fn draw_placeholder(p: &mut Painter, label: &str, r: Rect) {
    p.fill(r.x, r.y, r.w, r.h, 0x5000_0000);
    let mut x = r.x;
    while x < r.x + r.w {
        let w = 3.0f32.min(r.x + r.w - x);
        p.fill(x, r.y, w, 1.0, 0xA0FF_FFFF);
        p.fill(x, r.y + r.h - 1.0, w, 1.0, 0xA0FF_FFFF);
        x += 6.0;
    }
    let mut y = r.y;
    while y < r.y + r.h {
        let h = 3.0f32.min(r.y + r.h - y);
        p.fill(r.x, y, 1.0, h, 0xA0FF_FFFF);
        p.fill(r.x + r.w - 1.0, y, 1.0, h, 0xA0FF_FFFF);
        y += 6.0;
    }
    let w = p.atlas.font.width_str(label);
    p.text_plain(
        label,
        (r.x + (r.w - w) / 2.0).floor(),
        (r.y + (r.h - 8.0) / 2.0).floor(),
        0xFFFFFF,
        true,
    );
}

#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub align: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Line,
    Block,
    Follow,
}

pub struct Hud<'a> {
    layout: &'a HudLayout,
    frames: &'a mut Frames,
    editing: bool,
    device_scale: f32,
}

impl<'a> Hud<'a> {
    pub fn new(
        layout: &'a HudLayout,
        frames: &'a mut Frames,
        editing: bool,
        device_scale: f32,
    ) -> Self {
        Hud {
            layout,
            frames,
            editing,
            device_scale,
        }
    }

    pub fn shown(&self, id: ElementId) -> bool {
        if !id.available() {
            return false;
        }
        match self.layout.get(id).visibility {
            Visibility::Shown => true,
            Visibility::Hidden => self.editing && self.frames.layer == id.layer(),
            Visibility::Removed => false,
        }
    }

    pub fn editing(&self) -> bool {
        self.editing
    }

    pub fn place(&self, id: ElementId) -> Place {
        self.layout.get(id).place
    }

    pub fn natural_of(&self, id: ElementId, vw: f32, vh: f32) -> Rect {
        #[cfg(feature = "mobile_ui")]
        if id == ElementId::Chat {
            return id.placeholder(vw, vh);
        }
        self.frames.natural[id.index()].unwrap_or_else(|| id.placeholder(vw, vh))
    }

    pub fn attach(&mut self, child: ElementId, rect: Rect) {
        self.frames.screen[child.index()] = Some(rect);
    }

    pub fn arrange(
        &mut self,
        vw: f32,
        vh: f32,
        sizes: &[Option<Rect>; COUNT],
        home_at: impl Fn(Home) -> Option<(Anchor, Vec2)>,
        fixed_at: impl Fn(ElementId) -> Option<(Anchor, Vec2)>,
    ) -> [Option<Slot>; COUNT] {
        let device = self.device_scale;
        let mut slots = [None; COUNT];
        for stack in &self.layout.stacks {
            let (anchor, offset) = match stack.home {
                Some(home) if !stack.moved => home_at(home).unwrap_or((stack.anchor, stack.offset)),
                _ => (stack.anchor, stack.offset),
            };
            let members = self.layout.members(stack.id);
            let s = snap_scale(stack.scale, device);
            let (mut w, mut h) = (0.0f32, 0.0f32);
            for r in members.iter().filter_map(|id| sizes[id.index()]) {
                w = w.max(r.w);
                h += r.h;
            }
            if h <= 0.0 {
                continue;
            }
            let size = Vec2::new(w * s, h * s);
            let pos = place_box(anchor, offset, size, vw, vh, device);
            let mut y = pos.y;
            for id in members.iter() {
                let Some(r) = sizes[id.index()] else {
                    continue;
                };
                let mw = r.w * s;
                let x = match anchor.col() {
                    0 => pos.x,
                    1 => pos.x + (size.x - mw) / 2.0,
                    _ => pos.x + size.x - mw,
                };
                slots[id.index()] = Some(Slot {
                    x: snap_px(x, device),
                    y: snap_px(y, device),
                    scale: s,
                    align: anchor.col(),
                });
                y += r.h * s;
            }
            self.frames
                .stacks
                .push((stack.id, Rect::new(pos.x, pos.y, size.x, size.y)));
        }
        for id in ElementId::ALL {
            let Some(r) = sizes[id.index()] else {
                continue;
            };
            if slots[id.index()].is_some() {
                continue;
            }
            let free = match self.layout.get(id).place {
                Place::Free {
                    anchor,
                    offset,
                    scale,
                } => Some((anchor, offset, scale)),
                Place::Fixed => fixed_at(id).map(|(anchor, offset)| (anchor, offset, 1.0)),
                Place::Stacked { .. } => None,
            };
            if let Some((anchor, offset, scale)) = free {
                let s = snap_scale(scale, device);
                let pos = place_box(anchor, offset, r.size() * s, vw, vh, device);
                slots[id.index()] = Some(Slot {
                    x: pos.x,
                    y: pos.y,
                    scale: s,
                    align: anchor.col(),
                });
            }
        }
        slots
    }

    pub fn draw_at(
        &mut self,
        p: &mut Painter,
        id: ElementId,
        natural: Rect,
        slot: Slot,
        measured: bool,
        f: impl FnOnce(&mut Painter),
    ) {
        let kind = if measured { Kind::Block } else { Kind::Line };
        self.paint(p, id, natural, Some(slot), kind, None, f);
    }

    pub fn draw(
        &mut self,
        p: &mut Painter,
        vw: f32,
        vh: f32,
        id: ElementId,
        f: impl FnOnce(&mut Painter),
    ) {
        if !self.shown(id) {
            return;
        }
        let natural = self.natural_of(id, vw, vh);
        let slot = self.free_slot(id, natural, vw, vh);
        self.paint(p, id, natural, slot, Kind::Block, None, f);
    }

    #[cfg(feature = "hud_editor")]
    pub fn ghost(
        &mut self,
        p: &mut Painter,
        vw: f32,
        vh: f32,
        id: ElementId,
        label: &str,
        f: impl FnOnce(&mut Painter),
    ) {
        if !self.shown(id) || self.frames.layer != id.layer() {
            return;
        }
        let natural = self.natural_of(id, vw, vh);
        let slot = self.free_slot(id, natural, vw, vh);
        self.paint(p, id, natural, slot, Kind::Block, Some(label), f);
    }

    pub fn follow(
        &mut self,
        p: &mut Painter,
        vw: f32,
        vh: f32,
        id: ElementId,
        f: impl FnOnce(&mut Painter),
    ) {
        if self.layout.get(id).visibility != Visibility::Shown {
            return;
        }
        let natural = self.natural_of(id, vw, vh);
        let slot = self.free_slot(id, natural, vw, vh);
        self.paint(p, id, natural, slot, Kind::Follow, None, f);
    }

    fn free_slot(&self, id: ElementId, natural: Rect, vw: f32, vh: f32) -> Option<Slot> {
        let (anchor, offset, scale) = match self.layout.get(id).place {
            Place::Free {
                anchor,
                offset,
                scale,
            } => (anchor, offset, scale),
            Place::Fixed => id.default_free(vw, vh)?,
            Place::Stacked { .. } => return None,
        };
        let s = snap_scale(scale, self.device_scale);
        let pos = place_box(
            anchor,
            offset,
            natural.size() * s,
            vw,
            vh,
            self.device_scale,
        );
        Some(Slot {
            x: pos.x,
            y: pos.y,
            scale: s,
            align: anchor.col(),
        })
    }

    #[allow(clippy::too_many_arguments, reason = "one element's whole placement")]
    fn paint(
        &mut self,
        p: &mut Painter,
        id: ElementId,
        natural: Rect,
        slot: Option<Slot>,
        kind: Kind,
        ghost: Option<&str>,
        f: impl FnOnce(&mut Painter),
    ) {
        let preview = self.editing && kind != Kind::Follow;
        let active = self.frames.layer == id.layer();
        let label = ghost.unwrap_or(id.name());
        let start = p.positions.len();
        let index_start = p.indices.len();
        let xf = slot.map(|s| {
            (
                s.scale,
                s.x - s.scale * natural.x,
                s.y - s.scale * natural.y,
            )
        });
        let finish = |p: &mut Painter, (s, ox, oy): (f32, f32, f32)| -> bool {
            let empty = p.positions.len() == start;
            if ghost.is_some() && !empty {
                if let Some(b) = bbox(p, start) {
                    truncate(p, start, index_start);
                    let r = Rect::new((b.x - ox) / s, (b.y - oy) / s, b.w / s, b.h / s);
                    draw_placeholder(p, label, r);
                }
            } else if preview && empty && active {
                draw_placeholder(p, label, natural);
            }
            empty
        };
        let empty = match xf {
            Some((s, ox, oy)) => {
                let device = p.scale;
                p.scale = device * s;
                let empty = p.scaled(s, ox, oy, |p| {
                    f(p);
                    finish(p, (s, ox, oy))
                });
                p.scale = device;
                empty
            }
            None => {
                f(p);
                finish(p, (1.0, 0.0, 0.0))
            }
        };
        if self.editing && !active {
            for c in &mut p.colors[start..] {
                c[0] *= INACTIVE_RGB;
                c[1] *= INACTIVE_RGB;
                c[2] *= INACTIVE_RGB;
                c[3] *= INACTIVE_ALPHA;
            }
        }
        if kind == Kind::Follow {
            return;
        }
        self.frames.transforms.record(id, xf);
        if preview {
            self.frames.indices[id.index()] = Some((index_start, p.indices.len()));
            self.frames.placeholder[id.index()] = empty || ghost.is_some();
        }
        if let Some(slot) = slot {
            self.frames.screen[id.index()] = Some(Rect::new(
                slot.x,
                slot.y,
                natural.w * slot.scale,
                natural.h * slot.scale,
            ));
        }
        if kind != Kind::Block || (xf.is_none() && !self.editing) {
            return;
        }
        let Some(b) = bbox(p, start) else {
            return;
        };
        if !empty {
            let (s, ox, oy) = xf.unwrap_or((1.0, 0.0, 0.0));
            self.frames.natural[id.index()] =
                Some(Rect::new((b.x - ox) / s, (b.y - oy) / s, b.w / s, b.h / s));
        }
        if xf.is_none() {
            self.frames.screen[id.index()] = Some(b);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack_ids(l: &HudLayout) -> Vec<StackId> {
        l.stacks.iter().map(|s| s.id).collect()
    }

    #[test]
    fn default_puts_each_home_line_in_its_home_in_order() {
        let l = HudLayout::default();
        for home in HOMES {
            let stack = l.home_stack(home).expect("home exists");
            let orders: Vec<u16> = l
                .members(stack)
                .iter()
                .map(|id| id.home().expect("home line").1)
                .collect();
            assert!(
                orders.windows(2).all(|w| w[0] < w[1]),
                "{home:?}: {orders:?}"
            );
        }
    }

    #[test]
    fn a_stack_keeps_its_id_when_another_goes() {
        let mut l = HudLayout::default();
        let a = l.push_stack(Anchor::Center, Vec2::ZERO, 1.0, None, true);
        let b = l.push_stack(Anchor::Center, Vec2::ZERO, 1.0, None, true);
        l.set_members(a, &[ElementId::Fps]);
        l.set_members(b, &[ElementId::Ram]);
        l.reset(ElementId::Fps);
        assert!(l.stack(a).is_none(), "an emptied stack is pruned");
        assert_eq!(l.stack_of(ElementId::Ram), Some(b));
        assert!(stack_ids(&l).contains(&b));
    }

    #[test]
    fn members_insert_and_remove() {
        let l = HudLayout::default();
        let stack = l.home_stack(Home::Right).unwrap();
        let mut m = l.members(stack);
        let before = m.len();
        m.remove(ElementId::Effects);
        assert_eq!(m.len(), before - 1);
        m.insert(0, ElementId::Effects);
        assert_eq!(m[0], ElementId::Effects);
        assert_eq!(m.len(), before);
    }
}
