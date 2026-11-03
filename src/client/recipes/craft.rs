use std::ops::Range;

use super::book::{BookType, RecipeBook};
use super::display::{ResolvedDisplay, Shape};
use crate::session::{ContainerKind, FurnaceKind, SlotStack};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BookMenu {
    pub book: BookType,
    pub grid_width: u8,
    pub grid_height: u8,
    pub result_slot: u8,
    pub craft_slots: (u8, u8),
    inventory_start: u8,
}

impl BookMenu {
    pub const INVENTORY: Self = Self {
        book: BookType::Crafting,
        grid_width: 2,
        grid_height: 2,
        result_slot: 0,
        craft_slots: (1, 5),
        inventory_start: 9,
    };

    pub const CRAFTING_TABLE: Self = Self {
        book: BookType::Crafting,
        grid_width: 3,
        grid_height: 3,
        result_slot: 0,
        craft_slots: (1, 10),
        inventory_start: 10,
    };

    pub const fn furnace(kind: FurnaceKind) -> Self {
        Self {
            book: match kind {
                FurnaceKind::Furnace => BookType::Furnace,
                FurnaceKind::BlastFurnace => BookType::BlastFurnace,
                FurnaceKind::Smoker => BookType::Smoker,
            },
            grid_width: 0,
            grid_height: 0,
            result_slot: 2,
            craft_slots: (0, 3),
            inventory_start: 3,
        }
    }

    pub fn for_container(container_id: i32, kind: ContainerKind) -> Option<Self> {
        if container_id == 0 {
            return Some(Self::INVENTORY);
        }
        match kind {
            ContainerKind::Crafting => Some(Self::CRAFTING_TABLE),
            ContainerKind::Furnace(k) => Some(Self::furnace(k)),
            _ => None,
        }
    }

    pub fn craft_slots(&self) -> Range<usize> {
        self.craft_slots.0 as usize..self.craft_slots.1 as usize
    }

    fn is_furnace(&self) -> bool {
        self.book != BookType::Crafting
    }

    pub fn can_display(&self, display: &ResolvedDisplay) -> bool {
        let (w, h) = (self.grid_width as usize, self.grid_height as usize);
        match display.shape {
            _ if self.is_furnace() => display.shape == Shape::Furnace,
            Shape::Shaped { width, height } => w >= width as usize && h >= height as usize,
            Shape::Shapeless => w * h >= display.ingredients.len(),
            _ => false,
        }
    }

    pub fn stacked_contents(&self, slots: &[SlotStack]) -> StackedContents {
        let mut out = StackedContents::default();
        let main = self.inventory_start as usize..self.inventory_start as usize + 36;
        for s in slots.get(main).unwrap_or(&[]) {
            out.account_simple(s);
        }
        for s in slots.get(self.craft_slots()).unwrap_or(&[]) {
            if self.is_furnace() {
                out.account(s);
            } else {
                out.account_simple(s);
            }
        }
        out
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StackedContents(Vec<(&'static str, u32)>);

impl StackedContents {
    pub fn account_simple(&mut self, s: &SlotStack) {
        let enchanted = !s.extra.enchantments.is_empty() && s.item != "enchanted_book";
        if s.damage == 0 && !enchanted && s.extra.custom_name.is_none() {
            self.account(s);
        }
    }

    pub fn account(&mut self, s: &SlotStack) {
        if s.is_empty() {
            return;
        }
        match self.0.iter_mut().find(|(item, _)| *item == s.item) {
            Some((_, n)) => *n += s.count as u32,
            None => self.0.push((s.item, s.count as u32)),
        }
    }

    pub fn can_craft(&self, ingredients: &[super::book::Ingredient]) -> bool {
        let adj: Vec<Vec<usize>> = ingredients
            .iter()
            .map(|ing| {
                (0..self.0.len())
                    .filter(|&j| self.0[j].1 > 0 && ing.accepts(self.0[j].0))
                    .collect()
            })
            .collect();
        if adj.iter().any(Vec::is_empty) {
            return false;
        }
        let mut assign: Vec<Option<usize>> = vec![None; ingredients.len()];
        let mut used = vec![0u32; self.0.len()];
        for i in 0..ingredients.len() {
            let mut seen = vec![false; self.0.len()];
            if !self.augment(i, &adj, &mut assign, &mut used, &mut seen) {
                return false;
            }
        }
        true
    }

    fn augment(
        &self,
        i: usize,
        adj: &[Vec<usize>],
        assign: &mut [Option<usize>],
        used: &mut [u32],
        seen: &mut [bool],
    ) -> bool {
        for &j in &adj[i] {
            if seen[j] {
                continue;
            }
            seen[j] = true;
            if used[j] < self.0[j].1 {
                used[j] += 1;
                assign[i] = Some(j);
                return true;
            }
            for k in 0..assign.len() {
                if assign[k] == Some(j) && self.augment(k, adj, assign, used, seen) {
                    assign[i] = Some(j);
                    return true;
                }
            }
        }
        false
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CraftableStatus {
    Any,
    Craftable,
    NotCraftable,
}

#[derive(Clone, Debug, Default)]
pub struct Selection {
    selected: Vec<bool>,
    craftable: Vec<bool>,
}

impl Selection {
    pub fn compute(book: &RecipeBook, menu: BookMenu, slots: &[SlotStack]) -> Self {
        let contents = menu.stacked_contents(slots);
        let categories = menu.book.search_categories();
        let mut selected = vec![false; book.entries().len()];
        let mut craftable = vec![false; book.entries().len()];
        for (i, e) in book.entries().iter().enumerate() {
            if !categories.contains(&e.category) || !menu.can_display(&e.display) {
                continue;
            }
            selected[i] = true;
            craftable[i] = e
                .requirements
                .as_deref()
                .is_some_and(|r| contents.can_craft(r));
        }
        Self {
            selected,
            craftable,
        }
    }

    pub fn is_craftable(&self, entry: usize) -> bool {
        self.craftable.get(entry).copied().unwrap_or(false)
    }

    pub fn is_selected(&self, entry: usize) -> bool {
        self.selected.get(entry).copied().unwrap_or(false)
    }

    pub fn has_craftable(&self, book: &RecipeBook, collection: usize) -> bool {
        book.collection(collection)
            .iter()
            .any(|&e| self.is_craftable(e))
    }

    pub fn has_any_selected(&self, book: &RecipeBook, collection: usize) -> bool {
        book.collection(collection)
            .iter()
            .any(|&e| self.is_selected(e))
    }

    pub fn selected_recipes<'a>(
        &'a self,
        book: &'a RecipeBook,
        collection: usize,
        status: CraftableStatus,
    ) -> impl Iterator<Item = usize> + 'a {
        book.collection(collection)
            .iter()
            .copied()
            .filter(move |&e| match status {
                CraftableStatus::Any => self.is_selected(e),
                CraftableStatus::Craftable => self.is_craftable(e),
                CraftableStatus::NotCraftable => self.is_selected(e) && !self.is_craftable(e),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::super::book::Ingredient;
    use super::*;

    fn stack(item: &'static str, count: u8) -> SlotStack {
        SlotStack {
            item,
            count,
            ..Default::default()
        }
    }

    fn contents(items: &[(&'static str, u8)]) -> StackedContents {
        let mut c = StackedContents::default();
        items.iter().for_each(|&(i, n)| c.account(&stack(i, n)));
        c
    }

    fn ing(items: &[&'static str]) -> Ingredient {
        Ingredient::new(items.to_vec())
    }

    #[test]
    fn counts_limit_how_often_an_item_is_used() {
        let planks = ing(&["oak_planks", "spruce_planks"]);
        let four = vec![planks.clone(); 4];
        assert!(contents(&[("oak_planks", 2), ("spruce_planks", 2)]).can_craft(&four));
        assert!(!contents(&[("oak_planks", 3)]).can_craft(&four));
        assert!(contents(&[("oak_planks", 4)]).can_craft(&four));
    }

    #[test]
    fn matching_reassigns_a_greedy_pick() {
        let either = ing(&["stick", "oak_log"]);
        let stick = ing(&["stick"]);
        let c = contents(&[("stick", 1), ("oak_log", 1)]);
        assert!(c.can_craft(&[either.clone(), stick.clone()]));
        assert!(!c.can_craft(&[either, stick.clone(), stick]));
    }

    #[test]
    fn empty_ingredient_never_matches() {
        assert!(!contents(&[("stone", 64)]).can_craft(&[ing(&[])]));
        assert!(contents(&[]).can_craft(&[]));
    }

    #[test]
    fn simple_stacks_skip_damaged_named_and_enchanted() {
        let mut c = StackedContents::default();
        c.account_simple(&SlotStack {
            damage: 3,
            ..stack("wooden_sword", 1)
        });
        c.account_simple(&SlotStack {
            extra: crate::session::StackExtra {
                enchantments: vec![("sharpness".into(), 1)],
                ..Default::default()
            }
            .into(),
            ..stack("iron_sword", 1)
        });
        let mut named = stack("stick", 1);
        named.extra.make_mut().custom_name = Some(Vec::new());
        c.account_simple(&named);
        c.account_simple(&stack("stick", 2));
        assert_eq!(c, contents(&[("stick", 2)]));
    }

    #[test]
    fn menu_reads_its_own_slots() {
        let mut slots = vec![SlotStack::default(); 46];
        slots[1] = stack("oak_planks", 1);
        slots[9] = stack("oak_planks", 3);
        slots[5] = stack("oak_planks", 64);
        let c = BookMenu::INVENTORY.stacked_contents(&slots);
        assert_eq!(c, contents(&[("oak_planks", 4)]));
    }

    fn display(shape: Shape, cells: usize) -> ResolvedDisplay {
        let empty: super::super::display::Stacks = std::sync::Arc::from(Vec::new());
        ResolvedDisplay {
            shape,
            ingredients: vec![empty.clone(); cells].into(),
            result: empty.clone(),
            station: empty,
        }
    }

    #[test]
    fn can_display_follows_the_grid() {
        let inv = BookMenu::INVENTORY;
        assert!(inv.can_display(&display(
            Shape::Shaped {
                width: 2,
                height: 2
            },
            4
        )));
        assert!(!inv.can_display(&display(
            Shape::Shaped {
                width: 3,
                height: 1
            },
            3
        )));
        assert!(inv.can_display(&display(Shape::Shapeless, 4)));
        assert!(!inv.can_display(&display(Shape::Shapeless, 5)));
        assert!(!inv.can_display(&display(Shape::Furnace, 2)));
        let furnace = BookMenu::furnace(FurnaceKind::Smoker);
        assert_eq!(furnace.book, BookType::Smoker);
        assert!(furnace.can_display(&display(Shape::Furnace, 2)));
        assert!(!furnace.can_display(&display(Shape::Shapeless, 1)));
        assert_eq!(BookMenu::for_container(0, ContainerKind::None), Some(inv));
        assert_eq!(BookMenu::for_container(3, ContainerKind::Hopper), None);
    }
}
