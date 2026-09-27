use crate::gui::painter::Painter;
use crate::gui::screens::dim_background;
use crate::gui::slots::{self, CONTAINER_W, INV_H, INV_W, Layout};
use crate::gui::{GuiState, ScreenCtx, Snapshot, tooltip};
use crate::session::{ContainerKind, InvAction, SlotStack};

mod banner;
mod beacon;
mod book;
mod enchant;
mod horse;
mod merchant;
mod plain;
mod recipe;

pub struct ContainerUi {
    pub scroll: f32,
    pub dragging: bool,
    pub selected: i16,
    pub beacon: (i16, i16),
    pub merchant_title: Option<(i32, u32, String)>,
}

pub const NO_EFFECT: i16 = -1;

impl Default for ContainerUi {
    fn default() -> ContainerUi {
        ContainerUi {
            scroll: 0.0,
            dragging: false,
            selected: -1,
            beacon: (NO_EFFECT, NO_EFFECT),
            merchant_title: None,
        }
    }
}

impl ContainerUi {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

pub fn draw(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    kind: ContainerKind,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    match kind {
        ContainerKind::None => None,
        ContainerKind::Chest(rows) => plain::draw_chest(p, state, ctx, snap, rows, out),
        ContainerKind::Crafting => plain::draw_crafting_table(p, state, ctx, snap, out),
        ContainerKind::Furnace(k) => plain::draw_furnace(p, state, ctx, snap, k, out),
        ContainerKind::Hopper => plain::draw_hopper(p, state, ctx, snap, out),
        ContainerKind::Grindstone => plain::draw_grindstone(p, state, ctx, snap, out),
        ContainerKind::Smithing => plain::draw_smithing(p, state, ctx, snap, out),
        ContainerKind::Cartography => plain::draw_cartography(p, state, ctx, snap, out),
        ContainerKind::BrewingStand => plain::draw_brewing_stand(p, state, ctx, snap, out),
        ContainerKind::Stonecutter => recipe::draw_stonecutter(p, state, ctx, snap, out),
        ContainerKind::Loom => recipe::draw_loom(p, state, ctx, snap, out),
        ContainerKind::Enchantment => enchant::draw(p, state, ctx, snap, out),
        ContainerKind::Beacon => beacon::draw(p, state, ctx, snap, out),
        ContainerKind::Merchant => merchant::draw(p, state, ctx, snap, out),
        ContainerKind::Lectern => book::draw(p, state, ctx, snap, out),
        ContainerKind::Horse(columns) => horse::draw(p, state, ctx, snap, columns, out),
    }
}

pub fn window_origin(layout: Layout, ctx: &ScreenCtx) -> (f32, f32) {
    let (w, h) = layout.size();
    (((ctx.vw - w) / 2.0).floor(), ((ctx.vh - h) / 2.0).floor())
}

const LABEL_COLOR: u32 = 0x404040;

fn draw_labels(p: &mut Painter, snap: &Snapshot, left: f32, top: f32, img_h: f32, title_x: f32) {
    draw_title(p, &snap.container_title, left + title_x, top + 6.0);
    draw_inventory_label(p, left, top, img_h);
}

fn draw_inventory_label(p: &mut Painter, left: f32, top: f32, img_h: f32) {
    draw_inventory_label_at(p, left, top, img_h, 8.0);
}

fn draw_inventory_label_at(p: &mut Painter, left: f32, top: f32, img_h: f32, x: f32) {
    p.text_plain(
        inventory_label(),
        left + x,
        top + img_h - 94.0,
        LABEL_COLOR,
        false,
    );
}

fn draw_labels_centered(
    p: &mut Painter,
    snap: &Snapshot,
    left: f32,
    top: f32,
    img_w: f32,
    img_h: f32,
) {
    let w = title_width(p, &snap.container_title);
    draw_labels(p, snap, left, top, img_h, ((img_w - w) / 2.0).floor());
}

fn draw_title(p: &mut Painter, title: &[crate::text::Span], x: f32, y: f32) {
    let mut pen = x;
    for span in title {
        pen = p.text_plain(&span.text, pen, y, LABEL_COLOR, false);
    }
}

fn title_width(p: &Painter, title: &[crate::text::Span]) -> f32 {
    title
        .iter()
        .map(|s| p.atlas.font.width_str(&s.text))
        .sum::<f32>()
}

fn inventory_label() -> &'static str {
    use std::sync::OnceLock;
    static LABEL: OnceLock<String> = OnceLock::new();
    LABEL.get_or_init(|| tooltip::translate("container.inventory", &[]))
}

fn error_icon(p: &mut Painter, sprite: &str, x: f32, y: f32) {
    p.sprite(sprite, x, y, 28.0, 21.0);
}

pub use crate::gui::painter::Digits;

fn hovering(ctx: &ScreenCtx, x: f32, y: f32, w: f32, h: f32) -> bool {
    ctx.hovering(x, y, w, h)
}

fn button(ctx: &ScreenCtx, x: f32, y: f32, w: f32, h: f32) -> (bool, bool) {
    let hovered = hovering(ctx, x, y, w, h);
    (hovered, hovered && ctx.input.left_click)
}

pub struct Scroller {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub travel: f32,
    pub sprite: &'static str,
    pub disabled: &'static str,
}

impl Scroller {
    pub fn run(&self, p: &mut Painter, ctx: &ScreenCtx, ui: &mut ContainerUi, active: bool) {
        if !active {
            ui.scroll = 0.0;
            ui.dragging = false;
            p.sprite(self.disabled, self.x, self.y, self.w, self.h);
            return;
        }

        if ctx.input.left_click && hovering(ctx, self.x, self.y, self.w, self.travel + self.h) {
            ui.dragging = true;
        }
        if !ctx.input.left_down {
            ui.dragging = false;
        }
        if ui.dragging
            && let Some(m) = ctx.mouse()
        {
            ui.scroll = ((m.y - self.y - self.h / 2.0) / self.travel).clamp(0.0, 1.0);
        }

        p.sprite(
            self.sprite,
            self.x,
            self.y + (ui.scroll * self.travel).floor(),
            self.w,
            self.h,
        );
    }
}

pub fn scroll_rows(scroll: f32, wheel: f32, rows: usize, visible: usize) -> f32 {
    let offscreen = rows.saturating_sub(visible);
    if offscreen == 0 || wheel == 0.0 {
        return if offscreen == 0 { 0.0 } else { scroll };
    }
    (scroll - wheel / offscreen as f32).clamp(0.0, 1.0)
}

pub fn first_row(scroll: f32, rows: usize, visible: usize) -> usize {
    let offscreen = rows.saturating_sub(visible);
    (scroll * offscreen as f32 + 0.5) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_that_fits_does_not_scroll() {
        assert_eq!(scroll_rows(0.0, -5.0, 3, 12), 0.0);
        assert_eq!(scroll_rows(0.9, 1.0, 12, 12), 0.0);
        assert_eq!(first_row(1.0, 5, 12), 0);
    }

    #[test]
    fn the_wheel_moves_one_row_per_notch() {
        assert!((scroll_rows(0.0, -1.0, 16, 12) - 0.25).abs() < 1e-6);
        assert_eq!(scroll_rows(0.0, 1.0, 16, 12), 0.0);
        assert_eq!(scroll_rows(1.0, -1.0, 16, 12), 1.0);
    }

    #[test]
    fn the_last_scroll_position_shows_the_last_rows() {
        assert_eq!(first_row(0.0, 16, 12), 0);
        assert_eq!(first_row(1.0, 16, 12), 4);
        assert_eq!(first_row(0.5, 16, 12), 2);
    }

    #[test]
    fn digits_render_without_allocating() {
        assert_eq!(Digits::new(0).as_str(), "0");
        assert_eq!(Digits::new(7).as_str(), "7");
        assert_eq!(Digits::new(4294967295).as_str(), "4294967295");
        assert_eq!(Digits::new(102).as_str(), "102");
    }
}
