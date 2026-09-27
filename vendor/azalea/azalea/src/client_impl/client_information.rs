use azalea_client::ClientInformation;
use azalea_protocol::packets::game;
use tracing::debug;

use crate::{Client, client_impl::error::AzaleaResult};

impl Client {
    pub fn set_client_information(
        &self,
        client_information: ClientInformation,
    ) -> AzaleaResult<()> {
        self.query_self::<&mut ClientInformation, _>(|mut ci| {
            *ci = client_information.clone();
        })?;

        if self.logged_in() {
            debug!(
                "Sending client information (already logged in): {:?}",
                client_information
            );
            self.write_packet(game::s_client_information::ServerboundClientInformation {
                client_information,
            });
        }

        Ok(())
    }
}
