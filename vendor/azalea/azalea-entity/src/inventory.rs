use std::{cmp, collections::HashSet};

use azalea_chat::FormattedText;
use azalea_inventory::{
    ItemStack, ItemStackData, Menu, MenuLocation,
    components::EquipmentSlot,
    item::MaxStackSizeExt,
    operations::{
        ClickOperation, CloneClick, PickupAllClick, PickupClick, QuickCraftKind, QuickCraftStatus,
        QuickCraftStatusKind, QuickMoveClick, ThrowClick,
    },
    PlayerMenuLocation,
};

use azalea_inventory::CraftingMenuLocation;

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
}

impl Inventory {
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
                        && (self.quick_craft_kind == QuickCraftKind::Right
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
                                self.carried = ItemStack::Present(carried);
                                return self.reset_quick_craft();
                            };

                            slot = self.menu().slot(next_slot as usize).unwrap();
                            slot_index = next_slot;
                            item_stack = &self.carried;

                            if can_item_quick_replace(slot, item_stack, true)
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
                        let max_stack_size = new_carried.kind.max_stack_size();
                        if new_carried.count > max_stack_size {
                            new_carried.count = max_stack_size;
                        }

                        carried_count -= new_carried.count - slot_item_count;
                        let menu = match &mut self.container_menu {
                            Some(menu) => menu,
                            _ => &mut self.inventory_menu,
                        };
                        *menu.slot_mut(slot_index as usize).unwrap() =
                            ItemStack::Present(new_carried);
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
                let Some(slot_item) = self.menu().slot(slot) else {
                    return;
                };

                if self.try_item_click_behavior_override(operation, slot) {
                    return;
                }

                let is_left_click = matches!(pickup, PickupClick::Left { .. });

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
                                        .min(carried.kind.max_stack_size())
                                {
                                    self.carried = slot_item.clone();
                                    let slot_item = self.menu_mut().slot_mut(slot).unwrap();
                                    *slot_item = carried.into();
                                }
                            } else if slot_is_same_item_as_carried
                                && let Some(removed) = self.try_remove(
                                    slot,
                                    slot_item.count(),
                                    carried.kind.max_stack_size() - carried.count,
                                )
                            {
                                self.carried.as_present_mut().unwrap().count += removed.count();
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
                if self.craft_grid_range_for_result(slot).is_none() {
                    loop {
                        let new_slot_item = self.menu_mut().quick_move_stack(slot);
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
                new_carried.count = new_carried.kind.max_stack_size();
                self.carried = ItemStack::Present(new_carried);
            }
            ClickOperation::Throw(c) => {
                if self.carried.is_present() {
                    return;
                }

                let (ThrowClick::Single { slot: slot_index }
                | ThrowClick::All { slot: slot_index }) = c;
                let slot_index = *slot_index as usize;

                let Some(slot) = self.menu_mut().slot_mut(slot_index) else {
                    return;
                };
                let ItemStack::Present(slot_item) = slot else {
                    return;
                };

                let dropping_count = match c {
                    ThrowClick::Single { .. } => 1,
                    ThrowClick::All { .. } => slot_item.count,
                };

                let _dropping = slot_item.split(dropping_count as u32);
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

                for round in 0..2 {
                    let iterator: Box<dyn Iterator<Item = usize>> = if *reversed {
                        Box::new((0..self.menu().len()).rev())
                    } else {
                        Box::new(0..self.menu().len())
                    };

                    for i in iterator {
                        if target_slot_item.count < target_slot_item.kind.max_stack_size() {
                            let checking_slot = self.menu().slot(i).unwrap();
                            if let ItemStack::Present(checking_item) = checking_slot
                                && can_item_quick_replace(checking_slot, &target_slot, true)
                                && self.menu().may_pickup(i)
                                && (round != 0
                                    || checking_item.count != checking_item.kind.max_stack_size())
                            {
                                let checking_slot = self.menu_mut().slot_mut(i).unwrap();

                                let taken_item = checking_slot.split(checking_slot.count() as u32);

                                let target_slot = &mut self.carried;
                                let ItemStack::Present(target_slot_item) = target_slot else {
                                    unreachable!("target slot is not empty but is not present");
                                };
                                target_slot_item.count += taken_item.count();
                            }
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

    fn try_item_click_behavior_override(
        &self,
        _operation: &ClickOperation,
        _slot_item_index: usize,
    ) -> bool {
        false
    }

    fn safe_insert(&mut self, slot: usize, src_item: ItemStack, take_count: i32) -> ItemStack {
        let Some(slot_item) = self.menu_mut().slot_mut(slot) else {
            return src_item;
        };
        let ItemStack::Present(mut src_item) = src_item else {
            return src_item;
        };

        let take_count = cmp::min(
            cmp::min(take_count, src_item.count),
            src_item.kind.max_stack_size() - slot_item.count(),
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
        let mut slot_item = self.menu().slot(slot)?.clone();
        if !self.menu().allow_modification(slot) && limit < slot_item.count() {
            return None;
        }

        let count = count.min(limit);
        if count <= 0 {
            return None;
        }
        let removed = slot_item.split(count as u32);

        if removed.is_present() && slot_item.is_empty() {
            *self.menu_mut().slot_mut(slot).unwrap() = ItemStack::Empty;
        }

        Some(removed)
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
        return count <= item.kind.max_stack_size() as u16;
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
        QuickCraftKind::Middle => item.kind.max_stack_size(),
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
        };

        inventory.simulate_click(
            &ClickOperation::QuickMove(QuickMoveClick::Left { slot: 0 }),
            &PlayerAbilities::default(),
        );

        let new_slots = inventory.menu().slots();
        assert_eq!(&new_slots[0], &ItemStack::Empty);
        assert_eq!(
            &new_slots[*Menu::CRAFTING_PLAYER_SLOTS.start()],
            &spruce_planks
        );
    }
}
