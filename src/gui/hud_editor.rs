use std::collections::VecDeque;

use bevy::math::Vec2;

#[cfg(feature = "mobile_ui")]
use crate::gui::hud_layout::Area;
use crate::gui::hud_layout::{
    Anchor, COUNT, ElementId, Frames, GRIDS, Home, HudLayout, Layer, MAX_SCALE, MIN_SCALE, Panel,
    Place, Rect, StackId, Visibility, snap_scale,
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
#[cfg(feature = "mobile_ui")]
const EDGE_GRAB: f32 = 8.0;
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
#[cfg(feature = "mobile_ui")]
const AREA_FILL: u32 = 0x40FF_5555;
#[cfg(feature = "mobile_ui")]
const AREA_LINE: u32 = 0xC0FF_5555;
#[cfg(feature = "mobile_ui")]
const ZOOM_FILL: u32 = 0x4055_FF55;
#[cfg(feature = "mobile_ui")]
const ZOOM_LINE: u32 = 0xC055_FF55;
const BACKDROP: bool = true;
const PANEL: u32 = 0xFF10_1010;
const PANEL_HOT: u32 = 0xFF30_3030;
const LABEL_BG: u32 = 0xC000_0000;
const HINT: u32 = 0xAAAAAA;

const TOOLBAR_W: f32 = 300.0;
const TOOLBAR_H: f32 = if TOUCH { 20.0 } else { 14.0 };
const BUTTON_MIN: f32 = if TOUCH { 46.0 } else { 40.0 };
const GRIP_H: f32 = if TOUCH { 14.0 } else { 8.0 };
const GRIP_END: f32 = if TOUCH { 14.0 } else { 8.0 };
const SIDE_GRAB: f32 = if TOUCH { 16.0 } else { 8.0 };
const MENU_W: f32 = if TOUCH { 160.0 } else { 96.0 };
const MENU_ROW: f32 = if TOUCH { 20.0 } else { 13.0 };
const ADD_W: f32 = if TOUCH { 160.0 } else { 110.0 };

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Grab {
    Element(ElementId),
    Stack(StackId),
    #[cfg(feature = "mobile_ui")]
    Area(usize),
}

#[derive(Clone, Copy, PartialEq)]
enum DragKind {
    Move,
    Scale,
    #[cfg(feature = "mobile_ui")]
    Resize(u8),
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

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Done,
    Undo,
    Grid,
    #[cfg(feature = "mobile_ui")]
    Snap,
    #[cfg(not(feature = "mobile_ui"))]
    Gui,
    Add,
    #[cfg(feature = "mobile_ui")]
    NewArea,
    Reset,
}

#[derive(Clone, Copy)]
struct PanelDrag {
    side: f32,
    press: Vec2,
    start: Rect,
}

#[cfg_attr(not(feature = "mobile_ui"), allow(dead_code))]
#[derive(Clone, Copy)]
struct PanelPinch {
    start_dx: f32,
    start_w: f32,
    centre: Vec2,
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
    panel_drag: Option<PanelDrag>,
    panel_pinch: Option<PanelPinch>,
    bar_rect: Rect,
    vw: f32,
    vh: f32,
}

impl EditorState {
    pub fn load() -> Self {
        EditorState {
            layout: HudLayout::load(),
            ..Default::default()
        }
    }

    pub fn opened(&mut self) {
        self.clear_hand();
        self.selected = None;
        self.undo.clear();
        self.scroll_run = None;
        self.guides = [None; 2];
        self.adding = false;
        self.pinch = None;
        self.panel_drag = None;
        self.panel_pinch = None;
        self.touches.clear();
    }

    #[cfg(feature = "mobile_ui")]
    pub fn set_touches(&mut self, touches: impl Iterator<Item = Vec2>) {
        self.touches.clear();
        self.touches.extend(touches);
    }

    pub fn close(&mut self) {
        self.clear_hand();
        self.layout.save();
    }

    pub fn escape(&mut self) -> bool {
        std::mem::take(&mut self.adding) || self.menu.take().is_some()
    }

    fn set_panel(&mut self, centre: Vec2, width: f32) {
        let screen = Vec2::new(self.vw, self.vh).max(Vec2::ONE);
        self.layout.panel = Some(Panel {
            centre: centre / screen,
            width: width.clamp(BUTTON_MIN, self.vw.max(BUTTON_MIN)).round(),
        });
    }

    fn layer(&self) -> Layer {
        self.frames.layer
    }

    fn active(&self, id: ElementId) -> bool {
        id.layer() == self.layer() && id.available()
    }

    fn clear_hand(&mut self) {
        self.drag = None;
        self.drop = None;
        self.menu = None;
    }

    fn push_undo(&mut self) {
        if self.undo.len() == UNDO_DEPTH {
            self.undo.pop_front();
        }
        self.undo.push_back(self.layout.clone());
        self.scroll_run = None;
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo.pop_back() {
            self.layout = prev;
            self.clear_hand();
            self.scroll_run = None;
        }
    }

    fn rect_of(&self, g: Grab) -> Option<Rect> {
        match g {
            Grab::Element(id) => self.frames.screen[id.index()],
            Grab::Stack(si) => self.frames.stack_rect(si),
            #[cfg(feature = "mobile_ui")]
            Grab::Area(i) => self.layout.areas.get(i).map(|a| a.rect(self.vw, self.vh)),
        }
    }

    fn scalable(&self, g: Grab) -> bool {
        #[cfg(feature = "mobile_ui")]
        if matches!(g, Grab::Area(_)) {
            return false;
        }
        let _ = g;
        true
    }

    fn scale_of(&self, g: Grab) -> f32 {
        match g {
            Grab::Element(id) => self.layout.scale_of(id),
            Grab::Stack(si) => self.layout.stack(si).map_or(1.0, |s| s.scale),
            #[cfg(feature = "mobile_ui")]
            Grab::Area(_) => 1.0,
        }
    }

    fn scale_target(&self, g: Grab) -> Grab {
        match g {
            Grab::Element(id) => self.layout.stack_of(id).map_or(g, Grab::Stack),
            g => g,
        }
    }

    fn place_element(&mut self, id: ElementId, rect: Rect, scale: f32) {
        let anchor = Anchor::nearest(rect, self.vw, self.vh);
        let offset = anchor.offset_for(self.vw, self.vh, rect.size(), rect.pos());
        self.layout.set_free(id, anchor, offset, scale);
    }

    fn place_stack(&mut self, si: StackId, rect: Rect) {
        let (vw, vh) = (self.vw, self.vh);
        let Some(stack) = self.layout.stack_mut(si) else {
            return;
        };
        stack.moved = true;
        stack.anchor = Anchor::nearest(rect, vw, vh);
        stack.offset = stack.anchor.offset_for(vw, vh, rect.size(), rect.pos());
    }

    fn set_scale(&mut self, g: Grab, scale: f32, now: f32) {
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
                    self.place_element(id, rect, scale);
                }
            },
            #[cfg(feature = "mobile_ui")]
            Grab::Area(_) => return,
        }
        self.scaled_at = now;
    }

    fn handle_for(&self, g: Grab) -> Option<(Rect, Grab)> {
        if !self.scalable(g) {
            return None;
        }
        let target = self.scale_target(g);
        Some((handle_rect(self.rect_of(target)?), target))
    }

    fn stack_for(&self, g: Grab) -> Option<StackId> {
        match g {
            Grab::Stack(si) => Some(si),
            Grab::Element(id) => self.layout.stack_of(id),
            #[cfg(feature = "mobile_ui")]
            Grab::Area(_) => None,
        }
    }

    fn hot_at(&self, m: Vec2) -> Option<Grab> {
        #[cfg(feature = "mobile_ui")]
        if self.layer() == Layer::Areas {
            return self
                .layout
                .areas
                .iter()
                .rposition(|a| a.rect(self.vw, self.vh).contains(m))
                .map(Grab::Area);
        }
        if self.layer() == Layer::Extras {
            for &(si, r) in &self.frames.stacks {
                if bar_rect(r, self.vh).contains(m) {
                    return Some(Grab::Stack(si));
                }
            }
        }
        ElementId::ALL
            .into_iter()
            .filter(|id| self.active(*id))
            .filter_map(|id| self.frames.screen[id.index()].map(|r| (id, r)))
            .filter(|(_, r)| r.contains(m))
            .min_by(|(_, a), (_, b)| (a.w * a.h).total_cmp(&(b.w * b.h)))
            .map(|(id, _)| Grab::Element(id))
    }

    fn find_drop(&self, id: ElementId, m: Vec2) -> Option<Landing> {
        if self.layer() != Layer::Extras || !id.stackable() {
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
        ElementId::ALL.into_iter().find_map(|other| {
            let e = self.layout.get(other);
            let pairable = other != id
                && other.stackable()
                && self.active(other)
                && e.visibility == Visibility::Shown
                && matches!(e.place, Place::Free { .. });
            let r = self.frames.screen[other.index()].filter(|r| pairable && r.contains(m))?;
            Some(Landing::Pair {
                target: other,
                above: m.y < r.y + r.h / 2.0,
            })
        })
    }

    fn apply_drop(&mut self, id: ElementId, rect: Rect, drop: Landing) {
        match drop {
            Landing::Into { stack, index, .. } => self.layout.insert(id, stack, index),
            Landing::Pair { target, above } => {
                let Some(tr) = self.frames.screen[target.index()] else {
                    return;
                };
                let top = if above { tr.y - rect.h } else { tr.y };
                let r = Rect::new(tr.x, top, tr.w.max(rect.w), tr.h + rect.h);
                let anchor = Anchor::nearest(r, self.vw, self.vh);
                let offset = anchor.offset_for(self.vw, self.vh, r.size(), r.pos());
                let scale = self.layout.scale_of(target);
                let members = if above { [id, target] } else { [target, id] };
                self.layout.new_stack(anchor, offset, scale, &members);
            }
        }
    }

    fn snap_targets(&self, exclude: &[ElementId], skip: Grab) -> Vec<Rect> {
        let mut out: Vec<Rect> = ElementId::ALL
            .into_iter()
            .filter(|id| !exclude.contains(id))
            .filter_map(|id| self.frames.screen[id.index()])
            .collect();
        out.extend(
            self.frames
                .stacks
                .iter()
                .filter(|(si, _)| skip != Grab::Stack(*si))
                .map(|(_, r)| *r),
        );
        #[cfg(feature = "mobile_ui")]
        if self.layer() == Layer::Areas {
            out.extend(
                (0..self.layout.areas.len())
                    .filter(|i| skip != Grab::Area(*i))
                    .filter_map(|i| self.rect_of(Grab::Area(i))),
            );
        }
        out
    }

    fn remove(&mut self, g: Grab) {
        self.push_undo();
        let rects: [Option<Rect>; COUNT] = self.frames.screen;
        match g {
            Grab::Element(id) => self.layout.remove(id),
            Grab::Stack(si) => self.layout.delete_stack(si, &rects, self.vw, self.vh),
            #[cfg(feature = "mobile_ui")]
            Grab::Area(i) => {
                if self.layout.areas.get(i).is_some_and(|a| !a.zoom) {
                    self.layout.areas.remove(i);
                }
            }
        }
        self.clear_hand();
        self.selected = None;
    }

    fn reset(&mut self, g: Grab) {
        match g {
            Grab::Element(id) => {
                self.push_undo();
                self.layout.reset(id);
            }
            Grab::Stack(si) if self.layout.stack(si).is_some_and(|s| s.home.is_some()) => {
                self.push_undo();
                self.layout.reset_stack_position(si);
            }
            _ => {}
        }
    }

    fn split(&mut self, si: StackId) {
        self.push_undo();
        let rects: [Option<Rect>; COUNT] = self.frames.screen;
        self.layout.split(si, &rects, self.vw, self.vh);
        self.selected = None;
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

#[cfg(feature = "mobile_ui")]
fn edges_at(r: Rect, m: Vec2) -> u8 {
    let gx = EDGE_GRAB.min(r.w / 3.0);
    let gy = EDGE_GRAB.min(r.h / 3.0);
    let mut mask = 0;
    if m.x - r.x < gx {
        mask |= 1;
    } else if r.x + r.w - m.x < gx {
        mask |= 4;
    }
    if m.y - r.y < gy {
        mask |= 2;
    } else if r.y + r.h - m.y < gy {
        mask |= 8;
    }
    mask
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

fn snap(
    pos: Vec2,
    size: Vec2,
    others: &[Rect],
    vw: f32,
    vh: f32,
    grid: f32,
) -> (Vec2, [Option<Guide>; 2]) {
    let mut xs: Vec<(f32, u8)> = Vec::with_capacity(3 + others.len() * 5);
    let mut ys: Vec<(f32, u8)> = Vec::with_capacity(3 + others.len() * 5);
    xs.extend([
        (EDGE_MARGIN, 0),
        (vw - EDGE_MARGIN, 2),
        ((vw / 2.0).floor(), 1),
    ]);
    ys.extend([
        (EDGE_MARGIN, 0),
        (vh - EDGE_MARGIN, 2),
        ((vh / 2.0).floor(), 1),
    ]);
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

fn action_bar(st: &EditorState, g: Grab) -> Option<(Rect, Vec<(Rect, &'static str, Act)>)> {
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
        #[cfg(feature = "mobile_ui")]
        Grab::Area(_) => vec![("Delete", Act::Delete)],
    };
    const W: f32 = 40.0;
    const H: f32 = 18.0;
    let total = items.len() as f32 * (W + 2.0) - 2.0;
    let x = r.x.clamp(0.0, (st.vw - total).max(0.0)).floor();
    let above = !matches!(g, Grab::Stack(_)) && r.y >= H + 3.0;
    let y = if above {
        r.y - H - 3.0
    } else {
        r.y + r.h + 3.0
    };
    let y = y.clamp(0.0, (st.vh - H).max(0.0)).floor();
    let buttons = items
        .into_iter()
        .enumerate()
        .map(|(i, (label, act))| (Rect::new(x + i as f32 * (W + 2.0), y, W, H), label, act))
        .collect();
    Some((Rect::new(x, y, total, H), buttons))
}

fn act_on(st: &mut EditorState, g: Grab, act: Act) {
    match (act, g) {
        (Act::More, g) => {
            let (x, y) = st.rect_of(g).map_or((0.0, 0.0), |r| (r.x, r.y));
            st.menu = Some(Menu { grab: g, x, y });
        }
        (Act::Hide, Grab::Element(id)) => {
            st.push_undo();
            st.layout.get_mut(id).toggle_hidden();
        }
        (Act::Reset, g) => st.reset(g),
        (Act::Remove | Act::Delete, g) => st.remove(g),
        (Act::Split, Grab::Stack(si)) => st.split(si),
        _ => {}
    }
}

fn button(p: &mut Painter, ctx: &ScreenCtx, r: Rect, label: &str) -> bool {
    button_on(p, ctx, r, label, false)
}

fn button_on(p: &mut Painter, ctx: &ScreenCtx, r: Rect, label: &str, on: bool) -> bool {
    let hot = ctx.mouse().is_some_and(|m| r.contains(m));
    p.fill(
        r.x,
        r.y,
        r.w,
        r.h,
        if hot || on { PANEL_HOT } else { PANEL },
    );
    let line = match (on, hot) {
        (true, _) => STACK,
        (_, true) => OUTLINE_HOT,
        _ => OUTLINE,
    };
    p.outline(r.x, r.y, r.w, r.h, line);
    p.text_centered(
        label,
        r.x + r.w / 2.0,
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

fn hints(layer: Layer) -> &'static [&'static str] {
    #[cfg(feature = "mobile_ui")]
    return match layer {
        Layer::Areas => &["Drag: move. Drag an edge: resize. Touches here only turn the camera."],
        _ => &["Tap: select. Drag: move. Pinch: scale. Hold: options."],
    };
    #[cfg(not(feature = "mobile_ui"))]
    match layer {
        Layer::Extras => &[
            "Drag: move. Drop on a stack or line: stack. Shift: free.",
            "Scroll/corner: scale. Right-click: options. Bar: move stack.",
            "H: hide. Del: remove/delete (Add restores). R: reset. Ctrl+Z: undo.",
        ],
        _ => &[
            "Drag: move. Shift: free. Scroll/corner: scale. Right-click: options.",
            "H: hide. Del: remove (Add restores). R: reset. Ctrl+Z: undo.",
        ],
    }
}

struct Toolbar {
    rect: Rect,
    grip: Rect,
    sides: [Rect; 2],
    tabs: Vec<(Rect, Layer)>,
    tools: Vec<(Rect, Tool)>,
    hints: Vec<String>,
    hint_y: f32,
}

impl Toolbar {
    fn new(p: &Painter, vw: f32, vh: f32, layer: Layer, panel: Option<Panel>) -> Toolbar {
        let mut tools = vec![Tool::Done, Tool::Undo];
        #[cfg(feature = "mobile_ui")]
        tools.push(Tool::Snap);
        #[cfg(not(feature = "mobile_ui"))]
        tools.push(Tool::Gui);
        tools.push(Tool::Grid);
        match layer {
            #[cfg(feature = "mobile_ui")]
            Layer::Areas => tools.push(Tool::NewArea),
            Layer::Controls => {}
            _ => tools.push(Tool::Add),
        }
        tools.push(Tool::Reset);

        let w = panel
            .map_or(TOOLBAR_W, |pn| pn.width)
            .clamp(BUTTON_MIN, vw.max(BUTTON_MIN))
            .floor();
        let cols = (((w + 2.0) / (BUTTON_MIN + 2.0)).floor() as usize).max(1);
        let rows = |n: usize| n.div_ceil(cols.min(n).max(1));
        let hints = crate::gui::widgets::wrap(p, hints(layer), w);
        let button_rows = rows(Layer::ALL.len()) + rows(tools.len());
        let h = button_rows as f32 * (TOOLBAR_H + 2.0) + hints.len() as f32 * 10.0 + GRIP_H;
        let (x, y) = match panel {
            Some(pn) => (pn.centre.x * vw - w / 2.0, pn.centre.y * vh - h / 2.0),
            None => ((vw - w) / 2.0, (vh / 2.0).floor() + 14.0),
        };
        let x = x.min(vw - w).max(0.0).floor();
        let y = y.min(vh - h).max(0.0).floor();

        let grid = |n: usize, top: f32| {
            let per = cols.min(n).max(1);
            let bw = (w - 2.0 * (per as f32 - 1.0)) / per as f32;
            (0..n)
                .map(|i| {
                    let (col, row) = (i % per, i / per);
                    let in_row = per.min(n - row * per);
                    let indent = (per - in_row) as f32 * (bw + 2.0) / 2.0;
                    Rect::new(
                        (x + indent + col as f32 * (bw + 2.0)).floor(),
                        top + row as f32 * (TOOLBAR_H + 2.0),
                        bw.floor(),
                        TOOLBAR_H,
                    )
                })
                .collect::<Vec<_>>()
        };
        let tab_rects = grid(Layer::ALL.len(), y);
        let tools_top = y + rows(Layer::ALL.len()) as f32 * (TOOLBAR_H + 2.0);
        let tool_rects = grid(tools.len(), tools_top);
        let hint_y = y + button_rows as f32 * (TOOLBAR_H + 2.0);
        Toolbar {
            rect: Rect::new(x, y, w, h),
            grip: Rect::new(x, hint_y, w, y + h - hint_y),
            sides: [
                Rect::new(x - SIDE_GRAB, y, SIDE_GRAB, h),
                Rect::new(x + w, y, SIDE_GRAB, h),
            ],
            tabs: tab_rects.into_iter().zip(Layer::ALL).collect(),
            tools: tool_rects.into_iter().zip(tools).collect(),
            hints,
            hint_y,
        }
    }

    fn tool(&self, t: Tool) -> Option<Rect> {
        self.tools.iter().find(|(_, x)| *x == t).map(|(r, _)| *r)
    }

    fn handle_at(&self, m: Vec2) -> Option<f32> {
        let g = self.grip;
        if self.sides[0].contains(m) {
            Some(-1.0)
        } else if self.sides[1].contains(m) {
            Some(1.0)
        } else if !g.contains(m) {
            None
        } else if m.x < g.x + GRIP_END {
            Some(-1.0)
        } else if m.x >= g.x + g.w - GRIP_END {
            Some(1.0)
        } else {
            Some(0.0)
        }
    }
}

fn add_rect(add: Rect, bar: Rect, rows: usize, vw: f32) -> Rect {
    let h = MENU_ROW * (rows.max(1) as f32 + 1.0) + 2.0;
    let x = (add.x + add.w / 2.0 - ADD_W / 2.0).min(vw - ADD_W).max(0.0);
    let y = if bar.y >= h + 2.0 {
        bar.y - h - 2.0
    } else {
        bar.y + bar.h + 2.0
    };
    Rect::new(x.floor(), y.floor(), ADD_W, h)
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
    if BACKDROP {
        crate::gui::menu::dirt_background(p, ctx.vw, ctx.vh);
    }
    let grid = GRIDS[st.layout.grid.min(GRIDS.len() - 1)];
    let moving = st
        .drag
        .is_some_and(|d| d.kind != DragKind::Scale && d.moving);
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

fn keys(st: &mut EditorState, ctx: &ScreenCtx) {
    let input = ctx.input;
    if input.undo {
        st.undo();
    }
    let target = st
        .drag
        .map(|d| d.grab)
        .or(st.selected)
        .or_else(|| ctx.mouse().and_then(|m| st.hot_at(m)));
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
        st.remove(g);
    }
    if input.hud_reset {
        if let Some(g) = target {
            st.reset(g);
        }
        st.clear_hand();
    }
}

#[cfg(feature = "mobile_ui")]
fn gestures(st: &mut EditorState, ctx: &ScreenCtx) -> bool {
    let now = ctx.input.time;
    if let Some(d) = st.drag
        && !d.moving
        && ctx.input.left_down
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
    if st.touches.len() < 2 {
        st.pinch = None;
        st.panel_pinch = None;
        return false;
    }
    let (a, b) = (st.touches[0], st.touches[1]);
    if st.panel_pinch.is_none()
        && st.pinch.is_none()
        && st.bar_rect.contains(a)
        && st.bar_rect.contains(b)
    {
        st.push_undo();
        let r = st.bar_rect;
        st.panel_pinch = Some(PanelPinch {
            start_dx: (a.x - b.x).abs().max(8.0),
            start_w: r.w,
            centre: Vec2::new(r.x + r.w / 2.0, r.y + r.h / 2.0),
        });
    }
    if let Some(pp) = st.panel_pinch {
        let w = pp.start_w * (a.x - b.x).abs().max(1.0) / pp.start_dx;
        st.set_panel(pp.centre, w);
        st.panel_drag = None;
        st.drag = None;
        st.drop = None;
        return true;
    }
    let dist = a.distance(b);
    if st.pinch.is_none()
        && dist > 1.0
        && let Some(g) = st.selected.or_else(|| st.hot_at((a + b) / 2.0))
        && st.scalable(g)
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
        st.set_scale(pinch.target, next, now);
    }
    st.drag = None;
    st.drop = None;
    true
}

fn follow_drag(st: &mut EditorState, ctx: &ScreenCtx, grid: f32, free: bool) {
    let input = ctx.input;
    let now = input.time;
    st.guides = [None; 2];
    let Some(drag) = st.drag else {
        return;
    };
    let (vw, vh) = (st.vw, st.vh);
    let m = match ctx.mouse() {
        Some(m) if input.left_down => m,
        _ => {
            if let (Grab::Element(id), DragKind::Move, true, Some(drop)) =
                (drag.grab, drag.kind, drag.moving, st.drop)
                && let Some(rect) = st.frames.screen[id.index()]
            {
                st.apply_drop(id, rect, drop);
            }
            st.drag = None;
            st.drop = None;
            return;
        }
    };
    if !drag.moving {
        return;
    }
    let size = drag.start.size();
    let raw = m - drag.grip;
    let snapped = |st: &mut EditorState, exclude: &[ElementId]| {
        if free {
            return raw.round();
        }
        let others = st.snap_targets(exclude, drag.grab);
        let (pos, guides) = snap(raw, size, &others, vw, vh, grid);
        st.guides = guides;
        pos
    };
    match (drag.grab, drag.kind) {
        (Grab::Element(id), DragKind::Move) => {
            let pos = snapped(st, &[id]);
            st.place_element(
                id,
                Rect::new(pos.x, pos.y, size.x, size.y),
                drag.start_scale,
            );
            st.drop = if free { None } else { st.find_drop(id, m) };
        }
        (Grab::Stack(si), DragKind::Move) => {
            let members = st.layout.members(si);
            let pos = snapped(st, &members[..]);
            st.place_stack(si, Rect::new(pos.x, pos.y, size.x, size.y));
        }
        #[cfg(feature = "mobile_ui")]
        (Grab::Area(i), DragKind::Move) => {
            let pos = snapped(st, &[]);
            if let Some(a) = st.layout.areas.get_mut(i) {
                a.place(Rect::new(pos.x, pos.y, size.x, size.y), vw, vh);
            }
        }
        #[cfg(feature = "mobile_ui")]
        (Grab::Area(i), DragKind::Resize(mask)) => {
            let s = drag.start;
            let d = m - drag.press;
            let edge = |v: f32, extent: f32| {
                if free || grid <= 0.0 {
                    v.round()
                } else {
                    lattice(v, extent, grid)
                }
            };
            let (mut x0, mut y0, mut x1, mut y1) = (s.x, s.y, s.x + s.w, s.y + s.h);
            if mask & 1 != 0 {
                x0 = edge(x0 + d.x, vw).min(x1 - Area::MIN).max(0.0);
            }
            if mask & 4 != 0 {
                x1 = edge(x1 + d.x, vw).min(vw).max(x0 + Area::MIN);
            }
            if mask & 2 != 0 {
                y0 = edge(y0 + d.y, vh).min(y1 - Area::MIN).max(0.0);
            }
            if mask & 8 != 0 {
                y1 = edge(y1 + d.y, vh).min(vh).max(y0 + Area::MIN);
            }
            if let Some(a) = st.layout.areas.get_mut(i) {
                a.place(Rect::new(x0, y0, x1 - x0, y1 - y0), vw, vh);
            }
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
                Grab::Element(id) => st.place_element(id, rect, scale),
                Grab::Stack(si) => {
                    if let Some(s) = st.layout.stack_mut(si) {
                        s.scale = scale;
                    }
                    st.place_stack(si, rect);
                }
                #[cfg(feature = "mobile_ui")]
                Grab::Area(_) => {}
            }
            st.scaled_at = now;
        }
        #[cfg(feature = "mobile_ui")]
        (_, DragKind::Resize(_)) => {}
    }
}

fn press(st: &mut EditorState, ctx: &ScreenCtx, hot: Option<Grab>, m: Vec2) {
    let input = ctx.input;
    if input.left_click {
        st.selected = hot;
        let Some(g) = hot else {
            return;
        };
        let (kind, grab) = match st.handle_for(g) {
            Some((hr, target)) if !TOUCH && hr.contains(m) => (DragKind::Scale, target),
            _ => (DragKind::Move, g),
        };
        let Some(rect) = st.rect_of(grab) else {
            return;
        };
        #[cfg(feature = "mobile_ui")]
        let kind = match grab {
            Grab::Area(_) => match edges_at(rect, m) {
                0 => DragKind::Move,
                mask => DragKind::Resize(mask),
            },
            _ => kind,
        };
        st.drag = Some(Drag {
            grab,
            kind,
            grip: m - rect.pos(),
            start: rect,
            start_scale: st.scale_of(grab),
            press: m,
            pressed_at: input.time,
            moving: false,
        });
    } else if input.right_click {
        st.selected = hot;
        st.menu = hot.map(|grab| Menu {
            grab,
            x: m.x,
            y: m.y,
        });
    } else if input.scroll != 0.0
        && let Some(g) = hot
        && st.scalable(g)
    {
        let g = st.scale_target(g);
        if st.scroll_run != Some(g) {
            st.push_undo();
            st.scroll_run = Some(g);
        }
        let next = ((st.scale_of(g) + SCALE_STEP * input.scroll.signum()) * 10.0).round() / 10.0;
        st.set_scale(g, next, input.time);
    }
}

fn draw_overlap(p: &mut Painter, st: &EditorState) {
    let mut boxes: Vec<Rect> = if st.layer() == Layer::Extras {
        st.frames.stacks.iter().map(|(_, r)| *r).collect()
    } else {
        Vec::new()
    };
    for id in ElementId::ALL.into_iter().filter(|id| st.active(*id)) {
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
}

#[cfg(feature = "mobile_ui")]
fn draw_areas(p: &mut Painter, st: &EditorState, hot: Option<Grab>) {
    for i in 0..st.layout.areas.len() {
        let Some(r) = st.rect_of(Grab::Area(i)) else {
            continue;
        };
        let lit = hot == Some(Grab::Area(i)) || st.selected == Some(Grab::Area(i));
        let (fill, line, label) = if st.layout.areas[i].zoom {
            (ZOOM_FILL, ZOOM_LINE, "Zoom")
        } else {
            (AREA_FILL, AREA_LINE, "Look only")
        };
        #[cfg(feature = "tutorial")]
        crate::gui::tutorial::mark(
            if st.layout.areas[i].zoom {
                crate::gui::tutorial::Spot::ZoomArea
            } else {
                crate::gui::tutorial::Spot::LookArea
            },
            r,
        );
        p.fill(r.x, r.y, r.w, r.h, fill);
        p.outline(r.x, r.y, r.w, r.h, if lit { OUTLINE_HOT } else { line });
        if lit {
            for (x, y) in [
                (r.x, r.y),
                (r.x + r.w - HANDLE, r.y),
                (r.x, r.y + r.h - HANDLE),
            ] {
                p.fill(x, y, HANDLE, HANDLE, OUTLINE_HOT);
            }
            let hr = handle_rect(r);
            p.fill(hr.x, hr.y, hr.w, hr.h, OUTLINE_HOT);
        }
        let w = p.atlas.font.width_str(label);
        if w + 4.0 <= r.w && r.h >= 12.0 {
            p.text_centered(
                label,
                r.x + r.w / 2.0,
                (r.y + (r.h - 8.0) / 2.0).floor(),
                0xFFFFFF,
                true,
            );
        }
    }
}

fn draw_boxes(p: &mut Painter, st: &EditorState, hot: Option<Grab>, now: f32, vw: f32) {
    let is_hot = |g: Grab| hot == Some(g) || st.selected == Some(g);
    let ids = move || {
        ElementId::ALL
            .into_iter()
            .filter(move |id| st.active(*id))
            .filter_map(move |id| st.frames.screen[id.index()].map(|r| (id, r)))
    };
    for (id, r) in ids() {
        let (x, y, w, h) = (r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0);
        if st.layout.get(id).visibility != Visibility::Shown {
            p.fill(r.x, r.y, r.w, r.h, HIDDEN_FILL);
            p.outline(x, y, w, h, HIDDEN_OUTLINE);
        } else if !is_hot(Grab::Element(id)) {
            p.outline(x, y, w, h, OUTLINE);
        }
    }

    let hot_stack = (st.layer() == Layer::Extras)
        .then(|| hot.or(st.selected).and_then(|g| st.stack_for(g)))
        .flatten();
    if let Some(si) = hot_stack
        && let Some(r) = st.frames.stack_rect(si)
    {
        p.outline(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0, STACK);
        let bar = bar_rect(r, st.vh);
        p.fill(bar.x, bar.y, bar.w, bar.h, STACK_BAR);
        p.outline(bar.x, bar.y, bar.w, bar.h, STACK);
        let label = format!("Stack upwards ({})", st.layout.members(si).len());
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

    for (id, r) in ids().filter(|(id, _)| is_hot(Grab::Element(*id))) {
        let shown = st.layout.get(id).visibility == Visibility::Shown;
        if shown {
            p.outline(r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0, OUTLINE_HOT);
        }
        let stack = st
            .layout
            .stack_of(id)
            .filter(|_| st.layer() == Layer::Extras);
        if stack.is_none() && !TOUCH {
            let hr = handle_rect(r);
            p.fill(hr.x, hr.y, hr.w, hr.h, OUTLINE_HOT);
        }
        if !st.frames.placeholder[id.index()] {
            let hidden = if shown { "" } else { " (hidden)" };
            let name = format!("{}{hidden}", id.name());
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
            if let Some(r) = st.frames.stack_rect(stack) {
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
}

#[cfg(feature = "mobile_ui")]
fn draw_status(p: &mut Painter, st: &EditorState, vw: f32) {
    let status = if let Some(pinch) = st.pinch {
        Some(format!("{:.2}x", st.scale_of(pinch.target)))
    } else if let Some(d) = st.drag.filter(|d| d.moving) {
        match (d.kind, st.drop) {
            (DragKind::Resize(_), _) => st
                .rect_of(d.grab)
                .map(|r| format!("{:.0} x {:.0}", r.w, r.h)),
            (_, Some(Landing::Into { .. })) => Some("Into stack".to_owned()),
            (_, Some(Landing::Pair { target, .. })) => {
                Some(format!("New stack with {}", target.name()))
            }
            _ if st.guides.iter().any(Option::is_some) => Some("Snapped".to_owned()),
            _ => None,
        }
    } else {
        None
    };
    if let Some(text) = status {
        let x = p.centered_x(&text, vw / 2.0);
        tag(p, &text, x, 4.0, vw, 0xFFFF55);
    }
}

fn draw_menu(p: &mut Painter, st: &mut EditorState, ctx: &ScreenCtx, menu: Menu, r: Rect) {
    let now = ctx.input.time;
    p.fill(r.x, r.y, r.w, r.h, PANEL);
    p.outline(r.x, r.y, r.w, r.h, OUTLINE_HOT);
    let row = |i: f32| Rect::new(r.x + 1.0, r.y + 1.0 + i * MENU_ROW, r.w - 2.0, MENU_ROW);
    let title = match menu.grab {
        Grab::Element(id) => id.name().to_owned(),
        Grab::Stack(si) => format!("Stack upwards ({})", st.layout.members(si).len()),
        #[cfg(feature = "mobile_ui")]
        Grab::Area(_) => "Look-only area".to_owned(),
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

    if st.scalable(menu.grab) {
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
        p.text_centered(&label, sr.x + sr.w / 2.0, sr.y + 3.0, 0xFFFFFF, true);
        for (rect, text, step) in [(minus, "-", -SCALE_STEP), (plus, "+", SCALE_STEP)] {
            if button(p, ctx, rect, text) {
                st.push_undo();
                st.set_scale(target, ((scale + step) * 10.0).round() / 10.0, now);
            }
        }
    }

    match menu.grab {
        Grab::Element(id) => {
            if button(p, ctx, row(3.0), "Reset") {
                st.push_undo();
                st.layout.reset(id);
                st.layout.get_mut(id).visibility = match id.default_visibility() {
                    Visibility::Removed => Visibility::Shown,
                    v => v,
                };
                st.menu = None;
            }
            if !id.required() && button(p, ctx, row(4.0), "Remove") {
                st.remove(menu.grab);
            }
        }
        Grab::Stack(si) => {
            let home = st.layout.stack(si).is_some_and(|s| s.home.is_some());
            if home && button(p, ctx, row(3.0), "Reset position") {
                st.reset(menu.grab);
                st.menu = None;
            }
            if !home && button(p, ctx, row(3.0), "Split") {
                st.split(si);
                st.menu = None;
            }
            if button(p, ctx, row(4.0), "Delete") {
                st.remove(menu.grab);
            }
        }
        #[cfg(feature = "mobile_ui")]
        Grab::Area(_) => {
            if button(p, ctx, row(1.0), "Delete") {
                st.remove(menu.grab);
            }
        }
    }
}

pub fn draw(
    p: &mut Painter,
    st: &mut EditorState,
    options: &mut GuiOptions,
    ctx: &ScreenCtx,
) -> Option<Screen> {
    let (vw, vh) = (ctx.vw, ctx.vh);
    (st.vw, st.vh) = (vw, vh);
    let input = ctx.input;
    let now = input.time;
    let mouse = ctx.mouse();
    let grid = GRIDS[st.layout.grid.min(GRIDS.len() - 1)];
    let free = input.shift || st.free;

    keys(st, ctx);

    if let Some(mut drag) = st.drag
        && !drag.moving
        && mouse.is_some_and(|m| m.distance(drag.press) >= DRAG_THRESHOLD)
    {
        st.push_undo();
        drag.moving = true;
        if let (Grab::Element(id), DragKind::Move) = (drag.grab, drag.kind)
            && !matches!(st.layout.get(id).place, Place::Free { .. })
        {
            let scale = st.layout.scale_of(id);
            st.place_element(id, drag.start, scale);
            drag.start_scale = scale;
        }
        st.drag = Some(drag);
    }

    #[cfg(feature = "mobile_ui")]
    let pinching = gestures(st, ctx);
    #[cfg(not(feature = "mobile_ui"))]
    let pinching = false;

    follow_drag(st, ctx, grid, free);

    if let Some(pd) = st.panel_drag {
        match mouse {
            Some(m) if input.left_down && !pinching => {
                let centre = pd.start.pos() + pd.start.size() / 2.0;
                if pd.side == 0.0 {
                    st.set_panel(centre + m - pd.press, pd.start.w);
                } else {
                    st.set_panel(centre, pd.start.w + 2.0 * pd.side * (m.x - pd.press.x));
                }
            }
            _ => st.panel_drag = None,
        }
    }

    let layer = st.layer();
    let bar = Toolbar::new(p, vw, vh, layer, st.layout.panel);
    st.bar_rect = bar.rect;
    let menu_box = st.menu.map(|m| menu_rect(m, vw, vh));
    let removed: Vec<Restore> = ElementId::ALL
        .into_iter()
        .filter(|id| st.active(*id) && st.layout.get(*id).visibility == Visibility::Removed)
        .map(Restore::Element)
        .chain(
            st.layout
                .removed_homes()
                .filter(|_| layer == Layer::Extras)
                .map(Restore::Home),
        )
        .collect();
    let add_button = bar.tool(Tool::Add);
    let add_box = add_button
        .filter(|_| st.adding)
        .map(|b| add_rect(b, bar.rect, removed.len(), vw));
    let actions = if TOUCH && st.drag.is_none() && st.menu.is_none() && !st.adding && !pinching {
        st.selected.and_then(|g| action_bar(st, g))
    } else {
        None
    };
    let inside = |r: Option<Rect>| mouse.is_some_and(|m| r.is_some_and(|r| r.contains(m)));
    let over_ui = st.panel_drag.is_some()
        || inside(Some(bar.rect))
        || bar.sides.iter().any(|r| inside(Some(*r)))
        || inside(menu_box)
        || inside(add_box)
        || inside(actions.as_ref().map(|(r, _)| *r));
    let hot = match st.drag {
        Some(d) => Some(d.grab),
        None if over_ui => None,
        None => mouse.and_then(|m| st.hot_at(m)),
    };

    let clicked = input.left_click || input.right_click;
    let click_away = (st.menu.is_some() && clicked && !inside(menu_box))
        || (st.adding && clicked && !inside(add_box) && !inside(add_button));
    if click_away {
        st.menu = None;
        st.adding = false;
    } else if input.left_click
        && st.drag.is_none()
        && st.panel_drag.is_none()
        && !pinching
        && let Some(m) = mouse
        && let Some(side) = bar.handle_at(m)
    {
        st.push_undo();
        st.panel_drag = Some(PanelDrag {
            side,
            press: m,
            start: bar.rect,
        });
    } else if st.drag.is_none()
        && !over_ui
        && !pinching
        && let Some(m) = mouse
    {
        press(st, ctx, hot, m);
    }

    if let Some(g) = st.drag.map(|d| d.grab).or(st.selected) {
        match g {
            Grab::Element(id) => raise(p, &st.frames, &[id]),
            Grab::Stack(si) => raise(p, &st.frames, &st.layout.members(si)),
            #[cfg(feature = "mobile_ui")]
            Grab::Area(_) => {}
        }
    }

    let line = hairline(ctx);
    for guide in st.guides.iter().flatten() {
        if guide.vertical {
            p.fill(guide.at, 0.0, line, vh, GUIDE);
        } else {
            p.fill(0.0, guide.at, vw, line, GUIDE);
        }
    }

    #[cfg(feature = "mobile_ui")]
    if layer == Layer::Areas {
        draw_areas(p, st, hot);
    }
    draw_overlap(p, st);
    draw_boxes(p, st, hot, now, vw);

    if let (Some((_, buttons)), Some(g)) = (&actions, st.selected) {
        for (r, label, act) in buttons {
            if button(p, ctx, *r, label) {
                act_on(st, g, *act);
            }
        }
    }

    #[cfg(feature = "mobile_ui")]
    draw_status(p, st, vw);

    let r = bar.rect;
    p.fill(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 2.0, PANEL);
    for &(r, l) in &bar.tabs {
        #[cfg(feature = "tutorial")]
        crate::gui::tutorial::mark(crate::gui::tutorial::Spot::Tab(l), r);
        if button_on(p, ctx, r, l.label(), l == layer) && l != layer {
            st.frames.layer = l;
            st.selected = None;
            st.adding = false;
            st.clear_hand();
        }
    }
    let mut nav = None;
    for &(r, tool) in &bar.tools {
        #[cfg(feature = "tutorial")]
        if let Some(spot) = match tool {
            Tool::Done => Some(crate::gui::tutorial::Spot::Done),
            Tool::Reset => Some(crate::gui::tutorial::Spot::Reset),
            #[cfg(feature = "mobile_ui")]
            Tool::NewArea => Some(crate::gui::tutorial::Spot::NewArea),
            _ => None,
        } {
            crate::gui::tutorial::mark(spot, r);
        }
        match tool {
            Tool::Done => {
                if button(p, ctx, r, "Done") {
                    nav = Some(st.parent);
                }
            }
            Tool::Undo => {
                if button(p, ctx, r, "Undo") {
                    st.undo();
                }
            }
            Tool::Grid => {
                let label = if grid > 0.0 {
                    format!("Grid: {grid:.0}")
                } else {
                    "Grid: off".to_owned()
                };
                if button(p, ctx, r, &label) {
                    st.layout.grid = (st.layout.grid + 1) % GRIDS.len();
                }
            }
            #[cfg(feature = "mobile_ui")]
            Tool::Snap => {
                if button(p, ctx, r, if st.free { "Snap: off" } else { "Snap: on" }) {
                    st.free = !st.free;
                }
            }
            #[cfg(not(feature = "mobile_ui"))]
            Tool::Gui => {
                let label = match options.gui_scale {
                    0 => "GUI: Auto".to_owned(),
                    n => format!("GUI: {n}"),
                };
                let right = inside(Some(r)) && input.right_click;
                let left = button(p, ctx, r, &label);
                if left || right {
                    options.gui_scale = step_gui_scale(options.gui_scale, left, ctx);
                    crate::gui::save_options(options);
                    st.drag = None;
                    st.drop = None;
                }
            }
            Tool::Add => {
                let label = match removed.len() {
                    0 => "Add".to_owned(),
                    n => format!("Add ({n})"),
                };
                if button(p, ctx, r, &label) {
                    st.adding = !st.adding;
                    st.menu = None;
                }
            }
            #[cfg(feature = "mobile_ui")]
            Tool::NewArea => {
                if button(p, ctx, r, "New area") {
                    st.push_undo();
                    let size = Vec2::new(80.0, 60.0);
                    let pos = ((Vec2::new(vw, vh) - size) / 2.0).floor();
                    st.layout
                        .areas
                        .push(Area::at(Rect::new(pos.x, pos.y, size.x, size.y), vw, vh));
                    st.selected = Some(Grab::Area(st.layout.areas.len() - 1));
                }
            }
            Tool::Reset => {
                if button(p, ctx, r, "Reset") {
                    st.push_undo();
                    st.layout.reset_layer(layer);
                    st.layout.panel = None;
                    st.selected = None;
                }
            }
        }
    }
    #[cfg(feature = "mobile_ui")]
    let _ = options;
    let g = bar.grip;
    let lit = st.panel_drag.is_some() || st.panel_pinch.is_some();
    if lit {
        let r = bar.rect;
        p.outline(r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0, STACK);
    }
    let knob = if lit { STACK } else { OUTLINE };
    let mid = g.y + g.h - GRIP_H / 2.0 - 1.0;
    p.fill((g.x + g.w / 2.0 - 12.0).floor(), mid, 24.0, 2.0, knob);
    for x in [g.x + 2.0, g.x + g.w - 4.0] {
        p.fill(x, mid - 3.0, 2.0, 8.0, knob);
    }
    for (i, hint) in bar.hints.iter().enumerate() {
        p.text_centered(
            hint,
            g.x + g.w / 2.0,
            bar.hint_y + i as f32 * 10.0,
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
                Restore::Element(id) => id.name(),
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
        draw_menu(p, st, ctx, menu, r);
    }

    nav
}
