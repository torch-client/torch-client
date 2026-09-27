use azalea_client::test_utils::prelude::*;
use azalea_protocol::packets::ConnectionProtocol;
use azalea_world::WorldName;

#[test]
fn test_client_disconnect() {
    let _lock = init();

    let mut simulation = Simulation::new(ConnectionProtocol::Game);

    simulation.disconnect();
    simulation.tick();

    let is_connected = simulation.has_component::<WorldName>();
    assert!(!is_connected);

    simulation.tick();
}
