use std::ops::RangeInclusive;

use azalea_buf::AzBuf;
use azalea_registry::builtin::ItemKind;

use crate::{
    AnvilMenuLocation, BeaconMenuLocation, BlastFurnaceMenuLocation, BrewingStandMenuLocation,
    CartographyTableMenuLocation, Crafter3x3MenuLocation, CraftingMenuLocation,
    EnchantmentMenuLocation, FurnaceMenuLocation, Generic3x3MenuLocation, Generic9x1MenuLocation,
    Generic9x2MenuLocation, Generic9x3MenuLocation, Generic9x4MenuLocation, Generic9x5MenuLocation,
    Generic9x6MenuLocation, GrindstoneMenuLocation, HopperMenuLocation, ItemStack, ItemStackData,
    LecternMenuLocation, LoomMenuLocation, Menu, MenuLocation, MerchantMenuLocation, Player,
    PlayerMenuLocation, ShulkerBoxMenuLocation, SmithingMenuLocation, SmokerMenuLocation,
    StonecutterMenuLocation, item::MaxStackSizeExt,
};

#[derive(Clone, Debug)]
pub enum ClickOperation {
    Pickup(PickupClick),
    QuickMove(QuickMoveClick),
    Swap(SwapClick),
    Clone(CloneClick),
    Throw(ThrowClick),
    QuickCraft(QuickCraftClick),
    PickupAll(PickupAllClick),
}

#[derive(Clone, Debug)]
pub enum PickupClick {
    Left { slot: Option<u16> },
    Right { slot: Option<u16> },
    LeftOutside,
    RightOutside,
}
impl From<PickupClick> for ClickOperation {
    fn from(click: PickupClick) -> Self {
        ClickOperation::Pickup(click)
    }
}

#[derive(Clone, Debug)]
pub enum QuickMoveClick {
    Left { slot: u16 },
    Right { slot: u16 },
}
impl From<QuickMoveClick> for ClickOperation {
    fn from(click: QuickMoveClick) -> Self {
        ClickOperation::QuickMove(click)
    }
}

#[derive(Clone, Debug)]
pub struct SwapClick {
    pub source_slot: u16,
    pub target_slot: u8,
}

impl From<SwapClick> for ClickOperation {
    fn from(click: SwapClick) -> Self {
        ClickOperation::Swap(click)
    }
}
#[derive(Clone, Debug)]
pub struct CloneClick {
    pub slot: u16,
}
impl From<CloneClick> for ClickOperation {
    fn from(click: CloneClick) -> Self {
        ClickOperation::Clone(click)
    }
}
#[derive(Clone, Debug)]
pub enum ThrowClick {
    Single { slot: u16 },
    All { slot: u16 },
}
impl From<ThrowClick> for ClickOperation {
    fn from(click: ThrowClick) -> Self {
        ClickOperation::Throw(click)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuickCraftClick {
    pub kind: QuickCraftKind,
    pub status: QuickCraftStatus,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QuickCraftKind {
    Left,
    Right,
    Middle,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QuickCraftStatusKind {
    Start,
    Add,
    End,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QuickCraftStatus {
    Start,
    Add { slot: u16 },
    End,
}
impl From<QuickCraftStatus> for QuickCraftStatusKind {
    fn from(status: QuickCraftStatus) -> Self {
        match status {
            QuickCraftStatus::Start => QuickCraftStatusKind::Start,
            QuickCraftStatus::Add { .. } => QuickCraftStatusKind::Add,
            QuickCraftStatus::End => QuickCraftStatusKind::End,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PickupAllClick {
    pub slot: u16,
    pub reversed: bool,
}
impl From<PickupAllClick> for ClickOperation {
    fn from(click: PickupAllClick) -> Self {
        ClickOperation::PickupAll(click)
    }
}

impl ClickOperation {
    pub fn slot_num(&self) -> Option<u16> {
        match self {
            ClickOperation::Pickup(pickup) => match pickup {
                PickupClick::Left { slot } => *slot,
                PickupClick::Right { slot } => *slot,
                PickupClick::LeftOutside => None,
                PickupClick::RightOutside => None,
            },
            ClickOperation::QuickMove(quick_move) => match quick_move {
                QuickMoveClick::Left { slot } => Some(*slot),
                QuickMoveClick::Right { slot } => Some(*slot),
            },
            ClickOperation::Swap(swap) => Some(swap.source_slot),
            ClickOperation::Clone(clone) => Some(clone.slot),
            ClickOperation::Throw(throw) => match throw {
                ThrowClick::Single { slot } => Some(*slot),
                ThrowClick::All { slot } => Some(*slot),
            },
            ClickOperation::QuickCraft(quick_craft) => match quick_craft.status {
                QuickCraftStatus::Start => None,
                QuickCraftStatus::Add { slot } => Some(slot),
                QuickCraftStatus::End => None,
            },
            ClickOperation::PickupAll(pickup_all) => Some(pickup_all.slot),
        }
    }

    pub fn button_num(&self) -> u8 {
        match self {
            ClickOperation::Pickup(pickup) => match pickup {
                PickupClick::Left { .. } => 0,
                PickupClick::Right { .. } => 1,
                PickupClick::LeftOutside => 0,
                PickupClick::RightOutside => 1,
            },
            ClickOperation::QuickMove(quick_move) => match quick_move {
                QuickMoveClick::Left { .. } => 0,
                QuickMoveClick::Right { .. } => 1,
            },
            ClickOperation::Swap(swap) => swap.target_slot,
            ClickOperation::Clone(_) => 2,
            ClickOperation::Throw(throw) => match throw {
                ThrowClick::Single { .. } => 0,
                ThrowClick::All { .. } => 1,
            },
            ClickOperation::QuickCraft(quick_craft) => match quick_craft {
                QuickCraftClick {
                    kind: QuickCraftKind::Left,
                    status: QuickCraftStatus::Start,
                } => 0,
                QuickCraftClick {
                    kind: QuickCraftKind::Right,
                    status: QuickCraftStatus::Start,
                } => 4,
                QuickCraftClick {
                    kind: QuickCraftKind::Middle,
                    status: QuickCraftStatus::Start,
                } => 8,
                QuickCraftClick {
                    kind: QuickCraftKind::Left,
                    status: QuickCraftStatus::Add { .. },
                } => 1,
                QuickCraftClick {
                    kind: QuickCraftKind::Right,
                    status: QuickCraftStatus::Add { .. },
                } => 5,
                QuickCraftClick {
                    kind: QuickCraftKind::Middle,
                    status: QuickCraftStatus::Add { .. },
                } => 9,
                QuickCraftClick {
                    kind: QuickCraftKind::Left,
                    status: QuickCraftStatus::End,
                } => 2,
                QuickCraftClick {
                    kind: QuickCraftKind::Right,
                    status: QuickCraftStatus::End,
                } => 6,
                QuickCraftClick {
                    kind: QuickCraftKind::Middle,
                    status: QuickCraftStatus::End,
                } => 10,
            },
            ClickOperation::PickupAll(_) => 0,
        }
    }

    pub fn click_type(&self) -> ClickType {
        match self {
            ClickOperation::Pickup(_) => ClickType::Pickup,
            ClickOperation::QuickMove(_) => ClickType::QuickMove,
            ClickOperation::Swap(_) => ClickType::Swap,
            ClickOperation::Clone(_) => ClickType::Clone,
            ClickOperation::Throw(_) => ClickType::Throw,
            ClickOperation::QuickCraft(_) => ClickType::QuickCraft,
            ClickOperation::PickupAll(_) => ClickType::PickupAll,
        }
    }
}

#[derive(AzBuf, Clone, Copy, Debug, PartialEq)]
pub enum ClickType {
    Pickup = 0,
    QuickMove = 1,
    Swap = 2,
    Clone = 3,
    Throw = 4,
    QuickCraft = 5,
    PickupAll = 6,
}

fn armor_slot_for(item: ItemKind) -> Option<usize> {
    let id = item.to_str();
    Some(match id {
        _ if id.ends_with("_helmet") || id == "carved_pumpkin" || id.ends_with("_head") => 5,
        _ if id.ends_with("_chestplate") || id == "elytra" => 6,
        _ if id.ends_with("_leggings") => 7,
        _ if id.ends_with("_boots") => 8,
        _ => return None,
    })
}

impl Menu {
    pub fn quick_move_stack(&mut self, slot_index: usize) -> ItemStack {
        let slot = self.slot(slot_index);
        if slot.is_none() {
            return ItemStack::Empty;
        };

        let slot_location = self
            .location_for_slot(slot_index)
            .expect("we just checked to make sure the slot is Some above, so this shouldn't be able to error");
        match slot_location {
            MenuLocation::Player(l) => match l {
                PlayerMenuLocation::CraftResult => {
                    self.quick_move_to_player_reversed(slot_index);
                }
                PlayerMenuLocation::Craft => {
                    self.try_move_item_to_slots(slot_index, Player::INVENTORY_SLOTS);
                }
                PlayerMenuLocation::Armor => {
                    self.try_move_item_to_slots(slot_index, Player::INVENTORY_SLOTS);
                }
                _ => {

                    if let Some(pos) = armor_slot_for(self.slot(slot_index).unwrap().kind())
                        && self.slot(pos).is_some_and(ItemStack::is_empty)
                    {
                        self.try_move_item_to_slots(slot_index, pos..=pos);
                    } else if l == PlayerMenuLocation::Inventory {
                        if Player::is_hotbar_slot(slot_index) {
                            self.try_move_item_to_slots(
                                slot_index,
                                Player::INVENTORY_WITHOUT_HOTBAR_SLOTS,
                            );
                        } else {
                            self.try_move_item_to_slots(slot_index, Player::HOTBAR_SLOTS);
                        }
                    } else {
                        self.try_move_item_to_slots(slot_index, self.player_slots_range());
                    }
                }
            },
            MenuLocation::Generic9x1(l) => match l {
                Generic9x1MenuLocation::Contents => {
                    self.quick_move_to_player_reversed(slot_index);
                }
                Generic9x1MenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::GENERIC9X1_CONTENTS_SLOTS,
                    );
                }
            },
            MenuLocation::Generic9x2(l) => match l {
                Generic9x2MenuLocation::Contents => {
                    self.quick_move_to_player_reversed(slot_index);
                }
                Generic9x2MenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::GENERIC9X2_CONTENTS_SLOTS,
                    );
                }
            },
            MenuLocation::Generic9x3(l) => match l {
                Generic9x3MenuLocation::Contents => {
                    self.quick_move_to_player_reversed(slot_index);
                }
                Generic9x3MenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::GENERIC9X3_CONTENTS_SLOTS,
                    );
                }
            },
            MenuLocation::Generic9x4(l) => match l {
                Generic9x4MenuLocation::Contents => {
                    self.quick_move_to_player_reversed(slot_index);
                }
                Generic9x4MenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::GENERIC9X4_CONTENTS_SLOTS,
                    );
                }
            },
            MenuLocation::Generic9x5(l) => match l {
                Generic9x5MenuLocation::Contents => {
                    self.quick_move_to_player_reversed(slot_index);
                }
                Generic9x5MenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::GENERIC9X5_CONTENTS_SLOTS,
                    );
                }
            },
            MenuLocation::Generic9x6(l) => match l {
                Generic9x6MenuLocation::Contents => {
                    self.quick_move_to_player_reversed(slot_index);
                }
                Generic9x6MenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::GENERIC9X6_CONTENTS_SLOTS,
                    );
                }
            },
            MenuLocation::Generic3x3(l) => match l {
                Generic3x3MenuLocation::Contents => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
                Generic3x3MenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::GENERIC3X3_CONTENTS_SLOTS,
                    );
                }
            },
            MenuLocation::Crafter3x3(l) => match l {
                Crafter3x3MenuLocation::Contents => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
                Crafter3x3MenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::GENERIC3X3_CONTENTS_SLOTS,
                    );
                }
            },
            MenuLocation::Anvil(l) => match l {
                AnvilMenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::ANVIL_FIRST_SLOT..=Menu::ANVIL_SECOND_SLOT,
                    );
                }
                _ => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
            },
            MenuLocation::Beacon(l) => match l {
                BeaconMenuLocation::Payment => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
                BeaconMenuLocation::Player => {
                    self.try_move_item_to_slots(
                        slot_index,
                        Menu::BEACON_PAYMENT_SLOT..=Menu::BEACON_PAYMENT_SLOT,
                    );
                }
            },
            MenuLocation::BlastFurnace(l) => match l {
                BlastFurnaceMenuLocation::Player => {
                    self.try_move_item_to_slots(
                        slot_index,
                        Menu::BLAST_FURNACE_INGREDIENT_SLOT..=Menu::BLAST_FURNACE_FUEL_SLOT,
                    );
                }
                BlastFurnaceMenuLocation::Result => {
                    self.quick_move_to_player_reversed(slot_index);
                }
                _ => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
            },
            MenuLocation::BrewingStand(l) => match l {
                BrewingStandMenuLocation::Player => {
                    self.try_move_item_to_slots(
                        slot_index,
                        *Menu::BREWING_STAND_BOTTLES_SLOTS.start()
                            ..=Menu::BREWING_STAND_INGREDIENT_SLOT,
                    );
                }
                _ => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
            },
            MenuLocation::Crafting(l) => match l {
                CraftingMenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::CRAFTING_GRID_SLOTS,
                    );
                }
                CraftingMenuLocation::Result => {
                    self.quick_move_to_player_reversed(slot_index);
                }
                _ => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
            },
            MenuLocation::Enchantment(l) => match l {
                EnchantmentMenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::ENCHANTMENT_ITEM_SLOT..=Menu::ENCHANTMENT_LAPIS_SLOT,
                    );
                }
                _ => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
            },
            MenuLocation::Furnace(l) => match l {
                FurnaceMenuLocation::Player => {
                    self.try_move_item_to_slots(
                        slot_index,
                        Menu::FURNACE_INGREDIENT_SLOT..=Menu::FURNACE_FUEL_SLOT,
                    );
                }
                FurnaceMenuLocation::Result => {
                    self.quick_move_to_player_reversed(slot_index);
                }
                _ => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
            },
            MenuLocation::Grindstone(l) => match l {
                GrindstoneMenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::GRINDSTONE_INPUT_SLOT..=Menu::GRINDSTONE_ADDITIONAL_SLOT,
                    );
                }
                _ => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
            },
            MenuLocation::Hopper(l) => match l {
                HopperMenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::HOPPER_CONTENTS_SLOTS,
                    );
                }
                _ => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
            },
            MenuLocation::Lectern(l) => match l {
                LecternMenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::LECTERN_BOOK_SLOT..=Menu::LECTERN_BOOK_SLOT,
                    );
                }
                _ => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
            },
            MenuLocation::Loom(l) => match l {
                LoomMenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::LOOM_BANNER_SLOT..=Menu::LOOM_PATTERN_SLOT,
                    );
                }
                _ => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
            },
            MenuLocation::Merchant(l) => match l {
                MerchantMenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::MERCHANT_PAYMENTS_SLOTS,
                    );
                }
                _ => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
            },
            MenuLocation::ShulkerBox(l) => match l {
                ShulkerBoxMenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::SHULKER_BOX_CONTENTS_SLOTS,
                    );
                }
                _ => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
            },
            MenuLocation::Smithing(l) => match l {
                SmithingMenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::SMITHING_TEMPLATE_SLOT..=Menu::SMITHING_ADDITIONAL_SLOT,
                    );
                }
                _ => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
            },
            MenuLocation::Smoker(l) => match l {
                SmokerMenuLocation::Player => {
                    self.try_move_item_to_slots(
                        slot_index,
                        Menu::SMOKER_INGREDIENT_SLOT..=Menu::SMOKER_FUEL_SLOT,
                    );
                }
                SmokerMenuLocation::Result => {
                    self.quick_move_to_player_reversed(slot_index);
                }
                _ => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
            },
            MenuLocation::CartographyTable(l) => match l {
                CartographyTableMenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::CARTOGRAPHY_TABLE_MAP_SLOT..=Menu::CARTOGRAPHY_TABLE_ADDITIONAL_SLOT,
                    );
                }
                _ => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
            },
            MenuLocation::Stonecutter(l) => match l {
                StonecutterMenuLocation::Player => {
                    self.try_move_item_to_slots_or_toggle_hotbar(
                        slot_index,
                        Menu::STONECUTTER_INPUT_SLOT..=Menu::STONECUTTER_INPUT_SLOT,
                    );
                }
                _ => {
                    self.try_move_item_to_slots(slot_index, self.player_slots_range());
                }
            },
        }

        ItemStack::Empty
    }

    fn quick_move_to_player_reversed(&mut self, slot_index: usize) {
        self.try_move_item_to_slots_reversed(slot_index, self.player_slots_range());
    }

    fn try_move_item_to_slots_or_toggle_hotbar(
        &mut self,
        slot_index: usize,
        target_slot_indexes: RangeInclusive<usize>,
    ) {
        if !self.try_move_item_to_slots(slot_index, target_slot_indexes) {
            self.try_move_item_to_slots(
                slot_index,
                if self.is_hotbar_slot(slot_index) {
                    self.player_slots_without_hotbar_range()
                } else {
                    self.hotbar_slots_range()
                },
            );
        }
    }

    pub fn may_place(&self, _target_slot_index: usize, _item: &ItemStackData) -> bool {
        true
    }

    pub fn may_pickup(&self, _source_slot_index: usize) -> bool {
        true
    }

    pub fn allow_modification(&self, target_slot_index: usize) -> bool {
        if !self.may_pickup(target_slot_index) {
            return false;
        }
        let item = self.slot(target_slot_index).unwrap();
        item.as_present()
            .is_some_and(|item| self.may_place(target_slot_index, item))
    }

    pub fn max_stack_size(&self, _target_slot_index: usize) -> i32 {
        64
    }

    fn try_move_item_to_slots(
        &mut self,
        item_slot_index: usize,
        target_slot_indexes: RangeInclusive<usize>,
    ) -> bool {
        self.try_move_item_to_slot_order(item_slot_index, target_slot_indexes)
    }

    fn try_move_item_to_slots_reversed(
        &mut self,
        item_slot_index: usize,
        target_slot_indexes: RangeInclusive<usize>,
    ) -> bool {
        self.try_move_item_to_slot_order(item_slot_index, target_slot_indexes.rev())
    }

    fn try_move_item_to_slot_order(
        &mut self,
        item_slot_index: usize,
        target_slots: impl Iterator<Item = usize> + Clone,
    ) -> bool {
        let mut item_slot = self.slot(item_slot_index).unwrap().clone();

        if item_slot.kind().stackable() {
            for target_slot_index in target_slots.clone() {
                self.move_item_to_slot_if_stackable(&mut item_slot, target_slot_index);
                if item_slot.is_empty() {
                    break;
                }
            }
        }

        if item_slot.is_present() {
            for target_slot_index in target_slots {
                self.move_item_to_slot_if_empty(&mut item_slot, target_slot_index);
                if item_slot.is_empty() {
                    break;
                }
            }
        }

        let is_source_slot_now_empty = item_slot.is_empty();

        *self.slot_mut(item_slot_index).unwrap() = item_slot;
        is_source_slot_now_empty
    }

    fn move_item_to_slot_if_stackable(
        &mut self,
        item_slot: &mut ItemStack,
        target_slot_index: usize,
    ) {
        let ItemStack::Present(item) = item_slot else {
            return;
        };
        let target_slot = self.slot(target_slot_index).unwrap();
        if let ItemStack::Present(target_item) = target_slot {
            if self.may_place(target_slot_index, item)
                && target_item.is_same_item_and_components(item)
            {
                let slot_item_limit = self.max_stack_size(target_slot_index);
                let new_target_slot_data = item.split(i32::min(slot_item_limit, item.count) as u32);

                let target_slot = self.slot_mut(target_slot_index).unwrap();
                *target_slot = ItemStack::from(new_target_slot_data);

                item_slot.update_empty();
            }
        }
    }

    fn move_item_to_slot_if_empty(
        &mut self,
        source_item: &mut ItemStack,
        target_slot_index: usize,
    ) {
        let ItemStack::Present(source_item_data) = source_item else {
            return;
        };
        let target_slot = self.slot(target_slot_index).unwrap();
        if target_slot.is_empty() && self.may_place(target_slot_index, source_item_data) {
            let slot_item_limit = self.max_stack_size(target_slot_index);
            let new_target_slot_data =
                source_item_data.split(i32::min(slot_item_limit, source_item_data.count) as u32);
            source_item.update_empty();

            let target_slot = self.slot_mut(target_slot_index).unwrap();
            *target_slot = new_target_slot_data.into();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_move_stack_equips_armor_from_inventory() {
        let mut menu = Menu::Player(Player::default());
        *menu.slot_mut(9).unwrap() = ItemStack::new(ItemKind::DiamondHelmet, 1);

        menu.quick_move_stack(9);

        assert_eq!(menu.slot(5).unwrap().kind(), ItemKind::DiamondHelmet);
        assert!(menu.slot(9).unwrap().is_empty());
    }

    #[test]
    fn quick_move_stack_skips_occupied_armor_slot() {
        let mut menu = Menu::Player(Player::default());
        *menu.slot_mut(5).unwrap() = ItemStack::new(ItemKind::DiamondHelmet, 1);
        *menu.slot_mut(9).unwrap() = ItemStack::new(ItemKind::GoldenHelmet, 1);

        menu.quick_move_stack(9);

        assert_eq!(menu.slot(5).unwrap().kind(), ItemKind::DiamondHelmet);
        assert_eq!(menu.slot(9).unwrap().kind(), ItemKind::Air);
        assert!(
            (36..45).any(|i| menu.slot(i).unwrap().kind() == ItemKind::GoldenHelmet),
            "golden helmet should have gone to the hotbar instead of displacing the equipped helmet"
        );
    }
}
