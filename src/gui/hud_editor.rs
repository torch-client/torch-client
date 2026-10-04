use std::collections::VecDeque;

use bevy::math::Vec2;

use crate::gui::hud_layout::{
    Anchor, COUNT, ElementId, Frames, GRIDS, Home, HudLayout, MAX_SCALE, MIN_SCALE, Place, Rect,
    StackId, Visibility, snap_scale,
};
use crate::gui::painter::Painter;
#[cfg(not(feature = "mobile_ui"))]
use crate::gui::render::auto_gui_scale;
use crate::gui::{GuiOptions, Screen, ScreenCtx};

const SNAP: f32 = 4.0;
const EDGE_MARGIN: f32 = 2.0;
const SCALE_STEP: f32 = 0.1;
const TOUCH: bool = cfg!(feature = "mobile_ui");
const HANDLE: f32 = 4.0;
const BAR_H: f32 = if TOUCH { 16.0 } else { 10.0 };
const DRAG_THRESHOLD: f32 = if TOUCH { 6.0 } else { 2.0 };
#[cfg(feature = "mobile_ui")]
const LONG_PRESS: f32 = 0.4;
const UNDO_DEPTH: usize = 64;

const OUTLINE: u32 = 0x70FF_FFFF;
const OUTLINE_HOT: u32 = 0xFFFF_FFFF;
const HIDDEN_OUTLINE: u32 = 0xFFFF_5555;
const HIDDEN_FILL: u32 = 0x8000_0000;
const GUIDE: u32 = 0xFF55_FFFF;
const GRID_LINE: u32 = 0x40FF_FFFF;
const STACK: u32 = 0xFF5B_9BFF;
const STACK_BAR: u32 = 0xD02A_4A80;
const DROP: u32 = 0xFF5B_9BFF;
const OVERLAP: u32 = 0xFFFF_AA00;
const OVERLAP_FILL: u32 = 0x50FF_AA00;
const PANEL: u32 = 0xD010_1010;
const PANEL_HOT: u32 = 0xD040_4040;
const LABEL_BG: u32 = 0xC000_0000;
const HINT: u32 = 0xAAAAAA;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Grab {
    Element(ElementId),
    Stack(StackId),
}

#[derive(Clone, Copy, PartialEq)]
enum DragKind {
    Move,
    Scale,
}

#[derive(Clone, Copy)]
struct Drag {
    grab: Grab,
    kind: DragKind,
    grip: Vec2,
    start: Rect,
    start_scale: f32,
    press: Vec2,
    #[cfg_attr(not(feature = "mobile_ui"), allow(dead_code))]
    pressed_at: f32,
    moving: bool,
}

#[derive(Clone, Copy)]
struct Menu {
    grab: Grab,
    x: f32,
    y: f32,
}

#[derive(Clone, Copy)]
enum Landing {
    Into {
        stack: StackId,
        index: usize,
        line: Rect,
    },
    Pair {
        target: ElementId,
        above: bool,
    },
}

#[derive(Clone, Copy)]
enum Restore {
    Element(ElementId),
    Home(Home),
}

#[cfg_attr(not(feature = "mobile_ui"), allow(dead_code))]
#[derive(Clone, Copy)]
struct Pinch {
    target: Grab,
    start_dist: f32,
    start_scale: f32,
}

#[derive(Clone, Copy)]
enum Act {
    Hide,
    Reset,
    Remove,
    Split,
    Delete,
    More,
}

#[derive(Clone, Copy)]
struct Guide {
    vertical: bool,
    at: f32,
}

#[derive(Default)]
pub struct EditorState {
    pub layout: HudLayout,
    pub frames: Frames,
    pub parent: Screen,
    pub over_menu: bool,
    drag: Option<Drag>,
    selected: Option<Grab>,
    menu: Option<Menu>,
    undo: VecDeque<HudLayout>,
    scroll_run: Option<Grab>,
    guides: [Option<Guide>; 2],
    drop: Option<Landing>,
    adding: bool,
    free: bool,
    touches: Vec<Vec2>,
    pinch: Option<Pinch>,
    scaled_at: f32,
}

impl EditorState {
    pub fn load() -> Self {
        EditorState {
            layout: HudLayout::load(),
            ..Default::default()
        }
    }

    pub fn opened(&mut self) {
        self.drag = None;
        self.selected = None;
        self.menu = None;
        self.undo.clear();
        self.scroll_run = None;
        self.guides = [None; 2];
        self.drop = None;
        self.adding = false;
        self.pinch = None;
        self.touches.clear();
    }

    #[cfg(feature = "mobile_ui")]
    pub fn set_touches(&mut self, touches: impl Iterator<Item = Vec2>) {
        self.touches.clear();
        self.touches.extend(touches);
    }

    pub fn close(&mut self) {
        self.drag = None;
        self.menu = None;
        self.drop = None;
        self.layout.save();
    }

    pub fn escape(&mut self) -> bool {
        std::mem::take(&mut self.adding) || self.menu.take().is_some()
    }

    fn push_undo(&mut self) {
        if self.undo.len() == UNDO_DEPTH {
            self.undo.pop_front();
        }
        self.undo.push_back(self.layout.clone());
        self.scroll_run = None;
    }

    fn stack_rect(&self, si: StackId) -> Option<Rect> {
        self.frames.stack_rect(si)
    }

    fn rect_of(&self, g: Grab) -> Option<Rect> {
        match g {
            Grab::Element(id) => self.frames.screen[id.index()],
            Grab::Stack(si) => self.stack_rect(si),
        }
    }

    fn scale_of(&self, g: Grab) -> f32 {
        match g {
            Grab::Element(id) => self.layout.scale_of(id),
            Grab::Stack(si) => self.layout.stack(si).map_or(1.0, |s| s.scale),
        }
    }

    fn scale_target(&self, g: Grab) -> Grab {
        match g {
            Grab::Element(id) => self.layout.stack_of(id).map_or(g, Grab::Stack),
            g => g,
        }
    }

    fn place_element(&mut self, id: ElementId, rect: Rect, scale: f32, vw: f32, vh: f32) {
        let anchor = Anchor::nearest(rect, vw, vh);
        let offset = anchor.offset_for(vw, vh, rect.size(), rect.pos());
        self.layout.set_free(id, anchor, offset, scale);
    }

    fn place_stack(&mut self, si: StackId, rect: Rect, vw: f32, vh: f32) {
        let Some(stack) = self.layout.stack_mut(si) else {
            return;
        };
        stack.moved = true;
        stack.anchor = Anchor::nearest(rect, vw, vh);
        stack.offset = stack.anchor.offset_for(vw, vh, rect.size(), rect.pos());
    }

    fn set_scale(&mut self, g: Grab, scale: f32, vw: f32, vh: f32, now: f32) {
        let scale = scale.clamp(MIN_SCALE, MAX_SCALE);
        match self.scale_target(g) {
            Grab::Stack(si) => {
                if let Some(s) = self.layout.stack_mut(si) {
                    s.scale = scale;
                }
            }
            Grab::Element(id) => match self.layout.get(id).place {
                Place::Free { anchor, offset, .. } => {
                    self.layout.set_free(id, anchor, offset, scale)
                }
                _ => {
                    let Some(rect) = self.frames.screen[id.index()] else {
                        return;
                    };
                    self.place_element(id, rect, scale, vw, vh);
                }
            },
        }
        self.scaled_at = now;
    }

    fn handle_for(&self, g: Grab) -> Option<(Rect, Grab)> {
        let target = self.scale_target(g);
        Some((handle_rect(self.rect_of(target)?), target))
    }

    fn stack_for(&self, g: Grab) -> Option<StackId> {
        match g {
            Grab::Stack(si) => Some(si),
            Grab::Element(id) => self.layout.stack_of(id),
        }
    }

    fn hot_at(&self, m: Vec2, vh: f32) -> Option<Grab> {
        for &(si, r) in &self.frames.stacks {
            if bar_rect(r, vh).contains(m) {
                return Some(Grab::Stack(si));
            }
        }
        ElementId::ALL
            .into_iter()
            .filter_map(|id| self.frames.screen[id.index()].map(|r| (id, r)))
            .filter(|(_, r)| r.contains(m))
            .min_by(|(_, a), (_, b)| (a.w * a.h).total_cmp(&(b.w * b.h)))
            .map(|(id, _)| Grab::Element(id))
    }

    fn find_drop(&self, id: ElementId, m: Vec2) -> Option<Landing> {
        if !id.stackable() {
            return None;
        }
        for &(si, r) in &self.frames.stacks {
            let grown = Rect::new(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0);
            if !grown.contains(m) || self.layout.stack(si).is_none() {
                continue;
            }
            let members = self.layout.members(si);
            let mut index = members.len();
            let mut at = r.y + r.h;
            for (k, member) in members.iter().enumerate() {
                if let Some(mr) = self.frames.screen[member.index()]
                    && m.y < mr.y + mr.h / 2.0
                {
                    index = k;
                    at = mr.y;
                    break;
                }
            }
            return Some(Landing::Into {
                stack: si,
                index,
                line: Rect::new(r.x, at - 1.0, r.w, 2.0),
            });
        }
        for other in ElementId::ALL {
            let e = self.layout.get(other);
            if other == id || !other.stackable() || e.visibility != Visibility::Shown {
                continue;
            }
            if !matches!(e.place, Place::Free { .. }) {
                continue;
            }
            if let Some(r) = self.frames.screen[other.index()]
                && r.contains(m)
            {
                return Some(Landing::Pair {
                    target: other,
                    above: m.y < r.y + r.h / 2.0,
                });
            }
        }
        None
    }

    fn apply_drop(&mut self, id: ElementId, rect: Rect, drop: Landing, vw: f32, vh: f32) {
        match drop {
            Landing::Into { stack, index, .. } => self.layout.insert(id, stack, index),
            Landing::Pair { target, above } => {
                let Some(tr) = self.frames.screen[target.index()] else {
                    return;
                };
                let top = if above { tr.y - rect.h } else { tr.y };
                let size = Vec2::new(tr.w.max(rect.w), tr.h + rect.h);
                let pos = Vec2::new(tr.x, top);
                let r = Rect::new(pos.x, pos.y, size.x, size.y);
                let anchor = Anchor::nearest(r, vw, vh);
                let offset = anchor.offset_for(vw, vh, size, pos);
                let scale = self.layout.scale_of(target);
                let members = if above { [id, target] } else { [target, id] };
                self.layout.new_stack(anchor, offset, scale, &members);
            }
        }
    }
}

fn handle_rect(r: Rect) -> Rect {
    Rect::new(r.x + r.w - HANDLE, r.y + r.h - HANDLE, HANDLE, HANDLE)
}

fn bar_rect(r: Rect, vh: f32) -> Rect {
    let w = r.w.max(96.0);
    if r.y >= BAR_H + 1.0 || r.y + r.h + BAR_H + 1.0 > vh {
        Rect::new(r.x, r.y - BAR_H - 1.0, w, BAR_H)
    } else {
        Rect::new(r.x, r.y + r.h + 1.0, w, BAR_H)
    }
}

fn snap_axis(lo: f32, len: f32, lines: &[(f32, u8)]) -> (f32, Option<f32>) {
    let mut best: Option<(f32, f32, f32)> = None;
    for &(at, edge) in lines {
        let along = f32::from(edge) * len / 2.0;
        let d = (lo + along - at).abs();
        if d <= SNAP && best.is_none_or(|(bd, ..)| d < bd) {
            best = Some((d, at - along, at));
        }
    }
    match best {
        Some((_, lo, at)) => (lo, Some(at)),
        None => (lo, None),
    }
}

fn lattice(v: f32, extent: f32, grid: f32) -> f32 {
    let centre = (extent / 2.0).floor();
    centre + ((v - centre) / grid).round() * grid
}

fn grid_axis(lo: f32, len: f32, extent: f32, grid: f32) -> f32 {
    if grid <= 0.0 {
        return lo.round();
    }
    let mid = lo + len / 2.0;
    if mid < extent / 3.0 {
        lattice(lo, extent, grid)
    } else if mid > extent * 2.0 / 3.0 {
        lattice(lo + len, extent, grid) - len
    } else {
        lattice(mid, extent, grid) - len / 2.0
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "one drag's whole snapping context"
)]
fn snap(
    pos: Vec2,
    size: Vec2,
    exclude: &[ElementId],
    skip_stack: Option<StackId>,
    frames: &Frames,
    vw: f32,
    vh: f32,
    grid: f32,
) -> (Vec2, [Option<Guide>; 2]) {
    let mut xs: Vec<(f32, u8)> = vec![
        (EDGE_MARGIN, 0),
        (vw - EDGE_MARGIN, 2),
        ((vw / 2.0).floor(), 1),
    ];
    let mut ys: Vec<(f32, u8)> = vec![
        (EDGE_MARGIN, 0),
        (vh - EDGE_MARGIN, 2),
        ((vh / 2.0).floor(), 1),
    ];
    let others = ElementId::ALL
        .into_iter()
        .filter(|id| !exclude.contains(id))
        .filter_map(|id| frames.screen[id.index()])
        .chain(
            frames
                .stacks
                .iter()
                .filter(|(si, _)| Some(*si) != skip_stack)
                .map(|(_, r)| *r),
        );
    for r in others {
        xs.extend([
            (r.x, 0),
            (r.x + r.w, 2),
            (r.x + r.w, 0),
            (r.x, 2),
            (r.x + r.w / 2.0, 1),
        ]);
        ys.extend([
            (r.y, 0),
            (r.y + r.h, 2),
            (r.y + r.h, 0),
            (r.y, 2),
            (r.y + r.h / 2.0, 1),
        ]);
    }
    let (x, gx) = snap_axis(pos.x, size.x, &xs);
    let (y, gy) = snap_axis(pos.y, size.y, &ys);
    let x = if gx.is_some() {
        x
    } else {
        grid_axis(x, size.x, vw, grid)
    };
    let y = if gy.is_some() {
        y
    } else {
        grid_axis(y, size.y, vh, grid)
    };
    (
        Vec2::new(x, y),
        [
            gx.map(|at| Guide { vertical: true, at }),
            gy.map(|at| Guide {
                vertical: false,
                at,
            }),
        ],
    )
}

fn action_bar(
    st: &EditorState,
    g: Grab,
    vw: f32,
) -> Option<(Rect, Vec<(Rect, &'static str, Act)>)> {
    let r = st.rect_of(g)?;
    let items: Vec<(&'static str, Act)> = match g {
        Grab::Element(id) if id.required() => vec![("Reset", Act::Reset), ("...", Act::More)],
        Grab::Element(id) => {
            let hide = if st.layout.get(id).visibility == Visibility::Shown {
                "Hide"
            } else {
                "Show"
            };
            vec![
                (hide, Act::Hide),
                ("Reset", Act::Reset),
                ("Remove", Act::Remove),
                ("...", Act::More),
            ]
        }
        Grab::Stack(si) => {
            let first = if st.layout.stack(si).is_some_and(|s| s.home.is_some()) {
                ("Reset", Act::Reset)
            } else {
                ("Split", Act::Split)
            };
            vec![first, ("Delete", Act::Delete), ("...", Act::More)]
        }
    };
    const W: f32 = 40.0;
    const H: f32 = 18.0;
    let total = items.len() as f32 * (W + 2.0) - 2.0;
    let x = r.x.clamp(0.0, (vw - total).max(0.0)).floor();
    let above = matches!(g, Grab::Element(_)) && r.y >= H + 3.0;
    let y = if above {
        r.y - H - 3.0
    } else {
        r.y + r.h + 3.0
    }
    .floor();
    let buttons = items
        .into_iter()
        .enumerate()
        .map(|(i, (label, act))| (Rect::new(x + i as f32 * (W + 2.0), y, W, H), label, act))
        .collect();
    Some((Rect::new(x, y, total, H), buttons))
}

fn act_on(st: &mut EditorState, g: Grab, act: Act, vw: f32, vh: f32) {
    let rects: [Option<Rect>; COUNT] = st.frames.screen;
    match (act, g) {
        (Act::More, g) => {
            let (x, y) = st.rect_of(g).map_or((0.0, 0.0), |r| (r.x, r.y));
            st.menu = Some(Menu { grab: g, x, y });
            return;
        }
        (Act::Hide, Grab::Element(id)) => {
            st.push_undo();
            st.layout.get_mut(id).toggle_hidden();
        }
        (Act::Reset, Grab::Element(id)) => {
            st.push_undo();
            st.layout.reset(id);
        }
        (Act::Reset, Grab::Stack(si)) => {
            st.push_undo();
            st.layout.reset_stack_position(si);
        }
        (Act::Remove, Grab::Element(id)) => {
            st.push_undo();
            st.layout.remove(id);
            st.selected = None;
        }
        (Act::Split, Grab::Stack(si)) => {
            st.push_undo();
            st.layout.split(si, &rects, vw, vh);
            st.selected = None;
        }
        (Act::Delete, Grab::Stack(si)) => {
            st.push_undo();
            st.layout.delete_stack(si, &rects, vw, vh);
            st.selected = None;
        }
        _ => {}
    }
}

fn button(p: &mut Painter, ctx: &ScreenCtx, r: Rect, label: &str) -> bool {
    let hot = ctx.mouse().is_some_and(|m| r.contains(m));
    p.fill(r.x, r.y, r.w, r.h, if hot { PANEL_HOT } else { PANEL });
    p.outline(r.x, r.y, r.w, r.h, if hot { OUTLINE_HOT } else { OUTLINE });
    let w = p.atlas.font.width_str(label);
    p.text_plain(
        label,
        (r.x + (r.w - w) / 2.0).floor(),
        (r.y + (r.h - 8.0) / 2.0).floor(),
        0xFFFFFF,
        true,
    );
    hot && ctx.input.left_click
}

fn tag(p: &mut Painter, text: &str, x: f32, y: f32, vw: f32, color: u32) {
    let w = p.atlas.font.width_str(text);
    let x = x.clamp(0.0, (vw - w - 2.0).max(0.0)).floor();
    let y = y.max(0.0).floor();
    p.fill(x, y, w + 2.0, 10.0, LABEL_BG);
    p.text_plain(text, x + 1.0, y + 1.0, color, false);
}

const TOOLBAR_W: f32 = 300.0;
const TOOLBAR_BUTTONS: f32 = 6.0;
const TOOLBAR_H: f32 = if TOUCH { 20.0 } else { 14.0 };
#[cfg(feature = "mobile_ui")]
const HINTS: [&str; 1] = ["Tap: select. Drag: move. Pinch: scale. Hold: options."];
#[cfg(not(feature = "mobile_ui"))]
const HINTS: [&str; 3] = [
    "Drag: move. Drop on a stack or line: stack. Shift: free.",
    "Scroll/corner: scale. Right-click: options. Bar: move stack.",
    "H: hide. Del: remove/delete (Add restores). R: reset. Ctrl+Z: undo.",
];
const MENU_W: f32 = if TOUCH { 160.0 } else { 96.0 };
const MENU_ROW: f32 = if TOUCH { 20.0 } else { 13.0 };

const ADD_W: f32 = if TOUCH { 160.0 } else { 110.0 };

fn add_rect(toolbar: Rect, removed: usize) -> Rect {
    let h = MENU_ROW * (removed.max(1) as f32 + 1.0) + 2.0;
    let bw = (TOOLBAR_W - 2.0 * (TOOLBAR_BUTTONS - 1.0)) / TOOLBAR_BUTTONS;
    let x = toolbar.x + 4.0 * (bw + 2.0) + bw / 2.0 - ADD_W / 2.0;
    Rect::new(x.floor(), toolbar.y - h - 2.0, ADD_W, h)
}

fn toolbar_rect(vw: f32, vh: f32) -> Rect {
    Rect::new(
        ((vw - TOOLBAR_W) / 2.0).floor(),
        (vh / 2.0).floor() + 14.0,
        TOOLBAR_W,
        TOOLBAR_H + 2.0 + HINTS.len() as f32 * 10.0,
    )
}

fn menu_rect(menu: Menu, vw: f32, vh: f32) -> Rect {
    let h = MENU_ROW * 5.0 + 2.0;
    if TOUCH {
        return Rect::new(((vw - MENU_W) / 2.0).floor(), vh - h - 4.0, MENU_W, h);
    }
    Rect::new(
        menu.x.min(vw - MENU_W - 1.0).max(0.0),
        menu.y.min(vh - h - 1.0).max(0.0),
        MENU_W,
        h,
    )
}

fn hairline(ctx: &ScreenCtx) -> f32 {
    if ctx.input.device_scale > 0.0 {
        1.0 / ctx.input.device_scale
    } else {
        1.0
    }
}

pub fn draw_under(p: &mut Painter, st: &EditorState, ctx: &ScreenCtx) {
    let grid = GRIDS[st.layout.grid.min(GRIDS.len() - 1)];
    let moving = st
        .drag
        .is_some_and(|d| d.kind == DragKind::Move && d.moving);
    if grid <= 0.0 || ctx.input.shift || st.free || !moving {
        return;
    }
    let (vw, vh) = (ctx.vw, ctx.vh);
    let line = hairline(ctx);
    for (extent, vertical) in [(vw, true), (vh, false)] {
        let centre = (extent / 2.0).floor();
        let mut at = centre - (centre / grid).floor() * grid;
        while at < extent {
            if vertical {
                p.fill(at, 0.0, line, vh, GRID_LINE);
            } else {
                p.fill(0.0, at, vw, line, GRID_LINE);
            }
            at += grid;
        }
    }
}

fn raise(p: &mut Painter, frames: &Frames, ids: &[ElementId]) {
    let mut spans: Vec<(usize, usize)> = ids
        .iter()
        .filter_map(|id| frames.indices[id.index()])
        .filter(|(a, b)| a < b && *b <= p.indices.len())
        .collect();
    spans.sort_unstable();
    spans.dedup();
    let mut taken: Vec<Vec<u32>> = spans
        .iter()
        .rev()
        .map(|&(a, b)| p.indices.drain(a..b).collect())
        .collect();
    taken.reverse();
    for span in taken {
        p.indices.extend_from_slice(&span);
    }
}

#[cfg(not(feature = "mobile_ui"))]
fn step_gui_scale(value: u32, up: bool, ctx: &ScreenCtx) -> u32 {
    let window = ctx.input.scale;
    let max = auto_gui_scale(ctx.vw * window, ctx.vh * window).max(1.0) as u32;
    match (up, value) {
        (true, v) if v >= max => 0,
        (true, v) => v + 1,
        (false, 0) => max,
        (false, v) => v - 1,
    }
}

pub fn draw(
    p: &mut Painter,
    st: &mut EditorState,
    options: &mut GuiOptions,
    ctx: &ScreenCtx,
) -> Option<Screen> {
    let (vw, vh) = (ctx.vw, ctx.vh);
    let input = ctx.input;
    let now = input.time;
    let mouse = ctx.mouse();
    let grid = GRIDS[st.layout.grid.min(GRIDS.len() - 1)];
    let free = input.shift || st.free;

    if input.undo
        && let Some(prev) = st.undo.pop_back()
    {
        st.layout = prev;
        st.drag = None;
        st.menu = None;
        st.drop = None;
        st.scroll_run = None;
    }

    let target = st
        .drag
        .map(|d| d.grab)
        .or(st.selected)
        .or_else(|| mouse.and_then(|m| st.hot_at(m, vh)));
    if input.hud_hide
        && let Some(Grab::Element(id)) = target
        && !id.required()
    {
        st.push_undo();
        st.layout.get_mut(id).toggle_hidden();
        st.drag = None;
        st.drop = None;
    }
    if input.hud_remove
        && let Some(g) = target
        && !matches!(g, Grab::Element(id) if id.required())
    {
        st.push_undo();
        match g {
            Grab::Element(id) => st.layout.remove(id),
            Grab::Stack(si) => {
                let rects: [Option<Rect>; COUNT] = st.frames.screen;
                st.layout.delete_stack(si, &rects, vw, vh);
            }
        }
        st.drag = None;
        st.drop = None;
        st.menu = None;
        st.selected = None;
    }
    if input.hud_reset {
        match target {
            Some(Grab::Element(id)) => {
                st.push_undo();
                st.layout.reset(id);
            }
            Some(Grab::Stack(si)) => {
                if st.layout.stack(si).is_some_and(|s| s.home.is_some()) {
                    st.push_undo();
                    st.layout.reset_stack_position(si);
                }
            }
            None => {}
        }
        st.drag = None;
        st.drop = None;
        st.menu = None;
    }

    let starts_moving = st
        .drag
        .is_some_and(|d| !d.moving && mouse.is_some_and(|m| m.distance(d.press) >= DRAG_THRESHOLD));
    if starts_moving && let Some(mut drag) = st.drag {
        st.push_undo();
        drag.moving = true;
        if let (Grab::Element(id), DragKind::Move) = (drag.grab, drag.kind)
            && !matches!(st.layout.get(id).place, Place::Free { .. })
        {
            let scale = st.layout.scale_of(id);
            st.place_element(id, drag.start, scale, vw, vh);
            drag.start_scale = scale;
        }
        st.drag = Some(drag);
    }

    #[cfg(feature = "mobile_ui")]
    if let Some(d) = st.drag
        && !d.moving
        && input.left_down
        && now - d.pressed_at >= LONG_PRESS
    {
        st.drag = None;
        st.selected = Some(d.grab);
        st.menu = Some(Menu {
            grab: d.grab,
            x: d.press.x,
            y: d.press.y,
        });
    }

    #[cfg(feature = "mobile_ui")]
    let pinching = if st.touches.len() >= 2 {
        let (a, b) = (st.touches[0], st.touches[1]);
        let dist = a.distance(b);
        if st.pinch.is_none()
            && dist > 1.0
            && let Some(g) = st.selected.or_else(|| st.hot_at((a + b) / 2.0, vh))
        {
            let target = st.scale_target(g);
            st.push_undo();
            st.pinch = Some(Pinch {
                target,
                start_dist: dist,
                start_scale: st.scale_of(target),
            });
        }
        if let Some(pinch) = st.pinch {
            let next = (pinch.start_scale * dist / pinch.start_dist * 20.0).round() / 20.0;
            st.set_scale(pinch.target, next, vw, vh, now);
        }
        st.drag = None;
        st.drop = None;
        true
    } else {
        st.pinch = None;
        false
    };
    #[cfg(not(feature = "mobile_ui"))]
    let pinching = false;

    st.guides = [None; 2];
    if let Some(drag) = st.drag {
        match mouse {
            Some(_) if input.left_down && !drag.moving => {}
            Some(m) if input.left_down => {
                let size = drag.start.size();
                let raw = m - drag.grip;
                match (drag.grab, drag.kind) {
                    (Grab::Element(id), DragKind::Move) => {
                        let (pos, guides) = if free {
                            (raw.round(), [None; 2])
                        } else {
                            snap(raw, size, &[id], None, &st.frames, vw, vh, grid)
                        };
                        st.guides = guides;
                        let rect = Rect::new(pos.x, pos.y, size.x, size.y);
                        st.place_element(id, rect, drag.start_scale, vw, vh);
                        st.drop = if free { None } else { st.find_drop(id, m) };
                    }
                    (Grab::Stack(si), DragKind::Move) => {
                        let members = st.layout.members(si);
                        let (pos, guides) = if free {
                            (raw.round(), [None; 2])
                        } else {
                            snap(raw, size, &members, Some(si), &st.frames, vw, vh, grid)
                        };
                        st.guides = guides;
                        st.place_stack(si, Rect::new(pos.x, pos.y, size.x, size.y), vw, vh);
                    }
                    (grab, DragKind::Scale) => {
                        let fx = (m.x - drag.start.x) / drag.start.w.max(1.0);
                        let fy = (m.y - drag.start.y) / drag.start.h.max(1.0);
                        let scale = (drag.start_scale * fx.max(fy)).clamp(MIN_SCALE, MAX_SCALE);
                        let device = input.device_scale;
                        let unit = drag.start.size() / snap_scale(drag.start_scale, device);
                        let new = unit * snap_scale(scale, device);
                        let rect = Rect::new(drag.start.x, drag.start.y, new.x, new.y);
                        match grab {
                            Grab::Element(id) => st.place_element(id, rect, scale, vw, vh),
                            Grab::Stack(si) => {
                                if let Some(s) = st.layout.stack_mut(si) {
                                    s.scale = scale;
                                }
                                st.place_stack(si, rect, vw, vh);
                            }
                        }
                        st.scaled_at = now;
                    }
                }
            }
            _ => {
                if let (Grab::Element(id), DragKind::Move, true, Some(drop)) =
                    (drag.grab, drag.kind, drag.moving, st.drop)
                    && let Some(rect) = st.frames.screen[id.index()]
                {
                    st.apply_drop(id, rect, drop, vw, vh);
                }
                st.drag = None;
                st.drop = None;
            }
        }
    }

    let toolbar = toolbar_rect(vw, vh);
    let menu_box = st.menu.map(|m| menu_rect(m, vw, vh));
    let removed: Vec<Restore> = ElementId::ALL
        .into_iter()
        .filter(|id| st.layout.get(*id).visibility == Visibility::Removed)
        .map(Restore::Element)
        .chain(st.layout.removed_homes().map(Restore::Home))
        .collect();
    let add_box = st.adding.then(|| add_rect(toolbar, removed.len()));
    let actions = if TOUCH && st.drag.is_none() && st.menu.is_none() && !st.adding && !pinching {
        st.selected.and_then(|g| action_bar(st, g, vw))
    } else {
        None
    };
    let over_ui = mouse.is_some_and(|m| {
        toolbar.contains(m)
            || menu_box.is_some_and(|r| r.contains(m))
            || add_box.is_some_and(|r| r.contains(m))
            || actions.as_ref().is_some_and(|(r, _)| r.contains(m))
    });
    let hot = match st.drag {
        Some(d) => Some(d.grab),
        None if over_ui => None,
        None => mouse.and_then(|m| st.hot_at(m, vh)),
    };

    let bw = (TOOLBAR_W - 2.0 * (TOOLBAR_BUTTONS - 1.0)) / TOOLBAR_BUTTONS;
    let brect = |i: f32| Rect::new(toolbar.x + i * (bw + 2.0), toolbar.y, bw, TOOLBAR_H);
    let clicked = input.left_click || input.right_click;
    let inside = |r: Option<Rect>| mouse.is_some_and(|m| r.is_some_and(|r| r.contains(m)));
    let click_away = (st.menu.is_some() && clicked && !inside(menu_box))
        || (st.adding && clicked && !inside(add_box) && !inside(Some(brect(4.0))));
    if click_away {
        st.menu = None;
        st.adding = false;
    } else if st.drag.is_none()
        && !over_ui
        && !pinching
        && let Some(m) = mouse
    {
        if input.left_click {
            st.selected = hot;
            if let Some(g) = hot {
                let (kind, grab) = match st.handle_for(g) {
                    Some((hr, target)) if !TOUCH && hr.contains(m) => (DragKind::Scale, target),
                    _ => (DragKind::Move, g),
                };
                if let Some(rect) = st.rect_of(grab) {
                    st.drag = Some(Drag {
                        grab,
                        kind,
                        grip: m - rect.pos(),
                        start: rect,
                        start_scale: st.scale_of(grab),
                        press: m,
                        pressed_at: now,
                        moving: false,
                    });
                }
            }
        } else if input.right_click {
            st.selected = hot;
            st.menu = hot.map(|grab| Menu {
                grab,
                x: m.x,
                y: m.y,
            });
        } else if input.scroll != 0.0
            && let Some(g) = hot
        {
            let g = st.scale_target(g);
            if st.scroll_run != Some(g) {
                st.push_undo();
                st.scroll_run = Some(g);
            }
            let step = SCALE_STEP * input.scroll.signum();
            let next = ((st.scale_of(g) + step) * 10.0).round() / 10.0;
            st.set_scale(g, next, vw, vh, now);
        }
    }

    if let Some(g) = st.drag.map(|d| d.grab).or(st.selected) {
        let ids: Vec<ElementId> = match g {
            Grab::Element(id) => vec![id],
            Grab::Stack(si) => st.layout.members(si).to_vec(),
        };
        raise(p, &st.frames, &ids);
    }

    let line = hairline(ctx);
    for guide in st.guides.iter().flatten() {
        if guide.vertical {
            p.fill(guide.at, 0.0, line, vh, GUIDE);
        } else {
            p.fill(0.0, guide.at, vw, line, GUIDE);
        }
    }

    let mut boxes: Vec<Rect> = st.frames.stacks.iter().map(|(_, r)| *r).collect();
    for id in ElementId::ALL {
        let Some(r) = st.frames.screen[id.index()] else {
            continue;
        };
        let e = st.layout.get(id);
        let loose = match e.place {
            Place::Free { .. } => true,
            Place::Fixed => id != ElementId::Gamemode,
            Place::Stacked { .. } => false,
        };
        let ghost = id == ElementId::Debug && st.frames.placeholder[id.index()];
        if loose && e.visibility == Visibility::Shown && !ghost {
            boxes.push(r);
        }
    }
    let mut clashing = vec![false; boxes.len()];
    for i in 0..boxes.len() {
        for j in i + 1..boxes.len() {
            if let Some(o) = boxes[i].intersect(&boxes[j]) {
                clashing[i] = true;
                clashing[j] = true;
                p.fill(o.x, o.y, o.w, o.h, OVERLAP_FILL);
            }
        }
    }
    for (r, _) in boxes.iter().zip(&clashing).filter(|(_, c)| **c) {
        p.outline(r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0, OVERLAP);
    }

    let is_hot = |g: Grab| hot == Some(g) || st.selected == Some(g);
    for id in ElementId::ALL {
        let Some(r) = st.frames.screen[id.index()] else {
            continue;
        };
        let (x, y, w, h) = (r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0);
        if st.layout.get(id).visibility != Visibility::Shown {
            p.fill(r.x, r.y, r.w, r.h, HIDDEN_FILL);
            p.outline(x, y, w, h, HIDDEN_OUTLINE);
        } else if !is_hot(Grab::Element(id)) {
            p.outline(x, y, w, h, OUTLINE);
        }
    }

    let hot_stack = hot
        .or(st.selected)
        .and_then(|g| st.stack_for(g))
        .filter(|si| st.stack_rect(*si).is_some());
    if let Some(si) = hot_stack
        && let Some(r) = st.stack_rect(si)
    {
        p.outline(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0, STACK);
        let bar = bar_rect(r, vh);
        p.fill(bar.x, bar.y, bar.w, bar.h, STACK_BAR);
        p.outline(bar.x, bar.y, bar.w, bar.h, STACK);
        let n = st.layout.members(si).len();
        let label = format!("Stack upwards ({n})");
        p.text_plain(&label, bar.x + 2.0, bar.y + 1.0, 0xFFFFFF, false);
        if !TOUCH {
            let hr = handle_rect(r);
            p.fill(hr.x, hr.y, hr.w, hr.h, STACK);
        }
        let scaling = st
            .drag
            .is_some_and(|d| d.kind == DragKind::Scale && d.grab == Grab::Stack(si));
        if now - st.scaled_at < 1.0 || scaling {
            let s = format!("{:.1}x", st.scale_of(Grab::Stack(si)));
            tag(p, &s, r.x + r.w + 3.0, r.y + r.h - 10.0, vw, 0xFFFF55);
        }
    }

    for id in ElementId::ALL {
        let Some(r) = st.frames.screen[id.index()] else {
            continue;
        };
        if !is_hot(Grab::Element(id)) {
            continue;
        }
        let (x, y, w, h) = (r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0);
        let shown = st.layout.get(id).visibility == Visibility::Shown;
        if shown {
            p.outline(x, y, w, h, OUTLINE_HOT);
        }
        let stack = st.layout.stack_of(id);
        if stack.is_none() && !TOUCH {
            let hr = handle_rect(r);
            p.fill(hr.x, hr.y, hr.w, hr.h, OUTLINE_HOT);
        }
        if !st.frames.placeholder[id.index()] {
            let hidden = if shown { "" } else { " (hidden)" };
            let name = format!("{}{hidden}", id.label());
            let (tx, ty) = match stack {
                Some(si) => {
                    let right = st.layout.stack(si).is_some_and(|s| s.anchor.col() == 2);
                    let w = p.atlas.font.width_str(&name) + 4.0;
                    let tx = if right {
                        r.x - w - 3.0
                    } else {
                        r.x + r.w + 3.0
                    };
                    (tx, r.y - 0.5)
                }
                None if r.y >= 12.0 => (r.x, r.y - 12.0),
                None => (r.x, r.y + r.h + 2.0),
            };
            tag(p, &name, tx, ty, vw, 0xFFFFFF);
        }
        let scaling = st.drag.is_some_and(|d| d.kind == DragKind::Scale);
        if stack.is_none() && (now - st.scaled_at < 1.0 || scaling) {
            let s = format!("{:.1}x", st.layout.scale_of(id));
            tag(p, &s, r.x + r.w + 2.0, r.y + r.h - 10.0, vw, 0xFFFF55);
        }
    }

    match st.drop {
        Some(Landing::Into { stack, line, .. }) => {
            if let Some(r) = st.stack_rect(stack) {
                p.outline(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0, DROP);
            }
            p.fill(line.x - 2.0, line.y, line.w + 4.0, line.h, DROP);
        }
        Some(Landing::Pair { target, above }) => {
            if let Some(r) = st.frames.screen[target.index()] {
                p.outline(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0, DROP);
                let y = if above { r.y - 2.0 } else { r.y + r.h };
                p.fill(r.x - 2.0, y, r.w + 4.0, 2.0, DROP);
            }
        }
        None => {}
    }

    if let (Some((_, buttons)), Some(g)) = (&actions, st.selected) {
        for (r, label, act) in buttons {
            if button(p, ctx, *r, label) {
                act_on(st, g, *act, vw, vh);
            }
        }
    }

    if TOUCH {
        let status = if let Some(pinch) = st.pinch {
            Some(format!("{:.2}x", st.scale_of(pinch.target)))
        } else if st.drag.is_some_and(|d| d.moving) {
            match st.drop {
                Some(Landing::Into { .. }) => Some("Into stack".to_owned()),
                Some(Landing::Pair { target, .. }) => {
                    Some(format!("New stack with {}", target.label()))
                }
                None if st.guides.iter().any(Option::is_some) => Some("Snapped".to_owned()),
                None => None,
            }
        } else {
            None
        };
        if let Some(text) = status {
            let w = p.atlas.font.width_str(&text);
            tag(p, &text, ((vw - w) / 2.0).floor(), 4.0, vw, 0xFFFF55);
        }
    }

    let mut nav = None;
    if button(p, ctx, brect(0.0), "Done") {
        nav = Some(st.parent);
    }
    if button(p, ctx, brect(1.0), "Undo")
        && let Some(prev) = st.undo.pop_back()
    {
        st.layout = prev;
        st.scroll_run = None;
    }
    let grid_label = if grid > 0.0 {
        format!("Grid: {grid:.0}")
    } else {
        "Grid: off".to_owned()
    };
    if button(p, ctx, brect(if TOUCH { 3.0 } else { 2.0 }), &grid_label) {
        st.layout.grid = (st.layout.grid + 1) % GRIDS.len();
    }
    #[cfg(feature = "mobile_ui")]
    {
        let _ = options;
        let label = if st.free { "Snap: off" } else { "Snap: on" };
        if button(p, ctx, brect(2.0), label) {
            st.free = !st.free;
        }
    }
    #[cfg(not(feature = "mobile_ui"))]
    let scale_label = match options.gui_scale {
        0 => "GUI: Auto".to_owned(),
        n => format!("GUI: {n}"),
    };
    #[cfg(not(feature = "mobile_ui"))]
    let scale_rect = brect(3.0);
    #[cfg(not(feature = "mobile_ui"))]
    let scale_hot = mouse.is_some_and(|m| scale_rect.contains(m));
    #[cfg(not(feature = "mobile_ui"))]
    let left = button(p, ctx, scale_rect, &scale_label);
    #[cfg(not(feature = "mobile_ui"))]
    let right = scale_hot && input.right_click;
    #[cfg(not(feature = "mobile_ui"))]
    if left || right {
        options.gui_scale = step_gui_scale(options.gui_scale, left, ctx);
        crate::gui::save_options(options);
        st.drag = None;
        st.drop = None;
    }
    let add_label = if removed.is_empty() {
        "Add".to_owned()
    } else {
        format!("Add ({})", removed.len())
    };
    if button(p, ctx, brect(4.0), &add_label) {
        st.adding = !st.adding;
        st.menu = None;
    }
    if button(p, ctx, brect(5.0), "Reset all") {
        st.push_undo();
        let grid = st.layout.grid;
        st.layout = HudLayout::default();
        st.layout.grid = grid;
        st.selected = None;
    }
    for (i, hint) in HINTS.into_iter().enumerate() {
        let w = p.atlas.font.width_str(hint);
        p.text_plain(
            hint,
            ((vw - w) / 2.0).floor(),
            toolbar.y + TOOLBAR_H + 2.0 + i as f32 * 10.0,
            HINT,
            true,
        );
    }

    if let Some(r) = add_box {
        p.fill(r.x, r.y, r.w, r.h, PANEL);
        p.outline(r.x, r.y, r.w, r.h, OUTLINE_HOT);
        p.text_plain("Add back", r.x + 3.0, r.y + 3.0, 0xFFFF55, true);
        if removed.is_empty() {
            p.text_plain(
                "Nothing removed",
                r.x + 3.0,
                r.y + 3.0 + MENU_ROW,
                HINT,
                true,
            );
        }
        for (i, item) in removed.iter().enumerate() {
            let row = Rect::new(
                r.x + 1.0,
                r.y + 1.0 + (i as f32 + 1.0) * MENU_ROW,
                r.w - 2.0,
                MENU_ROW,
            );
            let label = match item {
                Restore::Element(id) => id.label(),
                Restore::Home(home) => home.label(),
            };
            if button(p, ctx, row, label) {
                st.push_undo();
                match *item {
                    Restore::Element(id) => {
                        st.layout.restore(id);
                        st.selected = Some(Grab::Element(id));
                    }
                    Restore::Home(home) => {
                        st.layout.restore_home(home);
                        st.selected = None;
                    }
                }
                st.adding = false;
            }
        }
    }

    if let (Some(menu), Some(r)) = (st.menu, menu_box) {
        p.fill(r.x, r.y, r.w, r.h, PANEL);
        p.outline(r.x, r.y, r.w, r.h, OUTLINE_HOT);
        let row = |i: f32| Rect::new(r.x + 1.0, r.y + 1.0 + i * MENU_ROW, r.w - 2.0, MENU_ROW);
        let title = match menu.grab {
            Grab::Element(id) => id.label().to_owned(),
            Grab::Stack(si) => format!("Stack upwards ({})", st.layout.members(si).len()),
        };
        p.text_plain(&title, r.x + 3.0, r.y + 3.0, 0xFFFF55, true);

        if let Grab::Element(id) = menu.grab
            && !id.required()
        {
            let shown = st.layout.get(id).visibility == Visibility::Shown;
            if button(p, ctx, row(1.0), if shown { "Hide" } else { "Show" }) {
                st.push_undo();
                st.layout.get_mut(id).toggle_hidden();
            }
        }

        let target = st.scale_target(menu.grab);
        let sr = row(2.0);
        let minus = Rect::new(sr.x, sr.y, 16.0, sr.h);
        let plus = Rect::new(sr.x + sr.w - 16.0, sr.y, 16.0, sr.h);
        let scale = st.scale_of(target);
        let label = if matches!((menu.grab, target), (Grab::Element(_), Grab::Stack(_))) {
            format!("Stack {scale:.1}x")
        } else {
            format!("{scale:.1}x")
        };
        let lw = p.atlas.font.width_str(&label);
        p.text_plain(
            &label,
            (sr.x + (sr.w - lw) / 2.0).floor(),
            sr.y + 3.0,
            0xFFFFFF,
            true,
        );
        for (rect, text, step) in [(minus, "-", -SCALE_STEP), (plus, "+", SCALE_STEP)] {
            if button(p, ctx, rect, text) {
                st.push_undo();
                let next = ((scale + step) * 10.0).round() / 10.0;
                st.set_scale(target, next, vw, vh, now);
            }
        }

        match menu.grab {
            Grab::Element(id) => {
                if button(p, ctx, row(3.0), "Reset") {
                    st.push_undo();
                    st.layout.reset(id);
                    st.layout.get_mut(id).visibility = if id.default_enabled() {
                        Visibility::Shown
                    } else {
                        Visibility::Hidden
                    };
                    st.menu = None;
                }
                if !id.required() && button(p, ctx, row(4.0), "Remove") {
                    st.push_undo();
                    st.layout.remove(id);
                    st.menu = None;
                    st.selected = None;
                }
            }
            Grab::Stack(si) => match st.layout.stack(si).and_then(|s| s.home) {
                Some(_) => {
                    if button(p, ctx, row(3.0), "Reset position") {
                        st.push_undo();
                        st.layout.reset_stack_position(si);
                        st.menu = None;
                    }
                }
                None => {
                    if button(p, ctx, row(3.0), "Split") {
                        st.push_undo();
                        let rects: [Option<Rect>; COUNT] = st.frames.screen;
                        st.layout.split(si, &rects, vw, vh);
                        st.menu = None;
                        st.selected = None;
                    }
                }
            },
        }
        if let Grab::Stack(si) = menu.grab
            && button(p, ctx, row(4.0), "Delete")
        {
            st.push_undo();
            let rects: [Option<Rect>; COUNT] = st.frames.screen;
            st.layout.delete_stack(si, &rects, vw, vh);
            st.menu = None;
            st.selected = None;
        }
    }

    nav
}
