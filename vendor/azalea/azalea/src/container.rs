use std::{fmt, fmt::Debug};

use azalea_chat::FormattedText;
use azalea_client::{
    inventory::{CloseContainerEvent, ContainerClickEvent},
    packet::game::ReceiveGamePacketEvent,
};
use azalea_core::position::BlockPos;
use azalea_entity::inventory::Inventory;
use azalea_inventory::{
    ItemStack, Menu,
    operations::{ClickOperation, PickupClick, QuickMoveClick},
};
use azalea_physics::collision::BlockWithShape;
use azalea_protocol::packets::game::ClientboundGamePacket;
use bevy_app::{App, Plugin, Update};
use bevy_ecs::{component::Component, prelude::MessageReader, system::Commands};
use derive_more::Deref;

use crate::{Client, client_impl::error::AzaleaResult};

pub struct ContainerPlugin;
impl Plugin for ContainerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_menu_opened_event);
    }
}

impl Client {
    pub async fn open_container_at(&self, pos: BlockPos) -> AzaleaResult<Option<ContainerHandle>> {
        self.open_container_at_with_timeout_ticks(pos, Some(20 * 5))
            .await
    }

    pub async fn open_container_at_with_timeout_ticks(
        &self,
        pos: BlockPos,
        timeout_ticks: Option<usize>,
    ) -> AzaleaResult<Option<ContainerHandle>> {
        let mut ticks = self.get_tick_broadcaster();
        for _ in 0..10 {
            let block = self
                .world()?
                .read()
                .get_block_state(pos)
                .unwrap_or_default();
            if !block.is_collision_shape_empty() {
                break;
            }
            let _ = ticks.recv().await;
        }

        self.ecs
            .write()
            .entity_mut(self.entity)
            .insert(WaitingForInventoryOpen);
        self.block_interact(pos);

        self.wait_for_container_open(timeout_ticks).await
    }

    pub async fn wait_for_container_open(
        &self,
        timeout_ticks: Option<usize>,
    ) -> AzaleaResult<Option<ContainerHandle>> {
        let mut ticks = self.get_tick_broadcaster();
        let mut elapsed_ticks = 0;
        while ticks.recv().await.is_ok() {
            let ecs = self.ecs.read();
            if ecs.get::<WaitingForInventoryOpen>(self.entity).is_none() {
                break;
            }

            elapsed_ticks += 1;
            if let Some(timeout_ticks) = timeout_ticks
                && elapsed_ticks >= timeout_ticks
            {
                return Ok(None);
            }
        }

        let inventory_id = self.component::<Inventory>()?.id;
        if inventory_id == 0 {
            Ok(None)
        } else {
            Ok(Some(ContainerHandle::new(inventory_id, self.clone())))
        }
    }

    pub fn open_inventory(&self) -> AzaleaResult<Option<ContainerHandle>> {
        let inventory = self.component::<Inventory>()?;
        Ok(if inventory.id == 0 {
            Some(ContainerHandle::new(0, self.clone()))
        } else {
            None
        })
    }

    pub fn get_inventory(&self) -> AzaleaResult<ContainerHandleRef> {
        Ok(ContainerHandleRef::new(
            self.component::<Inventory>()?.id,
            self.clone(),
        ))
    }

    pub fn get_held_item(&self) -> AzaleaResult<ItemStack> {
        Ok(self.component::<Inventory>()?.held_item().clone())
    }
}

pub struct ContainerHandleRef {
    id: i32,
    client: Client,
}
impl Debug for ContainerHandleRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContainerHandle")
            .field("id", &self.id())
            .finish()
    }
}
impl ContainerHandleRef {
    pub fn new(id: i32, client: Client) -> Self {
        Self { id, client }
    }

    pub fn close(&self) {
        self.client.ecs.write().trigger(CloseContainerEvent {
            entity: self.client.entity,
            id: self.id,
        });
    }

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn menu(&self) -> AzaleaResult<Option<Menu>> {
        self.map_inventory(|inv| {
            if self.id == 0 {
                inv.inventory_menu.clone()
            } else {
                inv.container_menu.clone().unwrap()
            }
        })
    }

    fn map_inventory<R>(&self, f: impl FnOnce(&Inventory) -> R) -> AzaleaResult<Option<R>> {
        self.client.query_self::<&Inventory, _>(|inv| {
            if inv.id == self.id {
                Some(f(inv))
            } else {
                None
            }
        })
    }

    pub fn contents(&self) -> Option<Vec<ItemStack>> {
        Some(self.menu().ok()??.contents())
    }

    pub fn slots(&self) -> Option<Vec<ItemStack>> {
        Some(self.menu().ok()??.slots())
    }

    pub fn title(&self) -> Option<FormattedText> {
        self.map_inventory(|inv| inv.container_menu_title.clone())
            .ok()??
    }

    pub fn left_click(&self, slot: impl Into<usize>) {
        self.click(PickupClick::Left {
            slot: Some(slot.into() as u16),
        });
    }
    pub fn shift_click(&self, slot: impl Into<usize>) {
        self.click(QuickMoveClick::Left {
            slot: slot.into() as u16,
        });
    }
    pub fn right_click(&self, slot: impl Into<usize>) {
        self.click(PickupClick::Right {
            slot: Some(slot.into() as u16),
        });
    }

    pub fn click(&self, operation: impl Into<ClickOperation>) {
        let operation = operation.into();
        self.client.ecs.write().trigger(ContainerClickEvent {
            entity: self.client.entity,
            window_id: self.id,
            operation,
        });
    }
}

#[derive(Deref)]
pub struct ContainerHandle(ContainerHandleRef);

impl Drop for ContainerHandle {
    fn drop(&mut self) {
        self.0.close();
    }
}
impl Debug for ContainerHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContainerHandle")
            .field("id", &self.id())
            .finish()
    }
}
impl ContainerHandle {
    fn new(id: i32, client: Client) -> Self {
        Self(ContainerHandleRef { id, client })
    }

    pub fn close(self) {
    }
}

#[derive(Component, Debug)]
pub struct WaitingForInventoryOpen;

pub fn handle_menu_opened_event(
    mut commands: Commands,
    mut events: MessageReader<ReceiveGamePacketEvent>,
) {
    for event in events.read() {
        if let ClientboundGamePacket::ContainerSetContent { .. } = event.packet.as_ref() {
            commands
                .entity(event.entity)
                .remove::<WaitingForInventoryOpen>();
        }
    }
}
