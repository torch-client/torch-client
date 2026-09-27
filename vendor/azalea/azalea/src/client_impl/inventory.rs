use azalea_client::inventory::SetSelectedHotbarSlotEvent;
use azalea_entity::inventory::Inventory;
use azalea_inventory::Menu;

use crate::{Client, client_impl::error::AzaleaResult};

impl Client {
    pub fn menu(&self) -> AzaleaResult<Menu> {
        Ok(self.component::<Inventory>()?.menu().clone())
    }

    pub fn selected_hotbar_slot(&self) -> AzaleaResult<u8> {
        Ok(self.component::<Inventory>()?.selected_hotbar_slot)
    }

    pub fn set_selected_hotbar_slot(&self, new_hotbar_slot_index: u8) {
        assert!(
            new_hotbar_slot_index < 9,
            "Hotbar slot index must be in the range 0..=8"
        );

        let mut ecs = self.ecs.write();
        ecs.trigger(SetSelectedHotbarSlotEvent {
            entity: self.entity,
            slot: new_hotbar_slot_index,
        });
    }
}
