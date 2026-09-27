use super::socket::{Frame, Socket};
use crate::log_warn;

pub(super) const HANDSHAKE_VERSION: u16 = 5;

fn offered_protocol() -> i32 {
    #[cfg(feature = "multiversion")]
    {
        crate::protocol::session()
    }
    #[cfg(not(feature = "multiversion"))]
    {
        azalea_protocol::packets::PROTOCOL_VERSION
    }
}

fn offered_version_name() -> &'static str {
    #[cfg(feature = "multiversion")]
    {
        use crate::protocol::version::{NATIVE, ProtocolVersion};
        let protocol = offered_protocol();
        if protocol == NATIVE.protocol {
            NATIVE.name
        } else {
            ProtocolVersion::from_protocol(protocol).map_or(NATIVE.name, |v| v.name)
        }
    }
    #[cfg(not(feature = "multiversion"))]
    {
        crate::ASSET_VERSION
    }
}

const CLIENT_VERSION: u8 = 0x01;
const CLIENT_REQUEST_LOGIN: u8 = 0x04;
const CLIENT_FINISH_LOGIN: u8 = 0x08;
const SERVER_VERSION: u8 = 0x02;
const VERSION_MISMATCH: u8 = 0x03;
const SERVER_ALLOW_LOGIN: u8 = 0x05;
const SERVER_DENY_LOGIN: u8 = 0x06;
const SERVER_FINISH_LOGIN: u8 = 0x09;
const SERVER_ERROR: u8 = 0xFF;

pub(super) async fn run(
    socket: &mut Socket,
    username: &str,
) -> eyre::Result<azalea_auth::game_profile::GameProfile> {
    send(socket, client_version(username)?).await?;

    let mut version = expect(socket, SERVER_VERSION).await?;
    let eagler = version.u16()?;
    let minecraft = version.u16()?;
    let brand = version.str()?;
    let server_version = version.str()?;
    if eagler != HANDSHAKE_VERSION {
        eyre::bail!(
            "the server negotiated eagler handshake version {eagler}, which was not offered"
        );
    }
    crate::log_info!(
        "net",
        "eagler: handshake v{eagler} against {brand} {server_version}, minecraft protocol {minecraft}"
    );
    if minecraft as i32 != offered_protocol() {
        log_warn!(
            "net",
            "eagler: the server picked minecraft protocol {minecraft} but this connection was \
             asked for {}; packets will not decode",
            offered_protocol()
        );
    }

    send(socket, request_login(username)?).await?;

    let mut allow = expect(socket, SERVER_ALLOW_LOGIN).await?;
    let name = allow.str()?;
    let uuid = uuid::Uuid::from_u64_pair(allow.u64()?, allow.u64()?);

    send(socket, vec![CLIENT_FINISH_LOGIN]).await?;
    expect(socket, SERVER_FINISH_LOGIN).await?;

    Ok(azalea_auth::game_profile::GameProfile::new(uuid, name))
}

fn client_version(username: &str) -> eyre::Result<Vec<u8>> {
    let mut out = vec![CLIENT_VERSION];
    out.push(2);
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&HANDSHAKE_VERSION.to_be_bytes());
    let protocol = u16::try_from(offered_protocol())
        .map_err(|_| eyre::eyre!("minecraft protocol version does not fit the eagler handshake"))?;
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&protocol.to_be_bytes());
    put_str(&mut out, "vanilla")?;
    put_str(&mut out, offered_version_name())?;
    out.push(0);
    put_str(&mut out, username)?;
    Ok(out)
}

fn request_login(username: &str) -> eyre::Result<Vec<u8>> {
    let mut out = vec![CLIENT_REQUEST_LOGIN];
    put_str(&mut out, username)?;
    out.push(0);
    out.push(0);
    out.push(0);
    out.push(0);
    out.push(0);
    out.push(0);
    Ok(out)
}

async fn send(socket: &mut Socket, packet: Vec<u8>) -> eyre::Result<()> {
    socket.send_binary(packet).await
}

async fn expect(socket: &mut Socket, opcode: u8) -> eyre::Result<Reader> {
    loop {
        let Some(message) = socket.recv().await? else {
            eyre::bail!("the server closed the connection during the handshake");
        };
        let payload = match message {
            Frame::Binary(payload) => payload,
            Frame::Close(Some(reason)) => {
                eyre::bail!("the server closed the connection: {reason}")
            }
            Frame::Close(None) => eyre::bail!("the server closed the connection"),
            Frame::Text(_) | Frame::Other => continue,
        };
        let mut reader = Reader::new(payload);
        let got = reader.u8()?;
        if got == opcode {
            return Ok(reader);
        }
        eyre::bail!("{}", describe(got, &mut reader));
    }
}

fn describe(opcode: u8, reader: &mut Reader) -> String {
    match opcode {
        SERVER_DENY_LOGIN => match reader.long_str() {
            Ok(message) => format!("the server refused the login: {message}"),
            Err(_) => "the server refused the login".to_string(),
        },
        SERVER_ERROR => {
            let code = reader.u8().unwrap_or(0);
            match reader.long_str() {
                Ok(message) => format!("the server reported an error ({code}): {message}"),
                Err(_) => format!("the server reported error code {code}"),
            }
        }
        VERSION_MISMATCH => {
            let allowed = reader
                .u16()
                .ok()
                .map(|count| {
                    (0..count)
                        .filter_map(|_| reader.u16().ok())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            format!(
                "the server does not support eagler handshake version {HANDSHAKE_VERSION}; it \
                 accepts {allowed:?}. Only version 5 is implemented, so this server cannot be \
                 joined; see src/eagler/handshake.rs."
            )
        }
        _ => format!("the server sent an unexpected handshake packet 0x{opcode:02x}"),
    }
}

fn put_str(out: &mut Vec<u8>, value: &str) -> eyre::Result<()> {
    let len = u8::try_from(value.len())
        .map_err(|_| eyre::eyre!("{value:?} is too long for a handshake field"))?;
    out.push(len);
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

struct Reader {
    buf: Vec<u8>,
    at: usize,
}

impl Reader {
    fn new(buf: Vec<u8>) -> Self {
        Self { buf, at: 0 }
    }

    fn take(&mut self, n: usize) -> eyre::Result<&[u8]> {
        let end = self
            .at
            .checked_add(n)
            .filter(|end| *end <= self.buf.len())
            .ok_or_else(|| eyre::eyre!("the handshake packet ended mid-field"))?;
        let slice = &self.buf[self.at..end];
        self.at = end;
        Ok(slice)
    }

    fn u8(&mut self) -> eyre::Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> eyre::Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> eyre::Result<u64> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn str(&mut self) -> eyre::Result<String> {
        let len = self.u8()? as usize;
        Ok(String::from_utf8_lossy(self.take(len)?).into_owned())
    }

    fn long_str(&mut self) -> eyre::Result<String> {
        let len = self.u16()? as usize;
        Ok(String::from_utf8_lossy(self.take(len)?).into_owned())
    }
}
