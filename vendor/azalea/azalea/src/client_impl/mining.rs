use azalea_client::mining::{LeftClickMine, Mining, StartMiningBlockEvent};
use azalea_core::position::BlockPos;

use crate::Client;

impl Client {
    pub fn start_mining(&self, position: BlockPos) {
        self.start_mining_with(position, false);
    }

    pub fn start_mining_with(&self, position: BlockPos, continuing: bool) {
        let mut ecs = self.ecs.write();

        ecs.write_message(StartMiningBlockEvent {
            entity: self.entity,
            position,
            force: true,
            continuing,
        });
    }

    pub fn start_mining_with_face(
        &self,
        position: BlockPos,
        face: azalea_core::direction::Direction,
        continuing: bool,
    ) {
        *azalea_client::mining::FORCED_FACE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((position, face));
        self.start_mining_with(position, continuing);
    }

    pub fn is_mining(&self) -> bool {
        self.component::<Mining>().is_ok()
    }

    pub fn left_click_mine(&self, enabled: bool) {
        let mut ecs = self.ecs.write();
        let mut entity_mut = ecs.entity_mut(self.entity);

        if enabled {
            entity_mut.insert(LeftClickMine);
        } else {
            entity_mut.remove::<LeftClickMine>();
        }
    }
}
