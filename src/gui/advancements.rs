use std::sync::Arc;

use bevy::math::Vec2;

use azalea_registry::identifier::Identifier;

use crate::client::advancements::{AdvancementTree, Display, Node};
use crate::gui::atlas::MISSING_TEXTURE;
use crate::gui::container::LABEL_COLOR;
use crate::gui::painter::Painter;
use crate::gui::tooltip::{self, label};
use crate::gui::{Screen, ScreenCtx, Snapshot, options};
use crate::session::SharedMutex;
use crate::text::{Font, Span};
use crate::util::mth;

const WINDOW_W: i32 = 252;
const WINDOW_H: i32 = 140;
const INSIDE_X: i32 = 9;
const INSIDE_Y: i32 = 18;
const INSIDE_W: i32 = 234;
const INSIDE_H: i32 = 113;
const SCROLL_SPEED: f32 = 16.0;
const LINE: i32 = 9;
const TITLE_MAX_W: f32 = 163.0;
const TITLE_MIN_W: i32 = 80;
const SPLIT_OFFSETS: [i32; 5] = [0, 10, -10, 25, -25];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TabKind {
    Above,
    Below,
    Left,
    Right,
}

const TAB_SPRITES: [[[&str; 3]; 2]; 4] = [
    [
        [
            "advancements/tab_above_left",
            "advancements/tab_above_middle",
            "advancements/tab_above_right",
        ],
        [
            "advancements/tab_above_left_selected",
            "advancements/tab_above_middle_selected",
            "advancements/tab_above_right_selected",
        ],
    ],
    [
        [
            "advancements/tab_below_left",
            "advancements/tab_below_middle",
            "advancements/tab_below_right",
        ],
        [
            "advancements/tab_below_left_selected",
            "advancements/tab_below_middle_selected",
            "advancements/tab_below_right_selected",
        ],
    ],
    [
        [
            "advancements/tab_left_top",
            "advancements/tab_left_middle",
            "advancements/tab_left_bottom",
        ],
        [
            "advancements/tab_left_top_selected",
            "advancements/tab_left_middle_selected",
            "advancements/tab_left_bottom_selected",
        ],
    ],
    [
        [
            "advancements/tab_right_top",
            "advancements/tab_right_middle",
            "advancements/tab_right_bottom",
        ],
        [
            "advancements/tab_right_top_selected",
            "advancements/tab_right_middle_selected",
            "advancements/tab_right_bottom_selected",
        ],
    ],
];

impl TabKind {
    const ALL: [TabKind; 4] = [
        TabKind::Above,
        TabKind::Below,
        TabKind::Left,
        TabKind::Right,
    ];

    fn size(self) -> (i32, i32) {
        match self {
            TabKind::Above | TabKind::Below => (28, 32),
            TabKind::Left | TabKind::Right => (32, 28),
        }
    }

    fn max(self) -> usize {
        match self {
            TabKind::Above | TabKind::Below => 8,
            TabKind::Left | TabKind::Right => 5,
        }
    }

    fn pos(self, i: i32) -> (i32, i32) {
        let (w, h) = self.size();
        match self {
            TabKind::Above => ((w + 4) * i, -h + 4),
            TabKind::Below => ((w + 4) * i, 136),
            TabKind::Left => (-w + 4, h * i),
            TabKind::Right => (248, h * i),
        }
    }

    fn icon_offset(self) -> (i32, i32) {
        match self {
            TabKind::Above => (6, 9),
            TabKind::Below => (6, 6),
            TabKind::Left => (10, 5),
            TabKind::Right => (6, 5),
        }
    }

    fn sprite(self, i: usize, selected: bool) -> &'static str {
        let part = if i == 0 {
            0
        } else if i == self.max() - 1 {
            2
        } else {
            1
        };
        TAB_SPRITES[self as usize][selected as usize][part]
    }

    fn contains(self, i: i32, xo: i32, yo: i32, m: Vec2) -> bool {
        let (x, y) = self.pos(i);
        let (x, y) = ((xo + x) as f32, (yo + y) as f32);
        let (w, h) = self.size();
        m.x > x && m.x < x + w as f32 && m.y > y && m.y < y + h as f32
    }
}

fn place(mut index: usize) -> Option<(TabKind, usize)> {
    for kind in TabKind::ALL {
        if index < kind.max() {
            return Some((kind, index));
        }
        index -= kind.max();
    }
    None
}

struct Widget {
    node: u32,
    parent: Option<u16>,
    x: i32,
    y: i32,
    width: i32,
    title: Vec<Vec<Span>>,
    description: Vec<Vec<Span>>,
    display: Arc<Display>,
}

struct Tab {
    root: u32,
    root_id: Identifier,
    kind: TabKind,
    index: usize,
    widgets: Vec<Widget>,
    order: Vec<u16>,
    min: (i32, i32),
    max: (i32, i32),
    background: &'static str,
    scroll: Vec2,
    centered: bool,
    fade: f32,
}

impl Tab {
    fn root(&self) -> &Widget {
        &self.widgets[0]
    }

    fn can_scroll_x(&self) -> bool {
        self.max.0 - self.min.0 > INSIDE_W
    }

    fn can_scroll_y(&self) -> bool {
        self.max.1 - self.min.1 > INSIDE_H
    }

    fn scroll(&mut self, dx: f32, dy: f32) {
        if self.can_scroll_x() {
            self.scroll.x = mth::clamp(self.scroll.x + dx, -(self.max.0 - INSIDE_W) as f32, 0.0);
        }
        if self.can_scroll_y() {
            self.scroll.y = mth::clamp(self.scroll.y + dy, -(self.max.1 - INSIDE_H) as f32, 0.0);
        }
    }
}

#[derive(Default)]
pub struct AdvancementsUi {
    pub parent: Screen,
    fresh: bool,
    built: Option<u32>,
    tabs: Vec<Tab>,
    drag_last: Option<Vec2>,
    scrolling: bool,
}

impl AdvancementsUi {
    pub fn opened(&mut self) {
        self.fresh = true;
        self.close();
    }

    pub fn close(&mut self) {
        self.built = None;
        self.tabs = Vec::new();
        self.drag_last = None;
        self.scrolling = false;
    }
}

fn width(font: &Font, spans: &[Span]) -> i32 {
    font.width(spans).ceil() as i32
}

fn optimal_lines(font: &Font, spans: &[Span], preferred: i32) -> Vec<Vec<Span>> {
    let mut best = Vec::new();
    let mut best_distance = f32::MAX;
    for offset in SPLIT_OFFSETS {
        let lines = font.wrap_nonempty(spans, (preferred - offset) as f32);
        let widest = lines.iter().map(|l| font.width(l)).fold(0.0, f32::max);
        let distance = (widest - preferred as f32).abs();
        if distance <= 10.0 {
            return lines;
        }
        if distance < best_distance {
            best_distance = distance;
            best = lines;
        }
    }
    best
}

fn progress_text(done: u16, total: u16) -> String {
    tooltip::translate(
        "advancements.progress",
        &[done.to_string(), total.to_string()],
    )
}

fn widget(font: &Font, node: u32, n: &Node, display: Arc<Display>, parent: Option<u16>) -> Widget {
    let title = font.wrap_nonempty(&display.title, TITLE_MAX_W);
    let title_w = title
        .iter()
        .map(|l| width(font, l))
        .max()
        .unwrap_or(0)
        .max(TITLE_MIN_W);
    let groups = n.groups();
    let progress_w = if groups <= 1 {
        0
    } else {
        font.width_str(&progress_text(groups, groups)).ceil() as i32 + 8
    };
    let mut longest = 29 + title_w + progress_w;
    let description = optimal_lines(font, &display.description, longest);
    for line in &description {
        longest = longest.max(width(font, line));
    }
    Widget {
        node,
        parent,
        x: (display.x * 28.0).floor() as i32,
        y: (display.y * 27.0).floor() as i32,
        width: longest + 3 + 5,
        title,
        description,
        display,
    }
}

fn background(p: &Painter, id: Option<&Identifier>) -> &'static str {
    const VANILLA: [&str; 5] = [
        "advancements/backgrounds/stone",
        "advancements/backgrounds/nether",
        "advancements/backgrounds/end",
        "advancements/backgrounds/adventure",
        "advancements/backgrounds/husbandry",
    ];
    id.filter(|id| id.namespace() == "minecraft")
        .and_then(|id| id.path().strip_prefix("gui/"))
        .and_then(|path| VANILLA.into_iter().find(|v| *v == path))
        .filter(|key| p.atlas.sheet(key).is_some())
        .unwrap_or(MISSING_TEXTURE)
}

fn build(p: &Painter, tree: &AdvancementTree) -> Vec<Tab> {
    let font = &p.atlas.font;
    let n = tree.nodes.len();
    let mut tab_of: Vec<Option<u16>> = vec![None; n];
    let mut visible: Vec<Option<u16>> = vec![None; n];
    let mut tabs: Vec<Tab> = Vec::new();
    for (i, node) in tree.nodes.iter().enumerate() {
        match node.parent {
            None => {
                let (Some(display), Some((kind, index))) = (&node.display, place(tabs.len()))
                else {
                    continue;
                };
                let root = widget(font, i as u32, node, display.clone(), None);
                let bg = background(p, display.background.as_ref());
                tab_of[i] = Some(tabs.len() as u16);
                visible[i] = Some(0);
                tabs.push(Tab {
                    root: i as u32,
                    root_id: node.id.clone(),
                    kind,
                    index,
                    widgets: vec![root],
                    order: Vec::new(),
                    min: (i32::MAX, i32::MAX),
                    max: (i32::MIN, i32::MIN),
                    background: bg,
                    scroll: Vec2::ZERO,
                    centered: false,
                    fade: 0.0,
                });
            }
            Some(parent) => {
                let parent = parent as usize;
                let Some(t) = tab_of[parent] else { continue };
                tab_of[i] = Some(t);
                visible[i] = visible[parent];
                if let Some(display) = &node.display {
                    let tab = &mut tabs[t as usize];
                    visible[i] = Some(tab.widgets.len() as u16);
                    tab.widgets.push(widget(
                        font,
                        i as u32,
                        node,
                        display.clone(),
                        visible[parent],
                    ));
                }
            }
        }
    }
    for tab in &mut tabs {
        for w in &tab.widgets {
            tab.min = (tab.min.0.min(w.x), tab.min.1.min(w.y));
            tab.max = (tab.max.0.max(w.x + 28), tab.max.1.max(w.y + 27));
        }
        let mut children: Vec<Vec<u16>> = vec![Vec::new(); tab.widgets.len()];
        for (i, w) in tab.widgets.iter().enumerate() {
            if let Some(parent) = w.parent {
                children[parent as usize].push(i as u16);
            }
        }
        let mut stack = vec![0u16];
        while let Some(w) = stack.pop() {
            tab.order.push(w);
            stack.extend(children[w as usize].iter().rev());
        }
    }
    tabs
}

fn shown(w: &Widget, node: &Node) -> bool {
    !w.display.hidden || node.is_done()
}

fn fill(p: &mut Painter, x0: i32, y0: i32, x1: i32, y1: i32, argb: u32) {
    let (x0, x1) = (x0.min(x1), x0.max(x1));
    let (y0, y1) = (y0.min(y1), y0.max(y1));
    if x1 > x0 && y1 > y0 {
        p.fill(
            x0 as f32,
            y0 as f32,
            (x1 - x0) as f32,
            (y1 - y0) as f32,
            argb,
        );
    }
}

fn hline(p: &mut Painter, x0: i32, x1: i32, y: i32, argb: u32) {
    let (x0, x1) = (x0.min(x1), x0.max(x1));
    fill(p, x0, y, x1 + 1, y + 1, argb);
}

fn vline(p: &mut Painter, x: i32, y0: i32, y1: i32, argb: u32) {
    let (y0, y1) = (y0.min(y1), y0.max(y1));
    fill(p, x, y0 + 1, x + 1, y1, argb);
}

fn connector(p: &mut Painter, parent: &Widget, w: &Widget, xo: i32, yo: i32, background: bool) {
    let dep_x = xo + parent.x + 13;
    let split_x = xo + parent.x + 26 + 4;
    let dep_y = yo + parent.y + 13;
    let my_x = xo + w.x + 13;
    let my_y = yo + w.y + 13;
    if background {
        let c = 0xFF00_0000;
        hline(p, split_x, dep_x, dep_y - 1, c);
        hline(p, split_x + 1, dep_x, dep_y, c);
        hline(p, split_x, dep_x, dep_y + 1, c);
        hline(p, my_x, split_x - 1, my_y - 1, c);
        hline(p, my_x, split_x - 1, my_y, c);
        hline(p, my_x, split_x - 1, my_y + 1, c);
        vline(p, split_x - 1, my_y, dep_y, c);
        vline(p, split_x + 1, my_y, dep_y, c);
    } else {
        let c = 0xFFFF_FFFF;
        hline(p, split_x, dep_x, dep_y, c);
        hline(p, my_x, split_x, my_y, c);
        vline(p, split_x, my_y, dep_y, c);
    }
}

fn draw_contents(p: &mut Painter, tab: &mut Tab, tree: &AdvancementTree, ox: i32, oy: i32) {
    if !tab.centered {
        tab.scroll = Vec2::new(
            (117 - (tab.max.0 + tab.min.0) / 2) as f32,
            (56 - (tab.max.1 + tab.min.1) / 2) as f32,
        );
        tab.centered = true;
    }
    let clip = p.push_clip(ox as f32, oy as f32, INSIDE_W as f32, INSIDE_H as f32);
    let sx = tab.scroll.x.floor() as i32;
    let sy = tab.scroll.y.floor() as i32;
    let (left, top) = (sx % 16, sy % 16);
    for x in -1..=15 {
        for y in -1..=8 {
            let tx = (ox + left + 16 * x) as f32;
            let ty = (oy + top + 16 * y) as f32;
            p.blit_sheet(tab.background, 0.0, 0.0, 16.0, 16.0, tx, ty);
        }
    }
    let (xo, yo) = (ox + sx, oy + sy);
    for background in [true, false] {
        for &i in &tab.order {
            let w = &tab.widgets[i as usize];
            if let Some(parent) = w.parent {
                connector(p, &tab.widgets[parent as usize], w, xo, yo, background);
            }
        }
    }
    for &i in &tab.order {
        let w = &tab.widgets[i as usize];
        let node = &tree.nodes[w.node as usize];
        if !shown(w, node) {
            continue;
        }
        let obtained = node.percent() >= 1.0;
        let (x, y) = ((xo + w.x) as f32, (yo + w.y) as f32);
        p.sprite(
            w.display.frame.frame_sprite(obtained),
            x + 3.0,
            y,
            26.0,
            26.0,
        );
        p.item_icon(&w.display.icon, x + 8.0, y + 5.0);
    }
    p.pop_clip(clip);
}

fn sprite_part(
    p: &mut Painter,
    name: &str,
    tw: i32,
    th: i32,
    u: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    let clip = p.push_clip(x as f32, y as f32, w as f32, h as f32);
    p.sprite(name, (x - u) as f32, y as f32, tw as f32, th as f32);
    p.pop_clip(clip);
}

#[allow(clippy::too_many_arguments)]
fn draw_hover(
    p: &mut Painter,
    w: &Widget,
    node: &Node,
    sx: i32,
    sy: i32,
    ox: i32,
    oy: i32,
    screen_x: i32,
    vw: i32,
) {
    let (bx, by) = (ox + sx + w.x, oy + sy + w.y);
    let title_bar_h = LINE * w.title.len() as i32 + LINE + 8;
    let title_top = by + (26 - title_bar_h) / 2;
    let title_bar_bottom = title_top + title_bar_h;
    let desc_text_h = w.description.len() as i32 * LINE;
    let desc_h = 6 + desc_text_h;
    let left_side = screen_x + sx + w.x + w.width + 26 >= vw;
    let progress = node
        .progress()
        .filter(|&(_, total)| total > 1)
        .map(|(c, t)| progress_text(c, t));
    let progress_w = progress
        .as_deref()
        .map_or(0, |s| p.atlas.font.width_str(s).ceil() as i32);
    let top_side = (title_bar_bottom - oy) + desc_h >= INSIDE_H;
    let amount = node.percent();
    let mut first_w = (amount * w.width as f32).floor() as i32;
    let (first, second, icon_obtained) = if amount >= 1.0 {
        first_w = w.width / 2;
        (true, true, true)
    } else if first_w < 2 {
        first_w = w.width / 2;
        (false, false, false)
    } else if first_w > w.width - 2 {
        first_w = w.width / 2;
        (true, true, false)
    } else {
        (true, false, false)
    };
    let second_w = w.width - first_w;
    let title_left = if left_side { bx - w.width + 26 + 6 } else { bx };
    let box_sprite = |obtained: bool| {
        if obtained {
            "advancements/box_obtained"
        } else {
            "advancements/box_unobtained"
        }
    };

    let background_h = title_bar_h + desc_h;
    if !w.description.is_empty() {
        let y = if top_side {
            title_bar_bottom - background_h
        } else {
            title_top
        };
        p.sprite(
            "advancements/title_box",
            title_left as f32,
            y as f32,
            w.width as f32,
            background_h as f32,
        );
    }
    if first != second {
        sprite_part(
            p,
            box_sprite(first),
            200,
            title_bar_h,
            0,
            title_left,
            title_top,
            first_w,
            title_bar_h,
        );
        sprite_part(
            p,
            box_sprite(second),
            200,
            title_bar_h,
            200 - second_w,
            title_left + first_w,
            title_top,
            second_w,
            title_bar_h,
        );
    } else {
        p.sprite(
            box_sprite(first),
            title_left as f32,
            title_top as f32,
            w.width as f32,
            title_bar_h as f32,
        );
    }
    p.sprite(
        w.display.frame.frame_sprite(icon_obtained),
        (bx + 3) as f32,
        by as f32,
        26.0,
        26.0,
    );

    let desc_left = title_left + 5;
    let (title_x, progress_x) = if left_side {
        (desc_left, bx - progress_w)
    } else {
        (bx + 32, bx + w.width - progress_w - 5)
    };
    lines(p, &w.title, title_x, title_top + 9);
    if let Some(text) = &progress {
        p.text_plain(
            text,
            progress_x as f32,
            (title_top + 9) as f32,
            0xFFFFFF,
            true,
        );
    }
    let desc_y = if top_side {
        title_top - desc_text_h + 1
    } else {
        title_bar_bottom
    };
    lines(p, &w.description, desc_left, desc_y);
    p.item_icon(&w.display.icon, (bx + 8) as f32, (by + 5) as f32);
}

fn lines(p: &mut Painter, lines: &[Vec<Span>], x: i32, y: i32) {
    for (i, line) in lines.iter().enumerate() {
        p.text(line, x as f32, (y + i as i32 * LINE) as f32, true);
    }
}

fn centered(p: &mut Painter, s: &str, mid: i32, y: i32) {
    p.text_centered(s, mid as f32 + 0.5, y as f32, 0xFFFFFF, true);
}

fn select(shared: &SharedMutex, tree: &AdvancementTree, root: Option<u32>) {
    let mut s = shared.lock().unwrap();
    let session = &mut s.session;
    if session.advancements.generation != tree.generation {
        return;
    }
    session.advancement_tab = root;
    if let Some(root) = root {
        session
            .seen_advancements
            .push(Some(tree.nodes[root as usize].id.clone()));
    }
}

pub fn draw(
    p: &mut Painter,
    ui: &mut AdvancementsUi,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    shared: &SharedMutex,
) -> Option<Screen> {
    let mut nav = None;
    crate::gui::screens::dim_background(p, ctx.vw, ctx.vh);
    options::draw_title(p, ctx, label!("gui.advancements"));
    if options::draw_done(p, ctx) {
        nav = Some(ui.parent);
    }

    let empty = AdvancementTree::default();
    let tree = snap.advancements.as_deref().unwrap_or(&empty);
    if ui.built != Some(tree.generation) {
        let mut tabs = build(p, tree);
        for tab in &mut tabs {
            if let Some(old) = ui.tabs.iter().find(|t| t.root_id == tab.root_id) {
                tab.scroll = old.scroll;
                tab.centered = old.centered;
                tab.fade = old.fade;
            }
        }
        ui.tabs = tabs;
        ui.built = Some(tree.generation);
    }

    let mut selected = snap.advancement_tab;
    if std::mem::take(&mut ui.fresh) {
        let tab = selected
            .filter(|&r| ui.tabs.iter().any(|t| t.root == r))
            .or_else(|| ui.tabs.first().map(|t| t.root));
        select(shared, tree, tab);
        selected = tab;
    }

    let xo = ((ctx.vw as i32) - WINDOW_W) / 2;
    let yo = ((ctx.vh as i32) - WINDOW_H) / 2;
    let mouse = ctx.mouse();

    if ctx.input.left_click
        && let Some(m) = mouse
        && let Some(tab) = ui
            .tabs
            .iter()
            .find(|t| t.kind.contains(t.index as i32, xo, yo, m))
    {
        select(shared, tree, Some(tab.root));
        selected = Some(tab.root);
    }

    let current = selected.and_then(|r| ui.tabs.iter().position(|t| t.root == r));

    match (ctx.input.left_down, mouse) {
        (true, Some(m)) => {
            if let Some(last) = ui.drag_last
                && m != last
            {
                if !ui.scrolling {
                    ui.scrolling = true;
                } else if let Some(i) = current {
                    ui.tabs[i].scroll(m.x - last.x, m.y - last.y);
                }
            }
            ui.drag_last = Some(m);
        }
        _ => {
            ui.drag_last = None;
            ui.scrolling = false;
        }
    }
    if ctx.input.scroll != 0.0
        && let Some(i) = current
    {
        ui.tabs[i].scroll(0.0, ctx.input.scroll * SCROLL_SPEED);
    }

    let (ox, oy) = (xo + INSIDE_X, yo + INSIDE_Y);
    match current {
        Some(i) => draw_contents(p, &mut ui.tabs[i], tree, ox, oy),
        None => {
            fill(p, ox, oy, ox + INSIDE_W, oy + INSIDE_H, 0xFF00_0000);
            let mid = ox + INSIDE_W / 2;
            centered(p, label!("advancements.empty"), mid, oy + 56 - LINE / 2);
            centered(
                p,
                label!("advancements.sad_label"),
                mid,
                oy + INSIDE_H - LINE,
            );
        }
    }

    p.blit_sheet(
        "advancements/window",
        0.0,
        0.0,
        WINDOW_W as f32,
        WINDOW_H as f32,
        xo as f32,
        yo as f32,
    );
    let strip = ui.tabs.len() > 1;
    if strip {
        for (i, tab) in ui.tabs.iter().enumerate() {
            let (x, y) = tab.kind.pos(tab.index as i32);
            let (w, h) = tab.kind.size();
            p.sprite(
                tab.kind.sprite(tab.index, current == Some(i)),
                (xo + x) as f32,
                (yo + y) as f32,
                w as f32,
                h as f32,
            );
        }
        for tab in &ui.tabs {
            let (x, y) = tab.kind.pos(tab.index as i32);
            let (dx, dy) = tab.kind.icon_offset();
            p.item_icon(
                &tab.root().display.icon,
                (xo + x + dx) as f32,
                (yo + y + dy) as f32,
            );
        }
    }
    match current.and_then(|i| ui.tabs[i].root().display.window_title.as_ref()) {
        Some(title) => {
            p.text(title, (xo + 8) as f32, (yo + 6) as f32, false);
        }
        None => {
            p.text_plain(
                label!("gui.advancements"),
                (xo + 8) as f32,
                (yo + 6) as f32,
                LABEL_COLOR,
                false,
            );
        }
    }

    let m = mouse.map(|m| (m.x.floor() as i32, m.y.floor() as i32));
    if let Some(i) = current {
        let tab = &mut ui.tabs[i];
        fill(
            p,
            ox,
            oy,
            ox + INSIDE_W,
            oy + INSIDE_H,
            ((tab.fade * 255.0).floor() as u32) << 24,
        );
        let (sx, sy) = (tab.scroll.x.floor() as i32, tab.scroll.y.floor() as i32);
        let hovered = m
            .map(|(mx, my)| (mx - ox, my - oy))
            .filter(|&(mx, my)| mx > 0 && mx < INSIDE_W && my > 0 && my < INSIDE_H)
            .and_then(|(mx, my)| {
                tab.widgets.iter().find(|w| {
                    let (x0, y0) = (sx + w.x, sy + w.y);
                    shown(w, &tree.nodes[w.node as usize])
                        && mx >= x0
                        && mx <= x0 + 26
                        && my >= y0
                        && my <= y0 + 26
                })
            });
        if let Some(w) = hovered {
            draw_hover(
                p,
                w,
                &tree.nodes[w.node as usize],
                sx,
                sy,
                ox,
                oy,
                xo,
                ctx.vw as i32,
            );
            tab.fade = mth::clamp(tab.fade + 0.02, 0.0, 0.3);
        } else {
            tab.fade = mth::clamp(tab.fade - 0.04, 0.0, 1.0);
        }
    }
    if strip && let Some(m) = mouse {
        for tab in &ui.tabs {
            if tab.kind.contains(tab.index as i32, xo, yo, m) {
                let title = std::slice::from_ref(&tab.root().display.title);
                tooltip::draw_lines(p, title, m.x, m.y, ctx.vw, ctx.vh);
            }
        }
    }
    nav
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_places_follow_the_type_order() {
        assert_eq!(place(0), Some((TabKind::Above, 0)));
        assert_eq!(place(7), Some((TabKind::Above, 7)));
        assert_eq!(place(8), Some((TabKind::Below, 0)));
        assert_eq!(place(16), Some((TabKind::Left, 0)));
        assert_eq!(place(21), Some((TabKind::Right, 0)));
        assert_eq!(place(25), Some((TabKind::Right, 4)));
        assert_eq!(place(26), None);
    }

    #[test]
    fn tab_sprites_pick_first_middle_last() {
        assert_eq!(
            TabKind::Above.sprite(0, false),
            "advancements/tab_above_left"
        );
        assert_eq!(
            TabKind::Above.sprite(7, true),
            "advancements/tab_above_right_selected"
        );
        assert_eq!(
            TabKind::Left.sprite(2, false),
            "advancements/tab_left_middle"
        );
        assert_eq!(
            TabKind::Right.sprite(4, false),
            "advancements/tab_right_bottom"
        );
    }

    #[test]
    fn scroll_clamps_like_vanilla() {
        let mut tab = Tab {
            root: 0,
            root_id: Identifier::new("a"),
            kind: TabKind::Above,
            index: 0,
            widgets: Vec::new(),
            order: Vec::new(),
            min: (-300, 0),
            max: (100, 27),
            background: MISSING_TEXTURE,
            scroll: Vec2::ZERO,
            centered: true,
            fade: 0.0,
        };
        tab.scroll(-50.0, -50.0);
        assert_eq!(tab.scroll, Vec2::new(134.0, 0.0));
    }
}
