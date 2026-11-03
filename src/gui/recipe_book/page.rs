use crate::client::recipes::{
    BookRequest, CraftableStatus, RecipeBook, ResolvedDisplay, Selection, Shape, Stacks, cycle,
    place_recipe,
};
use crate::gui::ScreenCtx;
use crate::gui::painter::Painter;
use crate::session::SlotStack;

pub(super) const ANIMATION_TIME: f32 = 15.0;
const OVERLAY_PITCH: i32 = 25;
const OVERLAY_BUTTON: f32 = 24.0;
const OVERLAY_ITEM_SCALE: f32 = 0.375;

pub(super) fn squeeze(anim: f32) -> f32 {
    1.0 + 0.1 * (anim / ANIMATION_TIME * std::f32::consts::PI).sin()
}

pub(super) struct Button {
    pub collection: usize,
    entries: Vec<(u32, Stacks)>,
    same_result: bool,
    craftable: bool,
    pub anim: f32,
}

impl Button {
    pub(super) fn new(
        book: &RecipeBook,
        sel: &Selection,
        collection: usize,
        filtering: bool,
        out: &mut Vec<BookRequest>,
    ) -> Self {
        let status = if filtering {
            CraftableStatus::Craftable
        } else {
            CraftableStatus::Any
        };
        let entries: Vec<(u32, Stacks)> = sel
            .selected_recipes(book, collection, status)
            .map(|e| {
                let e = book.entry(e);
                (e.id, e.display.result.clone())
            })
            .collect();
        let mut anim = 0.0;
        for &(id, _) in &entries {
            if book.will_highlight(id) {
                out.push(BookRequest::Seen(id));
                anim = ANIMATION_TIME;
            }
        }
        Self {
            collection,
            same_result: same_result(&entries),
            craftable: sel.has_craftable(book, collection),
            entries,
            anim,
        }
    }

    pub(super) fn many(&self) -> bool {
        self.entries.len() > 1
    }

    pub(super) fn only_option(&self) -> bool {
        self.entries.len() == 1
    }

    pub(super) fn current_recipe(&self, index: usize) -> Option<u32> {
        let n = self.entries.len();
        (n > 0).then(|| self.entries[index % n].0)
    }

    pub(super) fn display_stack(&self, index: usize) -> Option<&SlotStack> {
        let n = self.entries.len();
        if n == 0 {
            return None;
        }
        cycle(&self.entries[index % n].1, index / n)
    }

    pub(super) fn draw(&self, p: &mut Painter, x: f32, y: f32, index: usize) {
        let sprite = match (self.craftable, self.many()) {
            (true, true) => "recipe_book/slot_many_craftable",
            (true, false) => "recipe_book/slot_craftable",
            (false, true) => "recipe_book/slot_many_uncraftable",
            (false, false) => "recipe_book/slot_uncraftable",
        };
        p.sprite(sprite, x, y, 25.0, 25.0);
        let Some(stack) = self.display_stack(index) else {
            return;
        };
        let key = stack.model_key();
        let mut offset = 4.0;
        if self.many() && self.same_result {
            p.item_icon(&key, x + offset + 1.0, y + offset + 1.0);
            offset -= 1.0;
        }
        p.item_icon(&key, x + offset, y + offset);
    }
}

fn same_item(a: &SlotStack, b: &SlotStack) -> bool {
    a.item == b.item
        && *a
            == SlotStack {
                count: a.count,
                ..b.clone()
            }
}

fn same_result(entries: &[(u32, Stacks)]) -> bool {
    let mut items = entries.iter().flat_map(|(_, s)| s.iter());
    match items.next() {
        Some(first) => items.all(|s| same_item(first, s)),
        None => true,
    }
}

struct OverlayButton {
    id: u32,
    craftable: bool,
    slots: Vec<(f32, f32, Stacks)>,
}

pub(super) struct Overlay {
    x: i32,
    y: i32,
    max_row: usize,
    furnace: bool,
    buttons: Vec<OverlayButton>,
}

fn max_row(total: usize) -> usize {
    if total <= 16 { 4 } else { 5 }
}

pub(super) fn overlay_origin(total: usize, button: (i32, i32), centre: (i32, i32)) -> (i32, i32) {
    let w = OVERLAY_PITCH as f32;
    let max_row = max_row(total);
    let rows = (total as f32 / max_row as f32).ceil() as i32;
    let (mut x, mut y) = button;
    let right = (x + total.min(max_row) as i32 * OVERLAY_PITCH) as f32;
    let max_left = (centre.0 + 50) as f32;
    if right > max_left {
        x = (x as f32 - w * (((right - max_left) / w) as i32) as f32) as i32;
    }
    let bottom = (y + rows * OVERLAY_PITCH) as f32;
    let max_bottom = (centre.1 + 50) as f32;
    if bottom > max_bottom {
        y = (y as f32 - w * ((bottom - max_bottom) / w).ceil()) as i32;
    }
    let top = y as f32;
    let max_top = (centre.1 - 100) as f32;
    if top < max_top {
        y = (y as f32 - w * ((top - max_top) / w).ceil()) as i32;
    }
    (x, y)
}

fn ingredient_slots(display: &ResolvedDisplay, furnace: bool) -> Vec<(f32, f32, Stacks)> {
    let pos = |gx: usize, gy: usize, s: &Stacks| {
        (3.0 + gx as f32 * 7.0, 3.0 + gy as f32 * 7.0, s.clone())
    };
    let mut out = Vec::new();
    if furnace {
        if display.shape == Shape::Furnace
            && let Some(s) = display.ingredients.first()
            && !s.is_empty()
        {
            out.push(pos(1, 1, s));
        }
        return out;
    }
    match display.shape {
        Shape::Shaped { width, height } => {
            let cells = &display.ingredients;
            place_recipe(
                3,
                3,
                width as usize,
                height as usize,
                cells.len(),
                |cell, grid| {
                    if !cells[cell].is_empty() {
                        out.push(pos(grid % 3, grid / 3, &cells[cell]));
                    }
                },
            );
        }
        Shape::Shapeless => {
            for (i, s) in display.ingredients.iter().enumerate() {
                if !s.is_empty() {
                    out.push(pos(i % 3, i / 3, s));
                }
            }
        }
        _ => {}
    }
    out
}

impl Overlay {
    pub(super) fn new(
        book: &RecipeBook,
        sel: &Selection,
        collection: usize,
        filtering: bool,
        furnace: bool,
        button: (i32, i32),
        centre: (i32, i32),
    ) -> Self {
        let craftable = sel
            .selected_recipes(book, collection, CraftableStatus::Craftable)
            .map(|e| (e, true));
        let rest = sel
            .selected_recipes(book, collection, CraftableStatus::NotCraftable)
            .filter(|_| !filtering)
            .map(|e| (e, false));
        let buttons: Vec<OverlayButton> = craftable
            .chain(rest)
            .map(|(e, craftable)| {
                let e = book.entry(e);
                OverlayButton {
                    id: e.id,
                    craftable,
                    slots: ingredient_slots(&e.display, furnace),
                }
            })
            .collect();
        let (x, y) = overlay_origin(buttons.len(), button, centre);
        Self {
            x,
            y,
            max_row: max_row(buttons.len()),
            furnace,
            buttons,
        }
    }

    fn button_pos(&self, i: usize) -> (f32, f32) {
        let r = self.max_row;
        (
            (self.x + 4 + OVERLAY_PITCH * (i % r) as i32) as f32,
            (self.y + 5 + OVERLAY_PITCH * (i / r) as i32) as f32,
        )
    }

    pub(super) fn hit(&self, ctx: &ScreenCtx) -> Option<u32> {
        (0..self.buttons.len()).find_map(|i| {
            let (x, y) = self.button_pos(i);
            ctx.hovering(x, y, OVERLAY_BUTTON, OVERLAY_BUTTON)
                .then_some(self.buttons[i].id)
        })
    }

    pub(super) fn draw(&self, p: &mut Painter, ctx: &ScreenCtx, index: usize) {
        let n = self.buttons.len();
        let cols = n.min(self.max_row) as f32;
        let rows = n.div_ceil(self.max_row) as f32;
        let w = OVERLAY_PITCH as f32;
        p.sprite(
            "recipe_book/overlay_recipe",
            self.x as f32,
            self.y as f32,
            cols * w + 8.0,
            rows * w + 8.0,
        );
        for (i, b) in self.buttons.iter().enumerate() {
            let (x, y) = self.button_pos(i);
            let hovered = ctx.hovering(x, y, OVERLAY_BUTTON, OVERLAY_BUTTON);
            let sprite = match (self.furnace, b.craftable, hovered) {
                (false, true, false) => "recipe_book/crafting_overlay",
                (false, true, true) => "recipe_book/crafting_overlay_highlighted",
                (false, false, false) => "recipe_book/crafting_overlay_disabled",
                (false, false, true) => "recipe_book/crafting_overlay_disabled_highlighted",
                (true, true, false) => "recipe_book/furnace_overlay",
                (true, true, true) => "recipe_book/furnace_overlay_highlighted",
                (true, false, false) => "recipe_book/furnace_overlay_disabled",
                (true, false, true) => "recipe_book/furnace_overlay_disabled_highlighted",
            };
            p.sprite(sprite, x, y, OVERLAY_BUTTON, OVERLAY_BUTTON);
            for (px, py, stacks) in &b.slots {
                let Some(s) = cycle(stacks, index) else {
                    continue;
                };
                let (ox, oy) = (
                    x + 2.0 + px - 8.0 * OVERLAY_ITEM_SCALE,
                    y + 2.0 + py - 8.0 * OVERLAY_ITEM_SCALE,
                );
                p.scaled(OVERLAY_ITEM_SCALE, ox, oy, |p| {
                    p.stack_icon_as(s, 0.0, 0.0, s.item_use())
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn stacks(items: &[&'static str]) -> Stacks {
        items
            .iter()
            .map(|&item| SlotStack {
                item,
                count: 1,
                ..Default::default()
            })
            .collect::<Vec<_>>()
            .into()
    }

    fn button(entries: Vec<(u32, Stacks)>) -> Button {
        Button {
            collection: 0,
            same_result: same_result(&entries),
            craftable: false,
            entries,
            anim: 0.0,
        }
    }

    #[test]
    fn display_stack_walks_recipes_then_items() {
        let b = button(vec![
            (1, stacks(&["oak_planks", "birch_planks"])),
            (2, stacks(&["stick"])),
        ]);
        let shown: Vec<&str> = (0..4).map(|i| b.display_stack(i).unwrap().item).collect();
        assert_eq!(shown, ["oak_planks", "stick", "birch_planks", "stick"]);
        assert_eq!(b.current_recipe(3), Some(2));
        assert!(!b.only_option() && !b.same_result);
        assert!(button(vec![(1, stacks(&["stick"])), (2, stacks(&["stick"]))]).same_result);
        assert_eq!(button(Vec::new()).display_stack(5), None);
    }

    #[test]
    fn same_result_ignores_count_only() {
        let a = SlotStack {
            item: "stick",
            count: 4,
            ..Default::default()
        };
        let b = SlotStack {
            item: "stick",
            count: 1,
            ..Default::default()
        };
        assert!(same_item(&a, &b));
        let c = SlotStack {
            damage: 1,
            ..b.clone()
        };
        assert!(!same_item(&a, &c));
    }

    #[test]
    fn overlay_steps_back_inside_the_book() {
        assert_eq!(overlay_origin(3, (111, 31), (73, 96)), (61, 31));
        assert_eq!(overlay_origin(20, (11, 106), (73, 96)), (11, 31));
        assert_eq!(overlay_origin(2, (11, -30), (73, 96)), (11, -5));
    }

    #[test]
    fn overlay_ingredients_on_the_3x3_grid() {
        let one: Arc<[Stacks]> = vec![stacks(&["diamond"])].into();
        let shaped = ResolvedDisplay {
            shape: Shape::Shaped {
                width: 1,
                height: 1,
            },
            ingredients: one.clone(),
            result: stacks(&["diamond_block"]),
            station: stacks(&[]),
        };
        let at: Vec<(f32, f32)> = ingredient_slots(&shaped, false)
            .iter()
            .map(|s| (s.0, s.1))
            .collect();
        assert_eq!(at, [(10.0, 10.0)]);
        assert!(ingredient_slots(&shaped, true).is_empty());

        let shapeless = ResolvedDisplay {
            shape: Shape::Shapeless,
            ingredients: vec![stacks(&["a"]), stacks(&[]), stacks(&["b"]), stacks(&["c"])].into(),
            ..shaped.clone()
        };
        let at: Vec<(f32, f32)> = ingredient_slots(&shapeless, false)
            .iter()
            .map(|s| (s.0, s.1))
            .collect();
        assert_eq!(at, [(3.0, 3.0), (17.0, 3.0), (3.0, 10.0)]);

        let furnace = ResolvedDisplay {
            shape: Shape::Furnace,
            ..shaped
        };
        let at: Vec<(f32, f32)> = ingredient_slots(&furnace, true)
            .iter()
            .map(|s| (s.0, s.1))
            .collect();
        assert_eq!(at, [(10.0, 10.0)]);
    }
}
