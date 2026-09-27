use azalea_buf::AzBuf;
use azalea_protocol::packets::config::s_custom_payload::ServerboundCustomPayload;
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;

use super::packet::config::SendConfigPacketEvent;
use crate::{client_information::send_client_information, packet::login::InLoginState};

pub struct BrandPlugin;
impl Plugin for BrandPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, send_brand.before(send_client_information));
    }
}

pub fn send_brand(mut commands: Commands, mut removed: RemovedComponents<InLoginState>) {
    for entity in removed.read() {
        let mut brand_data = Vec::new();
        "vanilla".to_owned().azalea_write(&mut brand_data).unwrap();
        commands.trigger(SendConfigPacketEvent::new(
            entity,
            ServerboundCustomPayload {
                identifier: "brand".into(),
                data: brand_data.into(),
            },
        ));
    }
}
