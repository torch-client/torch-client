#![feature(min_specialization)]

pub mod components;
pub mod default_components;
pub mod item;
pub mod operations;
mod slot;

use std::ops::{Deref, DerefMut, RangeInclusive};

use azalea_inventory_macros::declare_menus;
pub use slot::{DataComponentPatch, ItemStack, ItemStackData, UntrustedItemStack};

#[derive(Clone, Debug)]
pub struct SlotList<const N: usize>([ItemStack; N]);
impl<const N: usize> Deref for SlotList<N> {
    type Target = [ItemStack; N];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<const N: usize> DerefMut for SlotList<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<const N: usize> Default for SlotList<N> {
    fn default() -> Self {
        SlotList([(); N].map(|_| ItemStack::Empty))
    }
}
impl<const N: usize> SlotList<N> {
    pub fn new(items: [ItemStack; N]) -> Self {
        SlotList(items)
    }
}

impl Menu {
    pub fn as_player(&self) -> &Player {
        self.try_as_player()
            .expect("Called `Menu::as_player` on a menu that wasn't `Player`.")
    }
    pub fn try_as_player(&self) -> Option<&Player> {
        if let Menu::Player(player) = &self {
            Some(player)
        } else {
            None
        }
    }

    pub fn as_player_mut(&mut self) -> &mut Player {
        self.try_as_player_mut()
            .expect("Called `Menu::as_player_mut` on a menu that wasn't `Player`.")
    }
    pub fn try_as_player_mut(&mut self) -> Option<&mut Player> {
        if let Menu::Player(player) = self {
            Some(player)
        } else {
            None
        }
    }
}

declare_menus! {
    Player {
        craft_result: 1,
        craft: 4,
        armor: 4,
        inventory: 36,
        offhand: 1,
    },
    Generic9x1 {
        contents: 9,
    },
    Generic9x2 {
        contents: 18,
    },
    Generic9x3 {
        contents: 27,
    },
    Generic9x4 {
        contents: 36,
    },
    Generic9x5 {
        contents: 45,
    },
    Generic9x6 {
        contents: 54,
    },
    Generic3x3 {
        contents: 9,
    },
    Crafter3x3 {
        contents: 9,
    },
    Anvil {
        first: 1,
        second: 1,
        result: 1,
    },
    Beacon {
        payment: 1,
    },
    BlastFurnace {
        ingredient: 1,
        fuel: 1,
        result: 1,
    },
    BrewingStand {
        bottles: 3,
        ingredient: 1,
        fuel: 1,
    },
    Crafting {
        result: 1,
        grid: 9,
    },
    Enchantment {
        item: 1,
        lapis: 1,
    },
    Furnace {
        ingredient: 1,
        fuel: 1,
        result: 1,
    },
    Grindstone {
        input: 1,
        additional: 1,
        result: 1,
    },
    Hopper {
        contents: 5,
    },
    Lectern {
        book: 1,
    },
    Loom {
        banner: 1,
        dye: 1,
        pattern: 1,
        result: 1,
    },
    Merchant {
        payments: 2,
        result: 1,
    },
    ShulkerBox {
        contents: 27,
    },
    Smithing {
        template: 1,
        base: 1,
        additional: 1,
        result: 1,
    },
    Smoker {
        ingredient: 1,
        fuel: 1,
        result: 1,
    },
    CartographyTable {
        map: 1,
        additional: 1,
        result: 1,
    },
    Stonecutter {
        input: 1,
        result: 1,
    },
}
