use azalea_inventory::operations::{
    ClickOperation, CloneClick, PickupAllClick, PickupClick, QuickCraftClick, QuickCraftKind,
    QuickCraftStatus, QuickMoveClick, SwapClick, ThrowClick,
};

use crate::gui::painter::{ITEM_SIZE, Painter};
use crate::gui::render::GuiInput;
use crate::gui::{ScreenCtx, Snapshot};
use crate::session::{CreativeAnswer, Gamemode, InvAction, SlotStack};

pub const INV_W: f32 = 176.0;
pub const INV_H: f32 = 166.0;

pub const CONTAINER_W: f32 = 176.0;

pub const MENU_SLOTS: usize = 46;

pub const PLAYER_SLOTS: usize = 36;

pub const SLOT_PITCH: f32 = 18.0;

const CAPPED_COUNT_COLOR: u32 = 0xFFFF55;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layout {
    PlayerMenu,
    Chest(u8),
    Crafting,
    Furnace,
    Hopper,
    ShulkerBox,
    Dispenser,
    Crafter(u16),
    Grindstone,
    Enchantment,
    Loom,
    Stonecutter,
    Cartography,
    Smithing,
    Beacon,
    BrewingStand,
    Merchant,
    Horse(u8),
    Anvil,
}

fn standard_inventory(k: usize, x: f32, y: f32) -> (f32, f32) {
    if k < 27 {
        (x + (k % 9) as f32 * 18.0, y + (k / 9) as f32 * 18.0)
    } else {
        (x + (k - 27) as f32 * 18.0, y + 58.0)
    }
}

#[derive(Clone, Copy, Debug)]
enum SlotRule {
    Any,
    Output,
    One,
    Only(&'static [&'static str]),
    OneOnly(&'static [&'static str]),
    FitsInContainer,
    Display,
}

impl SlotRule {
    fn may_place(self, item: &str) -> bool {
        match self {
            SlotRule::Any | SlotRule::One => true,
            SlotRule::Output | SlotRule::Display => false,
            SlotRule::Only(items) | SlotRule::OneOnly(items) => items.contains(&item),
            SlotRule::FitsInContainer => !item.ends_with("shulker_box"),
        }
    }

    fn one_only(self) -> bool {
        matches!(self, SlotRule::One | SlotRule::OneOnly(_))
    }
}

#[derive(Clone, Copy, Debug)]
struct SlotDef {
    pos: (f32, f32),
    icon: Option<&'static str>,
    rule: SlotRule,
}

const fn slot(x: f32, y: f32) -> SlotDef {
    SlotDef {
        pos: (x, y),
        icon: None,
        rule: SlotRule::Any,
    }
}

const fn output(x: f32, y: f32) -> SlotDef {
    SlotDef {
        pos: (x, y),
        icon: None,
        rule: SlotRule::Output,
    }
}

const fn special(x: f32, y: f32, icon: Option<&'static str>, rule: SlotRule) -> SlotDef {
    SlotDef {
        pos: (x, y),
        icon,
        rule,
    }
}

#[derive(Debug)]
struct MenuDef {
    size: (f32, f32),
    own: &'static [SlotDef],
    inv: (f32, f32),
    past_end: Option<(f32, f32)>,
    tail: &'static [SlotDef],
}

impl MenuDef {
    fn slot_count(&self) -> usize {
        self.own.len() + PLAYER_SLOTS + self.tail.len()
    }

    fn slot(&self, i: usize) -> Option<&SlotDef> {
        self.own.get(i).or_else(|| {
            i.checked_sub(self.own.len() + PLAYER_SLOTS)
                .and_then(|k| self.tail.get(k))
        })
    }

    fn slot_pos(&self, i: usize) -> (f32, f32) {
        if let Some(def) = self.slot(i) {
            return def.pos;
        }
        match self.past_end {
            Some(pos) if i >= self.slot_count() => pos,
            _ => standard_inventory(i - self.own.len(), self.inv.0, self.inv.1),
        }
    }

    fn rule(&self, i: usize) -> SlotRule {
        self.slot(i).map_or(SlotRule::Any, |def| def.rule)
    }
}

static CRAFTING: MenuDef = MenuDef {
    size: (INV_W, INV_H),
    own: &[
        output(124.0, 35.0),
        slot(30.0, 17.0),
        slot(48.0, 17.0),
        slot(66.0, 17.0),
        slot(30.0, 35.0),
        slot(48.0, 35.0),
        slot(66.0, 35.0),
        slot(30.0, 53.0),
        slot(48.0, 53.0),
        slot(66.0, 53.0),
    ],
    inv: (8.0, 84.0),
    past_end: Some((77.0, 62.0)),
    tail: &[],
};

static FURNACE: MenuDef = MenuDef {
    size: (INV_W, INV_H),
    own: &[slot(56.0, 17.0), slot(56.0, 53.0), output(116.0, 35.0)],
    inv: (8.0, 84.0),
    past_end: Some((77.0, 62.0)),
    tail: &[],
};

static HOPPER: MenuDef = MenuDef {
    size: (INV_W, 133.0),
    own: &[
        slot(44.0, 20.0),
        slot(62.0, 20.0),
        slot(80.0, 20.0),
        slot(98.0, 20.0),
        slot(116.0, 20.0),
    ],
    inv: (8.0, 51.0),
    past_end: None,
    tail: &[],
};

const fn grid<const N: usize>(x: f32, y: f32, cols: usize, rule: SlotRule) -> [SlotDef; N] {
    let mut out = [slot(0.0, 0.0); N];
    let mut i = 0;
    while i < N {
        out[i] = special(
            x + (i % cols) as f32 * 18.0,
            y + (i / cols) as f32 * 18.0,
            None,
            rule,
        );
        i += 1;
    }
    out
}

static SHULKER_BOX: MenuDef = MenuDef {
    size: (INV_W, 167.0),
    own: &grid::<27>(8.0, 18.0, 9, SlotRule::FitsInContainer),
    inv: (8.0, 84.0),
    past_end: None,
    tail: &[],
};

static DISPENSER: MenuDef = MenuDef {
    size: (INV_W, INV_H),
    own: &grid::<9>(62.0, 17.0, 3, SlotRule::Any),
    inv: (8.0, 84.0),
    past_end: None,
    tail: &[],
};

static CRAFTER: MenuDef = MenuDef {
    size: (INV_W, INV_H),
    own: &grid::<9>(26.0, 17.0, 3, SlotRule::Any),
    inv: (8.0, 84.0),
    past_end: None,
    tail: &[special(134.0, 35.0, None, SlotRule::Display)],
};

static GRINDSTONE: MenuDef = MenuDef {
    size: (INV_W, INV_H),
    own: &[slot(49.0, 19.0), slot(49.0, 40.0), output(129.0, 34.0)],
    inv: (8.0, 84.0),
    past_end: None,
    tail: &[],
};

static ENCHANTMENT: MenuDef = MenuDef {
    size: (INV_W, INV_H),
    own: &[
        special(15.0, 47.0, None, SlotRule::One),
        special(
            35.0,
            47.0,
            Some("container/slot/lapis_lazuli"),
            SlotRule::Only(&["lapis_lazuli"]),
        ),
    ],
    inv: (8.0, 84.0),
    past_end: None,
    tail: &[],
};

static LOOM: MenuDef = MenuDef {
    size: (INV_W, INV_H),
    own: &[
        special(13.0, 26.0, Some("container/slot/banner"), SlotRule::Any),
        special(33.0, 26.0, Some("container/slot/dye"), SlotRule::Any),
        special(
            23.0,
            45.0,
            Some("container/slot/banner_pattern"),
            SlotRule::Any,
        ),
        output(143.0, 57.0),
    ],
    inv: (8.0, 84.0),
    past_end: None,
    tail: &[],
};

static STONECUTTER: MenuDef = MenuDef {
    size: (INV_W, INV_H),
    own: &[slot(20.0, 33.0), output(143.0, 33.0)],
    inv: (8.0, 84.0),
    past_end: None,
    tail: &[],
};

static CARTOGRAPHY: MenuDef = MenuDef {
    size: (INV_W, INV_H),
    own: &[slot(15.0, 15.0), slot(15.0, 52.0), output(145.0, 39.0)],
    inv: (8.0, 84.0),
    past_end: None,
    tail: &[],
};

static SMITHING: MenuDef = MenuDef {
    size: (INV_W, INV_H),
    own: &[
        special(
            8.0,
            48.0,
            Some("container/slot/smithing_template_armor_trim"),
            SlotRule::Any,
        ),
        slot(26.0, 48.0),
        slot(44.0, 48.0),
        output(98.0, 48.0),
    ],
    inv: (8.0, 84.0),
    past_end: None,
    tail: &[],
};

static ANVIL: MenuDef = MenuDef {
    size: (INV_W, INV_H),
    own: &[slot(27.0, 47.0), slot(76.0, 47.0), output(134.0, 47.0)],
    inv: (8.0, 84.0),
    past_end: None,
    tail: &[],
};

static BEACON: MenuDef = MenuDef {
    size: (230.0, 219.0),
    own: &[special(
        136.0,
        110.0,
        None,
        SlotRule::OneOnly(&BEACON_PAYMENT_ITEMS),
    )],
    inv: (36.0, 137.0),
    past_end: None,
    tail: &[],
};

static BREWING_STAND: MenuDef = MenuDef {
    size: (INV_W, INV_H),
    own: &[
        special(
            56.0,
            51.0,
            Some("container/slot/potion"),
            SlotRule::OneOnly(&POTION_SLOT_ITEMS),
        ),
        special(
            79.0,
            58.0,
            Some("container/slot/potion"),
            SlotRule::OneOnly(&POTION_SLOT_ITEMS),
        ),
        special(
            102.0,
            51.0,
            Some("container/slot/potion"),
            SlotRule::OneOnly(&POTION_SLOT_ITEMS),
        ),
        slot(79.0, 17.0),
        special(
            17.0,
            17.0,
            Some("container/slot/brewing_fuel"),
            SlotRule::Only(&BREWING_FUEL),
        ),
    ],
    inv: (8.0, 84.0),
    past_end: None,
    tail: &[],
};

static MERCHANT: MenuDef = MenuDef {
    size: (276.0, INV_H),
    own: &[slot(136.0, 37.0), slot(162.0, 37.0), output(220.0, 37.0)],
    inv: (108.0, 84.0),
    past_end: None,
    tail: &[],
};

impl Layout {
    fn def(self) -> Option<&'static MenuDef> {
        Some(match self {
            Layout::PlayerMenu | Layout::Chest(_) | Layout::Horse(_) => return None,
            Layout::Crafting => &CRAFTING,
            Layout::Furnace => &FURNACE,
            Layout::Hopper => &HOPPER,
            Layout::ShulkerBox => &SHULKER_BOX,
            Layout::Dispenser => &DISPENSER,
            Layout::Crafter(_) => &CRAFTER,
            Layout::Grindstone => &GRINDSTONE,
            Layout::Enchantment => &ENCHANTMENT,
            Layout::Loom => &LOOM,
            Layout::Stonecutter => &STONECUTTER,
            Layout::Cartography => &CARTOGRAPHY,
            Layout::Smithing => &SMITHING,
            Layout::Anvil => &ANVIL,
            Layout::Beacon => &BEACON,
            Layout::BrewingStand => &BREWING_STAND,
            Layout::Merchant => &MERCHANT,
        })
    }

    pub fn size(self) -> (f32, f32) {
        if let Some(def) = self.def() {
            return def.size;
        }
        match self {
            Layout::Chest(rows) => (CONTAINER_W, 114.0 + rows as f32 * 18.0),
            _ => (INV_W, INV_H),
        }
    }

    pub fn slot_count(self) -> usize {
        if let Some(def) = self.def() {
            return def.slot_count();
        }
        match self {
            Layout::Chest(rows) => PLAYER_SLOTS + rows as usize * 9,
            Layout::Horse(columns) => PLAYER_SLOTS + 2 + columns as usize * 3,
            _ => MENU_SLOTS,
        }
    }

    pub fn slot_pos(self, i: usize) -> (f32, f32) {
        if let Some(def) = self.def() {
            return def.slot_pos(i);
        }
        match self {
            Layout::Horse(columns) => {
                let chest = columns as usize * 3;
                match i {
                    0 => (8.0, 18.0),
                    1 => (8.0, 36.0),
                    _ if i < 2 + chest => {
                        let k = i - 2;
                        (
                            80.0 + (k % columns as usize) as f32 * 18.0,
                            18.0 + (k / columns as usize) as f32 * 18.0,
                        )
                    }
                    _ => standard_inventory(i - 2 - chest, 8.0, 84.0),
                }
            }
            Layout::Chest(rows) => {
                let chest_slots = rows as usize * 9;
                if i < chest_slots {
                    return (8.0 + (i % 9) as f32 * 18.0, 18.0 + (i / 9) as f32 * 18.0);
                }
                standard_inventory(i - chest_slots, 8.0, 18.0 + rows as f32 * 18.0 + 13.0)
            }
            _ => match i {
                0 => (154.0, 28.0),
                1..=4 => {
                    let k = i - 1;
                    (98.0 + (k % 2) as f32 * 18.0, 18.0 + (k / 2) as f32 * 18.0)
                }
                5..=8 => (8.0, 8.0 + (i - 5) as f32 * 18.0),
                9..=44 => standard_inventory(i - 9, 8.0, 84.0),
                _ => (77.0, 62.0),
            },
        }
    }

    pub fn empty_icon(self, i: usize) -> Option<&'static str> {
        if let Some(def) = self.def() {
            return def.slot(i).and_then(|s| s.icon);
        }
        Some(match (self, i) {
            (Layout::PlayerMenu, 5) => "container/slot/helmet",
            (Layout::PlayerMenu, 6) => "container/slot/chestplate",
            (Layout::PlayerMenu, 7) => "container/slot/leggings",
            (Layout::PlayerMenu, 8) => "container/slot/boots",
            (Layout::PlayerMenu, OFFHAND_SLOT) => "container/slot/shield",
            (Layout::Horse(_), 0) => "container/slot/saddle",
            (Layout::Horse(_), 1) => "container/slot/horse_armor",
            _ => return None,
        })
    }

    pub fn may_place(self, slot: usize, item: &str) -> bool {
        if item.is_empty() || self.slot_disabled(slot) {
            return false;
        }
        if let Some(def) = self.def() {
            return def.rule(slot).may_place(item);
        }
        match self {
            Layout::PlayerMenu if slot == 0 => false,
            Layout::PlayerMenu if ARMOR_SLOTS.contains(&slot) => {
                armor_slot_for(item) == Some(8 - slot)
            }
            _ => true,
        }
    }

    fn is_output(self, slot: usize) -> bool {
        match self.def() {
            Some(def) => matches!(def.rule(slot), SlotRule::Output),
            None => self == Layout::PlayerMenu && slot == 0,
        }
    }

    pub fn may_pickup(self, slot: usize) -> bool {
        self.def()
            .is_none_or(|def| !matches!(def.rule(slot), SlotRule::Display))
    }

    pub fn slot_disabled(self, slot: usize) -> bool {
        matches!(self, Layout::Crafter(mask) if slot < 9 && mask >> slot & 1 == 1)
    }

    pub fn mode(self, gamemode: Gamemode) -> Mode {
        match (self, gamemode) {
            (Layout::PlayerMenu, Gamemode::Creative) => Mode::Local,
            _ => Mode::Protocol,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Protocol,
    Local,
}

#[derive(Default)]
pub struct SlotState {
    pub drag: Drag,
    pub last_click_slot: Option<usize>,
    pub cursor: Option<SlotStack>,
    cursor_base: SlotStack,
    cursor_age: u8,
    pub menu_preview: MenuPreview,
    bundle_hover: Option<(usize, i32)>,
    pub wheel_on_bundle: bool,
    bundle_click_pending: bool,
    stow_bundle_answer: bool,
    pub rest: Option<(f32, f32)>,
    #[cfg(feature = "mobile_ui")]
    pub touch_slots: Vec<[f32; 4]>,
    #[cfg(feature = "mobile_ui")]
    pub touch_lift: f32,
    #[cfg(feature = "mobile_ui")]
    pub tooltip_anchor: Option<(f32, f32)>,
}

impl SlotState {
    pub fn reset_for_screen_change(&mut self) {
        self.drag.clear();
        self.last_click_slot = None;
        self.cursor = None;
        self.cursor_age = 0;
        self.menu_preview = MenuPreview::default();
        self.bundle_hover = None;
        self.wheel_on_bundle = false;
        self.bundle_click_pending = false;
        self.rest = None;
    }

    pub fn stop_hovering(&mut self, slots: &[SlotStack]) -> Option<InvAction> {
        let (slot, wire_slot) = self.bundle_hover.take()?;
        slots.get(slot)?.bundle.as_ref()?;
        Some(InvAction::BundleSelect {
            slot: slot as u16,
            wire_slot,
            step: 0,
        })
    }

    pub fn land_bundle_click(
        &mut self,
        answer: CreativeAnswer,
        snap: &Snapshot,
        out: &mut Vec<InvAction>,
    ) {
        let pending = std::mem::take(&mut self.bundle_click_pending);
        if std::mem::take(&mut self.stow_bundle_answer) {
            if !answer.cursor.is_empty() {
                stow_creative_cursor(answer.cursor, &answer.slots, out);
            }
        } else if pending {
            self.set_cursor(answer.cursor, snap);
        }
    }

    pub fn stow_on_landing(&mut self) -> bool {
        self.stow_bundle_answer |= self.bundle_click_pending;
        self.bundle_click_pending
    }

    pub(crate) fn clicks_held(&self) -> bool {
        self.bundle_click_pending
    }

    pub(crate) fn note_bundle_click(&mut self, out: &[InvAction]) {
        if out
            .iter()
            .any(|a| matches!(a, InvAction::CreativeBundleClick { .. }))
        {
            self.bundle_click_pending = true;
            self.stow_bundle_answer = false;
        }
    }

    #[cfg(feature = "mobile_ui")]
    pub fn note_touch_slot(&mut self, x: f32, y: f32) {
        self.touch_slots
            .push([x - 1.0, y - 1.0, SLOT_PITCH, SLOT_PITCH]);
    }

    #[cfg(feature = "mobile_ui")]
    pub fn touch_slot_at(&self, m: bevy::math::Vec2) -> bool {
        let (mx, my) = (m.x, m.y + self.touch_lift);
        self.touch_slots
            .iter()
            .any(|&[x, y, w, h]| mx >= x && mx < x + w && my >= y && my < y + h)
    }

    pub fn take_cursor(&mut self) -> Option<SlotStack> {
        self.cursor.take().filter(|c| !c.is_empty())
    }

    pub fn cursor(&self, snap: &Snapshot) -> SlotStack {
        self.cursor.clone().unwrap_or_else(|| snap.carried.clone())
    }

    pub fn set_cursor(&mut self, cursor: SlotStack, snap: &Snapshot) {
        if cursor == snap.carried {
            self.cursor = None;
            return;
        }
        if self.cursor.as_ref() != Some(&cursor) {
            self.cursor_base = snap.carried.clone();
            self.cursor_age = 0;
        }
        self.cursor = Some(cursor);
    }

    fn retire_cursor(&mut self, snap: &Snapshot) {
        if self.cursor.is_none() {
            return;
        }
        self.cursor_age = self.cursor_age.saturating_add(1);
        if snap.carried != self.cursor_base || self.cursor_age >= COMMIT_MAX_FRAMES {
            self.cursor = None;
        }
    }

    pub fn display_cursor(&self, snap: &Snapshot) -> SlotStack {
        let mut cursor = self.cursor(snap);
        match self.drag.remainder {
            Some(0) => SlotStack::default(),
            Some(left) => {
                cursor.count = left;
                cursor
            }
            None => cursor,
        }
    }
}

#[derive(Default)]
pub struct Drag {
    pub kind: Option<DragKind>,
    pub slots: Vec<usize>,
    pub double: bool,
    pub remainder: Option<u8>,
    pub committed: bool,
    pub commit_carried: SlotStack,
    pub commit_age: u8,
}

const COMMIT_MAX_FRAMES: u8 = 40;

#[derive(Default)]
pub struct MenuPreview {
    slots: Option<Vec<SlotStack>>,
    base: Vec<SlotStack>,
    age: u8,
}

impl MenuPreview {
    fn set(&mut self, slots: Vec<SlotStack>, snap: &Snapshot) {
        self.base = (0..slots.len()).map(|i| snap.slot(i).clone()).collect();
        self.slots = Some(slots);
        self.age = 0;
    }

    fn slot(&self, i: usize) -> Option<&SlotStack> {
        self.slots.as_ref().and_then(|s| s.get(i))
    }
}

fn retire_menu_preview(preview: &mut MenuPreview, snap: &Snapshot, count: usize) {
    let Some(slots) = &preview.slots else { return };
    preview.age = preview.age.saturating_add(1);
    let landed = (0..count).all(|i| snap.slot(i) == &slots[i]);
    let moved = (0..count).any(|i| preview.base.get(i).is_some_and(|b| snap.slot(i) != b));
    if landed || moved || preview.age >= COMMIT_MAX_FRAMES {
        preview.slots = None;
    }
}

impl Drag {
    pub fn active(&self) -> bool {
        self.kind.is_some() && !self.committed
    }

    fn clear(&mut self) {
        self.kind = None;
        self.slots.clear();
        self.double = false;
        self.remainder = None;
        self.committed = false;
        self.commit_age = 0;
    }

    fn commit(&mut self, carried: SlotStack) {
        self.double = false;
        self.committed = true;
        self.commit_carried = carried;
        self.commit_age = 0;
    }

    fn recalculate_remainder(&mut self, layout: Layout, snap: &Snapshot, carried: &SlotStack) {
        let Some(kind) = self.kind else { return };
        if carried.is_empty() || self.slots.is_empty() {
            self.remainder = None;
            return;
        }
        let mut left = carried.count as i32;
        for &slot in &self.slots {
            let existing = snap.slot(slot).count;
            let cap = slot_capacity(layout, slot, carried.item);
            let taken = drag_slot_count(kind, self.slots.len(), carried, existing, cap);
            left -= i32::from(taken).saturating_sub(i32::from(existing)).max(0);
        }
        self.remainder = Some(left.max(0) as u8);
    }

    fn slot_preview(
        &self,
        layout: Layout,
        slot: usize,
        snap: &Snapshot,
        carried: &SlotStack,
    ) -> Option<(SlotStack, bool)> {
        let kind = self.kind?;
        if carried.is_empty() || !self.slots.contains(&slot) {
            return None;
        }
        let existing = snap.slot(slot).count;
        let cap = slot_capacity(layout, slot, carried.item);
        let want = kind
            .place_count(carried.count, self.slots.len())
            .saturating_add(existing);
        let mut preview = carried.clone();
        preview.count = want.min(cap);
        Some((preview, want > cap))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DragKind {
    Left,
    Right,
    Middle,
}

impl DragKind {
    fn azalea(self) -> QuickCraftKind {
        match self {
            DragKind::Left => QuickCraftKind::Left,
            DragKind::Right => QuickCraftKind::Right,
            DragKind::Middle => QuickCraftKind::Middle,
        }
    }

    fn place_count(self, carried: u8, slots: usize) -> u8 {
        match self {
            DragKind::Left => carried / slots.max(1) as u8,
            DragKind::Right => 1,
            DragKind::Middle => u8::MAX,
        }
    }
}

fn slot_capacity(layout: Layout, slot: usize, item: &str) -> u8 {
    let one_only = match layout.def() {
        Some(def) => def.rule(slot).one_only(),
        None => match layout {
            Layout::PlayerMenu => ARMOR_SLOTS.contains(&slot),
            Layout::Horse(_) => slot <= 1,
            _ => false,
        },
    };
    let container = if one_only { 1 } else { CONTAINER_MAX_STACK };
    container.min(item_max_stack(item))
}

fn drag_slot_count(kind: DragKind, slots: usize, carried: &SlotStack, existing: u8, cap: u8) -> u8 {
    kind.place_count(carried.count, slots)
        .saturating_add(existing)
        .min(cap)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlotClick {
    Pickup { right: bool },
    QuickMove { right: bool },
    Swap { button: u8 },
    Clone,
    Throw { all: bool },
    PickupAll,
}

pub fn resolve_slot_clicks(
    input: &GuiInput,
    carried_empty: bool,
    creative: bool,
    slot_has_item: bool,
) -> Vec<SlotClick> {
    let mut out = Vec::new();
    if input.middle_click && creative && carried_empty {
        out.push(SlotClick::Clone);
    } else if carried_empty {
        for right in [false, true] {
            let pressed = if right {
                input.right_click
            } else {
                input.left_click
            };
            if pressed {
                out.push(if input.shift {
                    SlotClick::QuickMove { right }
                } else {
                    SlotClick::Pickup { right }
                });
            }
        }
    }
    if carried_empty {
        for (i, pressed) in input.hotbar_keys.iter().enumerate() {
            if *pressed {
                out.push(SlotClick::Swap { button: i as u8 });
            }
        }
        if input.drop_key && slot_has_item {
            out.push(SlotClick::Throw { all: input.ctrl });
        }
        if input.swap_key {
            out.push(SlotClick::Swap { button: 40 });
        }
    }
    out
}

pub fn takes_slot(op: &ClickOperation, slot: u16) -> bool {
    match op {
        ClickOperation::Pickup(PickupClick::Left { slot: s } | PickupClick::Right { slot: s }) => {
            *s == Some(slot)
        }
        ClickOperation::QuickMove(
            QuickMoveClick::Left { slot: s } | QuickMoveClick::Right { slot: s },
        ) => *s == slot,
        ClickOperation::Throw(ThrowClick::Single { slot: s } | ThrowClick::All { slot: s }) => {
            *s == slot
        }
        ClickOperation::Swap(swap) => swap.source_slot == slot,
        _ => false,
    }
}

pub fn click_operation(click: SlotClick, slot: usize) -> ClickOperation {
    let s = slot as u16;
    match click {
        SlotClick::Pickup { right: false } => PickupClick::Left { slot: Some(s) }.into(),
        SlotClick::Pickup { right: true } => PickupClick::Right { slot: Some(s) }.into(),
        SlotClick::QuickMove { right: false } => QuickMoveClick::Left { slot: s }.into(),
        SlotClick::QuickMove { right: true } => QuickMoveClick::Right { slot: s }.into(),
        SlotClick::Swap { button } => SwapClick {
            source_slot: s,
            target_slot: button,
        }
        .into(),
        SlotClick::Clone => ClickOperation::Clone(CloneClick { slot: s }),
        SlotClick::Throw { all: true } => ThrowClick::All { slot: s }.into(),
        SlotClick::Throw { all: false } => ThrowClick::Single { slot: s }.into(),
        SlotClick::PickupAll => PickupAllClick {
            slot: s,
            reversed: false,
        }
        .into(),
    }
}

pub fn emit_outside_click(
    mode: Mode,
    right: bool,
    cursor: &mut SlotStack,
    out: &mut Vec<InvAction>,
) {
    if cursor.is_empty() {
        return;
    }
    if mode == Mode::Protocol {
        out.push(InvAction::Click(if right {
            PickupClick::RightOutside.into()
        } else {
            PickupClick::LeftOutside.into()
        }));
        return;
    }
    let count = if right { 1 } else { cursor.count };
    let mut stack = cursor.clone();
    stack.count = count;
    out.push(InvAction::CreativeDrop {
        stack: stack.into(),
    });
    cursor.count -= count;
    if cursor.count == 0 {
        *cursor = SlotStack::default();
    }
}

pub fn stow_creative_cursor(mut cursor: SlotStack, slots: &[SlotStack], out: &mut Vec<InvAction>) {
    if slots.len() >= MENU_SLOTS {
        let max = item_max_stack(cursor.item);
        for i in HOTBAR_SLOTS.chain(INV_SLOTS) {
            let existing = &slots[i];
            if !MenuModel::same_item(&cursor, existing) || existing.count >= max {
                continue;
            }
            let moved = (max - existing.count).min(cursor.count);
            let mut merged = existing.clone();
            merged.count += moved;
            out.push(InvAction::CreativeSet {
                slot: i as u16,
                stack: merged.into(),
            });
            cursor.count -= moved;
            if cursor.count == 0 {
                return;
            }
        }
        if let Some(i) = HOTBAR_SLOTS.chain(INV_SLOTS).find(|i| slots[*i].is_empty()) {
            out.push(InvAction::CreativeSet {
                slot: i as u16,
                stack: cursor.into(),
            });
            return;
        }
    }
    out.push(InvAction::CreativeDrop {
        stack: cursor.into(),
    });
}

const BEACON_PAYMENT_ITEMS: [&str; 5] = [
    "netherite_ingot",
    "emerald",
    "diamond",
    "gold_ingot",
    "iron_ingot",
];

const BREWING_FUEL: [&str; 1] = ["blaze_powder"];

const POTION_SLOT_ITEMS: [&str; 4] = [
    "potion",
    "splash_potion",
    "lingering_potion",
    "glass_bottle",
];

const CONTAINER_MAX_STACK: u8 = 99;

const ARMOR_SLOTS: std::ops::Range<usize> = 5..9;
const INV_SLOTS: std::ops::Range<usize> = 9..36;
const HOTBAR_SLOTS: std::ops::Range<usize> = 36..45;
const OFFHAND_SLOT: usize = 45;

fn armor_slot_for(id: &str) -> Option<usize> {
    Some(match id {
        _ if id.ends_with("_boots") => 0,
        _ if id.ends_with("_leggings") => 1,
        _ if id.ends_with("_chestplate") || id == "elytra" => 2,
        _ if id.ends_with("_helmet") || id == "carved_pumpkin" || id.ends_with("_head") => 3,
        _ => return None,
    })
}

fn is_offhand_item(id: &str) -> bool {
    id == "shield"
}

fn item_max_stack(id: &str) -> u8 {
    crate::generated_items::item(id)
        .map(|d| d.max_stack)
        .unwrap_or(64)
}

pub struct MenuModel {
    pub slots: Vec<SlotStack>,
    pub cursor: SlotStack,
    layout: Layout,
    pub picker: bool,
}

impl MenuModel {
    pub fn from_snapshot(snap: &Snapshot, cursor: SlotStack) -> MenuModel {
        MenuModel::for_layout(Layout::PlayerMenu, snap, cursor)
    }

    pub fn for_layout(layout: Layout, snap: &Snapshot, cursor: SlotStack) -> MenuModel {
        MenuModel {
            slots: (0..layout.slot_count())
                .map(|i| snap.slot(i).clone())
                .collect(),
            cursor,
            layout,
            picker: false,
        }
    }

    fn is_bundle_click(&self, click: SlotClick, slot: usize) -> bool {
        let SlotClick::Pickup { right } = click else {
            return false;
        };
        self.cursor.bundle.is_some()
            || (self.slots[slot].bundle.is_some() && (right || !self.cursor.is_empty()))
    }

    pub(crate) fn unselect_bundle(&self, click: SlotClick, slot: usize, out: &mut Vec<InvAction>) {
        if matches!(click, SlotClick::QuickMove { .. } | SlotClick::Swap { .. })
            && self.slots[slot].bundle.is_some()
        {
            let offset = if self.picker && HOTBAR_SLOTS.contains(&slot) {
                9
            } else {
                0
            };
            out.push(InvAction::BundleSelect {
                slot: slot as u16,
                wire_slot: (slot + offset) as i32,
                step: 0,
            });
        }
    }

    fn slot_max(&self, slot: usize, stack: &SlotStack) -> u8 {
        slot_capacity(self.layout, slot, stack.item)
    }

    fn safe_insert(&mut self, slot: usize, take: u8) {
        if self.cursor.is_empty() {
            return;
        }
        let room = self
            .slot_max(slot, &self.cursor.clone())
            .saturating_sub(self.slots[slot].count);
        let moved = take.min(self.cursor.count).min(room);
        if moved == 0 {
            return;
        }
        if self.slots[slot].is_empty() {
            let mut placed = self.cursor.clone();
            placed.count = moved;
            self.slots[slot] = placed;
        } else {
            self.slots[slot].count += moved;
        }
        self.cursor.count -= moved;
        if self.cursor.count == 0 {
            self.cursor = SlotStack::default();
        }
    }

    fn take(&mut self, slot: usize, count: u8) -> SlotStack {
        let mut taken = self.slots[slot].clone();
        if taken.is_empty() || count == 0 {
            return SlotStack::default();
        }
        let count = count.min(taken.count);
        taken.count = count;
        self.slots[slot].count -= count;
        if self.slots[slot].count == 0 {
            self.slots[slot] = SlotStack::default();
        }
        taken
    }

    fn same_item(a: &SlotStack, b: &SlotStack) -> bool {
        !a.is_empty()
            && !b.is_empty()
            && a.item == b.item
            && a.extra.potion == b.extra.potion
            && a.raw == b.raw
    }

    pub fn pickup(&mut self, slot: usize, right: bool) {
        if !self.layout.may_pickup(slot) {
            return;
        }
        if self.layout.is_output(slot) {
            self.take_output(slot);
            return;
        }
        let clicked = self.slots[slot].clone();
        let may_place = self.cursor.is_empty() || self.layout.may_place(slot, self.cursor.item);
        if clicked.is_empty() {
            if !self.cursor.is_empty() && may_place {
                let take = if right { 1 } else { self.cursor.count };
                self.safe_insert(slot, take);
            }
        } else if self.cursor.is_empty() {
            let take = if right {
                clicked.count.div_ceil(2)
            } else {
                clicked.count
            };
            self.cursor = self.take(slot, take);
        } else if Self::same_item(&self.cursor, &clicked) && may_place {
            let take = if right { 1 } else { self.cursor.count };
            self.safe_insert(slot, take);
        } else if may_place && self.cursor.count <= self.slot_max(slot, &self.cursor.clone()) {
            std::mem::swap(&mut self.cursor, &mut self.slots[slot]);
        }
    }

    fn take_output(&mut self, slot: usize) {
        let result = self.slots[slot].clone();
        if result.is_empty() {
            return;
        }
        if self.cursor.is_empty() {
            self.cursor = result;
        } else if Self::same_item(&self.cursor, &result)
            && self.cursor.count + result.count <= item_max_stack(result.item)
        {
            self.cursor.count += result.count;
        } else {
            return;
        }
        self.slots[slot] = SlotStack::default();
    }

    pub fn pickup_all(&mut self, slot: usize) {
        if self.cursor.is_empty() {
            return;
        }
        if !self.slots[slot].is_empty() {
            return;
        }
        let max = item_max_stack(self.cursor.item);
        for pass in 0..2 {
            for i in 0..self.slots.len() {
                if self.cursor.count >= max {
                    return;
                }
                if !Self::same_item(&self.cursor, &self.slots[i]) {
                    continue;
                }
                if pass == 0 && self.slots[i].count >= item_max_stack(self.slots[i].item) {
                    continue;
                }
                let room = max - self.cursor.count;
                let moved = room.min(self.slots[i].count);
                self.cursor.count += moved;
                self.slots[i].count -= moved;
                if self.slots[i].count == 0 {
                    self.slots[i] = SlotStack::default();
                }
            }
        }
    }

    pub fn quick_craft(&mut self, kind: DragKind, slots: &[usize]) {
        if self.cursor.is_empty() || slots.is_empty() {
            return;
        }
        if slots.len() == 1 {
            self.pickup(slots[0], kind == DragKind::Right);
            return;
        }
        let carried = self.cursor.clone();
        let mut remaining = carried.count as i32;
        for &slot in slots {
            let existing = self.slots[slot].clone();
            if !existing.is_empty() && !Self::same_item(&carried, &existing) {
                continue;
            }
            let cap = self.slot_max(slot, &carried);
            let new_count = drag_slot_count(kind, slots.len(), &carried, existing.count, cap);
            if new_count <= existing.count {
                continue;
            }
            let mut placed = carried.clone();
            placed.count = new_count;
            self.slots[slot] = placed;
            remaining -= (new_count - existing.count) as i32;
        }
        if remaining <= 0 {
            self.cursor = SlotStack::default();
        } else {
            self.cursor.count = remaining as u8;
        }
    }

    pub fn quick_move(&mut self, slot: usize) {
        while self.quick_move_stack(slot) {}
    }

    fn quick_move_stack(&mut self, slot: usize) -> bool {
        let stack = self.slots[slot].clone();
        if stack.is_empty() {
            return false;
        }
        let armor = armor_slot_for(stack.item).map(|k| 8 - k);
        if slot < 9 {
            self.move_stack_to(slot, 9, 45, slot == 0)
        } else if let Some(pos) = armor
            && self.slots[pos].is_empty()
        {
            self.move_stack_to(slot, pos, pos + 1, false)
        } else if is_offhand_item(stack.item) && self.slots[OFFHAND_SLOT].is_empty() {
            self.move_stack_to(slot, OFFHAND_SLOT, OFFHAND_SLOT + 1, false)
        } else if INV_SLOTS.contains(&slot) {
            self.move_stack_to(slot, 36, 45, false)
        } else if HOTBAR_SLOTS.contains(&slot) {
            self.move_stack_to(slot, 9, 36, false)
        } else {
            self.move_stack_to(slot, 9, 45, false)
        }
    }

    fn move_stack_to(&mut self, from: usize, start: usize, end: usize, backwards: bool) -> bool {
        let mut changed = false;
        let order: Vec<usize> = if backwards {
            (start..end).rev().collect()
        } else {
            (start..end).collect()
        };

        if item_max_stack(&self.slots[from].item) > 1 {
            for &dest in &order {
                if dest == from || self.slots[from].is_empty() {
                    continue;
                }
                let src = self.slots[from].clone();
                if !Self::same_item(&src, &self.slots[dest]) {
                    continue;
                }
                let room = self
                    .slot_max(dest, &src)
                    .saturating_sub(self.slots[dest].count);
                let moved = room.min(src.count);
                if moved > 0 {
                    self.slots[dest].count += moved;
                    self.slots[from].count -= moved;
                    if self.slots[from].count == 0 {
                        self.slots[from] = SlotStack::default();
                    }
                    changed = true;
                }
            }
        }

        for &dest in &order {
            if dest == from || self.slots[from].is_empty() {
                continue;
            }
            if !self.slots[dest].is_empty() {
                continue;
            }
            let src = self.slots[from].clone();
            let moved = self.slot_max(dest, &src).min(src.count);
            if moved == 0 {
                continue;
            }
            let mut placed = src;
            placed.count = moved;
            self.slots[dest] = placed;
            self.slots[from].count -= moved;
            if self.slots[from].count == 0 {
                self.slots[from] = SlotStack::default();
            }
            changed = true;
        }
        changed
    }

    pub fn swap(&mut self, slot: usize, button: u8) {
        let target = match button {
            0..=8 => 36 + button as usize,
            40 => OFFHAND_SLOT,
            _ => return,
        };
        if target == slot {
            return;
        }
        let source = self.slots[target].clone();
        let clicked = self.slots[slot].clone();
        if source.is_empty() && clicked.is_empty() {
            return;
        }
        if source.is_empty() {
            self.slots[target] = clicked;
            self.slots[slot] = SlotStack::default();
            return;
        }
        if !self.layout.may_place(slot, source.item) {
            return;
        }
        let max = self.slot_max(slot, &source);
        if source.count > max {
            if clicked.is_empty() {
                let moved = self.take(target, max);
                self.slots[slot] = moved;
            }
            return;
        }
        self.slots[target] = clicked;
        self.slots[slot] = source;
    }

    pub fn clone_slot(&mut self, slot: usize) {
        if !self.cursor.is_empty() || self.slots[slot].is_empty() {
            return;
        }
        let mut copy = self.slots[slot].clone();
        copy.count = item_max_stack(copy.item);
        self.cursor = copy;
    }

    pub fn throw(&mut self, slot: usize, all: bool) -> SlotStack {
        if !self.cursor.is_empty() {
            return SlotStack::default();
        }
        let count = if all { self.slots[slot].count } else { 1 };
        self.take(slot, count)
    }

    pub fn emit_creative_sets(&self, snap: &Snapshot, out: &mut Vec<InvAction>) {
        for i in 1..MENU_SLOTS {
            if self.slots[i] != *snap.slot(i) {
                out.push(InvAction::CreativeSet {
                    slot: i as u16,
                    stack: self.slots[i].clone().into(),
                });
            }
        }
    }
}

fn crafter_toggle(
    click: SlotClick,
    slot: usize,
    snap: &Snapshot,
    menu: &mut MenuModel,
    out: &mut Vec<InvAction>,
) {
    let Layout::Crafter(mask) = menu.layout else {
        return;
    };
    if slot >= 9 || !menu.slots[slot].is_empty() || snap.gamemode == Gamemode::Spectator {
        return;
    }
    let disabled = menu.layout.slot_disabled(slot);
    let enabled = match click {
        SlotClick::Pickup { .. } if disabled => true,
        SlotClick::Pickup { .. } if menu.cursor.is_empty() => false,
        SlotClick::Swap { button } if disabled => {
            let item = match button {
                0..=8 => snap.hotbar(button as usize),
                40 => snap.offhand(),
                _ => return,
            };
            if item.is_empty() {
                return;
            }
            true
        }
        _ => return,
    };
    out.push(InvAction::SlotStateChanged {
        slot: slot as u8,
        enabled,
    });
    let bit = 1 << slot;
    menu.layout = Layout::Crafter(if enabled { mask & !bit } else { mask | bit });
}

fn predicts(mode: Mode, layout: Layout, click: SlotClick, menu: &MenuModel, slot: usize) -> bool {
    !menu.is_bundle_click(click, slot)
        && (mode == Mode::Local
            || layout == Layout::PlayerMenu
            || matches!(click, SlotClick::Pickup { .. }))
}

pub fn apply_click(
    mode: Mode,
    layout: Layout,
    click: SlotClick,
    slot: usize,
    menu: &mut MenuModel,
    out: &mut Vec<InvAction>,
) {
    menu.unselect_bundle(click, slot, out);
    if mode == Mode::Protocol {
        out.push(InvAction::Click(click_operation(click, slot)));
    } else if let SlotClick::Pickup { right } = click
        && menu.is_bundle_click(click, slot)
    {
        out.push(InvAction::CreativeBundleClick {
            slot: slot as u16,
            right,
            cursor: menu.cursor.clone().into(),
        });
    }
    if !predicts(mode, layout, click, menu, slot) {
        return;
    }
    match click {
        SlotClick::Pickup { right } => menu.pickup(slot, right),
        SlotClick::QuickMove { .. } => menu.quick_move(slot),
        SlotClick::Swap { button } => menu.swap(slot, button),
        SlotClick::Clone => menu.clone_slot(slot),
        SlotClick::PickupAll => menu.pickup_all(slot),
        SlotClick::Throw { all } => {
            let dropped = menu.throw(slot, all);
            if mode == Mode::Local && !dropped.is_empty() {
                out.push(InvAction::CreativeDrop {
                    stack: dropped.into(),
                });
            }
        }
    }
}

pub fn slot_clicks(
    input: &GuiInput,
    mode: Mode,
    layout: Layout,
    slot: usize,
    snap: &Snapshot,
    cursor: &SlotStack,
    out: &mut Vec<InvAction>,
) -> (SlotStack, Option<Vec<SlotStack>>) {
    let clicks = resolve_slot_clicks(
        input,
        cursor.is_empty(),
        snap.is_creative(),
        !snap.slot(slot).is_empty(),
    );
    if clicks.is_empty() {
        return (cursor.clone(), None);
    }
    let mut menu = MenuModel::for_layout(layout, snap, cursor.clone());
    let predicted = clicks
        .iter()
        .all(|c| predicts(mode, layout, *c, &menu, slot));
    for click in clicks {
        crafter_toggle(click, slot, snap, &mut menu, out);
        apply_click(mode, layout, click, slot, &mut menu, out);
    }
    match mode {
        Mode::Local => {
            menu.emit_creative_sets(snap, out);
            (menu.cursor, None)
        }
        Mode::Protocol if predicted => (menu.cursor, Some(menu.slots)),
        Mode::Protocol => (cursor.clone(), None),
    }
}

pub fn panel(
    p: &mut Painter,
    layout: Layout,
    origin: (f32, f32),
    ctx: &ScreenCtx,
    snap: &Snapshot,
    state: &mut SlotState,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let (left, top) = origin;
    let mode = layout.mode(snap.gamemode);
    let count = snap.menu_slots.len().min(layout.slot_count());
    let hovered_index = (0..count).find(|i| {
        let (sx, sy) = layout.slot_pos(*i);
        ctx.hovering(left + sx - 1.0, top + sy - 1.0, SLOT_PITCH, SLOT_PITCH)
    });
    #[cfg(feature = "mobile_ui")]
    let hovered_index = hovered_index.or_else(|| {
        let at = (0..count).map(|i| {
            let (sx, sy) = layout.slot_pos(i);
            (i, (left + sx, top + sy))
        });
        nearest_slot(ctx, at)
    });
    if let Some(i) = hovered_index {
        let (sx, sy) = layout.slot_pos(i);
        state.rest = Some((left + sx, top + sy));
        #[cfg(feature = "mobile_ui")]
        {
            state.tooltip_anchor = state.rest;
        }
    }
    bundle_mouse(
        ctx.input,
        snap,
        hovered_index.map(|i| (i, i as i32)),
        state,
        out,
    );

    retire_committed_drag(&mut state.drag, snap);
    retire_menu_preview(&mut state.menu_preview, snap, count);
    if mode == Mode::Protocol {
        state.retire_cursor(snap);
    }

    let mut cursor = state.cursor(snap);

    let click_slot = hovered_index.filter(|_| !state.clicks_held());
    let drag_consumed_click = drag_step(ctx, snap, layout, click_slot, state, &mut cursor, out);

    if let Some(i) = hovered_index
        && layout.may_pickup(i)
    {
        let (sx, sy) = layout.slot_pos(i);
        p.slot_highlight_back(left + sx, top + sy);
    }

    for i in 0..count {
        let (sx, sy) = layout.slot_pos(i);
        #[cfg(feature = "mobile_ui")]
        state.note_touch_slot(left + sx, top + sy);
        draw_menu_slot(
            p,
            layout,
            i,
            snap,
            &state.drag,
            &cursor,
            state.menu_preview.slot(i),
            (left + sx, top + sy),
        );
    }

    let mut hovered = None;

    if let Some(i) = hovered_index {
        let (sx, sy) = layout.slot_pos(i);
        if layout.may_pickup(i) {
            p.slot_highlight_front(left + sx, top + sy);
        }
        let stack = snap.slot(i);
        if !stack.is_empty() {
            hovered = Some(stack.clone());
        }
        if !drag_consumed_click && click_slot.is_some() {
            let (new_cursor, preview) = slot_clicks(ctx.input, mode, layout, i, snap, &cursor, out);
            cursor = new_cursor;
            if let Some(slots) = preview {
                state.menu_preview.set(slots, snap);
            }
        }
    }

    let (w, h) = layout.size();
    let outside = !ctx.hovering(
        left - THROW_MARGIN,
        top - THROW_MARGIN,
        w + 2.0 * THROW_MARGIN,
        h + 2.0 * THROW_MARGIN,
    );
    if outside && !drag_consumed_click && !state.clicks_held() {
        for right in [false, true] {
            let released = if right {
                ctx.input.right_release
            } else {
                ctx.input.left_release
            };
            if released {
                emit_outside_click(mode, right, &mut cursor, out);
            }
        }
    }

    #[cfg(feature = "mobile_ui")]
    {
        if !outside {
            touch_release_place(
                ctx,
                snap,
                layout,
                mode,
                hovered_index,
                drag_consumed_click,
                state,
                &mut cursor,
                out,
            );
        }
        let target = if outside {
            DropTarget::World
        } else if let Some(i) = hovered_index {
            let (sx, sy) = layout.slot_pos(i);
            DropTarget::Slot((left + sx, top + sy), layout.may_place(i, cursor.item))
        } else {
            DropTarget::None
        };
        draw_drop_feedback(p, ctx, target, &cursor);
    }

    state.note_bundle_click(out);
    state.set_cursor(cursor, snap);
    hovered
}

pub(crate) fn bundle_mouse(
    input: &GuiInput,
    snap: &Snapshot,
    hovered: Option<(usize, i32)>,
    state: &mut SlotState,
    out: &mut Vec<InvAction>,
) {
    use azalea_inventory::bundle::number_of_items_to_show;

    if state.bundle_hover != hovered {
        out.extend(state.stop_hovering(&snap.menu_slots));
        state.bundle_hover = hovered;
    }
    let bundle = hovered.and_then(|(slot, _)| snap.slot(slot).bundle.as_ref());
    state.wheel_on_bundle = bundle.is_some_and(|b| number_of_items_to_show(b.items.len()) > 0);
    let wheel = if input.scroll != 0.0 {
        input.scroll
    } else {
        -input.scroll_x
    };
    if state.wheel_on_bundle
        && wheel != 0.0
        && let Some((slot, wire_slot)) = hovered
    {
        out.push(InvAction::BundleSelect {
            slot: slot as u16,
            wire_slot,
            step: wheel.signum() as i8,
        });
    }
}

#[cfg(feature = "mobile_ui")]
const SNAP_REACH: f32 = SLOT_PITCH / 2.0 + 4.0;

#[cfg(feature = "mobile_ui")]
pub(crate) fn nearest_slot(
    ctx: &ScreenCtx,
    slots: impl Iterator<Item = (usize, (f32, f32))>,
) -> Option<usize> {
    let m = ctx.mouse()?;
    slots
        .map(|(i, (x, y))| {
            let (dx, dy) = ((m.x - x - 8.0).abs(), (m.y - y - 8.0).abs());
            (i, dx.max(dy))
        })
        .filter(|(_, d)| *d <= SNAP_REACH)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

#[cfg(feature = "mobile_ui")]
pub(crate) const THROW_MARGIN: f32 = SLOT_PITCH / 2.0;
#[cfg(not(feature = "mobile_ui"))]
pub(crate) const THROW_MARGIN: f32 = 0.0;

fn retire_committed_drag(drag: &mut Drag, snap: &Snapshot) {
    if !drag.committed {
        return;
    }
    drag.commit_age = drag.commit_age.saturating_add(1);
    if snap.carried != drag.commit_carried || drag.commit_age >= COMMIT_MAX_FRAMES {
        drag.clear();
    }
}

pub(crate) fn drag_step(
    ctx: &ScreenCtx,
    snap: &Snapshot,
    layout: Layout,
    hovered: Option<usize>,
    state: &mut SlotState,
    cursor: &mut SlotStack,
    out: &mut Vec<InvAction>,
) -> bool {
    let input = ctx.input;
    let mode = layout.mode(snap.gamemode);
    let mut consumed = false;

    if !state.drag.active()
        && !cursor.is_empty()
        && let Some(slot) = hovered
    {
        let kind = if input.left_click {
            Some(DragKind::Left)
        } else if input.right_click {
            Some(DragKind::Right)
        } else if input.middle_click && snap.is_creative() {
            Some(DragKind::Middle)
        } else {
            None
        };
        if let Some(kind) = kind {
            consumed = true;
            state.drag.clear();
            state.drag.kind = Some(kind);
            state.drag.double =
                kind == DragKind::Left && input.double_click && state.last_click_slot == Some(slot);
        }
    }
    if input.left_click || input.right_click {
        state.last_click_slot = hovered;
    }

    #[cfg(feature = "mobile_ui")]
    if state.drag.active()
        && state.drag.kind == Some(DragKind::Left)
        && state.drag.slots.first().copied() != hovered
    {
        state.drag.slots.clear();
        state.drag.remainder = None;
    }
    if state.drag.active()
        && let Some(slot) = hovered
        && !state.drag.slots.contains(&slot)
    {
        let kind = state.drag.kind.expect("checked active");
        let room = kind == DragKind::Middle || (cursor.count as usize) > state.drag.slots.len();
        let target = snap.slot(slot);
        let replaceable = target.is_empty()
            || (target.item == cursor.item
                && target.extra.potion == cursor.extra.potion
                && target.count < slot_capacity(layout, slot, cursor.item));
        if room && replaceable && layout.may_place(slot, cursor.item) {
            state.drag.slots.push(slot);
            state.drag.recalculate_remainder(layout, snap, cursor);
        }
    }

    let aborted = match state.drag.kind {
        Some(DragKind::Left) => input.right_click,
        Some(DragKind::Right) => input.left_click,
        Some(DragKind::Middle) => input.left_click || input.right_click,
        None => false,
    };
    if aborted {
        state.drag.clear();
        return consumed;
    }

    if !state.drag.active() {
        return consumed;
    }
    let released = match state.drag.kind {
        Some(DragKind::Left) => !input.left_down,
        Some(DragKind::Right) => !input.right_down,
        Some(DragKind::Middle) => !input.middle_down,
        None => return consumed,
    };
    if !released {
        return consumed;
    }

    let kind = state.drag.kind.expect("checked active");
    let slots = state.drag.slots.clone();
    let double = state.drag.double;
    match mode {
        Mode::Local => state.drag.clear(),
        Mode::Protocol => state.drag.commit(snap.carried.clone()),
    }

    if double && let Some(&slot) = slots.first() {
        *cursor = apply_one(
            mode,
            layout,
            SlotClick::PickupAll,
            slot,
            snap,
            cursor,
            state,
            out,
        );
        return true;
    }

    if cursor.is_empty() {
        return consumed;
    }
    if slots.len() < 2 {
        if kind == DragKind::Middle {
            return true;
        }
        let click = SlotClick::Pickup {
            right: kind == DragKind::Right,
        };
        if let Some(slot) = slots.first().copied().or(hovered) {
            *cursor = apply_one(mode, layout, click, slot, snap, cursor, state, out);
        }
        return true;
    }

    match mode {
        Mode::Protocol => {
            let az = kind.azalea();
            let mut push = |status| {
                out.push(InvAction::Click(ClickOperation::QuickCraft(
                    QuickCraftClick {
                        kind: az.clone(),
                        status,
                    },
                )));
            };
            push(QuickCraftStatus::Start);
            for &slot in &slots {
                push(QuickCraftStatus::Add { slot: slot as u16 });
            }
            push(QuickCraftStatus::End);
        }
        Mode::Local => {
            let mut menu = MenuModel::from_snapshot(snap, cursor.clone());
            menu.quick_craft(kind, &slots);
            menu.emit_creative_sets(snap, out);
            *cursor = menu.cursor;
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn apply_one(
    mode: Mode,
    layout: Layout,
    click: SlotClick,
    slot: usize,
    snap: &Snapshot,
    cursor: &SlotStack,
    state: &mut SlotState,
    out: &mut Vec<InvAction>,
) -> SlotStack {
    let mut menu = MenuModel::for_layout(layout, snap, cursor.clone());
    crafter_toggle(click, slot, snap, &mut menu, out);
    let predicted = predicts(mode, layout, click, &menu, slot);
    apply_click(mode, layout, click, slot, &mut menu, out);
    match mode {
        Mode::Local => {
            menu.emit_creative_sets(snap, out);
            menu.cursor
        }
        Mode::Protocol if predicted => {
            state.drag.clear();
            state.menu_preview.set(menu.slots, snap);
            menu.cursor
        }
        Mode::Protocol => cursor.clone(),
    }
}

#[cfg(feature = "mobile_ui")]
const DROP_OK: u32 = 0xFF55_FF55;
#[cfg(feature = "mobile_ui")]
const DROP_BACK: u32 = 0xFFFF_5555;
#[cfg(feature = "mobile_ui")]
const DROP_WORLD: u32 = 0xFFFF_AA00;

#[cfg(feature = "mobile_ui")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn touch_release_place(
    ctx: &ScreenCtx,
    snap: &Snapshot,
    layout: Layout,
    mode: Mode,
    hovered: Option<usize>,
    drag_consumed: bool,
    state: &mut SlotState,
    cursor: &mut SlotStack,
    out: &mut Vec<InvAction>,
) -> bool {
    let released = ctx.input.left_release || ctx.input.right_release;
    if drag_consumed || !released || cursor.is_empty() || state.drag.active() || state.clicks_held()
    {
        return false;
    }
    let origin = state.last_click_slot;
    let target = match hovered {
        Some(slot) if Some(slot) == origin => return false,
        Some(slot) if layout.may_place(slot, cursor.item) => Some(slot),
        _ => origin,
    };
    let Some(target) = target else {
        return false;
    };
    *cursor = apply_one(
        mode,
        layout,
        SlotClick::Pickup { right: false },
        target,
        snap,
        cursor,
        state,
        out,
    );
    true
}

#[cfg(feature = "mobile_ui")]
#[derive(Clone, Copy)]
pub(crate) enum DropTarget {
    Slot((f32, f32), bool),
    World,
    Destroy,
    None,
}

#[cfg(feature = "mobile_ui")]
pub(crate) fn draw_drop_feedback(
    p: &mut Painter,
    ctx: &ScreenCtx,
    target: DropTarget,
    cursor: &SlotStack,
) {
    if cursor.is_empty() || !ctx.input.left_down {
        return;
    }
    let (x, y, w, h, colour) = match target {
        DropTarget::Slot((sx, sy), ok) => (
            sx - 1.0,
            sy - 1.0,
            SLOT_PITCH,
            SLOT_PITCH,
            if ok { DROP_OK } else { DROP_BACK },
        ),
        DropTarget::World | DropTarget::Destroy => {
            let Some(m) = ctx.mouse() else { return };
            let colour = match target {
                DropTarget::World => DROP_WORLD,
                _ => DROP_BACK,
            };
            (m.x - 9.0, m.y - 9.0, 18.0, 18.0, colour)
        }
        DropTarget::None => return,
    };
    p.outline(x, y, w, h, colour);
}

pub fn draw_menu_slot(
    p: &mut Painter,
    layout: Layout,
    i: usize,
    snap: &Snapshot,
    drag: &Drag,
    cursor: &SlotStack,
    menu_preview: Option<&SlotStack>,
    at: (f32, f32),
) {
    let (x, y) = at;
    match drag.slot_preview(layout, i, snap, cursor) {
        Some((preview, capped)) => {
            let color = if capped { CAPPED_COUNT_COLOR } else { 0xFFFFFF };
            p.stack_icon(&preview, x, y);
            p.item_decorations_colored(x, y, preview.count, 0, 0, color);
        }
        None => draw_slot(
            p,
            layout,
            i,
            menu_preview.unwrap_or_else(|| snap.slot(i)),
            x,
            y,
        ),
    }
}

pub fn draw_slot(p: &mut Painter, layout: Layout, i: usize, stack: &SlotStack, x: f32, y: f32) {
    if stack.is_empty() {
        if layout.slot_disabled(i) {
            p.sprite(
                "container/crafter/disabled_slot",
                x - 1.0,
                y - 1.0,
                18.0,
                18.0,
            );
        } else if let Some(sprite) = layout.empty_icon(i) {
            p.sprite(sprite, x, y, ITEM_SIZE, ITEM_SIZE);
        }
        return;
    }
    draw_stack(p, stack, x, y);
}

pub fn draw_stack(p: &mut Painter, stack: &SlotStack, x: f32, y: f32) {
    draw_stack_held(p, stack, x, y, None);
}

pub fn draw_stack_held(p: &mut Painter, stack: &SlotStack, x: f32, y: f32, use_ticks: Option<f32>) {
    if stack.is_empty() {
        return;
    }
    let use_ = crate::items::dispatch::ItemUse {
        using: use_ticks.is_some(),
        use_ticks: use_ticks.unwrap_or(0.0),
        owner: p.owner_at(x, y),
        ..stack.item_use()
    };
    p.stack_icon_as(stack, x, y, use_);
    if let Some(bundle) = &stack.bundle
        && let Some((width, color)) =
            azalea_inventory::bundle::bar(azalea_inventory::bundle::weight_safe(bundle.weight))
    {
        p.item_bar(x, y, f32::from(width), color);
    }
    p.item_decorations(x, y, stack.count, stack.damage, stack.max_damage);
    p.item_cooldown(x, y, stack.cooldown);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item: &'static str, count: u8) -> SlotStack {
        SlotStack {
            item,
            count,
            ..Default::default()
        }
    }

    fn snap(gamemode: Gamemode, filled: &[(usize, SlotStack)]) -> Snapshot {
        let mut menu_slots = vec![SlotStack::default(); MENU_SLOTS];
        for (i, s) in filled {
            menu_slots[*i] = s.clone();
        }
        Snapshot {
            menu_slots: menu_slots.into(),
            gamemode,
            ..Default::default()
        }
    }

    fn describe(a: &InvAction) -> String {
        match a {
            InvAction::Click(op) => match op {
                ClickOperation::Pickup(PickupClick::Left { slot: Some(s) }) => {
                    format!("pickup_left({s})")
                }
                ClickOperation::Pickup(PickupClick::Right { slot: Some(s) }) => {
                    format!("pickup_right({s})")
                }
                ClickOperation::Pickup(PickupClick::Left { slot: None }) => {
                    "pickup_left(-999)".into()
                }
                ClickOperation::Pickup(PickupClick::Right { slot: None }) => {
                    "pickup_right(-999)".into()
                }
                ClickOperation::Pickup(PickupClick::LeftOutside) => "drop_all".into(),
                ClickOperation::Pickup(PickupClick::RightOutside) => "drop_one".into(),
                ClickOperation::QuickMove(QuickMoveClick::Left { slot }) => {
                    format!("quick_move_left({slot})")
                }
                ClickOperation::QuickMove(QuickMoveClick::Right { slot }) => {
                    format!("quick_move_right({slot})")
                }
                ClickOperation::Swap(s) => format!("swap({},{})", s.source_slot, s.target_slot),
                ClickOperation::Clone(c) => format!("clone({})", c.slot),
                ClickOperation::Throw(ThrowClick::Single { slot }) => format!("throw_one({slot})"),
                ClickOperation::Throw(ThrowClick::All { slot }) => format!("throw_all({slot})"),
                ClickOperation::PickupAll(c) => format!("pickup_all({})", c.slot),
                ClickOperation::QuickCraft(q) => {
                    let kind = match q.kind {
                        QuickCraftKind::Left => "l",
                        QuickCraftKind::Right => "r",
                        QuickCraftKind::Middle => "m",
                    };
                    let stage = match q.status {
                        QuickCraftStatus::Start => "start".to_string(),
                        QuickCraftStatus::Add { slot } => format!("add{slot}"),
                        QuickCraftStatus::End => "end".to_string(),
                    };
                    format!("drag_{kind}_{stage}")
                }
            },
            InvAction::CreativeSet { slot, stack } => {
                if stack.is_empty() {
                    format!("set({slot},empty)")
                } else {
                    format!("set({slot},{}x{})", stack.item, stack.count)
                }
            }
            InvAction::CreativeDrop { stack } => {
                format!("creative_drop({}x{})", stack.item, stack.count)
            }
            InvAction::Close => "close".into(),
            InvAction::SlotStateChanged { slot, enabled } => {
                format!("slot_state({slot},{enabled})")
            }
            InvAction::CreativeBundleClick {
                slot,
                right,
                cursor,
            } => {
                format!(
                    "creative_bundle_click({slot},{right},{}x{})",
                    cursor.item, cursor.count
                )
            }
            InvAction::BundleSelect {
                slot,
                wire_slot,
                step,
            } => {
                format!("bundle_select({slot},{wire_slot},{step})")
            }
            InvAction::ButtonClick(_)
            | InvAction::SelectTrade(_)
            | InvAction::SetBeacon { .. }
            | InvAction::RenameItem(_) => "other".into(),
        }
    }

    fn run(
        gamemode: Gamemode,
        slots: &[(usize, SlotStack)],
        cursor: SlotStack,
        slot: usize,
        input: GuiInput,
    ) -> (Vec<String>, SlotStack) {
        let snap = snap(gamemode, slots);
        let mut out = Vec::new();
        let mode = Layout::PlayerMenu.mode(gamemode);
        let (after, _preview) = slot_clicks(
            &input,
            mode,
            Layout::PlayerMenu,
            slot,
            &snap,
            &cursor,
            &mut out,
        );
        (out.iter().map(describe).collect(), after)
    }

    fn left() -> GuiInput {
        GuiInput {
            left_click: true,
            ..Default::default()
        }
    }
    fn right() -> GuiInput {
        GuiInput {
            right_click: true,
            ..Default::default()
        }
    }

    #[test]
    fn survival_clicks_map_to_container_operations() {
        let filled = [(9, stack("stone", 20))];
        let cases: Vec<(GuiInput, &str)> = vec![
            (left(), "pickup_left(9)"),
            (right(), "pickup_right(9)"),
            (
                GuiInput {
                    shift: true,
                    ..left()
                },
                "quick_move_left(9)",
            ),
            (
                GuiInput {
                    shift: true,
                    ..right()
                },
                "quick_move_right(9)",
            ),
        ];
        for (input, want) in cases {
            let (acts, _) = run(Gamemode::Survival, &filled, SlotStack::default(), 9, input);
            assert_eq!(acts, vec![want.to_string()]);
        }
    }

    #[test]
    fn survival_hotbar_key_swaps_and_needs_a_free_cursor() {
        let mut input = GuiInput::default();
        input.hotbar_keys[2] = true;
        let filled = [(9, stack("stone", 20))];
        let (acts, _) = run(Gamemode::Survival, &filled, SlotStack::default(), 9, input);
        assert_eq!(acts, vec!["swap(9,2)".to_string()]);

        let mut input = GuiInput::default();
        input.hotbar_keys[2] = true;
        let (acts, _) = run(Gamemode::Survival, &filled, stack("dirt", 1), 9, input);
        assert!(acts.is_empty());
    }

    #[test]
    fn survival_throw_needs_an_item_and_a_free_cursor() {
        let filled = [(9, stack("stone", 20))];
        let q = GuiInput {
            drop_key: true,
            ..Default::default()
        };
        let (acts, _) = run(Gamemode::Survival, &filled, SlotStack::default(), 9, q);
        assert_eq!(acts, vec!["throw_one(9)".to_string()]);

        let ctrl_q = GuiInput {
            drop_key: true,
            ctrl: true,
            ..Default::default()
        };
        let (acts, _) = run(Gamemode::Survival, &filled, SlotStack::default(), 9, ctrl_q);
        assert_eq!(acts, vec!["throw_all(9)".to_string()]);

        let q = GuiInput {
            drop_key: true,
            ..Default::default()
        };
        let (acts, _) = run(Gamemode::Survival, &[], SlotStack::default(), 9, q);
        assert!(acts.is_empty());
        let q = GuiInput {
            drop_key: true,
            ..Default::default()
        };
        let (acts, _) = run(Gamemode::Survival, &filled, stack("dirt", 1), 9, q);
        assert!(acts.is_empty());
    }

    #[test]
    fn middle_click_clones_only_in_creative() {
        let filled = [(9, stack("stone", 20))];
        let mid = GuiInput {
            middle_click: true,
            ..Default::default()
        };
        let (acts, _) = run(Gamemode::Survival, &filled, SlotStack::default(), 9, mid);
        assert!(acts.is_empty(), "survival has no pick-block in a container");

        let mid = GuiInput {
            middle_click: true,
            ..Default::default()
        };
        let (acts, cursor) = run(Gamemode::Creative, &filled, SlotStack::default(), 9, mid);
        assert!(acts.is_empty());
        assert_eq!((cursor.item, cursor.count), ("stone", 64));
    }

    #[test]
    fn outside_click_drops_the_cursor() {
        let mut out = Vec::new();
        let mut cursor = stack("stone", 5);
        emit_outside_click(Mode::Protocol, false, &mut cursor, &mut out);
        assert_eq!(
            out.iter().map(describe).collect::<Vec<_>>(),
            vec!["drop_all"]
        );

        let mut out = Vec::new();
        let mut cursor = stack("stone", 5);
        emit_outside_click(Mode::Protocol, true, &mut cursor, &mut out);
        assert_eq!(
            out.iter().map(describe).collect::<Vec<_>>(),
            vec!["drop_one"]
        );

        let mut out = Vec::new();
        let mut cursor = stack("stone", 5);
        emit_outside_click(Mode::Local, true, &mut cursor, &mut out);
        assert_eq!(
            out.iter().map(describe).collect::<Vec<_>>(),
            vec!["creative_drop(stonex1)"]
        );
        assert_eq!(cursor.count, 4);
        emit_outside_click(Mode::Local, false, &mut cursor, &mut out);
        assert!(cursor.is_empty());
    }

    #[test]
    fn creative_left_click_places_the_whole_stack() {
        let mut menu = MenuModel::from_snapshot(&snap(Gamemode::Creative, &[]), stack("stone", 32));
        menu.pickup(9, false);
        assert_eq!(menu.slots[9].count, 32);
        assert!(menu.cursor.is_empty());
    }

    #[test]
    fn creative_left_click_merges_up_to_the_max_stack() {
        let filled = [(9, stack("stone", 50))];
        let mut menu =
            MenuModel::from_snapshot(&snap(Gamemode::Creative, &filled), stack("stone", 32));
        menu.pickup(9, false);
        assert_eq!(menu.slots[9].count, 64);
        assert_eq!(menu.cursor.count, 18);
    }

    #[test]
    fn creative_left_click_swaps_a_different_item() {
        let filled = [(9, stack("dirt", 10))];
        let mut menu =
            MenuModel::from_snapshot(&snap(Gamemode::Creative, &filled), stack("stone", 32));
        menu.pickup(9, false);
        assert_eq!(menu.slots[9].count, 32);
        assert_eq!((menu.cursor.item, menu.cursor.count), ("dirt", 10));
    }

    #[test]
    fn creative_left_click_takes_the_whole_slot() {
        let filled = [(9, stack("stone", 40))];
        let (acts, cursor) = run(Gamemode::Creative, &filled, SlotStack::default(), 9, left());
        assert_eq!(acts, vec!["set(9,empty)".to_string()]);
        assert_eq!(cursor.count, 40);
    }

    #[test]
    fn creative_right_click_places_one_and_takes_half() {
        let mut menu = MenuModel::from_snapshot(&snap(Gamemode::Creative, &[]), stack("stone", 32));
        menu.pickup(9, true);
        assert_eq!(menu.slots[9].count, 1);
        assert_eq!(menu.cursor.count, 31);

        let filled = [(9, stack("stone", 10))];
        let mut menu =
            MenuModel::from_snapshot(&snap(Gamemode::Creative, &filled), stack("stone", 32));
        menu.pickup(9, true);
        assert_eq!(menu.slots[9].count, 11);
        assert_eq!(menu.cursor.count, 31);

        let filled = [(9, stack("stone", 9))];
        let (acts, cursor) = run(
            Gamemode::Creative,
            &filled,
            SlotStack::default(),
            9,
            right(),
        );
        assert_eq!(acts, vec!["set(9,stonex4)".to_string()]);
        assert_eq!(cursor.count, 5);
    }

    #[test]
    fn creative_shift_click_moves_between_rows() {
        let filled = [(9, stack("stone", 40))];
        let shift = GuiInput {
            shift: true,
            ..left()
        };
        let (acts, cursor) = run(Gamemode::Creative, &filled, SlotStack::default(), 9, shift);
        assert_eq!(
            acts,
            vec!["set(9,empty)".to_string(), "set(36,stonex40)".to_string()]
        );
        assert!(cursor.is_empty());

        let filled = [(36, stack("stone", 40)), (10, stack("stone", 30))];
        let shift = GuiInput {
            shift: true,
            ..left()
        };
        let (acts, _) = run(Gamemode::Creative, &filled, SlotStack::default(), 36, shift);
        assert_eq!(
            acts,
            vec![
                "set(9,stonex6)".to_string(),
                "set(10,stonex64)".to_string(),
                "set(36,empty)".to_string(),
            ]
        );
    }

    #[test]
    fn creative_hotbar_key_swaps_two_slots() {
        let filled = [(9, stack("stone", 40)), (38, stack("dirt", 5))];
        let mut input = GuiInput::default();
        input.hotbar_keys[2] = true;
        let (acts, _) = run(Gamemode::Creative, &filled, SlotStack::default(), 9, input);
        assert_eq!(
            acts,
            vec!["set(9,dirtx5)".to_string(), "set(38,stonex40)".to_string()]
        );
    }

    #[test]
    fn creative_throw_empties_by_one_or_by_stack() {
        let filled = [(9, stack("stone", 40))];
        let q = GuiInput {
            drop_key: true,
            ..Default::default()
        };
        let (acts, _) = run(Gamemode::Creative, &filled, SlotStack::default(), 9, q);
        assert_eq!(
            acts,
            vec![
                "creative_drop(stonex1)".to_string(),
                "set(9,stonex39)".to_string()
            ]
        );

        let ctrl_q = GuiInput {
            drop_key: true,
            ctrl: true,
            ..Default::default()
        };
        let (acts, _) = run(Gamemode::Creative, &filled, SlotStack::default(), 9, ctrl_q);
        assert_eq!(
            acts,
            vec![
                "creative_drop(stonex40)".to_string(),
                "set(9,empty)".to_string()
            ]
        );
    }

    #[test]
    fn creative_never_sets_the_craft_result_slot() {
        let filled = [(0, stack("stone", 3))];
        let (acts, cursor) = run(Gamemode::Creative, &filled, SlotStack::default(), 0, left());
        assert!(acts.is_empty(), "unexpected {acts:?}");
        assert_eq!(cursor.count, 3);
    }

    #[test]
    fn armor_slots_take_only_their_own_piece_and_only_one() {
        let mut menu = MenuModel::from_snapshot(&snap(Gamemode::Creative, &[]), stack("stone", 8));
        menu.pickup(5, false);
        assert!(menu.slots[5].is_empty(), "the helmet slot refuses a block");
        assert_eq!(menu.cursor.count, 8, "and the cursor keeps the whole stack");

        let mut menu =
            MenuModel::from_snapshot(&snap(Gamemode::Creative, &[]), stack("diamond_helmet", 3));
        menu.pickup(5, false);
        assert_eq!(menu.slots[5].count, 1);
        assert_eq!(menu.cursor.count, 2);
    }

    #[test]
    fn shulker_box_and_dispenser_layouts() {
        let shulker = Layout::ShulkerBox;
        assert_eq!(shulker.size(), (176.0, 167.0));
        assert_eq!(shulker.slot_count(), 63);
        assert_eq!(shulker.slot_pos(0), (8.0, 18.0));
        assert_eq!(shulker.slot_pos(26), (152.0, 54.0));
        assert_eq!(shulker.slot_pos(27), (8.0, 84.0));
        assert_eq!(shulker.slot_pos(62), (152.0, 142.0));
        assert!(shulker.may_place(4, "stone"));
        assert!(!shulker.may_place(4, "shulker_box"));
        assert!(!shulker.may_place(4, "red_shulker_box"));
        assert!(
            shulker.may_place(30, "red_shulker_box"),
            "player rows take anything"
        );

        let dispenser = Layout::Dispenser;
        assert_eq!(dispenser.size(), (176.0, 166.0));
        assert_eq!(dispenser.slot_count(), 45);
        assert_eq!(dispenser.slot_pos(0), (62.0, 17.0));
        assert_eq!(dispenser.slot_pos(4), (80.0, 35.0));
        assert_eq!(dispenser.slot_pos(8), (98.0, 53.0));
        assert_eq!(dispenser.slot_pos(9), (8.0, 84.0));
        assert!(dispenser.may_place(0, "red_shulker_box"));
    }

    #[test]
    fn crafter_layout_and_slot_toggles() {
        let crafter = Layout::Crafter(0b10);
        assert_eq!(crafter.size(), (176.0, 166.0));
        assert_eq!(crafter.slot_count(), 46);
        assert_eq!(crafter.slot_pos(0), (26.0, 17.0));
        assert_eq!(crafter.slot_pos(8), (62.0, 53.0));
        assert_eq!(crafter.slot_pos(9), (8.0, 84.0));
        assert_eq!(crafter.slot_pos(44), (152.0, 142.0));
        assert_eq!(crafter.slot_pos(45), (134.0, 35.0));
        assert!(crafter.may_place(0, "stone"));
        assert!(!crafter.may_place(1, "stone"), "disabled slot");
        assert!(crafter.may_place(10, "stone"));
        assert!(!crafter.may_place(45, "stone"), "result slot");
        assert!(!crafter.may_pickup(45));
        assert!(crafter.may_pickup(0));

        let survival = snap(Gamemode::Survival, &[(2, stack("stone", 1))]);
        let click = |snap: &Snapshot, input: GuiInput, slot: usize| {
            let mut out = Vec::new();
            slot_clicks(
                &input,
                Mode::Protocol,
                crafter,
                slot,
                snap,
                &SlotStack::default(),
                &mut out,
            );
            out.iter().map(describe).collect::<Vec<_>>()
        };
        assert_eq!(
            click(&survival, left(), 0),
            ["slot_state(0,false)", "pickup_left(0)"]
        );
        assert_eq!(
            click(&survival, right(), 1),
            ["slot_state(1,true)", "pickup_right(1)"]
        );
        assert_eq!(
            click(&survival, left(), 2),
            ["pickup_left(2)"],
            "occupied slot"
        );
        assert_eq!(
            click(&survival, left(), 10),
            ["pickup_left(10)"],
            "player row"
        );
        let spectator = snap(Gamemode::Spectator, &[]);
        assert_eq!(click(&spectator, left(), 0), ["pickup_left(0)"]);

        let mut key = GuiInput::default();
        key.hotbar_keys[3] = true;
        assert_eq!(click(&survival, key.clone(), 0), ["swap(0,3)"]);
        assert_eq!(click(&survival, key.clone(), 1), ["swap(1,3)"]);
        let mut hotbar = vec![SlotStack::default(); 10];
        hotbar[3] = stack("stone", 4);
        let holding = Snapshot {
            hotbar: hotbar.into(),
            ..snap(Gamemode::Survival, &[])
        };
        assert_eq!(click(&holding, key, 1), ["slot_state(1,true)", "swap(1,3)"]);

        let mut state = SlotState::default();
        let mut out = Vec::new();
        let after = apply_one(
            Mode::Protocol,
            crafter,
            SlotClick::Pickup { right: false },
            1,
            &survival,
            &stack("stone", 5),
            &mut state,
            &mut out,
        );
        let out: Vec<_> = out.iter().map(describe).collect();
        assert_eq!(out, ["slot_state(1,true)", "pickup_left(1)"]);
        assert!(after.is_empty());
    }

    #[test]
    fn the_craft_result_slot_never_accepts_a_stack() {
        for layout in [Layout::PlayerMenu, Layout::Crafting] {
            assert!(!layout.may_place(0, "stone"), "{layout:?} result slot");
        }
        let mut menu = MenuModel::from_snapshot(&snap(Gamemode::Creative, &[]), stack("stone", 8));
        menu.pickup(0, false);
        assert!(menu.slots[0].is_empty());
        assert_eq!(
            menu.cursor.count, 8,
            "the stack is still in hand, not destroyed"
        );
    }

    #[test]
    fn a_full_matching_slot_is_not_a_drag_target() {
        let full = slot_capacity(Layout::PlayerMenu, 9, "stone");
        let snapshot = snap(Gamemode::Creative, &[(9, stack("stone", full))]);
        let carried = stack("stone", 6);
        let mut menu = MenuModel::from_snapshot(&snapshot, carried);
        menu.quick_craft(DragKind::Left, &[10, 11]);
        assert_eq!((menu.slots[10].count, menu.slots[11].count), (3, 3));
        assert!(menu.cursor.is_empty());
    }

    #[test]
    fn quick_move_equips_armor() {
        let filled = [(9, stack("diamond_helmet", 1))];
        let shift = GuiInput {
            shift: true,
            ..left()
        };
        let (acts, _) = run(Gamemode::Creative, &filled, SlotStack::default(), 9, shift);
        assert_eq!(
            acts,
            vec![
                "set(5,diamond_helmetx1)".to_string(),
                "set(9,empty)".to_string()
            ]
        );
    }

    #[test]
    fn outside_click_drops_once_per_press() {
        let mut out = Vec::new();
        let mut cursor = stack("stone", 5);
        emit_outside_click(Mode::Local, false, &mut cursor, &mut out);
        assert_eq!(
            out.iter().map(describe).collect::<Vec<_>>(),
            vec!["creative_drop(stonex5)"]
        );
        assert!(cursor.is_empty(), "the whole stack left the cursor");

        emit_outside_click(Mode::Local, false, &mut cursor, &mut out);
        assert_eq!(out.len(), 1, "an empty cursor drops nothing");
    }

    #[test]
    fn pickup_all_rakes_partial_stacks_first() {
        let mut menu = MenuModel::from_snapshot(
            &snap(
                Gamemode::Creative,
                &[
                    (9, stack("stone", 64)),
                    (10, stack("stone", 20)),
                    (11, stack("stone", 5)),
                ],
            ),
            stack("stone", 30),
        );
        menu.pickup_all(20);

        assert_eq!(menu.cursor.count, 64, "the cursor fills to a full stack");
        assert!(menu.slots[10].is_empty(), "partial taken whole");
        assert!(menu.slots[11].is_empty(), "partial taken whole");
        assert_eq!(
            menu.slots[9].count,
            64 - 9,
            "the full stack covers only the shortfall"
        );
    }

    #[test]
    fn pickup_all_needs_a_loaded_cursor_and_an_empty_slot() {
        let filled = [(9, stack("stone", 64))];
        let mut menu =
            MenuModel::from_snapshot(&snap(Gamemode::Creative, &filled), SlotStack::default());
        menu.pickup_all(20);
        assert!(menu.cursor.is_empty());
        assert_eq!(menu.slots[9].count, 64);

        let mut menu =
            MenuModel::from_snapshot(&snap(Gamemode::Creative, &filled), stack("stone", 1));
        menu.pickup_all(9);
        assert_eq!(menu.cursor.count, 1);
        assert_eq!(menu.slots[9].count, 64);
    }

    #[test]
    fn pickup_all_ignores_a_different_item() {
        let filled = [(9, stack("dirt", 40))];
        let mut menu =
            MenuModel::from_snapshot(&snap(Gamemode::Creative, &filled), stack("stone", 3));
        menu.pickup_all(20);
        assert_eq!(menu.cursor.count, 3);
        assert_eq!(menu.slots[9].count, 40);
    }

    #[test]
    fn left_drag_splits_evenly_and_keeps_the_remainder() {
        let mut menu = MenuModel::from_snapshot(&snap(Gamemode::Creative, &[]), stack("stone", 10));
        menu.quick_craft(DragKind::Left, &[9, 10, 11]);
        assert_eq!(
            (
                menu.slots[9].count,
                menu.slots[10].count,
                menu.slots[11].count
            ),
            (3, 3, 3)
        );
        assert_eq!(
            menu.cursor.count, 1,
            "the indivisible remainder stays on the cursor"
        );
    }

    #[test]
    fn left_drag_fills_empty_slots() {
        let mut menu = MenuModel::from_snapshot(&snap(Gamemode::Creative, &[]), stack("stone", 4));
        menu.quick_craft(DragKind::Left, &[9, 10]);
        assert_eq!(menu.slots[9].count, 2);
        assert_eq!(menu.slots[10].count, 2);
        assert!(menu.cursor.is_empty());
    }

    #[test]
    fn right_drag_places_one_each() {
        let mut menu = MenuModel::from_snapshot(&snap(Gamemode::Creative, &[]), stack("stone", 10));
        menu.quick_craft(DragKind::Right, &[9, 10, 11]);
        assert_eq!(
            (
                menu.slots[9].count,
                menu.slots[10].count,
                menu.slots[11].count
            ),
            (1, 1, 1)
        );
        assert_eq!(menu.cursor.count, 7);
    }

    #[test]
    fn drag_merges_into_matching_stacks_and_skips_others() {
        let mut menu = MenuModel::from_snapshot(
            &snap(
                Gamemode::Creative,
                &[(9, stack("stone", 1)), (10, stack("dirt", 1))],
            ),
            stack("stone", 6),
        );
        menu.quick_craft(DragKind::Left, &[9, 10, 11]);
        assert_eq!(menu.slots[9].count, 3);
        assert_eq!((menu.slots[10].item, menu.slots[10].count), ("dirt", 1));
        assert_eq!(menu.slots[11].count, 2);
        assert_eq!(menu.cursor.count, 2);
    }

    #[test]
    fn a_single_slot_drag_is_just_a_click() {
        let mut menu = MenuModel::from_snapshot(&snap(Gamemode::Creative, &[]), stack("stone", 8));
        menu.quick_craft(DragKind::Left, &[9]);
        assert_eq!(menu.slots[9].count, 8);
        assert!(menu.cursor.is_empty());

        let mut menu = MenuModel::from_snapshot(&snap(Gamemode::Creative, &[]), stack("stone", 8));
        menu.quick_craft(DragKind::Right, &[9]);
        assert_eq!(menu.slots[9].count, 1);
        assert_eq!(menu.cursor.count, 7);
    }

    #[test]
    fn middle_drag_fills_every_slot_to_a_full_stack() {
        let mut menu = MenuModel::from_snapshot(&snap(Gamemode::Creative, &[]), stack("stone", 2));
        menu.quick_craft(DragKind::Middle, &[9, 10]);
        assert_eq!(menu.slots[9].count, 64);
        assert_eq!(menu.slots[10].count, 64);
    }

    #[test]
    fn a_press_with_an_empty_cursor_is_a_pickup_not_a_drag() {
        let clicks = resolve_slot_clicks(&left(), true, false, true);
        assert_eq!(clicks, vec![SlotClick::Pickup { right: false }]);
        let clicks = resolve_slot_clicks(&left(), false, false, true);
        assert!(clicks.is_empty(), "unexpected {clicks:?}");
    }

    #[test]
    fn a_protocol_drag_sends_the_three_stage_sequence() {
        let mut out = Vec::new();
        let mut menu = MenuModel::from_snapshot(&snap(Gamemode::Survival, &[]), stack("stone", 6));
        let _ = &mut menu;
        for status in [
            QuickCraftStatus::Start,
            QuickCraftStatus::Add { slot: 9 },
            QuickCraftStatus::Add { slot: 10 },
            QuickCraftStatus::End,
        ] {
            out.push(InvAction::Click(ClickOperation::QuickCraft(
                QuickCraftClick {
                    kind: QuickCraftKind::Left,
                    status,
                },
            )));
        }
        assert_eq!(
            out.iter().map(describe).collect::<Vec<_>>(),
            vec!["drag_l_start", "drag_l_add9", "drag_l_add10", "drag_l_end"]
        );
    }

    fn dragging(kind: DragKind, slots: &[usize]) -> Drag {
        Drag {
            kind: Some(kind),
            slots: slots.to_vec(),
            double: false,
            remainder: None,
            ..Default::default()
        }
    }

    #[test]
    fn drag_preview_shows_the_share_and_shrinks_the_cursor() {
        let snapshot = snap(Gamemode::Survival, &[]);
        let carried = stack("stone", 9);
        let mut drag = dragging(DragKind::Left, &[9, 10, 11]);
        drag.recalculate_remainder(Layout::PlayerMenu, &snapshot, &carried);

        for slot in [9, 10, 11] {
            let (preview, capped) = drag
                .slot_preview(Layout::PlayerMenu, slot, &snapshot, &carried)
                .unwrap();
            assert_eq!(preview.count, 3, "an even split of 9 over three slots");
            assert!(!capped);
        }
        assert_eq!(drag.remainder, Some(0));
        assert!(
            drag.slot_preview(Layout::PlayerMenu, 12, &snapshot, &carried)
                .is_none()
        );
    }

    #[test]
    fn drag_over_a_slot_above_its_capacity_takes_nothing() {
        let snapshot = snap(Gamemode::Survival, &[(5, stack("stone", 64))]);
        let carried = stack("stone", 8);
        let mut drag = dragging(DragKind::Left, &[5, 9]);
        drag.recalculate_remainder(Layout::PlayerMenu, &snapshot, &carried);

        assert_eq!(drag.remainder, Some(4));
    }

    #[test]
    fn drag_over_only_an_over_capacity_slot_keeps_the_whole_stack() {
        let snapshot = snap(Gamemode::Survival, &[(5, stack("stone", 3))]);
        let carried = stack("stone", 6);
        let mut drag = dragging(DragKind::Left, &[5]);
        drag.recalculate_remainder(Layout::PlayerMenu, &snapshot, &carried);

        assert_eq!(drag.remainder, Some(6));
    }

    #[test]
    fn drag_preview_merges_onto_what_a_slot_already_holds() {
        let snapshot = snap(Gamemode::Survival, &[(9, stack("stone", 2))]);
        let carried = stack("stone", 4);
        let mut drag = dragging(DragKind::Left, &[9, 10]);
        drag.recalculate_remainder(Layout::PlayerMenu, &snapshot, &carried);

        let (preview, _) = drag
            .slot_preview(Layout::PlayerMenu, 9, &snapshot, &carried)
            .unwrap();
        assert_eq!(preview.count, 4, "2 already there plus a share of 2");
        let (preview, _) = drag
            .slot_preview(Layout::PlayerMenu, 10, &snapshot, &carried)
            .unwrap();
        assert_eq!(preview.count, 2);
        assert_eq!(drag.remainder, Some(0));
    }

    #[test]
    fn drag_preview_flags_a_clamped_share() {
        let snapshot = snap(Gamemode::Survival, &[]);
        let carried = stack("stone", 4);
        let mut drag = dragging(DragKind::Left, &[5, 9]);
        drag.recalculate_remainder(Layout::PlayerMenu, &snapshot, &carried);

        let (preview, capped) = drag
            .slot_preview(Layout::PlayerMenu, 5, &snapshot, &carried)
            .unwrap();
        assert_eq!(preview.count, 1, "an armour slot takes one");
        assert!(capped, "a clamped share draws its count in yellow");

        let (preview, capped) = drag
            .slot_preview(Layout::PlayerMenu, 9, &snapshot, &carried)
            .unwrap();
        assert_eq!(preview.count, 2, "an ordinary slot takes the whole share");
        assert!(!capped);

        assert_eq!(drag.remainder, Some(1));
    }

    #[test]
    fn only_the_player_menu_caps_its_armour_slots() {
        assert_eq!(slot_capacity(Layout::PlayerMenu, 5, "stone"), 1);
        assert_eq!(slot_capacity(Layout::Chest(3), 5, "stone"), 64);
        assert_eq!(slot_capacity(Layout::Crafting, 5, "stone"), 64);
    }

    #[test]
    fn the_preview_matches_what_the_release_places() {
        let snapshot = snap(Gamemode::Creative, &[(9, stack("stone", 2))]);
        let carried = stack("stone", 7);
        let slots = [9, 10, 11];
        let mut drag = dragging(DragKind::Left, &slots);
        drag.recalculate_remainder(Layout::PlayerMenu, &snapshot, &carried);
        let previewed: Vec<u8> = slots
            .iter()
            .map(|s| {
                drag.slot_preview(Layout::PlayerMenu, *s, &snapshot, &carried)
                    .unwrap()
                    .0
                    .count
            })
            .collect();
        let remainder = drag.remainder.unwrap();

        let mut menu = MenuModel::from_snapshot(&snapshot, carried);
        menu.quick_craft(DragKind::Left, &slots);
        let placed: Vec<u8> = slots.iter().map(|s| menu.slots[*s].count).collect();

        assert_eq!(previewed, placed);
        assert_eq!(remainder, menu.cursor.count);
    }

    struct Frame {
        input: GuiInput,
        hovered: Option<usize>,
    }

    fn press(slot: usize) -> Frame {
        Frame {
            input: GuiInput {
                left_click: true,
                left_down: true,
                ..Default::default()
            },
            hovered: Some(slot),
        }
    }
    fn hold(slot: usize) -> Frame {
        Frame {
            input: GuiInput {
                left_down: true,
                ..Default::default()
            },
            hovered: Some(slot),
        }
    }
    fn release(slot: Option<usize>) -> Frame {
        Frame {
            input: GuiInput {
                left_release: true,
                ..Default::default()
            },
            hovered: slot,
        }
    }

    fn gesture(
        gamemode: Gamemode,
        filled: &[(usize, SlotStack)],
        cursor: SlotStack,
        frames: Vec<Frame>,
    ) -> (Vec<String>, SlotStack) {
        let snapshot = snap(gamemode, filled);
        let mut state = SlotState::default();
        let mut cursor = cursor;
        let mut out = Vec::new();
        for frame in frames {
            let ctx = ScreenCtx {
                input: &frame.input,
                vw: 400.0,
                vh: 240.0,
            };
            drag_step(
                &ctx,
                &snapshot,
                Layout::PlayerMenu,
                frame.hovered,
                &mut state,
                &mut cursor,
                &mut out,
            );
        }
        (out.iter().map(describe).collect(), cursor)
    }

    #[test]
    fn releasing_over_a_different_item_swaps() {
        let (acts, cursor) = gesture(
            Gamemode::Creative,
            &[(9, stack("dirt", 10))],
            stack("stone", 32),
            vec![press(9), release(Some(9))],
        );
        assert_eq!(acts, vec!["set(9,stonex32)".to_string()]);
        assert_eq!((cursor.item, cursor.count), ("dirt", 10));
    }

    #[test]
    fn a_press_and_release_in_one_frame_acts_once() {
        let snapshot = snap(Gamemode::Creative, &[]);
        let mut state = SlotState::default();
        let mut cursor = stack("stone", 8);
        let mut out = Vec::new();
        let input = GuiInput {
            left_click: true,
            left_release: true,
            ..Default::default()
        };
        let ctx = ScreenCtx {
            input: &input,
            vw: 400.0,
            vh: 240.0,
        };
        let consumed = drag_step(
            &ctx,
            &snapshot,
            Layout::PlayerMenu,
            Some(9),
            &mut state,
            &mut cursor,
            &mut out,
        );
        assert!(
            consumed,
            "the frame's click belongs to the drag, not to a second pickup"
        );
        assert_eq!(
            out.iter().map(describe).collect::<Vec<_>>(),
            vec!["set(9,stonex8)"]
        );
        assert!(cursor.is_empty());
    }

    #[cfg(not(feature = "mobile_ui"))]
    #[test]
    fn a_sweep_spreads_over_every_slot_it_crossed() {
        let (acts, cursor) = gesture(
            Gamemode::Creative,
            &[],
            stack("stone", 9),
            vec![press(9), hold(10), hold(11), release(Some(11))],
        );
        assert_eq!(
            acts,
            vec![
                "set(9,stonex3)".to_string(),
                "set(10,stonex3)".to_string(),
                "set(11,stonex3)".to_string(),
            ]
        );
        assert!(cursor.is_empty());
    }

    #[cfg(feature = "mobile_ui")]
    #[test]
    fn a_touch_sweep_moves_the_stack_to_where_it_lifts() {
        let (acts, cursor) = gesture(
            Gamemode::Creative,
            &[],
            stack("stone", 9),
            vec![press(9), hold(10), hold(11), release(Some(11))],
        );
        assert_eq!(acts, vec!["set(11,stonex9)".to_string()]);
        assert!(cursor.is_empty());
    }

    #[cfg(feature = "mobile_ui")]
    #[test]
    fn a_touch_sweep_lifted_between_slots_keeps_the_stack() {
        let (acts, cursor) = gesture(
            Gamemode::Creative,
            &[],
            stack("stone", 9),
            vec![press(9), hold(10), release(None)],
        );
        assert!(acts.is_empty(), "unexpected {acts:?}");
        assert_eq!(cursor.count, 9);
    }

    #[cfg(feature = "mobile_ui")]
    #[test]
    fn a_release_with_no_slot_to_return_to_keeps_the_stack() {
        let snapshot = Snapshot {
            carried: stack("stone", 8),
            ..snap(Gamemode::Survival, &[])
        };
        let mut state = SlotState::default();
        let mut cursor = snapshot.carried.clone();
        let mut out = Vec::new();
        let input = GuiInput {
            left_release: true,
            ..Default::default()
        };
        let ctx = ScreenCtx {
            input: &input,
            vw: 400.0,
            vh: 240.0,
        };
        touch_release_place(
            &ctx,
            &snapshot,
            Layout::Stonecutter,
            Mode::Protocol,
            None,
            false,
            &mut state,
            &mut cursor,
            &mut out,
        );
        assert_eq!(out.len(), 0, "nothing is sent");
        assert_eq!(cursor.count, 8, "the stack the server holds stays in hand");
    }

    #[test]
    fn a_stale_cursor_prediction_gives_up_eventually() {
        let snapshot = snap(Gamemode::Survival, &[]);
        let mut state = SlotState::default();
        state.set_cursor(stack("stone", 8), &snapshot);
        for _ in 0..COMMIT_MAX_FRAMES - 1 {
            state.retire_cursor(&snapshot);
            let cursor = state.cursor(&snapshot);
            state.set_cursor(cursor, &snapshot);
        }
        assert_eq!(state.cursor(&snapshot).count, 8, "still waiting");
        state.retire_cursor(&snapshot);
        assert!(
            state.cursor(&snapshot).is_empty(),
            "the server's cursor wins"
        );
    }

    #[test]
    fn a_cursor_prediction_yields_to_the_next_server_update() {
        let before = snap(Gamemode::Survival, &[]);
        let mut state = SlotState::default();
        state.set_cursor(stack("stone", 1), &before);

        let landed = Snapshot {
            carried: stack("stone", 2),
            ..snap(Gamemode::Survival, &[])
        };
        state.retire_cursor(&landed);
        assert_eq!(state.cursor(&landed).count, 2);
    }

    #[test]
    fn a_screen_change_forgets_the_predicted_cursor() {
        let snapshot = snap(Gamemode::Survival, &[]);
        let mut state = SlotState::default();
        state.set_cursor(stack("stone", 8), &snapshot);
        state.reset_for_screen_change();
        assert!(state.cursor(&snapshot).is_empty());
    }

    #[test]
    fn a_stowed_creative_cursor_merges_then_fills_then_drops() {
        let slots = snap(
            Gamemode::Creative,
            &[(36, stack("stone", 60)), (37, stack("dirt", 1))],
        )
        .menu_slots;
        let mut out = Vec::new();
        stow_creative_cursor(stack("stone", 10), &slots, &mut out);
        assert_eq!(
            out.iter().map(describe).collect::<Vec<_>>(),
            vec!["set(36,stonex64)", "set(38,stonex6)"]
        );

        let full = vec![stack("dirt", 64); MENU_SLOTS];
        let mut out = Vec::new();
        stow_creative_cursor(stack("stone", 10), &full, &mut out);
        assert_eq!(
            out.iter().map(describe).collect::<Vec<_>>(),
            vec!["creative_drop(stonex10)"]
        );
    }

    #[test]
    fn the_other_button_aborts_a_drag() {
        let abort = Frame {
            input: GuiInput {
                right_click: true,
                left_down: true,
                ..Default::default()
            },
            hovered: Some(11),
        };
        let (acts, cursor) = gesture(
            Gamemode::Creative,
            &[],
            stack("stone", 9),
            vec![press(9), hold(10), abort, release(Some(11))],
        );
        assert!(acts.is_empty(), "nothing is placed: {acts:?}");
        assert_eq!(cursor.count, 9, "the stack is still in hand");
    }

    #[test]
    fn a_drag_never_starts_empty_handed() {
        let snapshot = snap(Gamemode::Creative, &[(9, stack("stone", 4))]);
        let mut state = SlotState::default();
        let mut cursor = SlotStack::default();
        let mut out = Vec::new();
        let input = GuiInput {
            left_click: true,
            left_down: true,
            ..Default::default()
        };
        let ctx = ScreenCtx {
            input: &input,
            vw: 400.0,
            vh: 240.0,
        };
        let consumed = drag_step(
            &ctx,
            &snapshot,
            Layout::PlayerMenu,
            Some(9),
            &mut state,
            &mut cursor,
            &mut out,
        );
        assert!(!consumed, "the click is the pickup path's");
        assert!(out.is_empty());
    }

    #[test]
    fn a_stationary_clone_press_places_nothing() {
        let mid = Frame {
            input: GuiInput {
                middle_click: true,
                middle_down: true,
                ..Default::default()
            },
            hovered: Some(9),
        };
        let up = Frame {
            input: GuiInput::default(),
            hovered: Some(9),
        };
        let (acts, cursor) = gesture(Gamemode::Creative, &[], stack("stone", 8), vec![mid, up]);
        assert!(acts.is_empty(), "unexpected {acts:?}");
        assert_eq!(cursor.count, 8);
    }

    use azalea::entity::PlayerAbilities;
    use azalea::entity::inventory::Inventory;
    use azalea_inventory::ItemStack;
    use azalea_registry::builtin::ItemKind;

    fn az_stack(kind: ItemKind, count: i32) -> ItemStack {
        ItemStack::new(kind, count)
    }

    #[test]
    fn azalea_predicts_a_left_drag() {
        let mut inv = Inventory::default();
        let abilities = PlayerAbilities::default();
        inv.carried = az_stack(ItemKind::Stone, 9);

        for status in [
            QuickCraftStatus::Start,
            QuickCraftStatus::Add { slot: 9 },
            QuickCraftStatus::Add { slot: 10 },
            QuickCraftStatus::Add { slot: 11 },
            QuickCraftStatus::End,
        ] {
            inv.simulate_click(
                &ClickOperation::QuickCraft(QuickCraftClick {
                    kind: QuickCraftKind::Left,
                    status,
                }),
                &abilities,
            );
        }

        let slots = inv.inventory_menu.slots();
        for slot in [9, 10, 11] {
            assert_eq!(slots[slot].count(), 3, "slot {slot} takes an even third");
        }
        assert!(inv.carried.is_empty(), "the whole stack left the cursor");
    }

    #[test]
    fn azalea_drags_into_empty_slots() {
        let mut inv = Inventory::default();
        let abilities = PlayerAbilities::default();
        inv.carried = az_stack(ItemKind::Stone, 4);

        for status in [
            QuickCraftStatus::Start,
            QuickCraftStatus::Add { slot: 9 },
            QuickCraftStatus::Add { slot: 10 },
            QuickCraftStatus::End,
        ] {
            inv.simulate_click(
                &ClickOperation::QuickCraft(QuickCraftClick {
                    kind: QuickCraftKind::Left,
                    status,
                }),
                &abilities,
            );
        }

        let slots = inv.inventory_menu.slots();
        assert_eq!((slots[9].count(), slots[10].count()), (2, 2));
        assert!(inv.carried.is_empty());
    }

    #[test]
    fn azalea_clears_the_cursor_on_either_outside_spelling() {
        for op in [
            ClickOperation::Pickup(PickupClick::LeftOutside),
            ClickOperation::Pickup(PickupClick::Left { slot: None }),
        ] {
            let mut inv = Inventory::default();
            inv.carried = az_stack(ItemKind::Stone, 12);
            inv.simulate_click(&op, &PlayerAbilities::default());
            assert!(inv.carried.is_empty(), "{op:?} left the cursor loaded");
        }
    }

    #[test]
    fn azalea_drops_one_per_right_click_outside() {
        for op in [
            ClickOperation::Pickup(PickupClick::RightOutside),
            ClickOperation::Pickup(PickupClick::Right { slot: None }),
        ] {
            let mut inv = Inventory::default();
            inv.carried = az_stack(ItemKind::Stone, 3);
            inv.simulate_click(&op, &PlayerAbilities::default());
            assert_eq!(inv.carried.count(), 2, "{op:?} should drop exactly one");
            inv.simulate_click(&op, &PlayerAbilities::default());
            inv.simulate_click(&op, &PlayerAbilities::default());
            assert!(
                inv.carried.is_empty(),
                "{op:?} should empty the cursor at the third"
            );
        }
    }

    #[test]
    fn azalea_keeps_both_menus_player_rows_in_step() {
        use azalea_inventory::{Menu, SlotList};

        let mut inv = Inventory::default();
        inv.id = 1;
        inv.container_menu = Some(Menu::Crafting {
            result: ItemStack::Empty,
            grid: SlotList::default(),
            player: SlotList::default(),
        });

        let player_first = inv.inventory_menu.player_slots_range();
        *inv.inventory_menu.slot_mut(*player_first.start()).unwrap() = az_stack(ItemKind::Stone, 5);
        inv.sync_player_slots(false);
        let container_first = inv.container_menu.as_ref().unwrap().player_slots_range();
        assert_eq!(
            inv.container_menu
                .as_ref()
                .unwrap()
                .slot(*container_first.start())
                .unwrap()
                .count(),
            5,
            "the crafting menu shows what the player menu was told"
        );

        *inv.container_menu
            .as_mut()
            .unwrap()
            .slot_mut(*container_first.start() + 1)
            .unwrap() = az_stack(ItemKind::Dirt, 7);
        inv.sync_player_slots(true);
        assert_eq!(
            inv.inventory_menu
                .slot(*player_first.start() + 1)
                .unwrap()
                .count(),
            7,
            "the player menu shows what the container was told"
        );
    }

    #[test]
    fn azalea_keeps_a_click_into_the_container_s_player_rows() {
        use azalea_inventory::{Menu, SlotList};

        let mut inv = Inventory::default();
        inv.id = 1;
        inv.container_menu = Some(Menu::Crafting {
            result: ItemStack::Empty,
            grid: SlotList::default(),
            player: SlotList::default(),
        });
        inv.carried = az_stack(ItemKind::Stone, 5);

        let slot = *inv
            .container_menu
            .as_ref()
            .unwrap()
            .player_slots_range()
            .start();
        inv.simulate_click(
            &ClickOperation::Pickup(PickupClick::Left {
                slot: Some(slot as u16),
            }),
            &PlayerAbilities::default(),
        );

        assert!(inv.carried.is_empty(), "the whole stack went into the slot");
        assert_eq!(
            inv.menu().slot(slot).unwrap().count(),
            5,
            "the open menu, which is what the GUI and `changed_slots` read, keeps the click"
        );
        let player_slot = *inv.inventory_menu.player_slots_range().start();
        assert_eq!(
            inv.inventory_menu.slot(player_slot).unwrap().count(),
            5,
            "and the HUD's view follows it"
        );
    }

    #[test]
    fn azalea_tracks_a_state_id_per_menu() {
        use azalea_inventory::{Menu, SlotList};

        let mut inv = Inventory::default();
        inv.id = 1;
        inv.container_menu = Some(Menu::Crafting {
            result: ItemStack::Empty,
            grid: SlotList::default(),
            player: SlotList::default(),
        });

        inv.container_state_id = 7;
        inv.state_id = 3;
        assert_eq!(
            inv.menu_state_id(),
            7,
            "a click in the container reports the container's"
        );

        inv.container_menu = None;
        assert_eq!(inv.menu_state_id(), 3);
    }

    #[test]
    fn a_protocol_place_holds_its_preview_until_the_snapshot_catches_up() {
        let snapshot = Snapshot {
            carried: stack("stone", 8),
            ..snap(Gamemode::Survival, &[])
        };
        let mut state = SlotState::default();
        let mut cursor = snapshot.carried.clone();
        let mut out = Vec::new();

        fn step(
            input: GuiInput,
            snapshot: &Snapshot,
            state: &mut SlotState,
            cursor: &mut SlotStack,
            out: &mut Vec<InvAction>,
        ) {
            let ctx = ScreenCtx {
                input: &input,
                vw: 400.0,
                vh: 240.0,
            };
            drag_step(
                &ctx,
                snapshot,
                Layout::Chest(3),
                Some(9),
                state,
                cursor,
                out,
            );
        }

        let press = GuiInput {
            left_click: true,
            left_down: true,
            ..Default::default()
        };
        let release = GuiInput {
            left_release: true,
            ..Default::default()
        };
        step(press, &snapshot, &mut state, &mut cursor, &mut out);
        step(release, &snapshot, &mut state, &mut cursor, &mut out);

        assert_eq!(out.len(), 1, "one click queued");
        assert!(cursor.is_empty(), "the place is predicted at once");
        assert!(!state.drag.active());
        assert_eq!(
            state.menu_preview.slot(9).map(|s| s.count),
            Some(8),
            "the slot shows what is about to land in it"
        );

        let count = Layout::Chest(3).slot_count();
        for _ in 0..3 {
            step(
                GuiInput::default(),
                &snapshot,
                &mut state,
                &mut cursor,
                &mut out,
            );
            retire_menu_preview(&mut state.menu_preview, &snapshot, count);
        }
        assert_eq!(out.len(), 1, "the release must not re-fire");
        assert!(state.menu_preview.slot(9).is_some(), "still waiting");

        let mut applied = snap(Gamemode::Survival, &[(9, stack("stone", 8))]);
        let mut resized = applied.menu_slots.to_vec();
        resized.resize(count, SlotStack::default());
        applied.menu_slots = resized.into();
        retire_menu_preview(&mut state.menu_preview, &applied, count);
        assert!(state.menu_preview.slot(9).is_none());
    }

    #[test]
    fn a_container_pickup_is_predicted() {
        let snapshot = snap(Gamemode::Survival, &[(3, stack("stone", 20))]);
        let mut out = Vec::new();
        let (cursor, preview) = slot_clicks(
            &left(),
            Mode::Protocol,
            Layout::Chest(3),
            3,
            &snapshot,
            &SlotStack::default(),
            &mut out,
        );
        assert_eq!(
            out.iter().map(describe).collect::<Vec<_>>(),
            vec!["pickup_left(3)"]
        );
        assert_eq!(cursor.count, 20);
        assert!(preview.unwrap()[3].is_empty());
    }

    #[test]
    fn a_container_quick_move_is_left_to_the_server() {
        let snapshot = snap(Gamemode::Survival, &[(3, stack("stone", 20))]);
        let mut out = Vec::new();
        let shift = GuiInput {
            shift: true,
            ..left()
        };
        let (cursor, preview) = slot_clicks(
            &shift,
            Mode::Protocol,
            Layout::Chest(3),
            3,
            &snapshot,
            &SlotStack::default(),
            &mut out,
        );
        assert_eq!(out.len(), 1);
        assert!(cursor.is_empty());
        assert!(preview.is_none());
    }

    #[test]
    fn an_output_slot_gives_its_whole_result_and_takes_nothing() {
        let snapshot = snap(Gamemode::Survival, &[(0, stack("stick", 4))]);
        let mut menu = MenuModel::for_layout(Layout::Crafting, &snapshot, SlotStack::default());
        menu.pickup(0, true);
        assert_eq!(menu.cursor.count, 4, "a right click takes the whole result");
        assert!(menu.slots[0].is_empty());

        let mut menu = MenuModel::for_layout(Layout::Crafting, &snapshot, stack("stick", 62));
        menu.pickup(0, false);
        assert_eq!(
            menu.cursor.count, 62,
            "no room for the result on the cursor"
        );
        assert_eq!(menu.slots[0].count, 4);

        let mut menu = MenuModel::for_layout(Layout::Crafting, &snapshot, stack("stick", 10));
        menu.pickup(0, false);
        assert_eq!(menu.cursor.count, 14);
    }

    #[test]
    fn a_preview_yields_to_any_change_in_the_snapshot() {
        let before = snap(Gamemode::Survival, &[(0, stack("stick", 4))]);
        let mut preview = MenuPreview::default();
        let mut predicted = before.menu_slots.to_vec();
        predicted[0] = SlotStack::default();
        preview.set(predicted, &before);

        let crafted = snap(
            Gamemode::Survival,
            &[(0, stack("stick", 4)), (1, stack("oak_planks", 1))],
        );
        retire_menu_preview(&mut preview, &crafted, MENU_SLOTS);
        assert!(
            preview.slot(0).is_none(),
            "the bot's view replaces the guess"
        );
    }

    #[test]
    fn a_landed_commit_retires_before_its_preview_can_double_count() {
        let carried = stack("stone", 2);
        let before = snap(Gamemode::Survival, &[]);
        let mut drag = dragging(DragKind::Right, &[9]);
        drag.commit(before.carried.clone());

        let (preview, _) = drag
            .slot_preview(Layout::Chest(3), 9, &before, &carried)
            .expect("the in-flight preview stands in for the click");
        assert_eq!(preview.count, 1);

        let mut landed = snap(Gamemode::Survival, &[(9, stack("stone", 1))]);
        landed.carried = stack("stone", 1);
        assert_eq!(
            drag.slot_preview(Layout::Chest(3), 9, &landed, &carried)
                .map(|(s, _)| s.count),
            Some(2),
            "drawn against the landed snapshot the preview would double the slot"
        );

        retire_committed_drag(&mut drag, &landed);
        assert!(
            drag.slot_preview(Layout::Chest(3), 9, &landed, &carried)
                .is_none()
        );
    }

    #[test]
    fn a_new_press_supersedes_a_commit_still_in_flight() {
        let snapshot = snap(Gamemode::Survival, &[]);
        let mut state = SlotState::default();
        let mut cursor = stack("stone", 8);
        let mut out = Vec::new();

        state.drag = dragging(DragKind::Left, &[9]);
        state.drag.remainder = Some(0);
        state.drag.commit(snapshot.carried.clone());

        let press = GuiInput {
            left_click: true,
            left_down: true,
            ..Default::default()
        };
        let ctx = ScreenCtx {
            input: &press,
            vw: 400.0,
            vh: 240.0,
        };
        drag_step(
            &ctx,
            &snapshot,
            Layout::Chest(3),
            Some(11),
            &mut state,
            &mut cursor,
            &mut out,
        );

        assert!(state.drag.active(), "the new gesture owns the drag");
        assert!(!state.drag.committed);
        assert_eq!(
            state.drag.slots,
            vec![11],
            "only the slot the new sweep covers"
        );
        assert!(
            state
                .drag
                .slot_preview(Layout::Chest(3), 9, &snapshot, &cursor)
                .is_none(),
            "the superseded gesture stops drawing"
        );
        assert_eq!(
            state
                .drag
                .slot_preview(Layout::Chest(3), 11, &snapshot, &cursor)
                .map(|(s, _)| s.count),
            Some(8),
            "the new gesture draws its own preview, so the stack stays visible"
        );
    }

    #[test]
    fn a_committed_preview_gives_up_eventually() {
        let snapshot = snap(Gamemode::Survival, &[]);
        let mut drag = dragging(DragKind::Left, &[9]);
        drag.commit(snapshot.carried.clone());
        for _ in 0..COMMIT_MAX_FRAMES {
            assert!(drag.committed);
            retire_committed_drag(&mut drag, &snapshot);
        }
        assert!(
            !drag.committed,
            "the preview gives up after {COMMIT_MAX_FRAMES} frames"
        );
    }

    fn bundle(items: Vec<SlotStack>) -> SlotStack {
        SlotStack {
            item: "bundle",
            count: 1,
            bundle: Some(std::sync::Arc::new(crate::session::BundleView {
                items,
                selected: -1,
                weight: Some(azalea_inventory::bundle::Fraction::ZERO),
                shows: true,
            })),
            ..Default::default()
        }
    }

    fn wheel(
        state: &mut SlotState,
        snap: &Snapshot,
        slot: Option<usize>,
        scroll: f32,
    ) -> Vec<String> {
        let input = GuiInput {
            scroll,
            ..Default::default()
        };
        let mut out = Vec::new();
        bundle_mouse(&input, snap, slot.map(|i| (i, i as i32)), state, &mut out);
        out.iter().map(describe).collect()
    }

    #[test]
    fn the_wheel_selects_only_in_a_bundle_with_items() {
        let filled = [
            (9, bundle(vec![stack("stone", 1)])),
            (10, bundle(Vec::new())),
        ];
        let snap = snap(Gamemode::Survival, &filled);
        let mut state = SlotState::default();
        assert_eq!(
            wheel(&mut state, &snap, Some(9), 1.0),
            ["bundle_select(9,9,1)"]
        );
        assert!(state.wheel_on_bundle);

        let mut state = SlotState::default();
        assert!(wheel(&mut state, &snap, Some(10), 1.0).is_empty());
        assert!(!state.wheel_on_bundle);
    }

    #[test]
    fn leaving_a_bundle_slot_unselects_it() {
        let snap = snap(
            Gamemode::Survival,
            &[(9, bundle(Vec::new())), (10, stack("stone", 1))],
        );
        let mut state = SlotState::default();
        assert!(wheel(&mut state, &snap, Some(9), 0.0).is_empty());
        assert!(wheel(&mut state, &snap, Some(9), 0.0).is_empty());
        assert_eq!(
            wheel(&mut state, &snap, Some(10), 0.0),
            ["bundle_select(9,9,0)"]
        );
        assert!(wheel(&mut state, &snap, None, 0.0).is_empty());
    }

    #[test]
    fn a_quick_move_unselects_before_it_clicks() {
        let shift = GuiInput {
            left_click: true,
            shift: true,
            ..Default::default()
        };
        let (acts, _) = run(
            Gamemode::Survival,
            &[(9, bundle(Vec::new()))],
            SlotStack::default(),
            9,
            shift,
        );
        assert_eq!(acts[0], "bundle_select(9,9,0)");
        assert_eq!(acts.len(), 2);
    }

    #[test]
    fn a_protocol_bundle_click_is_not_predicted() {
        let filled = [(9, bundle(Vec::new()))];
        let snapshot = snap(Gamemode::Survival, &filled);
        let mut out = Vec::new();
        let cursor = stack("stone", 4);
        let mut menu = MenuModel::for_layout(Layout::PlayerMenu, &snapshot, cursor.clone());
        let click = SlotClick::Pickup { right: false };
        apply_click(
            Mode::Protocol,
            Layout::PlayerMenu,
            click,
            9,
            &mut menu,
            &mut out,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(menu.cursor, cursor);
    }

    #[test]
    fn a_creative_bundle_click_carries_the_cursor_to_the_bot_thread() {
        let filled = [(9, bundle(Vec::new()))];
        let (acts, after) = run(
            Gamemode::Creative,
            &filled,
            SlotStack::default(),
            9,
            right(),
        );
        assert_eq!(acts, ["creative_bundle_click(9,true,x0)"]);
        assert!(after.is_empty());
    }

    fn answer(cursor: SlotStack) -> CreativeAnswer {
        CreativeAnswer {
            cursor,
            slots: vec![SlotStack::default(); MENU_SLOTS].into(),
        }
    }

    fn pending_click() -> SlotState {
        let mut state = SlotState::default();
        state.note_bundle_click(&[InvAction::CreativeBundleClick {
            slot: 9,
            right: true,
            cursor: Box::default(),
        }]);
        state
    }

    #[test]
    fn a_landed_creative_click_is_the_cursor() {
        let snapshot = snap(Gamemode::Creative, &[]);
        let mut state = pending_click();
        assert!(state.clicks_held());
        let mut out = Vec::new();
        state.land_bundle_click(answer(stack("stone", 2)), &snapshot, &mut out);
        assert!(!state.clicks_held());
        assert!(out.is_empty());
        assert_eq!(state.cursor(&snapshot).count, 2);
    }

    #[test]
    fn a_screen_closed_on_a_creative_click_stows_the_answer() {
        let snapshot = snap(Gamemode::Creative, &[]);
        let mut state = pending_click();
        assert!(state.stow_on_landing());
        state.reset_for_screen_change();

        let mut out = Vec::new();
        state.land_bundle_click(answer(stack("stone", 3)), &snapshot, &mut out);
        let acts: Vec<_> = out.iter().map(describe).collect();
        assert_eq!(acts.len(), 1);
        assert!(acts[0].contains("stone"), "{acts:?}");
        assert!(state.cursor(&snapshot).is_empty());

        let mut state = pending_click();
        assert!(state.stow_on_landing());
        let mut out = Vec::new();
        state.land_bundle_click(answer(SlotStack::default()), &snapshot, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn an_idle_screen_stows_its_own_cursor() {
        assert!(!SlotState::default().stow_on_landing());
    }
}
