use std::collections::HashMap;

use azalea_buf::AzBuf;
use azalea_protocol_macros::ClientboundGamePacket;

#[derive(AzBuf, ClientboundGamePacket, Clone, Debug, PartialEq)]
pub struct ClientboundCustomReportDetails {

    pub details: HashMap<String, String>,
}
