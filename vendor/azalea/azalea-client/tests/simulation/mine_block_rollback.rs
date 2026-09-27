use azalea_client::{mining::StartMiningBlockEvent, test_utils::prelude::*};
use azalea_core::position::{BlockPos, ChunkPos};
use azalea_protocol::packets::{
    ConnectionProtocol,
    game::{ClientboundBlockChangedAck, ClientboundBlockUpdate},
};
use azalea_registry::builtin::BlockKind;

#[test]
fn test_mine_block_rollback() {
    let _lock = init();

    let mut simulation = Simulation::new(ConnectionProtocol::Game);
    simulation.receive_packet(default_login_packet());

    simulation.receive_packet(make_basic_empty_chunk(ChunkPos::new(0, 0), (384 + 64) / 16));
    simulation.tick();

    let pos = BlockPos::new(1, 2, 3);
    simulation.receive_packet(ClientboundBlockUpdate {
        pos,
        block_state: BlockKind::Tnt.into(),
    });
    simulation.tick();
    assert_eq!(simulation.get_block_state(pos), Some(BlockKind::Tnt.into()));
    println!("set serverside tnt");

    simulation.write_message(StartMiningBlockEvent {
        entity: simulation.entity,
        position: pos,
        force: true,
    });
    simulation.tick();
    assert_eq!(simulation.get_block_state(pos), Some(BlockKind::Air.into()));
    println!("set clientside air");

    simulation.receive_packet(ClientboundBlockChangedAck { seq: 1 });
    simulation.tick();
    assert_eq!(simulation.get_block_state(pos), Some(BlockKind::Tnt.into()));
    println!("reset serverside tnt");
}
