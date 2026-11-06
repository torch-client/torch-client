use azalea_registry::{builtin::ItemKind, tags::items as tags};

use crate::{
    ItemStack, ItemStackData,
    components::{
        Bees, BundleContents, DataComponentTrait, Damage, MaxDamage, MaxStackSize, Unbreakable,
    },
    default_components::get_default_component,
};

pub const NO_SELECTED_ITEM: i32 = -1;
const BUNDLE_IN_BUNDLE_WEIGHT: Fraction = Fraction::new(1, 16);
const BEEHIVE_WEIGHT: Fraction = Fraction::ONE;
pub const MAX_SHOWN_GRID_ITEMS: usize = 12;
const OVERFLOWING_MAX_SHOWN_GRID_ITEMS: usize = 11;
pub const MAX_SHOWN_GRID_ITEMS_X: usize = 4;
const FULL_BAR_COLOR: u32 = 0xFF5454;
const BAR_COLOR: u32 = 0x7087FF;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fraction {
    pub numerator: i32,
    pub denominator: i32,
}

impl Fraction {
    pub const ZERO: Fraction = Fraction::new(0, 1);
    pub const ONE: Fraction = Fraction::new(1, 1);

    pub const fn new(numerator: i32, denominator: i32) -> Fraction {
        Fraction { numerator, denominator }
    }

    fn reduced(numerator: i128, denominator: i128) -> Option<Fraction> {
        fn gcd(a: i128, b: i128) -> i128 {
            if b == 0 { a } else { gcd(b, a % b) }
        }
        let divisor = gcd(numerator.abs(), denominator).max(1);
        Some(Fraction {
            numerator: i32::try_from(numerator / divisor).ok()?,
            denominator: i32::try_from(denominator / divisor).ok()?,
        })
    }

    fn parts(self) -> (i128, i128) {
        (i128::from(self.numerator), i128::from(self.denominator))
    }

    pub fn add(self, other: Fraction) -> Option<Fraction> {
        let ((a, b), (c, d)) = (self.parts(), other.parts());
        Self::reduced(a * d + c * b, b * d)
    }

    pub fn subtract(self, other: Fraction) -> Option<Fraction> {
        let ((a, b), (c, d)) = (self.parts(), other.parts());
        Self::reduced(a * d - c * b, b * d)
    }

    pub fn times(self, count: i32) -> Option<Fraction> {
        let (a, b) = self.parts();
        Self::reduced(a * i128::from(count), b)
    }

    fn quotient(self, other: Fraction) -> i32 {
        let ((a, b), (c, d)) = (self.parts(), other.parts());
        (a * d / (b * c)) as i32
    }

    pub fn mul_and_truncate(self, factor: i32) -> i32 {
        let (a, b) = self.parts();
        (a * i128::from(factor) / b) as i32
    }

    pub fn is_full(self) -> bool {
        self.numerator >= self.denominator
    }

    pub fn is_zero(self) -> bool {
        self.numerator == 0
    }
}

pub fn is_bundle(kind: ItemKind) -> bool {
    tags::BUNDLES.contains(&kind)
}

pub fn max_stack_size(item: &ItemStackData) -> i32 {
    item.get_unless_removed::<MaxStackSize>().map_or(1, |max| max.count)
}

pub fn is_damageable(item: &ItemStackData) -> bool {
    item.get_unless_removed::<MaxDamage>().is_some()
        && item.get_unless_removed::<Unbreakable>().is_none()
        && item.get_unless_removed::<Damage>().is_some()
}

fn is_stackable(item: &ItemStackData) -> bool {
    let damaged = is_damageable(item)
        && item.get_unless_removed::<Damage>().is_some_and(|damage| damage.amount > 0);
    max_stack_size(item) > 1 && !damaged
}

pub fn can_item_be_in_bundle(item: &ItemStackData) -> bool {
    !item.is_empty() && !tags::SHULKER_BOXES.contains(&item.kind)
}

pub fn item_weight(item: &ItemStackData) -> Option<Fraction> {
    if let Some(nested) = item.get_unless_removed::<BundleContents>() {
        return contents_weight(&nested)?.add(BUNDLE_IN_BUNDLE_WEIGHT);
    }
    if item.get_unless_removed::<Bees>().is_some_and(|bees| !bees.occupants.is_empty()) {
        return Some(BEEHIVE_WEIGHT);
    }
    Some(Fraction::new(1, max_stack_size(item).max(1)))
}

pub fn contents_weight(contents: &BundleContents) -> Option<Fraction> {
    contents.items.iter().filter_map(ItemStack::as_present).try_fold(Fraction::ZERO, |sum, item| {
        sum.add(item_weight(item)?.times(item.count)?)
    })
}

pub fn number_of_items_to_show(size: usize) -> usize {
    let available = if size > MAX_SHOWN_GRID_ITEMS {
        OVERFLOWING_MAX_SHOWN_GRID_ITEMS
    } else {
        MAX_SHOWN_GRID_ITEMS
    };
    let on_last_row = size % MAX_SHOWN_GRID_ITEMS_X;
    let gap = if on_last_row == 0 { 0 } else { MAX_SHOWN_GRID_ITEMS_X - on_last_row };
    size.min(available - gap)
}

pub fn bar(weight: Fraction) -> Option<(u8, u32)> {
    (weight.numerator > 0).then(|| {
        let width = (1 + weight.mul_and_truncate(12)).min(13) as u8;
        (width, if weight.is_full() { FULL_BAR_COLOR } else { BAR_COLOR })
    })
}

pub fn weight_safe(weight: Option<Fraction>) -> Fraction {
    weight.unwrap_or(Fraction::ONE)
}

#[derive(Clone, Debug)]
pub struct Mutable {
    items: Vec<ItemStack>,
    weight: Fraction,
    selected: i32,
}

impl Mutable {
    pub fn new(contents: &BundleContents) -> Mutable {
        match contents_weight(contents) {
            Some(weight) => Mutable {
                items: contents.items.clone(),
                weight,
                selected: contents.selected_item,
            },
            None => Mutable { items: Vec::new(), weight: Fraction::ZERO, selected: NO_SELECTED_ITEM },
        }
    }

    fn find_stack_index(&self, item: &ItemStackData) -> Option<usize> {
        if !is_stackable(item) {
            return None;
        }
        self.items
            .iter()
            .position(|held| held.as_present().is_some_and(|h| h.is_same_item_and_components(item)))
    }

    pub fn max_amount_to_add(&self, item_weight: Fraction) -> i32 {
        Fraction::ONE.subtract(self.weight).map_or(0, |left| left.quotient(item_weight).max(0))
    }

    pub fn try_insert(&mut self, stack: &mut ItemStack) -> i32 {
        let Some(item) = stack.as_present().filter(|item| can_item_be_in_bundle(item)) else {
            return 0;
        };
        let Some(weight) = item_weight(item) else {
            return 0;
        };
        let amount = item.count.min(self.max_amount_to_add(weight));
        if amount == 0 {
            return 0;
        }
        let Some(total) = weight.times(amount).and_then(|added| self.weight.add(added)) else {
            return 0;
        };
        self.weight = total;
        let index = self.find_stack_index(item);
        let mut moved = stack.split(amount as u32);
        if let Some(index) = index {
            let mut merged = self.items.remove(index);
            if let Some(merged) = merged.as_present_mut() {
                merged.count += amount;
            }
            moved = merged;
        }
        self.items.insert(0, moved);
        amount
    }

    pub fn toggle_selected_item(&mut self, selected: i32) {
        self.selected = toggled(self.selected, selected, self.items.len());
    }

    pub fn remove_one(&mut self) -> Option<ItemStack> {
        if self.items.is_empty() {
            return None;
        }
        let index = if outside_bounds(self.selected, self.items.len()) { 0 } else { self.selected };
        let stack = self.items.remove(index as usize);
        let removed = stack
            .as_present()
            .and_then(item_weight)
            .and_then(|weight| weight.times(stack.count()))
            .and_then(|weight| self.weight.subtract(weight));
        self.weight = removed.unwrap_or(self.weight);
        self.toggle_selected_item(NO_SELECTED_ITEM);
        Some(stack)
    }

    pub fn into_contents(self) -> BundleContents {
        BundleContents { items: self.items, selected_item: self.selected }
    }
}

fn outside_bounds(selected: i32, size: usize) -> bool {
    selected < 0 || selected as usize >= size
}

pub fn toggled(current: i32, selected: i32, size: usize) -> i32 {
    if current != selected && !outside_bounds(selected, size) {
        selected
    } else {
        NO_SELECTED_ITEM
    }
}

pub fn set_contents(item: &mut ItemStackData, contents: BundleContents) {
    let default = get_default_component::<BundleContents>(item.kind);
    if default.as_ref() == Some(&contents) {
        item.component_patch.clear_kind(BundleContents::KIND);
        return;
    }
    unsafe {
        item.component_patch.unchecked_insert_component(BundleContents::KIND, Some(contents.into()));
    }
}

pub fn toggle_selected_item(stack: &mut ItemStack, selected: i32) {
    let ItemStack::Present(item) = stack else { return };
    let Some(mut contents) = item.get_unless_removed::<BundleContents>().map(|c| Mutable::new(&c))
    else {
        return;
    };
    contents.toggle_selected_item(selected);
    set_contents(item, contents.into_contents());
}

#[cfg(test)]
mod tests {
    use azalea_registry::builtin::EntityKind;

    use super::*;
    use crate::components::BeehiveOccupant;

    fn stack(kind: ItemKind, count: i32) -> ItemStack {
        ItemStack::new(kind, count)
    }

    fn bundle(items: Vec<ItemStack>) -> ItemStack {
        stack(ItemKind::Bundle, 1).with_component(BundleContents { items, selected_item: -1 })
    }

    fn contents(items: Vec<ItemStack>) -> BundleContents {
        BundleContents { items, selected_item: -1 }
    }

    fn weight_of(stack: &ItemStack) -> Option<Fraction> {
        item_weight(stack.as_present().unwrap())
    }

    #[test]
    fn weights_are_exact_fractions() {
        assert_eq!(weight_of(&stack(ItemKind::Stone, 1)), Some(Fraction::new(1, 64)));
        assert_eq!(weight_of(&stack(ItemKind::EnderPearl, 1)), Some(Fraction::new(1, 16)));
        assert_eq!(weight_of(&stack(ItemKind::DiamondSword, 1)), Some(Fraction::ONE));
        assert_eq!(weight_of(&bundle(Vec::new())), Some(Fraction::new(1, 16)));
        let nested = bundle(vec![stack(ItemKind::Stone, 32)]);
        assert_eq!(weight_of(&nested), Some(Fraction::new(9, 16)));
        let mixed = contents(vec![stack(ItemKind::Stone, 3), stack(ItemKind::EnderPearl, 5)]);
        assert_eq!(contents_weight(&mixed), Some(Fraction::new(23, 64)));
    }

    #[test]
    fn a_hive_with_bees_fills_the_bundle() {
        assert_eq!(weight_of(&stack(ItemKind::Beehive, 1)), Some(Fraction::new(1, 64)));
        let bee = BeehiveOccupant {
            kind: EntityKind::Bee,
            entity_data: Default::default(),
            ticks_in_hive: 0,
            min_ticks_in_hive: 0,
        };
        let full = stack(ItemKind::Beehive, 1).with_component(Bees { occupants: vec![bee] });
        assert_eq!(weight_of(&full), Some(Fraction::ONE));
    }

    #[test]
    fn insert_stops_at_a_full_bundle_and_leaves_the_rest() {
        let mut bundle = Mutable::new(&contents(vec![stack(ItemKind::EnderPearl, 15)]));
        let mut stone = stack(ItemKind::Stone, 10);
        assert_eq!(bundle.try_insert(&mut stone), 4);
        assert_eq!(stone.count(), 6);
        assert_eq!(bundle.try_insert(&mut stone), 0);
        assert_eq!(stone.count(), 6);
        assert_eq!(bundle.into_contents().items[0].kind(), ItemKind::Stone);
    }

    #[test]
    fn insert_merges_to_the_front() {
        let held = vec![stack(ItemKind::Dirt, 1), stack(ItemKind::Stone, 2)];
        let mut bundle = Mutable::new(&contents(held));
        let mut stone = stack(ItemKind::Stone, 3);
        assert_eq!(bundle.try_insert(&mut stone), 3);
        assert!(stone.is_empty());
        let items = bundle.into_contents().items;
        assert_eq!((items[0].kind(), items[0].count()), (ItemKind::Stone, 5));
        assert_eq!(items[1].kind(), ItemKind::Dirt);
    }

    #[test]
    fn a_shulker_box_never_fits() {
        let mut bundle = Mutable::new(&contents(Vec::new()));
        let mut shulker = stack(ItemKind::ShulkerBox, 1);
        assert_eq!(bundle.try_insert(&mut shulker), 0);
        assert_eq!(shulker.count(), 1);
    }

    #[test]
    fn remove_takes_the_selected_stack_and_clears_the_selection() {
        let held = vec![stack(ItemKind::Dirt, 1), stack(ItemKind::Stone, 2)];
        let mut bundle = Mutable::new(&contents(held));
        bundle.toggle_selected_item(1);
        assert_eq!(bundle.remove_one().unwrap().kind(), ItemKind::Stone);
        assert_eq!(bundle.selected, NO_SELECTED_ITEM);
        assert_eq!(bundle.weight, Fraction::new(1, 64));
        assert_eq!(bundle.remove_one().unwrap().kind(), ItemKind::Dirt);
        assert_eq!(bundle.weight, Fraction::ZERO);
        assert!(bundle.remove_one().is_none());
    }

    #[test]
    fn selecting_the_same_index_twice_clears_it() {
        assert_eq!(toggled(-1, 2, 3), 2);
        assert_eq!(toggled(2, 2, 3), -1);
        assert_eq!(toggled(1, 3, 3), -1);
        assert_eq!(toggled(1, -1, 3), -1);
    }

    #[test]
    fn the_grid_keeps_whole_rows() {
        let shown: Vec<_> = [0, 1, 4, 5, 12, 13, 14, 16].map(number_of_items_to_show).into();
        assert_eq!(shown, [0, 1, 4, 5, 12, 8, 9, 11]);
    }

    #[test]
    fn the_bar_follows_the_weight() {
        assert_eq!(bar(Fraction::ZERO), None);
        assert_eq!(bar(Fraction::new(1, 64)), Some((1, BAR_COLOR)));
        assert_eq!(bar(Fraction::new(1, 2)), Some((7, BAR_COLOR)));
        assert_eq!(bar(Fraction::ONE), Some((13, FULL_BAR_COLOR)));
    }

    #[test]
    fn an_emptied_bundle_drops_out_of_the_patch() {
        let mut held = bundle(vec![stack(ItemKind::Stone, 1)]);
        let item = held.as_present_mut().unwrap();
        let mut contents = Mutable::new(&item.get_unless_removed::<BundleContents>().unwrap());
        contents.remove_one();
        set_contents(item, contents.into_contents());
        assert_eq!(held, stack(ItemKind::Bundle, 1));
    }

    #[test]
    fn selection_is_not_part_of_equality() {
        let mut selected = bundle(vec![stack(ItemKind::Stone, 1)]);
        let plain = selected.clone();
        toggle_selected_item(&mut selected, 0);
        assert_eq!(selected, plain);
        assert_eq!(selected.get_component::<BundleContents>().unwrap().selected_item, 0);
    }
}
