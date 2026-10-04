use std::borrow::Cow;
use std::sync::OnceLock;

use crate::generated_items::{CREATIVE_TABS, TabRow, item};
use crate::gui::painter::Painter;
use crate::gui::screens::dim_background;
use crate::gui::slots::{self, Drag, Layout, MenuModel, Mode, SlotClick, SlotState};
use crate::gui::widgets::TextBox;
use crate::gui::{ScreenCtx, Snapshot};
use crate::items::potions;
use crate::session::{InvAction, SlotStack};

const IMAGE_W: f32 = 195.0;
const IMAGE_H: f32 = 136.0;

const GRID_X: f32 = 9.0;
const GRID_Y: f32 = 18.0;
const PITCH: f32 = 18.0;
const COLS: usize = 9;
const ROWS: usize = 5;
const PAGE: usize = COLS * ROWS;

const HOTBAR_X: f32 = 9.0;
const HOTBAR_Y: f32 = 112.0;

const SCROLL_X: f32 = 175.0;
const SCROLL_Y: f32 = 18.0;
const SCROLL_TRACK_W: f32 = 14.0;
const SCROLL_TRACK_H: f32 = 112.0;
const SCROLLER_W: f32 = 12.0;
const SCROLLER_H: f32 = 15.0;

const SEARCH_X: f32 = 82.0;
const SEARCH_Y: f32 = 6.0;
const SEARCH_W: f32 = 80.0;
const SEARCH_H: f32 = 9.0;

const TAB_W: f32 = 26.0;
const TAB_H: f32 = 32.0;
const TAB_SPACING: f32 = 27.0;

const TITLE_COLOR: u32 = 0x40_4040;

const TAB_SEARCH: usize = CREATIVE_TABS.len();
const TAB_INVENTORY: usize = CREATIVE_TABS.len() + 1;
const TAB_COUNT: usize = CREATIVE_TABS.len() + 2;

pub struct CreativeState {
    pub just_opened: bool,
    pub tab: usize,
    pub scroll: f32,
    pub search: TextBox,
    pub dragging_scrollbar: bool,
    pub visible: Vec<ListItem>,
    pub dirty: bool,
    pub hovered_tab_title: Option<&'static str>,
}

impl CreativeState {
    pub fn search_focused(&self) -> bool {
        self.tab == TAB_SEARCH
    }

    pub fn open(&mut self) {
        self.just_opened = true;
    }
}

impl Default for CreativeState {
    fn default() -> Self {
        CreativeState {
            just_opened: false,
            tab: 0,
            scroll: 0.0,
            search: TextBox::new(50, "Search…"),
            dragging_scrollbar: false,
            visible: Vec::new(),
            dirty: true,
            hovered_tab_title: None,
        }
    }
}

#[derive(Clone, Copy)]
struct TabInfo {
    title: &'static str,
    icon: &'static str,
    top_row: bool,
    column: u8,
    aligned_right: bool,
    background: &'static str,
    can_scroll: bool,
    show_title: bool,
}

fn tab_info(i: usize) -> TabInfo {
    match i {
        TAB_SEARCH => TabInfo {
            title: "Search Items",
            icon: "compass",
            top_row: true,
            column: 6,
            aligned_right: true,
            background: "container/creative_inventory/tab_item_search",
            can_scroll: true,
            show_title: true,
        },
        TAB_INVENTORY => TabInfo {
            title: "Survival Inventory",
            icon: "chest",
            top_row: false,
            column: 6,
            aligned_right: true,
            background: "container/creative_inventory/tab_inventory",
            can_scroll: false,
            show_title: false,
        },
        _ => {
            let t = &CREATIVE_TABS[i.min(CREATIVE_TABS.len() - 1)];
            TabInfo {
                title: t.title,
                icon: t.icon,
                top_row: t.row == TabRow::Top,
                column: t.column,
                aligned_right: false,
                background: "container/creative_inventory/tab_items",
                can_scroll: true,
                show_title: true,
            }
        }
    }
}

fn tab_x(t: &TabInfo) -> f32 {
    if t.aligned_right {
        IMAGE_W - TAB_SPACING * (7.0 - t.column as f32) + 1.0
    } else {
        TAB_SPACING * t.column as f32
    }
}

fn tab_hit_y(t: &TabInfo) -> f32 {
    if t.top_row { -TAB_H } else { IMAGE_H }
}

fn tab_draw_y(t: &TabInfo) -> f32 {
    if t.top_row { -28.0 } else { IMAGE_H - 4.0 }
}

const TAB_SPRITES: [[[&str; 7]; 2]; 2] = [
    [
        [
            "container/creative_inventory/tab_top_unselected_1",
            "container/creative_inventory/tab_top_unselected_2",
            "container/creative_inventory/tab_top_unselected_3",
            "container/creative_inventory/tab_top_unselected_4",
            "container/creative_inventory/tab_top_unselected_5",
            "container/creative_inventory/tab_top_unselected_6",
            "container/creative_inventory/tab_top_unselected_7",
        ],
        [
            "container/creative_inventory/tab_top_selected_1",
            "container/creative_inventory/tab_top_selected_2",
            "container/creative_inventory/tab_top_selected_3",
            "container/creative_inventory/tab_top_selected_4",
            "container/creative_inventory/tab_top_selected_5",
            "container/creative_inventory/tab_top_selected_6",
            "container/creative_inventory/tab_top_selected_7",
        ],
    ],
    [
        [
            "container/creative_inventory/tab_bottom_unselected_1",
            "container/creative_inventory/tab_bottom_unselected_2",
            "container/creative_inventory/tab_bottom_unselected_3",
            "container/creative_inventory/tab_bottom_unselected_4",
            "container/creative_inventory/tab_bottom_unselected_5",
            "container/creative_inventory/tab_bottom_unselected_6",
            "container/creative_inventory/tab_bottom_unselected_7",
        ],
        [
            "container/creative_inventory/tab_bottom_selected_1",
            "container/creative_inventory/tab_bottom_selected_2",
            "container/creative_inventory/tab_bottom_selected_3",
            "container/creative_inventory/tab_bottom_selected_4",
            "container/creative_inventory/tab_bottom_selected_5",
            "container/creative_inventory/tab_bottom_selected_6",
            "container/creative_inventory/tab_bottom_selected_7",
        ],
    ],
];

fn tab_sprite(t: &TabInfo, selected: bool) -> &'static str {
    let edge = usize::from(!t.top_row);
    let state = usize::from(selected);
    let n = (t.column as usize).min(6);
    TAB_SPRITES[edge][state][n]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ListItem {
    Plain(&'static str),
    EnchantedBook {
        enchantment: &'static str,
        level: i32,
    },
    Potion {
        item: &'static str,
        potion: &'static str,
    },
}

impl ListItem {
    fn base_id(&self) -> &'static str {
        match self {
            ListItem::Plain(id) => id,
            ListItem::EnchantedBook { .. } => "enchanted_book",
            ListItem::Potion { item, .. } => item,
        }
    }

    fn enchantments(&self) -> Vec<(String, i32)> {
        match self {
            ListItem::EnchantedBook { enchantment, level } => {
                vec![(enchantment.to_string(), *level)]
            }
            _ => Vec::new(),
        }
    }

    fn potion(&self) -> Option<potions::PotionContents> {
        match self {
            ListItem::Potion { potion, .. } => Some(potions::PotionContents {
                potion: Some((*potion).to_string()),
                ..Default::default()
            }),
            _ => None,
        }
    }

    fn model_key(&self) -> Cow<'static, str> {
        match self.potion() {
            Some(contents) => Cow::Owned(potions::tint_key(self.base_id(), contents.color())),
            None => Cow::Borrowed(self.base_id()),
        }
    }
}

fn enchanted_books(all_levels: bool) -> Vec<ListItem> {
    let mut enchantments = crate::gui::tooltip::all_enchantments();
    enchantments.sort();
    let mut out = Vec::new();
    for (id, max_level) in enchantments {
        if all_levels {
            for level in 1..=max_level {
                out.push(ListItem::EnchantedBook {
                    enchantment: id,
                    level,
                });
            }
        } else {
            out.push(ListItem::EnchantedBook {
                enchantment: id,
                level: max_level,
            });
        }
    }
    out
}

fn potion_variants(item: &'static str) -> Vec<ListItem> {
    potions::POTIONS
        .iter()
        .map(|p| ListItem::Potion { item, potion: p.id })
        .collect()
}

fn expand(id: &'static str) -> Vec<ListItem> {
    if id == "enchanted_book" {
        enchanted_books(false)
    } else if potions::is_potion_item(id) {
        potion_variants(id)
    } else {
        vec![ListItem::Plain(id)]
    }
}

fn tab_items(i: usize) -> Vec<ListItem> {
    CREATIVE_TABS[i]
        .items
        .iter()
        .flat_map(|&id| expand(id))
        .collect()
}

fn search_corpus() -> &'static [ListItem] {
    static CORPUS: OnceLock<Vec<ListItem>> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for tab in CREATIVE_TABS {
            for &id in tab.items {
                if id != "enchanted_book" && seen.insert(id) {
                    out.extend(expand(id));
                }
            }
        }
        out.extend(enchanted_books(true));
        out
    })
}

fn search_filter(query: &str) -> Vec<ListItem> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return search_corpus().to_vec();
    }
    if q.starts_with('#') {
        return Vec::new();
    }
    search_corpus()
        .iter()
        .filter(|it| match it {
            ListItem::Plain(id) => {
                let Some(def) = item(id) else { return false };
                def.name.to_lowercase().contains(&q) || def.id.contains(&q)
            }
            ListItem::EnchantedBook { enchantment, .. } => {
                let name = crate::gui::tooltip::translate(
                    &format!("enchantment.minecraft.{enchantment}"),
                    &[],
                );
                name.to_lowercase().contains(&q)
                    || enchantment.contains(&q)
                    || "enchanted book".contains(&q)
            }
            ListItem::Potion { item, potion } => {
                let key = potions::PotionContents {
                    potion: Some((*potion).to_string()),
                    ..Default::default()
                }
                .name_key(item);
                let name = crate::gui::tooltip::translate(&key, &[]);
                name.to_lowercase().contains(&q) || item.contains(&q) || potion.contains(&q)
            }
        })
        .cloned()
        .collect()
}

fn rebuild(st: &mut CreativeState) {
    st.visible = match st.tab {
        TAB_SEARCH => search_filter(&st.search.text),
        TAB_INVENTORY => Vec::new(),
        i => tab_items(i.min(CREATIVE_TABS.len() - 1)),
    };
    st.dirty = false;
}

fn row_count(len: usize) -> i32 {
    (len as i32).div_euclid(9) + i32::from(len % 9 != 0) - ROWS as i32
}

fn can_scroll(len: usize) -> bool {
    len > PAGE
}

fn row_index_for_scroll(scroll: f32, len: usize) -> usize {
    let rows = row_count(len);
    if rows <= 0 {
        return 0;
    }
    (((scroll * rows as f32) as f64 + 0.5) as i32).max(0) as usize
}

fn scroll_for_row(row: usize, len: usize) -> f32 {
    let rows = row_count(len);
    if rows <= 0 {
        return 0.0;
    }
    (row as f32 / rows as f32).clamp(0.0, 1.0)
}

fn subtract_input_from_scroll(scroll: f32, input: f32, len: usize) -> f32 {
    let rows = row_count(len);
    if rows <= 0 {
        return 0.0;
    }
    (scroll - input / rows as f32).clamp(0.0, 1.0)
}

fn grid_cell_at(mx: f32, my: f32, left: f32, top: f32) -> Option<usize> {
    let rx = mx - (left + GRID_X - 1.0);
    let ry = my - (top + GRID_Y - 1.0);
    if rx < 0.0 || ry < 0.0 {
        return None;
    }
    let (c, r) = ((rx / PITCH) as usize, (ry / PITCH) as usize);
    if c >= COLS || r >= ROWS {
        return None;
    }
    Some(r * COLS + c)
}

fn max_stack(it: &ListItem) -> u8 {
    item(it.base_id()).map(|d| d.max_stack).unwrap_or(64)
}

fn list_stack(it: &ListItem, count: u8) -> SlotStack {
    let id = it.base_id();
    let max_damage = item(id).map(|d| d.max_damage).unwrap_or(0);
    SlotStack {
        item: id,
        count,
        max_damage,
        enchantments: it.enchantments(),
        potion: it.potion(),
        ..Default::default()
    }
}

fn first_free_slot(snap: &Snapshot) -> Option<usize> {
    (36..45).chain(9..36).find(|i| snap.slot(*i).is_empty())
}

pub fn draw(
    p: &mut Painter,
    st: &mut CreativeState,
    slot_state: &mut SlotState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let left = ((ctx.vw - IMAGE_W) / 2.0).floor();
    let top = ((ctx.vh - IMAGE_H) / 2.0).floor();

    st.tab = st.tab.min(TAB_COUNT - 1);
    st.hovered_tab_title = None;

    st.search.focused = st.tab == TAB_SEARCH;
    if std::mem::take(&mut st.just_opened) {
        st.search.sync_keyboard();
    } else if st.search.handle_input(ctx.input) {
        st.dirty = true;
        st.scroll = 0.0;
    }
    if st.dirty {
        let row = row_index_for_scroll(st.scroll, st.visible.len());
        rebuild(st);
        st.scroll = scroll_for_row(row, st.visible.len());
    }

    let info = tab_info(st.tab);
    let len = st.visible.len();
    let scrollable = info.can_scroll && can_scroll(len);

    let mut cursor = slot_state.cursor(snap);

    dim_background(p, ctx.vw, ctx.vh);

    for i in 0..TAB_COUNT {
        if i != st.tab {
            draw_tab(p, &tab_info(i), false, left, top);
        }
    }
    p.blit_sheet(info.background, 0.0, 0.0, IMAGE_W, IMAGE_H, left, top);

    if info.show_title {
        p.text_plain(info.title, left + 8.0, top + SEARCH_Y, TITLE_COLOR, false);
    }
    if st.tab == TAB_SEARCH {
        let frame = p.frame;
        st.search.draw(
            p,
            left + SEARCH_X,
            top + SEARCH_Y,
            SEARCH_W,
            SEARCH_H,
            frame,
        );
    }

    if info.can_scroll {
        scrollbar_input(st, ctx, left, top, scrollable);
        let sprite = if scrollable {
            "container/creative_inventory/scroller"
        } else {
            "container/creative_inventory/scroller_disabled"
        };
        let sy = top + SCROLL_Y + (95.0 * st.scroll).floor();
        p.sprite(sprite, left + SCROLL_X, sy, SCROLLER_W, SCROLLER_H);
    } else {
        st.dragging_scrollbar = false;
    }

    draw_tab(p, &info, true, left, top);

    let mut clicked_tab = None;
    for i in 0..TAB_COUNT {
        let t = tab_info(i);
        let (tx, ty) = (left + tab_x(&t), top + tab_hit_y(&t));
        if ctx.hovering(tx, ty, TAB_W + 1.0, TAB_H + 1.0) {
            if ctx.hovering(tx + 2.0, ty + 2.0, 23.0, 29.0) {
                st.hovered_tab_title = Some(t.title);
            }
            if ctx.input.left_click {
                clicked_tab = Some(i);
            }
        }
    }

    let mut hovered = None;
    let hovered_slot = if st.tab == TAB_INVENTORY {
        draw_inventory_tab(
            p,
            ctx,
            snap,
            left,
            top,
            &slot_state.drag,
            &mut cursor,
            &mut hovered,
            out,
        )
    } else {
        draw_item_grid(p, st, ctx, snap, left, top, &mut cursor, &mut hovered, out);
        draw_hotbar_row(
            p,
            ctx,
            snap,
            left,
            top,
            &slot_state.drag,
            &cursor,
            &mut hovered,
        )
    };

    let consumed = slots::drag_step(
        ctx,
        snap,
        Layout::PlayerMenu,
        hovered_slot,
        slot_state,
        &mut cursor,
        out,
    );
    if !consumed && let Some(slot) = hovered_slot {
        slot_click(slot, snap, ctx, &mut cursor, out, st.tab != TAB_INVENTORY);
    }

    if scrollable && ctx.input.scroll != 0.0 && ctx.hovering(left, top, IMAGE_W, IMAGE_H) {
        st.scroll = subtract_input_from_scroll(st.scroll, ctx.input.scroll, len);
    }

    let on_tab = (0..TAB_COUNT).any(|i| {
        let t = tab_info(i);
        ctx.hovering(
            left + tab_x(&t),
            top + tab_hit_y(&t),
            TAB_W + 1.0,
            TAB_H + 1.0,
        )
    });
    let outside = !ctx.hovering(left, top, IMAGE_W, IMAGE_H) && !on_tab;
    if outside {
        for right in [false, true] {
            let released = if right {
                ctx.input.right_release
            } else {
                ctx.input.left_release
            };
            if released {
                slots::emit_outside_click(Mode::Local, right, &mut cursor, out);
            }
        }
    }

    #[cfg(feature = "mobile_ui")]
    {
        let destroy = if st.tab == TAB_INVENTORY {
            ctx.hovering(left + 173.0 - 1.0, top + 112.0 - 1.0, PITCH, PITCH)
        } else {
            ctx.mouse()
                .and_then(|m| grid_cell_at(m.x, m.y, left, top))
                .is_some()
        };
        if destroy {
            if ctx.input.left_release && !cursor.is_empty() {
                cursor = SlotStack::default();
            }
        } else {
            slots::touch_release_place(
                ctx,
                snap,
                Layout::PlayerMenu,
                Mode::Local,
                hovered_slot,
                consumed,
                slot_state,
                &mut cursor,
                out,
            );
        }
        let target = if destroy {
            slots::DropTarget::Destroy
        } else if outside {
            slots::DropTarget::World
        } else if let Some(i) = hovered_slot
            && let Some(pos) = creative_slot_pos(st.tab, i, left, top)
        {
            slots::DropTarget::Slot(pos, Layout::PlayerMenu.may_place(i, cursor.item))
        } else {
            slots::DropTarget::None
        };
        slots::draw_drop_feedback(p, ctx, target, &cursor);
    }

    if let Some(i) = clicked_tab {
        select_tab(st, i);
    }

    slot_state.set_cursor(cursor.clone(), snap);

    if let Some(title) = st.hovered_tab_title
        && let Some(m) = ctx.mouse()
    {
        draw_tab_tooltip(p, title, m.x, m.y, ctx.vw);
    }

    crate::gui::screens::draw_effects_panel(p, ctx, &snap.active_effects, left, top, IMAGE_W);

    if cursor.is_empty() && st.hovered_tab_title.is_none() {
        hovered
    } else {
        None
    }
}

fn select_tab(st: &mut CreativeState, tab: usize) {
    if st.tab == tab {
        return;
    }
    st.tab = tab;
    st.scroll = 0.0;
    st.dragging_scrollbar = false;
    st.dirty = true;
    st.search.clear();
    st.search.focused = tab == TAB_SEARCH;
}

fn draw_tab(p: &mut Painter, t: &TabInfo, selected: bool, left: f32, top: f32) {
    let x = left + tab_x(t);
    let y = top + tab_draw_y(t);
    p.sprite(tab_sprite(t, selected), x, y, TAB_W, TAB_H);
    let iy = y + 8.0 + if t.top_row { 1.0 } else { -1.0 };
    p.item_icon(t.icon, x + 5.0, iy);
}

fn draw_tab_tooltip(p: &mut Painter, title: &str, mx: f32, my: f32, vw: f32) {
    let w = p.atlas.font.width_str(title);
    let (x, y) = crate::gui::tooltip::position_flat(mx, my, vw, w);
    p.fill(x - 3.0, y - 3.0, w + 6.0, 14.0, 0xF0100010);
    p.text_str(title, x, y, 0xFFFFFF, false);
}

fn scrollbar_input(st: &mut CreativeState, ctx: &ScreenCtx, left: f32, top: f32, scrollable: bool) {
    if ctx.input.left_click
        && ctx.hovering(
            left + SCROLL_X,
            top + SCROLL_Y,
            SCROLL_TRACK_W,
            SCROLL_TRACK_H,
        )
    {
        st.dragging_scrollbar = scrollable;
    }
    if ctx.input.left_release || !ctx.input.left_down {
        st.dragging_scrollbar = false;
    }
    if st.dragging_scrollbar
        && let Some(m) = ctx.mouse()
    {
        st.scroll =
            ((m.y - (top + SCROLL_Y) - 7.5) / (SCROLL_TRACK_H - SCROLLER_H)).clamp(0.0, 1.0);
    }
    if !scrollable {
        st.scroll = 0.0;
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_item_grid(
    p: &mut Painter,
    st: &mut CreativeState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    left: f32,
    top: f32,
    cursor: &mut SlotStack,
    hovered: &mut Option<SlotStack>,
    out: &mut Vec<InvAction>,
) {
    let len = st.visible.len();
    let base = row_index_for_scroll(st.scroll, len) * COLS;
    let hover_cell = ctx.mouse().and_then(|m| grid_cell_at(m.x, m.y, left, top));
    for cell in 0..PAGE {
        let Some(it) = st.visible.get(base + cell).copied() else {
            if hover_cell == Some(cell)
                && !cursor.is_empty()
                && (ctx.input.left_click || ctx.input.right_click)
            {
                *cursor = SlotStack::default();
            }
            continue;
        };
        let x = left + GRID_X + (cell % COLS) as f32 * PITCH;
        let y = top + GRID_Y + (cell / COLS) as f32 * PITCH;
        p.item_icon(&it.model_key(), x, y);
        if hover_cell == Some(cell) {
            p.slot_highlight(x, y);
            *hovered = Some(list_stack(&it, 1));
            let idle_cursor = cursor.is_empty();
            list_click(&it, ctx, cursor, out);
            if ctx.input.left_click && ctx.input.shift && idle_cursor {
                list_shift_click(&it, snap, out);
            }
        }
    }
}

fn same_item(cursor: &SlotStack, it: &ListItem) -> bool {
    cursor.item == it.base_id()
        && cursor.enchantments == it.enchantments()
        && cursor.potion == it.potion()
}

fn list_click(it: &ListItem, ctx: &ScreenCtx, cursor: &mut SlotStack, out: &mut Vec<InvAction>) {
    let input = ctx.input;
    let max = max_stack(it);
    let idle = cursor.is_empty();
    if input.left_click {
        if cursor.is_empty() {
            if input.shift {
                return;
            }
            *cursor = list_stack(it, max);
        } else if same_item(cursor, it) {
            cursor.count = if input.shift {
                max
            } else {
                (cursor.count + 1).min(max)
            };
        } else {
            *cursor = SlotStack::default();
        }
    }
    if input.right_click {
        if cursor.is_empty() {
            *cursor = list_stack(it, 1);
        } else if cursor.count > 1 {
            cursor.count -= 1;
        } else {
            *cursor = SlotStack::default();
        }
    }
    if !idle {
        return;
    }
    for (i, pressed) in input.hotbar_keys.iter().enumerate() {
        if *pressed {
            out.push(InvAction::CreativeSet {
                slot: (36 + i) as u16,
                stack: list_stack(it, max),
            });
        }
    }
    if input.drop_key {
        let count = if input.ctrl { max } else { 1 };
        out.push(InvAction::CreativeDrop {
            stack: list_stack(it, count),
        });
    }
}

fn list_shift_click(it: &ListItem, snap: &Snapshot, out: &mut Vec<InvAction>) {
    if let Some(slot) = first_free_slot(snap) {
        out.push(InvAction::CreativeSet {
            slot: slot as u16,
            stack: list_stack(it, max_stack(it)),
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_hotbar_row(
    p: &mut Painter,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    left: f32,
    top: f32,
    drag: &Drag,
    cursor: &SlotStack,
    hovered: &mut Option<SlotStack>,
) -> Option<usize> {
    let mut hovered_slot = None;
    for i in 0..9 {
        let x = left + HOTBAR_X + i as f32 * PITCH;
        let y = top + HOTBAR_Y;
        slots::draw_menu_slot(
            p,
            Layout::PlayerMenu,
            36 + i,
            snap,
            drag,
            cursor,
            None,
            (x, y),
        );
        let stack = snap.hotbar(i);
        if ctx.hovering(x - 1.0, y - 1.0, PITCH, PITCH) {
            p.slot_highlight(x, y);
            if !stack.is_empty() {
                *hovered = Some(stack.clone());
            }
            hovered_slot = Some(36 + i);
        }
    }
    hovered_slot
}

#[cfg(feature = "mobile_ui")]
fn creative_slot_pos(tab: usize, slot: usize, left: f32, top: f32) -> Option<(f32, f32)> {
    if tab == TAB_INVENTORY {
        return inventory_tab_slot_pos(slot).map(|(x, y)| (left + x, top + y));
    }
    (36..45)
        .contains(&slot)
        .then(|| (left + HOTBAR_X + (slot - 36) as f32 * PITCH, top + HOTBAR_Y))
}

fn inventory_tab_slot_pos(i: usize) -> Option<(f32, f32)> {
    match i {
        0..=4 => None,
        5..=8 => {
            let pos = i - 5;
            Some((
                54.0 + (pos / 2) as f32 * 54.0,
                6.0 + (pos % 2) as f32 * 27.0,
            ))
        }
        45 => Some((35.0, 20.0)),
        _ => {
            let pos = i - 9;
            let y = if i >= 36 {
                112.0
            } else {
                54.0 + (pos / 9) as f32 * PITCH
            };
            Some((9.0 + (pos % 9) as f32 * PITCH, y))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_inventory_tab(
    p: &mut Painter,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    left: f32,
    top: f32,
    drag: &Drag,
    cursor: &mut SlotStack,
    hovered: &mut Option<SlotStack>,
    out: &mut Vec<InvAction>,
) -> Option<usize> {
    let mut hovered_slot = None;
    for i in 5..=45 {
        let Some((sx, sy)) = inventory_tab_slot_pos(i) else {
            continue;
        };
        let (x, y) = (left + sx, top + sy);
        slots::draw_menu_slot(p, Layout::PlayerMenu, i, snap, drag, cursor, None, (x, y));
        let stack = snap.slot(i);
        if ctx.hovering(x - 1.0, y - 1.0, PITCH, PITCH) {
            p.slot_highlight(x, y);
            if !stack.is_empty() {
                *hovered = Some(stack.clone());
            }
            hovered_slot = Some(i);
        }
    }

    let (dx, dy) = (left + 173.0, top + 112.0);
    if ctx.hovering(dx - 1.0, dy - 1.0, PITCH, PITCH) {
        p.slot_highlight(dx, dy);
        if ctx.input.shift && (ctx.input.left_click || ctx.input.right_click) {
            let mut menu = MenuModel::from_snapshot(snap, cursor.clone());
            for slot in menu.slots.iter_mut().skip(1) {
                *slot = SlotStack::default();
            }
            menu.emit_creative_sets(snap, out);
        } else if ctx.input.left_click || ctx.input.right_click {
            *cursor = SlotStack::default();
        }
    }
    hovered_slot
}

fn slot_click(
    slot: usize,
    snap: &Snapshot,
    ctx: &ScreenCtx,
    cursor: &mut SlotStack,
    out: &mut Vec<InvAction>,
    items_tab: bool,
) {
    let clicks = slots::resolve_slot_clicks(
        ctx.input,
        cursor.is_empty(),
        true,
        !snap.slot(slot).is_empty(),
    );
    if clicks.is_empty() {
        return;
    }
    let mut menu = MenuModel::from_snapshot(snap, cursor.clone());
    for click in clicks {
        if items_tab && matches!(click, SlotClick::QuickMove { .. }) {
            menu.slots[slot] = SlotStack::default();
            continue;
        }
        slots::apply_click(Mode::Local, Layout::PlayerMenu, click, slot, &mut menu, out);
    }
    menu.emit_creative_sets(snap, out);
    *cursor = menu.cursor;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[ListItem]) -> Vec<&str> {
        list.iter().map(ListItem::base_id).collect()
    }

    #[test]
    fn search_matches_name_and_id() {
        let by_name = search_filter("Diamond Sword");
        assert!(
            ids(&by_name).contains(&"diamond_sword"),
            "name query missed the sword"
        );
        let by_id = search_filter("diamond_pick");
        assert!(ids(&by_id).contains(&"diamond_pickaxe"));
        assert!(!ids(&by_id).contains(&"diamond_sword"));
    }

    #[test]
    fn potions_expand_per_registry_entry() {
        let food = CREATIVE_TABS
            .iter()
            .position(|t| t.id == "food_and_drinks")
            .expect("food tab");
        let items = tab_items(food);
        for item in ["potion", "splash_potion", "lingering_potion"] {
            let cells: Vec<&str> = items
                .iter()
                .filter_map(|it| match it {
                    ListItem::Potion { item: i, potion } if *i == item => Some(*potion),
                    _ => None,
                })
                .collect();
            assert_eq!(cells.len(), potions::POTIONS.len(), "{item}");
            assert_eq!(cells[0], "water");
        }
        let combat = CREATIVE_TABS
            .iter()
            .position(|t| t.id == "combat")
            .expect("combat tab");
        assert_eq!(
            tab_items(combat)
                .iter()
                .filter(|it| matches!(
                    it,
                    ListItem::Potion {
                        item: "tipped_arrow",
                        ..
                    }
                ))
                .count(),
            potions::POTIONS.len()
        );
    }

    #[test]
    fn search_finds_a_potion_by_its_effect() {
        let hits = search_filter("night vision");
        let potions: Vec<&ListItem> = hits
            .iter()
            .filter(|it| matches!(it, ListItem::Potion { .. }))
            .collect();
        assert_eq!(potions.len(), 8, "{:?}", ids(&hits));
        assert!(
            search_filter("splash_potion")
                .iter()
                .all(|it| it.base_id() == "splash_potion")
        );
    }

    #[test]
    fn search_is_case_insensitive() {
        assert_eq!(search_filter("STONE"), search_filter("stone"));
        assert!(!search_filter("stone").is_empty());
    }

    #[test]
    fn tag_query_is_empty() {
        assert!(search_filter("#planks").is_empty());
        assert!(search_filter("#").is_empty());
    }

    #[test]
    fn empty_query_returns_everything() {
        assert_eq!(search_filter("").len(), search_corpus().len());
        assert!(search_corpus().len() > 500);
    }

    #[test]
    fn row_arithmetic() {
        assert_eq!(row_count(300), 29);
        assert_eq!(row_index_for_scroll(0.0, 300), 0);
        assert_eq!(row_index_for_scroll(0.5, 300), 15);
        assert_eq!(row_index_for_scroll(1.0, 300), 29);
        assert_eq!(scroll_for_row(0, 300), 0.0);
        assert_eq!(scroll_for_row(29, 300), 1.0);
        assert_eq!((29 + ROWS) * COLS, 306);
    }

    #[test]
    fn short_lists_never_scroll() {
        assert!(!can_scroll(45));
        assert!(can_scroll(46));
        assert!(row_count(45) <= 0);
        assert_eq!(row_index_for_scroll(1.0, 45), 0);
        assert_eq!(subtract_input_from_scroll(0.0, -1.0, 45), 0.0);
    }

    #[test]
    fn wheel_steps_one_row() {
        let s = subtract_input_from_scroll(0.0, -1.0, 300);
        assert_eq!(row_index_for_scroll(s, 300), 1);
        let top = subtract_input_from_scroll(s, 1.0, 300);
        assert_eq!(row_index_for_scroll(top, 300), 0);
        assert_eq!(subtract_input_from_scroll(1.0, -1.0, 300), 1.0);
    }

    #[test]
    fn cursor_to_grid_cell() {
        assert_eq!(grid_cell_at(9.0, 18.0, 0.0, 0.0), Some(0));
        assert_eq!(grid_cell_at(8.0, 17.0, 0.0, 0.0), Some(0));
        assert_eq!(grid_cell_at(7.9, 17.0, 0.0, 0.0), None);
        assert_eq!(grid_cell_at(26.0, 17.0, 0.0, 0.0), Some(1));
        assert_eq!(grid_cell_at(9.0, 35.0, 0.0, 0.0), Some(COLS));
        assert_eq!(grid_cell_at(153.0, 90.0, 0.0, 0.0), Some(44));
        assert_eq!(grid_cell_at(170.0, 107.0, 0.0, 0.0), None);
        assert_eq!(grid_cell_at(109.0, 68.0, 100.0, 50.0), Some(0));
    }

    #[test]
    fn tab_positions_match_vanilla() {
        let building = tab_info(0);
        assert_eq!(tab_x(&building), 0.0);
        assert_eq!(tab_hit_y(&building), -32.0);
        assert_eq!(tab_draw_y(&building), -28.0);
        assert_eq!(
            tab_sprite(&building, true),
            "container/creative_inventory/tab_top_selected_1"
        );

        let search = tab_info(TAB_SEARCH);
        assert_eq!(tab_x(&search), IMAGE_W - 27.0 + 1.0);
        assert_eq!(
            tab_sprite(&search, false),
            "container/creative_inventory/tab_top_unselected_7"
        );

        let inventory = tab_info(TAB_INVENTORY);
        assert_eq!(tab_draw_y(&inventory), 132.0);
        assert_eq!(tab_hit_y(&inventory), 136.0);
        assert!(!inventory.can_scroll);
        assert_eq!(
            tab_sprite(&inventory, true),
            "container/creative_inventory/tab_bottom_selected_7"
        );
    }

    #[test]
    fn inventory_tab_slot_layout() {
        assert_eq!(inventory_tab_slot_pos(0), None);
        assert_eq!(inventory_tab_slot_pos(5), Some((54.0, 6.0)));
        assert_eq!(inventory_tab_slot_pos(8), Some((108.0, 33.0)));
        assert_eq!(inventory_tab_slot_pos(9), Some((9.0, 54.0)));
        assert_eq!(inventory_tab_slot_pos(36), Some((9.0, 112.0)));
        assert_eq!(inventory_tab_slot_pos(45), Some((35.0, 20.0)));
    }
}

#[cfg(test)]
mod visual {
    use std::sync::Arc;

    use super::*;
    use crate::gui::atlas::build_gui_atlas;
    use crate::gui::preview::{output_dir, rasterize, upscale};
    use crate::gui::render::GuiInput;
    use crate::gui::{GuiState, Screen};
    use crate::session::{Gamemode, SessionState, SharedMutex, SharedState};
    use bevy::prelude::Vec2;

    #[test]
    fn preview_creative_states() {
        let atlas = build_gui_atlas(&crate::assets_root());
        let (vw, vh) = (427.0_f32, 240.0_f32);
        let shared: Arc<SharedMutex> = Arc::new(SharedMutex::new(SharedState {
            in_world: true,
            session: SessionState {
                menu_slots: vec![SlotStack::default(); 46],
                gamemode: Gamemode::Creative,
                ..Default::default()
            },
            ..Default::default()
        }));
        let mk = |mouse: Option<Vec2>| GuiInput {
            size: Vec2::new(vw, vh),
            scale: 3.0,
            mouse,
            ..Default::default()
        };
        let cases: Vec<(usize, f32, Option<Vec2>, &str)> = vec![
            (
                TAB_SEARCH,
                0.0,
                Some(Vec2::new(vw / 2.0 + 80.0, vh / 2.0 - 80.0)),
                "cr_search.png",
            ),
            (TAB_INVENTORY, 0.0, None, "cr_inventory.png"),
            (0, 1.0, None, "cr_scrolled.png"),
        ];
        for (tab, scroll, mouse, name) in cases {
            let mut state = GuiState {
                screen: Screen::Creative,
                ..Default::default()
            };
            state.creative.tab = tab;
            state.creative.scroll = scroll;
            let mut p = Painter::new(&atlas, 0);
            crate::gui::screens::draw(
                &mut p,
                &mut state,
                &mk(mouse),
                &shared,
                #[cfg(feature = "skins")]
                &mut Default::default(),
            );
            state.creative.scroll = scroll;
            let mut p = Painter::new(&atlas, 0);
            crate::gui::screens::draw(
                &mut p,
                &mut state,
                &mk(mouse),
                &shared,
                #[cfg(feature = "skins")]
                &mut Default::default(),
            );
            let img = rasterize(&p, vw as u32, vh as u32, [70, 110, 170, 255]);
            upscale(&img, 2).save(output_dir().join(name)).unwrap();
        }
    }
}
