use std::{
    io::{self, Cursor, Write},
    mem::ManuallyDrop,
    num::NonZeroU32,
};

use azalea_buf::{AzBuf, AzBufVar, BufReadError};
use azalea_inventory::{
    DataComponentPatch, ItemStack, ItemStackData, read_item_kind, write_item_kind,
};
use azalea_protocol_macros::ClientboundGamePacket;
use azalea_registry::builtin::ItemKind;

#[derive(AzBuf, ClientboundGamePacket, Clone, Debug, PartialEq)]
pub struct ClientboundMerchantOffers {
    #[var]
    pub container_id: i32,
    pub offers: Vec<MerchantOffer>,
    #[var]
    pub villager_level: u32,
    #[var]
    pub villager_xp: u32,
    pub show_progress: bool,
    pub can_restock: bool,
}

#[derive(AzBuf, Clone, Debug, PartialEq)]
pub struct MerchantOffer {
    pub base_cost_a: ItemCost,
    pub result: ItemStack,
    pub cost_b: Option<ItemCost>,
    pub out_of_stock: bool,
    pub uses: i32,
    pub max_uses: i32,
    pub xp: i32,
    pub special_price_diff: i32,
    pub price_multiplier: f32,
    pub demand: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemCost {
    pub item: ItemKind,
    pub count: i32,
    pub components: DataComponentExactPredicate,
    pub modded_item: Option<NonZeroU32>,
}
impl AzBuf for ItemCost {
    fn azalea_read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        let (item, modded_item) = read_item_kind(buf)?;
        Ok(Self {
            item,
            count: i32::azalea_read_var(buf)?,
            components: DataComponentExactPredicate::azalea_read(buf)?,
            modded_item,
        })
    }
    fn azalea_write(&self, buf: &mut impl Write) -> io::Result<()> {
        write_item_kind(self.item, self.modded_item, buf)?;
        self.count.azalea_write_var(buf)?;
        self.components.azalea_write(buf)
    }
}
impl ItemCost {
    pub fn into_item_stack(self) -> ItemStackData {
        let mut component_patch = DataComponentPatch::default();
        for component in self.components.expected {
            let component = ManuallyDrop::new(component);
            let value = unsafe { std::ptr::read(component.value()) };
            unsafe {
                component_patch.unchecked_insert_component(*component.kind(), Some(value));
            }
        }

        let mut stack = ItemStackData {
            kind: self.item,
            count: self.count,
            component_patch,
        };
        stack.set_modded_kind(self.modded_item);
        stack
    }
}

pub use azalea_inventory::components::{DataComponentExactPredicate, TypedDataComponent};
