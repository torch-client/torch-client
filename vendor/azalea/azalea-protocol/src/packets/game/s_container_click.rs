use std::{
    io::{self, Cursor, Write},
    num::NonZeroU32,
};

use azalea_buf::{AzBuf, AzBufVar, BufReadError};
use azalea_core::{checksum::Checksum, registry_holder::RegistryHolder};
use azalea_inventory::{ItemStack, operations::ClickType, read_item_kind, write_item_kind};
use azalea_protocol_macros::ServerboundGamePacket;
use azalea_registry::builtin::{DataComponentKind, ItemKind};
use indexmap::IndexMap;

#[derive(AzBuf, Clone, Debug, PartialEq, ServerboundGamePacket)]
pub struct ServerboundContainerClick {
    #[var]
    pub container_id: i32,
    #[var]
    pub state_id: u32,
    pub slot_num: i16,
    pub button_num: u8,
    pub click_type: ClickType,
    pub changed_slots: IndexMap<u16, HashedStack>,
    pub carried_item: HashedStack,
}

#[derive(AzBuf, Clone, Debug, PartialEq)]
pub struct HashedStack(pub Option<HashedActualItem>);

#[derive(Clone, Debug, PartialEq)]
pub struct HashedActualItem {
    pub kind: ItemKind,
    pub count: i32,
    pub components: HashedPatchMap,
    pub modded_kind: Option<NonZeroU32>,
}
impl AzBuf for HashedActualItem {
    fn azalea_read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        let (kind, modded_kind) = read_item_kind(buf)?;
        Ok(Self {
            kind,
            count: i32::azalea_read_var(buf)?,
            components: HashedPatchMap::azalea_read(buf)?,
            modded_kind,
        })
    }
    fn azalea_write(&self, buf: &mut impl Write) -> io::Result<()> {
        write_item_kind(self.kind, self.modded_kind, buf)?;
        self.count.azalea_write_var(buf)?;
        self.components.azalea_write(buf)
    }
}

#[derive(AzBuf, Clone, Debug, PartialEq)]
pub struct HashedPatchMap {
    #[limit(256)]
    pub added_components: Vec<(DataComponentKind, Checksum)>,
    #[limit(256)]
    pub removed_components: Vec<DataComponentKind>,
}

impl HashedStack {
    pub fn from_item_stack(item: &ItemStack, registries: &RegistryHolder) -> Self {
        let ItemStack::Present(item) = item else {
            return HashedStack(None);
        };

        let mut added_components = Vec::new();
        let mut removed_components = Vec::new();

        for (kind, data) in item.component_patch.iter() {
            if let Some(data) = data {
                added_components.push((kind, data.crc_hash(registries)));
            } else {
                removed_components.push(kind);
            }
        }

        let components = HashedPatchMap {
            added_components,
            removed_components,
        };
        let item = HashedActualItem {
            kind: item.kind,
            count: item.count,
            components,
            modded_kind: item.modded_kind(),
        };
        Self(Some(item))
    }
}
