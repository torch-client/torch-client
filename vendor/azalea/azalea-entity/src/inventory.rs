use std::{cmp, collections::HashSet};

use azalea_chat::FormattedText;
use azalea_inventory::{
    ItemStack, ItemStackData, Menu, MenuLocation,
    bundle::{
        Mutable, NO_SELECTED_ITEM, can_item_be_in_bundle, is_bundle, item_weight, max_stack_size,
        set_contents, toggle_selected_item,
    },
    components::{BundleContents, EquipmentSlot},
    operations::{
        ClickOperation, CloneClick, PickupAllClick, PickupClick, QuickCraftKind, QuickCraftStatus,
        QuickCraftStatusKind, QuickMoveClick, ThrowClick,
    },
    PlayerMenuLocation,
};

use azalea_inventory::{CraftingMenuLocation, MerchantMenuLocation};

use crate::PlayerAbilities;

#[cfg_attr(feature = "bevy_ecs", derive(bevy_ecs::component::Component))]
#[derive(Clone, Debug)]
pub struct Inventory {
    pub inventory_menu: azalea_inventory::Menu,

    pub id: i32,
    pub container_menu: Option<azalea_inventory::Menu>,
    pub container_menu_title: Option<FormattedText>,
    pub carried: ItemStack,
    pub state_id: u32,
    pub container_state_id: u32,

    pub quick_craft_status: QuickCraftStatusKind,
    pub quick_craft_kind: QuickCraftKind,
    pub quick_craft_slots: HashSet<u16>,

    pub selected_hotbar_slot: u8,

    pub crafter_disabled_slots: u16,

    pub bundle_sound: Option<BundleSound>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BundleSound {
    Insert,
    InsertFail,
    RemoveOne,
}

impl Inventory {
    pub fn set_crafter_slot_disabled(&mut self, slot: usize, disabled: bool) {
        let crafter_open = matches!(
            self.container_menu,
            Some(azalea_inventory::Menu::Crafter3x3 { .. })
        );
        if slot >= 9 || !crafter_open {
            return;
        }
        if disabled {
            self.crafter_disabled_slots |= 1 << slot;
        } else {
            self.crafter_disabled_slots &= !(1 << slot);
        }
    }

    pub fn menu(&self) -> &azalea_inventory::Menu {
        match &self.container_menu {
            Some(menu) => menu,
            _ => &self.inventory_menu,
        }
    }

    pub fn menu_mut(&mut self) -> &mut azalea_inventory::Menu {
        match &mut self.container_menu {
            Some(menu) => menu,
            _ => &mut self.inventory_menu,
        }
    }

    pub fn sync_player_slots(&mut self, from_container: bool) {
        let Some(container) = &mut self.container_menu else {
            return;
        };
        let container_range = container.player_slots_range();
        let player_range = self.inventory_menu.player_slots_range();
        let count = (container_range.end() - container_range.start() + 1)
            .min(player_range.end() - player_range.start() + 1);
        for i in 0..count {
            let container_slot = container_range.start() + i;
            let player_slot = player_range.start() + i;
            if from_container {
                let Some(src) = container.slot(container_slot).cloned() else {
                    continue;
                };
                if let Some(dst) = self.inventory_menu.slot_mut(player_slot) {
                    *dst = src;
                }
            } else {
                let Some(src) = self.inventory_menu.slot(player_slot).cloned() else {
                    continue;
                };
                if let Some(dst) = container.slot_mut(container_slot) {
                    *dst = src;
                }
            }
        }
    }

    fn set_player_slot(&mut self, inventory_menu_index: usize, item: ItemStack) {
        let player_range = self.inventory_menu.player_slots_range();
        if let Some(dst) = self.inventory_menu.slot_mut(inventory_menu_index) {
            *dst = item.clone();
        }
        let Some(container) = &mut self.container_menu else {
            return;
        };
        let Some(offset) = inventory_menu_index.checked_sub(*player_range.start()) else {
            return;
        };
        let container_range = container.player_slots_range();
        if offset > *container_range.end() - *container_range.start() {
            return;
        }
        if let Some(dst) = container.slot_mut(*container_range.start() + offset) {
            *dst = item;
        }
    }

    pub fn menu_state_id(&self) -> u32 {
        if self.container_menu.is_some() {
            self.container_state_id
        } else {
            self.state_id
        }
    }

    fn on_take_craft_result(&mut self, slot: usize) {
        let Some(grid) = self.craft_grid_range_for_result(slot) else {
            return;
        };
        for i in grid {
            let Some(item) = self.menu_mut().slot_mut(i) else {
                continue;
            };
            if let ItemStack::Present(data) = item {
                data.count -= 1;
                if data.count <= 0 {
                    *item = ItemStack::Empty;
                }
            }
        }
    }

    fn craft_grid_range_for_result(&self, slot: usize) -> Option<std::ops::Range<usize>> {
        match self.menu().location_for_slot(slot)? {
            MenuLocation::Player(PlayerMenuLocation::CraftResult) => Some(1..5),
            MenuLocation::Crafting(CraftingMenuLocation::Result) => Some(1..10),
            _ => None,
        }
    }

    pub fn simulate_click(&mut self, operation: &ClickOperation, player_abilities: &PlayerAbilities) {
        self.bundle_sound = None;
        self.simulate_click_inner(operation, player_abilities);
        self.sync_player_slots(true);
    }

    fn simulate_click_inner(
        &mut self,
        operation: &ClickOperation,
        player_abilities: &PlayerAbilities,
    ) {
        if let ClickOperation::QuickCraft(quick_craft) = operation {
            let last_quick_craft_status = self.quick_craft_status.clone();
            self.quick_craft_status = QuickCraftStatusKind::from(quick_craft.status.clone());

            if self.carried.is_empty() {
                return self.reset_quick_craft();
            }
            if (last_quick_craft_status == QuickCraftStatusKind::Start
                || last_quick_craft_status == QuickCraftStatusKind::End
                || self.quick_craft_status != QuickCraftStatusKind::End)
                && (self.quick_craft_status != last_quick_craft_status)
            {
                return self.reset_quick_craft();
            }
            if self.quick_craft_status == QuickCraftStatusKind::Start {
                self.quick_craft_kind = quick_craft.kind.clone();
                let valid = match self.quick_craft_kind {
                    QuickCraftKind::Left | QuickCraftKind::Right => true,
                    QuickCraftKind::Middle => player_abilities.instant_break,
                };
                if valid {
                    self.quick_craft_status = QuickCraftStatusKind::Add;
                    self.quick_craft_slots.clear();
                } else {
                    self.reset_quick_craft();
                }
                return;
            }
            if let QuickCraftStatus::Add { slot } = quick_craft.status {
                let slot_item = self.menu().slot(slot as usize);
                if let Some(slot_item) = slot_item
                    && let ItemStack::Present(carried) = &self.carried
                {
                    if can_item_quick_replace(slot_item, &self.carried, true)
                        && self.menu().may_place(slot as usize, carried)
                        && (self.quick_craft_kind == QuickCraftKind::Middle
                            || carried.count as usize > self.quick_craft_slots.len())
                    {
                        self.quick_craft_slots.insert(slot);
                    }
                }
                return;
            }
            if self.quick_craft_status == QuickCraftStatusKind::End {
                if !self.quick_craft_slots.is_empty() {
                    if self.quick_craft_slots.len() == 1 {
                        let slot = *self.quick_craft_slots.iter().next().unwrap();
                        self.reset_quick_craft();
                        self.simulate_click_inner(
                            &match self.quick_craft_kind {
                                QuickCraftKind::Left => {
                                    PickupClick::Left { slot: Some(slot) }.into()
                                }
                            QuickCraftKind::Right => {
                                    PickupClick::Right { slot: Some(slot) }.into()
                                }
                                QuickCraftKind::Middle => {
                                    return;
                                }
                            },
                            player_abilities,
                        );
                        return;
                    }

                    let ItemStack::Present(mut carried) = self.carried.clone() else {
                        return self.reset_quick_craft();
                    };

                    let mut carried_count = carried.count;
                    let mut quick_craft_slots_iter = self.quick_craft_slots.iter();

                    loop {
                        let mut slot: &ItemStack;
                        let mut slot_index: u16;
                        let mut item_stack: &ItemStack;

                        loop {
                            let Some(&next_slot) = quick_craft_slots_iter.next() else {
                                carried.count = carried_count;
                                self.carried = carried.into();
                                return self.reset_quick_craft();
                            };

                            slot = self.menu().slot(next_slot as usize).unwrap();
                            slot_index = next_slot;
                            item_stack = &self.carried;

                            if can_item_quick_replace(slot, item_stack, true)
                                    && self.menu().may_place(next_slot as usize, &carried)
                                    && (
                                        self.quick_craft_kind == QuickCraftKind::Middle
                                        || item_stack.count()  >= self.quick_craft_slots.len() as i32
                                    )
                            {
                                break;
                            }
                        }

                        let slot_item_count = slot.count();

                        let mut new_carried = carried.clone();
                        get_quick_craft_slot_count(
                            &self.quick_craft_slots,
                            &self.quick_craft_kind,
                            &mut new_carried,
                            slot_item_count,
                        );
                        let limit = max_stack_size(&new_carried)
                            .min(self.menu().max_stack_size(slot_index as usize));
                        if new_carried.count > limit {
                            new_carried.count = limit;
                        }

                        carried_count -= new_carried.count - slot_item_count;
                        let menu = match &mut self.container_menu {
                            Some(menu) => menu,
                            _ => &mut self.inventory_menu,
                        };
                        *menu.slot_mut(slot_index as usize).unwrap() = new_carried.into();
                    }
                }
            } else {
                return self.reset_quick_craft();
            }
        }
        if self.quick_craft_status != QuickCraftStatusKind::Start {
            return self.reset_quick_craft();
        }

        match operation {
            ClickOperation::Pickup(PickupClick::Left { slot: None } | PickupClick::LeftOutside)
                if self.carried.is_present() =>
            {

                self.carried = ItemStack::Empty;
            }
            ClickOperation::Pickup(PickupClick::Right { slot: None } | PickupClick::RightOutside)
                if self.carried.is_present() =>
            {
                let _item = self.carried.split(1);
            }
            &ClickOperation::Pickup(
                ref pickup @ (PickupClick::Left { slot: Some(slot) }
                | PickupClick::Right { slot: Some(slot) }),
            ) => {
                let slot = slot as usize;
                if self.menu().slot(slot).is_none() {
                    return;
                }

                let is_left_click = matches!(pickup, PickupClick::Left { .. });
                if self.try_item_click_behavior_override(is_left_click, slot) {
                    return;
                }
                let Some(slot_item) = self.menu().slot(slot) else {
                    return;
                };

                match slot_item {
                    ItemStack::Empty => {
                        if self.carried.is_present() {
                            let place_count = if is_left_click {
                                self.carried.count()
                            } else {
                                1
                            };
                            self.carried =
                                self.safe_insert(slot, self.carried.clone(), place_count);
                        }
                    }
                    ItemStack::Present(_) => {
                        if !self.menu().may_pickup(slot) {
                            return;
                        }
                        if let ItemStack::Present(carried) = self.carried.clone() {
                            let slot_is_same_item_as_carried = slot_item
                                .as_present()
                                .is_some_and(|s| carried.is_same_item_and_components(s));

                            if self.menu().may_place(slot, &carried) {
                                if slot_is_same_item_as_carried {
                                    let place_count = if is_left_click { carried.count } else { 1 };
                                    self.carried =
                                        self.safe_insert(slot, self.carried.clone(), place_count);
                                } else if carried.count
                                    <= self
                                        .menu()
                                        .max_stack_size(slot)
                                        .min(max_stack_size(&carried))
                                {
                                    self.carried = slot_item.clone();
                                    let slot_item = self.menu_mut().slot_mut(slot).unwrap();
                                    *slot_item = carried.into();
                                }
                            } else if slot_is_same_item_as_carried
                                && let Some(removed) = self.try_remove(
                                    slot,
                                    slot_item.count(),
                                    max_stack_size(&carried) - carried.count,
                                )
                            {
                                self.carried.as_present_mut().unwrap().count += removed.count();
                                self.on_take_craft_result(slot);
                            }
                        } else {
                            let pickup_count = if is_left_click {
                                slot_item.count()
                            } else {
                                (slot_item.count() + 1) / 2
                            };
                            if let Some(new_slot_item) =
                                self.try_remove(slot, pickup_count, i32::MAX)
                            {
                                self.carried = new_slot_item;
                                self.on_take_craft_result(slot);
                            }
                        }
                    }
                }
            }
            &ClickOperation::QuickMove(
                QuickMoveClick::Left { slot } | QuickMoveClick::Right { slot },
            ) => {
                let slot = slot as usize;
                let merchant_result = matches!(
                    self.menu().location_for_slot(slot),
                    Some(MenuLocation::Merchant(MerchantMenuLocation::Result))
                );
                if !self.menu().may_pickup(slot) {
                    return;
                }
                if self.craft_grid_range_for_result(slot).is_none() && !merchant_result {
                    let disabled = self.crafter_disabled_slots;
                    loop {
                        let new_slot_item = self
                            .menu_mut()
                            .quick_move_stack_with_disabled(slot, disabled);
                        let slot_item = self.menu().slot(slot).unwrap();
                        if new_slot_item.is_empty() || slot_item.kind() != new_slot_item.kind() {
                            break;
                        }
                    }
                }
            }
            ClickOperation::Swap(s) => {
                let source_slot_index = s.source_slot as usize;
                let button = s.target_slot;

                let player_range = self.inventory_menu.player_slots_range();
                let target_slot_index = match button {
                    0..=8 => *player_range.end() - 8 + button as usize,
                    40 => *player_range.end() + 1,
                    _ => return,
                };

                let Some(source_slot) = self.menu().slot(source_slot_index).cloned() else {
                    return;
                };
                let Some(target_slot) = self.inventory_menu.slot(target_slot_index).cloned()
                else {
                    return;
                };
                if source_slot.is_empty() && target_slot.is_empty() {
                    return;
                }

                if target_slot.is_empty() {
                    if self.menu().may_pickup(source_slot_index) {
                        self.set_player_slot(target_slot_index, source_slot);
                        *self.menu_mut().slot_mut(source_slot_index).unwrap() = ItemStack::Empty;
                    }
                } else if source_slot.is_empty() {
                    let target_item = target_slot
                        .as_present()
                        .expect("target slot was already checked to not be empty");
                    if self.menu().may_place(source_slot_index, target_item) {
                        let source_max_stack_size = self.menu().max_stack_size(source_slot_index);

                        let mut target_slot = target_slot;
                        let new_source_slot =
                            target_slot.split(source_max_stack_size.try_into().unwrap());
                        self.set_player_slot(target_slot_index, target_slot);
                        *self.menu_mut().slot_mut(source_slot_index).unwrap() = new_source_slot;
                    }
                } else if self.menu().may_pickup(source_slot_index) {
                    let ItemStack::Present(target_item) = &target_slot else {
                        unreachable!("target slot is not empty but is not present");
                    };
                    if self.menu().may_place(source_slot_index, target_item) {
                        let source_max_stack = self.menu().max_stack_size(source_slot_index);
                        if target_slot.count() > source_max_stack {

                            let mut target_slot = target_slot;
                            let new_source_slot =
                                target_slot.split(source_max_stack.try_into().unwrap());
                            self.set_player_slot(target_slot_index, target_slot);
                            *self.menu_mut().slot_mut(source_slot_index).unwrap() = new_source_slot;
                        } else {
                            self.set_player_slot(target_slot_index, source_slot);
                            *self.menu_mut().slot_mut(source_slot_index).unwrap() = target_slot;
                        }
                    }
                }
            }
            ClickOperation::Clone(CloneClick { slot }) => {
                if !player_abilities.instant_break || self.carried.is_present() {
                    return;
                }
                let Some(source_slot) = self.menu().slot(*slot as usize) else {
                    return;
                };
                let ItemStack::Present(source_item) = source_slot else {
                    return;
                };
                let mut new_carried = source_item.clone();
                new_carried.count = max_stack_size(&new_carried);
                self.carried = ItemStack::Present(new_carried);
            }
            ClickOperation::Throw(c) => {
                if self.carried.is_present() {
                    return;
                }

                let (ThrowClick::Single { slot: slot_index }
                | ThrowClick::All { slot: slot_index }) = c;
                let slot_index = *slot_index as usize;
                let Some(held) = self.menu().slot(slot_index).map(ItemStack::count) else {
                    return;
                };
                let dropping_count = match c {
                    ThrowClick::Single { .. } => 1,
                    ThrowClick::All { .. } => held,
                };

                if self.try_remove(slot_index, dropping_count, i32::MAX).is_some() {
                    self.on_take_craft_result(slot_index);
                }
            }
            ClickOperation::PickupAll(PickupAllClick {
                slot: source_slot_index,
                reversed,
            }) => {
                let source_slot_index = *source_slot_index as usize;

                let source_slot = self.menu().slot(source_slot_index).unwrap();
                let target_slot = self.carried.clone();

                if target_slot.is_empty()
                    || (source_slot.is_present() && self.menu().may_pickup(source_slot_index))
                {
                    return;
                }

                let ItemStack::Present(target_slot_item) = &target_slot else {
                    unreachable!("target slot is not empty but is not present");
                };

                let max = max_stack_size(target_slot_item);
                for round in 0..2 {
                    let iterator: Box<dyn Iterator<Item = usize>> = if *reversed {
                        Box::new((0..self.menu().len()).rev())
                    } else {
                        Box::new(0..self.menu().len())
                    };

                    for i in iterator {
                        let carried = self.carried.count();
                        if carried >= max {
                            break;
                        }
                        let checking_slot = self.menu().slot(i).unwrap();
                        if let ItemStack::Present(checking_item) = checking_slot
                            && can_item_quick_replace(checking_slot, &target_slot, true)
                            && self.menu().may_pickup(i)
                            && self.menu().can_take_item_for_pick_all(i)
                            && (round != 0 || checking_item.count != max_stack_size(checking_item))
                        {
                            let count = checking_item.count;
                            let Some(taken_item) = self.try_remove(i, count, max - carried) else {
                                continue;
                            };

                            let ItemStack::Present(carried_item) = &mut self.carried else {
                                unreachable!("target slot is not empty but is not present");
                            };
                            carried_item.count += taken_item.count();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn reset_quick_craft(&mut self) {
        self.quick_craft_status = QuickCraftStatusKind::Start;
        self.quick_craft_slots.clear();
    }

    pub fn held_item(&self) -> &ItemStack {
        self.get_equipment(EquipmentSlot::Mainhand)
            .expect("The main hand item should always be present")
    }

    fn try_item_click_behavior_override(&mut self, primary: bool, slot: usize) -> bool {
        self.bundle_stacked_on_other(slot, primary) || self.bundle_other_stacked_on_me(slot, primary)
    }

    fn bundle_stacked_on_other(&mut self, slot: usize, primary: bool) -> bool {
        let contents = self.carried.as_present().filter(|item| is_bundle(item.kind));
        let Some(mut contents) = contents
            .and_then(|item| item.get_unless_removed::<BundleContents>())
            .map(|contents| Mutable::new(&contents))
        else {
            return false;
        };
        let other = self.menu().slot(slot).cloned().unwrap_or_default();
        if primary && let ItemStack::Present(other) = &other {
            let inserted = self.bundle_try_transfer(&mut contents, slot, other);
            self.bundle_sound =
                Some(if inserted > 0 { BundleSound::Insert } else { BundleSound::InsertFail });
        } else if !primary && other.is_empty() {
            if let Some(removed) = contents.remove_one() {
                let count = removed.count();
                let mut remainder = self.safe_insert(slot, removed, count);
                if remainder.count() > 0 {
                    contents.try_insert(&mut remainder);
                } else {
                    self.bundle_sound = Some(BundleSound::RemoveOne);
                }
            }
        } else {
            return false;
        }
        if let ItemStack::Present(bundle) = &mut self.carried {
            set_contents(bundle, contents.into_contents());
        }
        true
    }

    fn bundle_try_transfer(
        &mut self,
        contents: &mut Mutable,
        slot: usize,
        other: &ItemStackData,
    ) -> i32 {
        let Some(weight) = item_weight(other).filter(|_| can_item_be_in_bundle(other)) else {
            return 0;
        };
        let limit = contents.max_amount_to_add(weight);
        let Some(mut taken) = self.try_remove(slot, other.count, limit) else {
            return 0;
        };
        self.on_take_craft_result(slot);
        contents.try_insert(&mut taken)
    }

    fn bundle_other_stacked_on_me(&mut self, slot: usize, primary: bool) -> bool {
        let clicked = self.menu().slot(slot).and_then(ItemStack::as_present);
        let Some(clicked) = clicked.filter(|item| is_bundle(item.kind)) else {
            return false;
        };
        let carried_empty = self.carried.is_empty();
        let contents = clicked.get_unless_removed::<BundleContents>().map(|c| Mutable::new(&c));
        if primary && carried_empty {
            self.toggle_bundle_selection(slot);
            return false;
        }
        let Some(mut contents) = contents else {
            return false;
        };
        let allowed = self.menu().allow_modification(slot);
        if primary {
            self.bundle_sound = Some(if allowed && contents.try_insert(&mut self.carried) > 0 {
                BundleSound::Insert
            } else {
                BundleSound::InsertFail
            });
        } else if carried_empty {
            if allowed && let Some(removed) = contents.remove_one() {
                self.bundle_sound = Some(BundleSound::RemoveOne);
                self.carried = removed;
            }
        } else {
            self.toggle_bundle_selection(slot);
            return false;
        }
        if let Some(ItemStack::Present(bundle)) = self.menu_mut().slot_mut(slot) {
            set_contents(bundle, contents.into_contents());
        }
        true
    }

    fn toggle_bundle_selection(&mut self, slot: usize) {
        if let Some(stack) = self.menu_mut().slot_mut(slot) {
            toggle_selected_item(stack, NO_SELECTED_ITEM);
        }
    }

    fn safe_insert(&mut self, slot: usize, src_item: ItemStack, take_count: i32) -> ItemStack {
        let ItemStack::Present(mut src_item) = src_item else {
            return src_item;
        };
        let disabled = slot < 9 && self.crafter_disabled_slots >> slot & 1 == 1;
        if disabled || !self.menu().may_place(slot, &src_item) {
            return src_item.into();
        }
        let slot_max = self.menu().max_stack_size(slot);
        let Some(slot_item) = self.menu_mut().slot_mut(slot) else {
            return src_item.into();
        };

        let take_count = cmp::min(
            cmp::min(take_count, src_item.count),
            cmp::min(slot_max, max_stack_size(&src_item)) - slot_item.count(),
        );
        if take_count <= 0 {
            return src_item.into();
        }
        let take_count = take_count as u32;

        if slot_item.is_empty() {
            *slot_item = src_item.split(take_count).into();
        } else if let ItemStack::Present(slot_item) = slot_item
            && slot_item.is_same_item_and_components(&src_item)
        {
            src_item.count -= take_count as i32;
            slot_item.count += take_count as i32;
        }

        src_item.into()
    }

    fn try_remove(&mut self, slot: usize, count: i32, limit: i32) -> Option<ItemStack> {
        if !self.menu().may_pickup(slot) {
            return None;
        }
        let held = self.menu().slot(slot)?.count();
        if !self.menu().allow_modification(slot) && limit < held {
            return None;
        }

        let count = count.min(limit);
        if count <= 0 {
            return None;
        }
        let count = if self.menu().result_takes_whole_stack(slot) { held } else { count };
        let removed = self.menu_mut().slot_mut(slot)?.split(count as u32);
        removed.is_present().then_some(removed)
    }

    pub fn get_equipment(&self, equipment_slot: EquipmentSlot) -> Option<&ItemStack> {
        let player = self.inventory_menu.as_player();
        let item = match equipment_slot {
            EquipmentSlot::Mainhand => {
                let menu = self.menu();
                let main_hand_slot_idx =
                    *menu.hotbar_slots_range().start() + self.selected_hotbar_slot as usize;
                menu.slot(main_hand_slot_idx)?
            }
            EquipmentSlot::Offhand => &player.offhand,
            EquipmentSlot::Feet => &player.armor[3],
            EquipmentSlot::Legs => &player.armor[2],
            EquipmentSlot::Chest => &player.armor[1],
            EquipmentSlot::Head => &player.armor[0],
            EquipmentSlot::Body => {
                return None;
            }
            EquipmentSlot::Saddle => {
                return None;
            }
        };
        Some(item)
    }
}

fn can_item_quick_replace(
    target_slot: &ItemStack,
    item: &ItemStack,
    ignore_item_count: bool,
) -> bool {
    let slot_is_empty = target_slot.is_empty();
    if let (ItemStack::Present(target_slot), ItemStack::Present(item)) = (target_slot, item)
        && item.is_same_item_and_components(target_slot)
    {
        let count = target_slot.count as u16
            + if ignore_item_count {
                0
            } else {
                item.count as u16
            };
        return count <= max_stack_size(item) as u16;
    }
    slot_is_empty
}

fn get_quick_craft_slot_count(
    quick_craft_slots: &HashSet<u16>,
    quick_craft_kind: &QuickCraftKind,
    item: &mut ItemStackData,
    slot_item_count: i32,
) {
    item.count = match quick_craft_kind {
        QuickCraftKind::Left => item.count / quick_craft_slots.len() as i32,
        QuickCraftKind::Right => 1,
        QuickCraftKind::Middle => max_stack_size(item),
    };
    item.count += slot_item_count;
}

impl Default for Inventory {
    fn default() -> Self {
        Inventory {
            inventory_menu: Menu::Player(azalea_inventory::Player::default()),
            id: 0,
            container_menu: None,
            container_menu_title: None,
            carried: ItemStack::Empty,
            state_id: 0,
            container_state_id: 0,
            quick_craft_status: QuickCraftStatusKind::Start,
            quick_craft_kind: QuickCraftKind::Middle,
            quick_craft_slots: HashSet::new(),
            selected_hotbar_slot: 0,
            crafter_disabled_slots: 0,
            bundle_sound: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use azalea_inventory::SlotList;
    use azalea_registry::builtin::ItemKind;

    use super::*;

    #[test]
    fn test_simulate_shift_click_in_crafting_table() {
        let spruce_planks = ItemStack::new(ItemKind::SprucePlanks, 4);

        let mut inventory = Inventory {
            inventory_menu: Menu::Player(azalea_inventory::Player::default()),
            id: 1,
            container_menu: Some(Menu::Crafting {
                result: spruce_planks.clone(),
                grid: SlotList::default(),
                player: SlotList::default(),
            }),
            container_menu_title: None,
            carried: ItemStack::Empty,
            state_id: 0,
            container_state_id: 0,
            quick_craft_status: QuickCraftStatusKind::Start,
            quick_craft_kind: QuickCraftKind::Middle,
            quick_craft_slots: HashSet::new(),
            selected_hotbar_slot: 0,
            crafter_disabled_slots: 0,
            bundle_sound: None,
        };

        inventory.simulate_click(
            &ClickOperation::QuickMove(QuickMoveClick::Left { slot: 0 }),
            &PlayerAbilities::default(),
        );

        let new_slots = inventory.menu().slots();
        assert_eq!(&new_slots[0], &spruce_planks);
        assert!(new_slots[*Menu::CRAFTING_PLAYER_SLOTS.start()].is_empty());
    }

    #[test]
    fn a_right_click_takes_a_whole_craft_result() {
        let mut inv = inventory(ItemStack::Empty, ItemStack::Empty);
        *inv.inventory_menu.slot_mut(0).unwrap() = ItemStack::new(ItemKind::Stick, 4);
        click(&mut inv, 0, true);
        assert_eq!(inv.carried.count(), 4);
        assert!(inv.menu().slot(0).unwrap().is_empty());
    }

    #[test]
    fn merging_a_craft_result_into_the_cursor_consumes_the_grid() {
        let mut inv = inventory(ItemStack::Empty, ItemStack::new(ItemKind::Stick, 4));
        *inv.inventory_menu.slot_mut(0).unwrap() = ItemStack::new(ItemKind::Stick, 4);
        *inv.inventory_menu.slot_mut(1).unwrap() = ItemStack::new(ItemKind::OakPlanks, 2);
        click(&mut inv, 0, false);
        assert_eq!(inv.carried.count(), 8);
        assert_eq!(inv.menu().slot(1).unwrap().count(), 1);
    }

    #[test]
    fn a_drag_pays_out_no_more_than_a_slot_holds() {
        use azalea_inventory::operations::{QuickCraftClick, QuickCraftStatus};

        let helmets = ItemStack::new(ItemKind::DiamondHelmet, 1)
            .with_component(azalea_inventory::components::MaxStackSize { count: 4 });
        let mut inv = inventory(ItemStack::Empty, ItemStack::Empty);
        inv.carried = helmets;
        inv.carried.as_present_mut().unwrap().count = 4;
        for status in [
            QuickCraftStatus::Start,
            QuickCraftStatus::Add { slot: 5 },
            QuickCraftStatus::Add { slot: 9 },
            QuickCraftStatus::End,
        ] {
            let operation = ClickOperation::QuickCraft(QuickCraftClick {
                kind: QuickCraftKind::Left,
                status,
            });
            inv.simulate_click(&operation, &PlayerAbilities::default());
        }
        assert_eq!(inv.menu().slot(5).unwrap().count(), 1);
        assert_eq!(inv.menu().slot(9).unwrap().count(), 2);
        assert_eq!(inv.carried.count(), 1);
    }

    #[test]
    fn throwing_the_last_item_leaves_an_empty_slot() {
        let mut inv = inventory(ItemStack::new(ItemKind::Stone, 1), ItemStack::Empty);
        let operation = ClickOperation::Throw(ThrowClick::Single { slot: 9 });
        inv.simulate_click(&operation, &PlayerAbilities::default());
        assert_eq!(inv.menu().slot(9), Some(&ItemStack::Empty));
    }

    fn bundle(items: Vec<ItemStack>) -> ItemStack {
        ItemStack::new(ItemKind::Bundle, 1).with_component(BundleContents {
            items,
            selected_item: NO_SELECTED_ITEM,
        })
    }

    fn contents(stack: &ItemStack) -> Vec<(ItemKind, i32)> {
        let contents = stack.get_component::<BundleContents>().unwrap();
        contents.items.iter().map(|i| (i.kind(), i.count())).collect()
    }

    fn inventory(clicked: ItemStack, carried: ItemStack) -> Inventory {
        let mut inventory = Inventory { carried, ..Inventory::default() };
        *inventory.inventory_menu.slot_mut(9).unwrap() = clicked;
        inventory
    }

    fn click(inventory: &mut Inventory, slot: u16, right: bool) {
        let operation = if right {
            PickupClick::Right { slot: Some(slot) }
        } else {
            PickupClick::Left { slot: Some(slot) }
        };
        inventory.simulate_click(&operation.into(), &PlayerAbilities::default());
    }

    #[test]
    fn a_carried_stack_goes_into_a_bundle_in_a_slot() {
        let mut inv = inventory(bundle(Vec::new()), ItemStack::new(ItemKind::Stone, 10));
        click(&mut inv, 9, false);
        assert!(inv.carried.is_empty());
        assert_eq!(contents(inv.menu().slot(9).unwrap()), [(ItemKind::Stone, 10)]);
        assert_eq!(inv.bundle_sound, Some(BundleSound::Insert));
    }

    #[test]
    fn a_full_bundle_refuses_and_keeps_the_cursor() {
        let full = bundle(vec![ItemStack::new(ItemKind::DiamondSword, 1)]);
        let mut inv = inventory(full, ItemStack::new(ItemKind::Stone, 10));
        click(&mut inv, 9, false);
        assert_eq!(inv.carried.count(), 10);
        assert_eq!(inv.bundle_sound, Some(BundleSound::InsertFail));
    }

    #[test]
    fn a_carried_bundle_takes_part_of_a_stack_and_leaves_the_rest() {
        let held = bundle(vec![ItemStack::new(ItemKind::EnderPearl, 15)]);
        let mut inv = inventory(ItemStack::new(ItemKind::Stone, 10), held);
        click(&mut inv, 9, false);
        assert_eq!(inv.menu().slot(9).unwrap().count(), 6);
        assert_eq!(contents(&inv.carried)[0], (ItemKind::Stone, 4));
        assert_eq!(inv.bundle_sound, Some(BundleSound::Insert));
    }

    #[test]
    fn right_clicking_a_bundle_takes_its_selected_stack_onto_the_cursor() {
        let held = vec![ItemStack::new(ItemKind::Dirt, 1), ItemStack::new(ItemKind::Stone, 2)];
        let mut inv = inventory(bundle(held), ItemStack::Empty);
        toggle_selected_item(inv.inventory_menu.slot_mut(9).unwrap(), 1);
        click(&mut inv, 9, true);
        assert_eq!((inv.carried.kind(), inv.carried.count()), (ItemKind::Stone, 2));
        assert_eq!(contents(inv.menu().slot(9).unwrap()), [(ItemKind::Dirt, 1)]);
        assert_eq!(inv.bundle_sound, Some(BundleSound::RemoveOne));
    }

    #[test]
    fn a_carried_bundle_right_clicked_on_an_empty_slot_empties_into_it() {
        let held = bundle(vec![ItemStack::new(ItemKind::Stone, 3)]);
        let mut inv = inventory(ItemStack::Empty, held);
        click(&mut inv, 9, true);
        assert_eq!(inv.menu().slot(9).unwrap().count(), 3);
        assert_eq!(inv.carried, ItemStack::new(ItemKind::Bundle, 1));
        assert_eq!(inv.bundle_sound, Some(BundleSound::RemoveOne));
    }

    #[test]
    fn a_slot_that_refuses_the_removed_stack_returns_it_to_the_front() {
        let held = vec![ItemStack::new(ItemKind::Dirt, 1), ItemStack::new(ItemKind::Stone, 2)];
        let mut carried = bundle(held);
        toggle_selected_item(&mut carried, 1);
        let mut inv = inventory(ItemStack::Empty, carried);
        click(&mut inv, 5, true);
        assert!(inv.menu().slot(5).unwrap().is_empty());
        assert_eq!(contents(&inv.carried), [(ItemKind::Stone, 2), (ItemKind::Dirt, 1)]);
        assert_eq!(inv.bundle_sound, None);
    }

    #[test]
    fn a_result_slot_never_allows_modification() {
        let mut inv = inventory(ItemStack::Empty, ItemStack::new(ItemKind::Stone, 1));
        *inv.inventory_menu.slot_mut(0).unwrap() = bundle(Vec::new());
        click(&mut inv, 0, false);
        assert_eq!(inv.carried.count(), 1);
        assert_eq!(inv.bundle_sound, Some(BundleSound::InsertFail));

        let almost_full = bundle(vec![ItemStack::new(ItemKind::EnderPearl, 15)]);
        let mut inv = inventory(ItemStack::Empty, almost_full);
        *inv.inventory_menu.slot_mut(0).unwrap() = ItemStack::new(ItemKind::Stone, 10);
        click(&mut inv, 0, false);
        assert_eq!(inv.menu().slot(0).unwrap().count(), 10);
        assert_eq!(inv.bundle_sound, Some(BundleSound::InsertFail));
    }

    #[test]
    fn a_plain_left_click_picks_the_bundle_up_and_clears_its_selection() {
        let mut held = bundle(vec![ItemStack::new(ItemKind::Stone, 1)]);
        toggle_selected_item(&mut held, 0);
        let mut inv = inventory(held, ItemStack::Empty);
        click(&mut inv, 9, false);
        assert!(inv.menu().slot(9).unwrap().is_empty());
        let selected = inv.carried.get_component::<BundleContents>().unwrap().selected_item;
        assert_eq!(selected, NO_SELECTED_ITEM);
    }

    #[test]
    fn right_clicking_a_bundle_with_a_stack_in_hand_is_an_ordinary_click() {
        let mut inv = inventory(bundle(Vec::new()), ItemStack::new(ItemKind::Stone, 1));
        click(&mut inv, 9, true);
        assert_eq!(inv.menu().slot(9).unwrap().kind(), ItemKind::Stone);
        assert_eq!(inv.carried.kind(), ItemKind::Bundle);
        assert_eq!(inv.bundle_sound, None);
    }

    #[test]
    fn a_right_click_pickup_leaves_the_other_half_in_the_slot() {
        let mut inv = inventory(ItemStack::new(ItemKind::Stone, 9), ItemStack::Empty);
        click(&mut inv, 9, true);
        assert_eq!(inv.carried.count(), 5);
        assert_eq!(inv.menu().slot(9).unwrap().count(), 4);
    }
}
