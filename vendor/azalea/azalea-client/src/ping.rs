use std::io;

use azalea_protocol::{
    address::{ResolvableAddr, ServerAddr},
    connect::{Connection, ConnectionError, Proxy},
    packets::{
        ClientIntention, PROTOCOL_VERSION,
        handshake::{
            ClientboundHandshakePacket, ServerboundHandshakePacket,
            s_intention::ServerboundIntention,
        },
        status::{
            ClientboundStatusPacket, c_status_response::ClientboundStatusResponse,
            s_status_request::ServerboundStatusRequest,
        },
    },
    resolve,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PingError {
    #[error("{0}")]
    Resolve(#[from] resolve::ResolveError),
    #[error("{0}")]
    Connection(#[from] ConnectionError),
    #[error("{0}")]
    ReadPacket(#[from] Box<azalea_protocol::read::ReadPacketError>),
    #[error("{0}")]
    WritePacket(#[from] io::Error),
    #[error("The given address could not be parsed into a ServerAddress")]
    InvalidAddress,
}

pub async fn ping_server(
    address: impl ResolvableAddr,
) -> Result<ClientboundStatusResponse, PingError> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = address;
        return Err(PingError::Connection(ConnectionError::Io(
            std::io::Error::other("this target has no TCP sockets to ping with"),
        )));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let address = address.resolve().await?;
        let conn = Connection::new(&address.socket).await?;
        ping_server_with_connection(address.server, conn).await
    }
}

pub async fn ping_server_with_proxy(
    address: impl ResolvableAddr,
    proxy: Proxy,
) -> Result<ClientboundStatusResponse, PingError> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (address, proxy);
        return Err(PingError::Connection(ConnectionError::Io(
            std::io::Error::other("this target has no TCP sockets to ping with"),
        )));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let address = address.resolve().await?;
        let conn = Connection::new_with_proxy(&address.socket, proxy).await?;
        ping_server_with_connection(address.server, conn).await
    }
}

pub async fn ping_server_with_connection(
    address: ServerAddr,
    mut conn: Connection<ClientboundHandshakePacket, ServerboundHandshakePacket>,
) -> Result<ClientboundStatusResponse, PingError> {
    conn.write(ServerboundIntention {
        protocol_version: PROTOCOL_VERSION,
        hostname: address.host.clone(),
        port: address.port,
        intention: ClientIntention::Status,
    })
    .await?;
    let mut conn = conn.status();

    conn.write(ServerboundStatusRequest {}).await?;

    let packet = conn.read().await?;

    loop {
        match packet {
            ClientboundStatusPacket::StatusResponse(p) => return Ok(p),
            ClientboundStatusPacket::PongResponse(_) => {
            }
        }
    }
}
