use azalea_core::registry_holder::RegistryHolder;
use azalea_inventory::components as comp;
use azalea_inventory::{ItemStack, ItemStackData};

use super::Opts;
use crate::blockentities::banner::BannerLayer;
use crate::gui::book;
use crate::gui::painter::Painter;
use crate::session::{SlotStack, StackExtra};
use crate::text::Span;

const GAP: f32 = 2.0;

const BANNER_EDGE: f32 = 64.0;
const ITEM_EDGE: f32 = 16.0;

const BOOK_SCALE: f32 = 0.7;
const BOOK_PAGE_LEFT: f32 = 20.0;
const BOOK_PAGE_TOP: f32 = 1.0;
const BOOK_PAGE_W: f32 = 146.0;
const BOOK_PAGE_H: f32 = 180.0;

const SHEET: &str = "container/shulker_box";
const EDGE: f32 = 7.0;
const SLOT: f32 = 18.0;
const SHEET_ROW_Y: f32 = 17.0;
const SHEET_RIGHT_X: f32 = 169.0;
const SHEET_BOTTOM_Y: f32 = 159.0;

pub enum Preview {
    Grid(Grid),
    Map { id: i32, size: f32 },
    Banner(SlotStack),
    Book { page: Vec<Span>, pages: usize },
}

pub struct Grid {
    cols: usize,
    rows: usize,
    tint: u32,
    items: Vec<(usize, SlotStack)>,
}

impl Preview {
    pub fn width(&self) -> f32 {
        match self {
            Preview::Grid(grid) => grid.width(),
            Preview::Map { size, .. } => *size,
            Preview::Banner(_) => BANNER_EDGE,
            Preview::Book { .. } => (BOOK_PAGE_W * BOOK_SCALE).ceil(),
        }
    }

    pub fn height(&self) -> f32 {
        let own = match self {
            Preview::Grid(grid) => grid.height(),
            Preview::Map { size, .. } => *size,
            Preview::Banner(_) => BANNER_EDGE,
            Preview::Book { .. } => (BOOK_PAGE_H * BOOK_SCALE).ceil(),
        };
        own + GAP
    }

    pub fn draw(&self, p: &mut Painter, x: f32, y: f32, w: f32) {
        match self {
            Preview::Grid(grid) => grid.draw(p, x, y),
            Preview::Map { id, size } => {
                crate::gui::map_view::draw(p, *id, x, y, *size);
            }
            Preview::Banner(stack) => {
                let left = x + ((w - BANNER_EDGE) / 2.0).floor();
                p.scaled_in(BANNER_EDGE / ITEM_EDGE, left, y, |p| {
                    p.stack_icon(stack, 0.0, 0.0)
                });
            }
            Preview::Book { page, pages } => {
                let left = x - BOOK_PAGE_LEFT * BOOK_SCALE;
                let top = y - BOOK_PAGE_TOP * BOOK_SCALE;
                p.scaled_in(BOOK_SCALE, left, top, |p| {
                    p.blit_sheet("book", 0.0, 0.0, book::IMAGE_W, book::IMAGE_H, 0.0, 0.0);
                    book::indicator(p, 0.0, 0.0, 0, *pages);
                    book::page_text(p, 0.0, 0.0, page);
                });
            }
        }
    }
}

impl Grid {
    fn width(&self) -> f32 {
        2.0 * EDGE + self.cols as f32 * SLOT
    }

    fn height(&self) -> f32 {
        2.0 * EDGE + self.rows as f32 * SLOT
    }

    fn draw(&self, p: &mut Painter, x: f32, y: f32) {
        let inner = self.cols as f32 * SLOT;
        let right = x + EDGE + inner;
        let strip = |p: &mut Painter, sy: f32, h: f32, at: f32| {
            p.blit_sheet_tinted(SHEET, 0.0, sy, EDGE + inner, h, x, at, self.tint);
            p.blit_sheet_tinted(SHEET, SHEET_RIGHT_X, sy, EDGE, h, right, at, self.tint);
        };
        strip(p, 0.0, EDGE, y);
        for row in 0..self.rows {
            strip(p, SHEET_ROW_Y, SLOT, y + EDGE + row as f32 * SLOT);
        }
        strip(p, SHEET_BOTTOM_Y, EDGE, y + EDGE + self.rows as f32 * SLOT);
        for (i, stack) in &self.items {
            let (col, row) = (i % self.cols, i / self.cols);
            let sx = x + EDGE + 1.0 + col as f32 * SLOT;
            let sy = y + EDGE + 1.0 + row as f32 * SLOT;
            crate::gui::slots::draw_stack(p, stack, sx, sy);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Grid,
    Map,
    Book,
    Banner,
    Bucket,
}

fn kind(stack: &SlotStack, data: &ItemStackData, opts: &Opts) -> Option<Kind> {
    if opts.containers && has_items(data) {
        Some(Kind::Grid)
    } else if opts.maps
        && stack.item == "filled_map"
        && stack.map_id.and_then(crate::gui::map_view::meta).is_some()
    {
        Some(Kind::Map)
    } else if opts.books && matches!(stack.item, "writable_book" | "written_book") {
        Some(Kind::Book)
    } else if opts.banners && is_bannerlike(stack.item, data) {
        Some(Kind::Banner)
    } else if opts.buckets && super::rows::is_mob_bucket(stack.item) {
        Some(Kind::Bucket)
    } else {
        None
    }
}

pub(crate) fn applies(stack: &SlotStack, data: &ItemStackData, opts: &Opts) -> bool {
    kind(stack, data, opts).is_some()
}

pub(crate) fn build(
    stack: &SlotStack,
    data: &ItemStackData,
    registries: Option<&RegistryHolder>,
    opts: &Opts,
) -> Option<Preview> {
    if !opts.previewing {
        return None;
    }
    match kind(stack, data, opts)? {
        Kind::Grid => grid(stack.item, data, registries).map(Preview::Grid),
        Kind::Map => {
            let id = stack.map_id?;
            crate::gui::map_view::meta(id)?;
            Some(Preview::Map {
                id,
                size: opts.map_size,
            })
        }
        Kind::Book => {
            let book = stack.extra.book.as_deref()?;
            let page = book.pages.first()?.clone();
            Some(Preview::Book {
                page,
                pages: book.pages.len(),
            })
        }
        Kind::Banner => banner(stack, data).map(Preview::Banner),
        Kind::Bucket => None,
    }
}

fn has_items(data: &ItemStackData) -> bool {
    crate::items::component::<comp::Container>(data)
        .is_some_and(|c| c.items.iter().any(ItemStack::is_present))
}

fn shape(item: &str, len: usize) -> (usize, usize) {
    match item {
        "dispenser" | "dropper" | "crafter" => (3, 3),
        "hopper" => (5, 1),
        "decorated_pot" => (1, 1),
        "chest" | "trapped_chest" | "barrel" => (9, 3),
        _ if item == "shulker_box"
            || item.ends_with("_shulker_box")
            || item.ends_with("copper_chest") =>
        {
            (9, 3)
        }
        _ => (9, len.div_ceil(9).max(1)),
    }
}

fn tint(item: &str) -> u32 {
    let dye = item
        .strip_suffix("_shulker_box")
        .and_then(crate::util::dye::id_by_name)
        .map_or(0xFF_FFFF, |id| crate::util::dye::by_id(id).texture_diffuse);
    0xFF00_0000 | dye
}

fn grid(item: &str, data: &ItemStackData, registries: Option<&RegistryHolder>) -> Option<Grid> {
    let container = crate::items::component::<comp::Container>(data)?;
    let (cols, rows) = shape(item, container.items.len());
    let items = container
        .items
        .iter()
        .take(cols * rows)
        .enumerate()
        .filter(|(_, stack)| stack.is_present())
        .map(|(i, stack)| {
            (
                i,
                crate::play::inventory_bridge::slot_stack(stack, registries, false),
            )
        })
        .collect();
    Some(Grid {
        cols,
        rows,
        tint: tint(item),
        items,
    })
}

fn is_bannerlike(item: &str, data: &ItemStackData) -> bool {
    item.ends_with("_banner")
        || crate::items::component::<comp::ProvidesBannerPatterns>(data).is_some()
        || (item == "shield"
            && crate::items::component::<comp::BannerPatterns>(data)
                .is_some_and(|b| !b.patterns.is_empty()))
}

const WHITE: u8 = 0;

fn banner(stack: &SlotStack, data: &ItemStackData) -> Option<SlotStack> {
    if stack.item.ends_with("_banner") || stack.item == "shield" {
        return Some(stack.clone());
    }
    let pattern = provided_pattern(stack, data)?;
    let layer = BannerLayer {
        asset: crate::blockentities::banner::asset_for(&pattern),
        color: WHITE,
    };
    Some(SlotStack {
        item: "gray_banner",
        count: 1,
        extra: StackExtra {
            banner_layers: Some(vec![layer].into_boxed_slice()),
            ..Default::default()
        }
        .into(),
        ..Default::default()
    })
}

fn provided_pattern(stack: &SlotStack, data: &ItemStackData) -> Option<String> {
    if let Some(first) = stack
        .extra
        .banner_patterns
        .as_deref()
        .and_then(<[_]>::first)
    {
        return Some(first.to_string());
    }
    let provided = crate::items::component::<comp::ProvidesBannerPatterns>(data)?;
    let azalea_registry::HolderSet::Named { key, .. } = &provided.key else {
        return None;
    };
    if key.namespace() != "minecraft" {
        return None;
    }
    let tag = crate::util::datapack::entry("tags/banner_pattern", key.path())?;
    tag.get("values")?
        .as_array()?
        .iter()
        .filter_map(crate::util::datapack::bare_id)
        .find(|id| !id.starts_with('#'))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_shapes_follow_the_menus() {
        assert_eq!(shape("shulker_box", 3), (9, 3));
        assert_eq!(shape("red_shulker_box", 27), (9, 3));
        assert_eq!(shape("chest", 1), (9, 3));
        assert_eq!(shape("waxed_oxidized_copper_chest", 1), (9, 3));
        assert_eq!(shape("barrel", 1), (9, 3));
        assert_eq!(shape("dispenser", 1), (3, 3));
        assert_eq!(shape("dropper", 9), (3, 3));
        assert_eq!(shape("crafter", 2), (3, 3));
        assert_eq!(shape("hopper", 5), (5, 1));
        assert_eq!(shape("decorated_pot", 1), (1, 1));
        assert_eq!(shape("furnace", 3), (9, 1));
        assert_eq!(shape("chiseled_bookshelf", 0), (9, 1));
        assert_eq!(shape("modded_crate", 19), (9, 3));
    }

    #[test]
    fn a_three_row_grid_is_the_shulker_panel_width() {
        let grid = Grid {
            cols: 9,
            rows: 3,
            tint: 0,
            items: Vec::new(),
        };
        assert_eq!(grid.width(), 176.0);
        assert_eq!(grid.height(), 68.0);
    }

    #[test]
    fn only_dyed_shulker_boxes_are_tinted() {
        assert_eq!(tint("red_shulker_box"), 0xFF00_0000 | 11546150);
        assert_eq!(tint("shulker_box"), 0xFFFF_FFFF);
        assert_eq!(tint("chest"), 0xFFFF_FFFF);
    }
}
