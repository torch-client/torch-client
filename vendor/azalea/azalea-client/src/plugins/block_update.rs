use azalea_block::BlockState;
use azalea_core::position::BlockPos;
use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;

use crate::{
    chunks::handle_receive_chunk_event, interact::BlockStatePredictionHandler,
    local_player::WorldHolder,
};

pub struct BlockUpdatePlugin;
impl Plugin for BlockUpdatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                handle_block_update_event.after(handle_receive_chunk_event),
                handle_queued_block_changed_acks.after(handle_block_update_event),
            ),
        );
    }
}

#[derive(Clone, Component, Debug, Default)]
pub struct QueuedServerBlockUpdates {
    pub list: Vec<(BlockPos, BlockState)>,
}

pub fn handle_block_update_event(
    mut query: Query<(
        &mut QueuedServerBlockUpdates,
        &WorldHolder,
        &mut BlockStatePredictionHandler,
    )>,
) {
    for (mut queued, world_holder, mut prediction_handler) in query.iter_mut() {
        let world = world_holder.shared.read();
        for (pos, block_state) in queued.list.drain(..) {
            if !prediction_handler.update_known_server_state(pos, block_state) {
                world.chunks.set_block_state(pos, block_state);
            }
        }
    }
}

#[derive(Clone, Component, Debug, Default)]
pub struct QueuedBlockChangedAcks {
    pub latest_seq: Option<u32>,
}

pub fn handle_queued_block_changed_acks(
    mut query: Query<(
        &mut QueuedBlockChangedAcks,
        &WorldHolder,
        &mut BlockStatePredictionHandler,
    )>,
) {
    for (mut queued, world_holder, mut prediction_handler) in query.iter_mut() {
        if let Some(seq) = queued.latest_seq.take() {
            let world = world_holder.shared.read();
            prediction_handler.end_prediction_up_to(seq, &world);
        }
    }
}
