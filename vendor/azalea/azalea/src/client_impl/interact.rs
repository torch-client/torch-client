use azalea_client::interact::{EntityInteractEvent, StartUseItemEvent, pick::HitResultComponent};
use azalea_core::{hit_result::HitResult, position::BlockPos};
use azalea_protocol::packets::game::s_interact::InteractionHand;
use bevy_ecs::entity::Entity;

use crate::{Client, client_impl::error::AzaleaResult};

impl Client {
    pub fn hit_result(&self) -> AzaleaResult<HitResult> {
        Ok((**self.component::<HitResultComponent>()?).clone())
    }

    pub fn block_interact(&self, position: BlockPos) {
        self.ecs.write().write_message(StartUseItemEvent {
            entity: self.entity,
            hand: InteractionHand::MainHand,
            force_block: Some(position),
        });
    }

    pub fn entity_interact(&self, entity: Entity) {
        self.ecs.write().trigger(EntityInteractEvent {
            client: self.entity,
            target: entity,
            location: None,
        });
    }

    pub fn start_use_item(&self) {
        self.ecs.write().write_message(StartUseItemEvent {
            entity: self.entity,
            hand: InteractionHand::MainHand,
            force_block: None,
        });
    }
}
