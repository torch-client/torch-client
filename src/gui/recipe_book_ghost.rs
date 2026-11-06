use crate::client::recipes::{
    BookMenu, BookRequest, BookType, ResolvedDisplay, Shape, Stacks, TypeSettings, cycle,
    place_recipe,
};
use crate::gui::painter::Painter;
use crate::gui::recipe_book::{self, BookCtx};
use crate::gui::render::GuiInput;
use crate::gui::slots::{Layout, SLOT_PITCH};
use crate::gui::{GuiState, Screen, ScreenCtx, Snapshot};
use crate::session::{Gamemode, InvAction, SlotStack};
use azalea_inventory::operations::{
    ClickOperation, PickupClick, QuickCraftStatus, QuickMoveClick, ThrowClick,
};

const GHOST_BACK: u32 = 0x30FF_0000;
const GHOST_FRONT: u32 = 0x30FF_FFFF;

const NARROW_WIDTH: f32 = 379.0;

#[derive(Default)]
pub struct GhostSlots {
    slots: Vec<GhostSlot>,
}

struct GhostSlot {
    slot: usize,
    items: Stacks,
    result: bool,
}

impl GhostSlots {
    pub fn clear(&mut self) {
        self.slots.clear();
    }

    fn set(&mut self, slot: usize, items: &Stacks, result: bool) {
        if !items.is_empty() {
            self.slots.retain(|g| g.slot != slot);
            self.slots.push(GhostSlot {
                slot,
                items: items.clone(),
                result,
            });
        }
    }

    pub fn fill(&mut self, menu: BookMenu, display: &ResolvedDisplay, slots: &[SlotStack]) {
        self.clear();
        self.set(menu.result_slot as usize, &display.result, true);
        let first = menu.craft_slots.0 as usize;
        let ing = &display.ingredients;
        if menu.book != BookType::Crafting {
            if display.shape == Shape::Furnace && ing.len() == 2 {
                self.set(first, &ing[0], false);
                if slots.get(first + 1).is_none_or(SlotStack::is_empty) {
                    self.set(first + 1, &ing[1], false);
                }
            }
            return;
        }
        match display.shape {
            Shape::Shaped { width, height } => place_recipe(
                menu.grid_width as usize,
                menu.grid_height as usize,
                width as usize,
                height as usize,
                ing.len(),
                |cell, grid| self.set(first + grid, &ing[cell], false),
            ),
            Shape::Shapeless => {
                for (i, items) in ing.iter().take(menu.craft_slots().len()).enumerate() {
                    self.set(first + i, items, false);
                }
            }
            _ => {}
        }
    }

    pub fn draw(
        &self,
        p: &mut Painter,
        layout: Layout,
        origin: (f32, f32),
        index: usize,
        big_result: bool,
    ) {
        for g in &self.slots {
            let Some(item) = cycle(&g.items, index) else {
                continue;
            };
            let (sx, sy) = layout.slot_pos(g.slot);
            let (x, y) = (origin.0 + sx, origin.1 + sy);
            if g.result && big_result {
                p.fill(x - 4.0, y - 4.0, 24.0, 24.0, GHOST_BACK);
            } else {
                p.fill(x, y, 16.0, 16.0, GHOST_BACK);
            }
            p.item_icon_for(&item.model_key(), item.item, item.item_use(), x, y, false);
            p.fill(x, y, 16.0, 16.0, GHOST_FRONT);
            if g.result {
                p.item_decorations(x, y, item.count, item.damage, item.max_damage);
            }
        }
    }

    fn hovered(
        &self,
        ctx: &ScreenCtx,
        layout: Layout,
        origin: (f32, f32),
        index: usize,
    ) -> Option<SlotStack> {
        self.slots.iter().find_map(|g| {
            let (sx, sy) = layout.slot_pos(g.slot);
            ctx.hovering(
                origin.0 + sx - 1.0,
                origin.1 + sy - 1.0,
                SLOT_PITCH,
                SLOT_PITCH,
            )
            .then(|| cycle(&g.items, index).cloned())
            .flatten()
        })
    }
}

pub fn book_menu(screen: Screen, container_id: i32) -> Option<(BookMenu, i32)> {
    match screen {
        Screen::Inventory => Some((BookMenu::INVENTORY, 0)),
        Screen::Container(kind) if container_id != 0 => {
            BookMenu::for_container(container_id, kind).map(|m| (m, container_id))
        }
        _ => None,
    }
}

pub fn left_pos(screen: Screen, ctx: &ScreenCtx, snap: &Snapshot, w: f32) -> f32 {
    match (&snap.recipe_book, book_menu(screen, snap.container_id)) {
        (Some(book), Some((menu, _))) => {
            let open = book.settings().get(menu.book).open;
            recipe_book::left_pos(open, ctx.vw as i32, w as i32) as f32
        }
        _ => ((ctx.vw - w) / 2.0).floor(),
    }
}

fn clicked_slot(op: &ClickOperation) -> Option<u16> {
    match op {
        ClickOperation::Pickup(PickupClick::Left { slot } | PickupClick::Right { slot }) => *slot,
        ClickOperation::Pickup(_) => None,
        ClickOperation::QuickMove(
            QuickMoveClick::Left { slot } | QuickMoveClick::Right { slot },
        ) => Some(*slot),
        ClickOperation::Swap(s) => Some(s.source_slot),
        ClickOperation::Clone(c) => Some(c.slot),
        ClickOperation::Throw(ThrowClick::Single { slot } | ThrowClick::All { slot }) => {
            Some(*slot)
        }
        ClickOperation::QuickCraft(q) => match q.status {
            QuickCraftStatus::Add { slot } => Some(slot),
            _ => None,
        },
        ClickOperation::PickupAll(p) => Some(p.slot),
    }
}

fn is_crafting_slot(menu: BookMenu, slot: usize) -> bool {
    slot == menu.result_slot as usize || menu.craft_slots().contains(&slot)
}

pub struct Frame<'a> {
    pub origin: (f32, f32),
    pub ctx: ScreenCtx<'a>,
}

#[allow(clippy::too_many_arguments)]
pub fn draw_screen(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    layout: Layout,
    button: (f32, f32),
    big_result: bool,
    out: &mut Vec<InvAction>,
    container: impl FnOnce(
        &mut Painter,
        &mut GuiState,
        &Frame,
        &mut Vec<InvAction>,
    ) -> Option<SlotStack>,
) -> Option<SlotStack> {
    let (w, h) = layout.size();
    let top = ((ctx.vh - h) / 2.0).floor();
    crate::gui::screens::dim_background(p, ctx.vw, ctx.vh);
    let (Some(book), Some((menu, container_id))) = (
        &snap.recipe_book,
        book_menu(state.screen, snap.container_id),
    ) else {
        let frame = Frame {
            origin: (((ctx.vw - w) / 2.0).floor(), top),
            ctx: ctx.with_input(ctx.input),
        };
        return container(p, state, &frame, out);
    };
    let open = book.settings().get(menu.book).open;
    let left = recipe_book::left_pos(open, ctx.vw as i32, w as i32) as f32;
    let hidden = open && ctx.vw < NARROW_WIDTH;
    let bctx = BookCtx {
        book,
        menu,
        slots: &snap.menu_slots,
        container_id,
        spectator: snap.gamemode == Gamemode::Spectator,
        ctrl_down: ctx.input.ctrl,
    };

    let mut book_input: GuiInput = ctx.input.clone();
    book_input.escape = false;
    let chat_key = ctx.input.pressed_key.is_some()
        && ctx.input.pressed_key == state.keybinds.key(crate::gui::keybinds::Action::Chat);
    let keys_taken = recipe_book::key_pressed(
        &mut state.recipe_book,
        &bctx,
        &book_input,
        chat_key,
        &mut state.book_requests,
    );

    let now = ctx.input.time;
    let dt_ticks = state.book_clock.map_or(0.0, |t| (now - t).max(0.0) * 20.0);
    state.book_clock = Some(now);
    recipe_book::draw(
        p,
        &mut state.recipe_book,
        ctx,
        &bctx,
        dt_ticks,
        &mut state.book_requests,
    );

    let over_book = ctx.mouse().is_some_and(|m| {
        recipe_book::blocks_container_click(&state.recipe_book, &bctx, m.x, m.y, left, top, w, h)
    });
    let mut masked: GuiInput = ctx.input.clone();
    if keys_taken {
        masked.hotbar_keys = [false; 9];
        masked.drop_key = false;
        masked.swap_key = false;
    }
    if over_book {
        masked.left_click = false;
        masked.right_click = false;
        masked.middle_click = false;
        masked.double_click = false;
        if !state.slots.drag.active() {
            masked.left_release = false;
            masked.right_release = false;
        }
    }

    let mut hovered = None;
    if !hidden {
        let frame = Frame {
            origin: (left, top),
            ctx: ctx.with_input(&masked),
        };
        let before = out.len();
        hovered = container(p, state, &frame, out);
        let crafting_click = out[before..].iter().any(|a| {
            matches!(a, InvAction::Click(op) if clicked_slot(op).is_some_and(|s| is_crafting_slot(menu, s as usize)))
        });
        if crafting_click {
            state.ghost.clear();
            state.recipe_book.crafting_slot_clicked();
        }
        let index = state.recipe_book.slot_select_index();
        state.ghost.draw(p, layout, (left, top), index, big_result);
        if let Some(ghost) = state.ghost.hovered(&frame.ctx, layout, (left, top), index) {
            hovered = Some(ghost);
        }
        recipe_book::toggle_button(
            p,
            ctx,
            &bctx,
            left + button.0,
            (ctx.vh / 2.0).floor() + button.1,
            &mut state.book_requests,
        );
    }
    recipe_book::draw_tooltip(p, &state.recipe_book, ctx, state.advanced_tooltips);
    hovered
}

pub fn key_before_close(
    state: &GuiState,
    shared: &crate::session::SharedMutex,
    escape: bool,
    vw: f32,
) -> bool {
    if !escape {
        return state.recipe_book.search_focused();
    }
    let mut s = shared.lock().unwrap();
    let Some((menu, _)) = book_menu(state.screen, s.session.container_id) else {
        return false;
    };
    let settings = s.session.recipes.book.settings().get(menu.book);
    if !settings.open || vw >= NARROW_WIDTH || s.session.gamemode == Gamemode::Spectator {
        return false;
    }
    s.session.recipes.request(BookRequest::Settings {
        book: menu.book,
        settings: TypeSettings {
            open: false,
            ..settings
        },
    });
    true
}

pub fn flush(state: &mut GuiState, recipes: &mut crate::client::recipes::RecipeBookState) {
    for r in state.book_requests.drain(..) {
        if matches!(r, BookRequest::Place { .. }) {
            state.ghost.clear();
        }
        recipes.request(r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::FurnaceKind;

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

    fn display(shape: Shape, ingredients: Vec<Stacks>) -> ResolvedDisplay {
        ResolvedDisplay {
            shape,
            ingredients: ingredients.into(),
            result: stacks(&["stick"]),
            station: stacks(&[]),
        }
    }

    fn filled(g: &GhostSlots) -> Vec<(usize, bool)> {
        let mut v: Vec<_> = g.slots.iter().map(|s| (s.slot, s.result)).collect();
        v.sort();
        v
    }

    #[test]
    fn shaped_ghost_centres_and_skips_blank_cells() {
        let mut g = GhostSlots::default();
        let d = display(
            Shape::Shaped {
                width: 1,
                height: 2,
            },
            vec![stacks(&["oak_planks"]), stacks(&["oak_planks"])],
        );
        g.fill(BookMenu::CRAFTING_TABLE, &d, &[]);
        assert_eq!(filled(&g), [(0, true), (2, false), (5, false)]);
        let d = display(
            Shape::Shaped {
                width: 2,
                height: 1,
            },
            vec![stacks(&[]), stacks(&["stone"])],
        );
        g.fill(BookMenu::INVENTORY, &d, &[]);
        assert_eq!(filled(&g), [(0, true), (2, false)]);
    }

    #[test]
    fn shapeless_ghost_fills_in_order_up_to_the_grid() {
        let mut g = GhostSlots::default();
        let d = display(Shape::Shapeless, vec![stacks(&["dye"]); 5]);
        g.fill(BookMenu::INVENTORY, &d, &[]);
        assert_eq!(
            filled(&g),
            [(0, true), (1, false), (2, false), (3, false), (4, false)]
        );
    }

    #[test]
    fn furnace_fuel_only_into_an_empty_slot() {
        let menu = BookMenu::furnace(FurnaceKind::Smoker);
        let d = display(
            Shape::Furnace,
            vec![stacks(&["beef"]), stacks(&["coal", "charcoal"])],
        );
        let mut g = GhostSlots::default();
        let mut slots = vec![SlotStack::default(); 39];
        g.fill(menu, &d, &slots);
        assert_eq!(filled(&g), [(0, false), (1, false), (2, true)]);
        slots[1] = SlotStack {
            item: "coal",
            count: 3,
            ..Default::default()
        };
        g.fill(menu, &d, &slots);
        assert_eq!(filled(&g), [(0, false), (2, true)]);
    }

    #[test]
    fn crafting_slots_per_menu() {
        let inv = BookMenu::INVENTORY;
        assert!(is_crafting_slot(inv, 0) && is_crafting_slot(inv, 4) && !is_crafting_slot(inv, 5));
        let furnace = BookMenu::furnace(FurnaceKind::Furnace);
        assert!(is_crafting_slot(furnace, 2) && !is_crafting_slot(furnace, 3));
        assert_eq!(
            book_menu(Screen::Container(crate::session::ContainerKind::Crafter), 4),
            None
        );
        assert_eq!(
            book_menu(
                Screen::Container(crate::session::ContainerKind::Crafting),
                0
            ),
            None
        );
    }
}
