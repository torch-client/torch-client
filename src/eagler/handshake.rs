use super::socket::{Frame, Socket};
use crate::{log_info, log_warn};

pub(super) const HANDSHAKE_VERSIONS: [u16; 2] = [5, 4];

fn offered_protocol() -> i32 {
    crate::util::session_protocol()
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
const CLIENT_PROFILE_DATA: u8 = 0x07;
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
    skin: Option<&[u8]>,
) -> eyre::Result<(azalea_auth::game_profile::GameProfile, u16)> {
    send(socket, client_version(username)?).await?;

    let mut version = expect(socket, SERVER_VERSION).await?;
    let eagler = version.u16()?;
    let minecraft = version.u16()?;
    let brand = version.str()?;
    let server_version = version.str()?;
    if !HANDSHAKE_VERSIONS.contains(&eagler) {
        eyre::bail!(
            "the server ({brand} {server_version}) negotiated eagler handshake version \
             {eagler}, which was not offered (offered {HANDSHAKE_VERSIONS:?})"
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

    send(socket, request_login(username, eagler)?).await?;

    let mut allow = expect(socket, SERVER_ALLOW_LOGIN).await?;
    let name = allow.str()?;
    let uuid = uuid::Uuid::from_u64_pair(allow.u64()?, allow.u64()?);

    if let Some(skin) = skin {
        send(socket, profile_data("skin_v2", skin)?).await?;
    }
    send(socket, vec![CLIENT_FINISH_LOGIN]).await?;
    expect(socket, SERVER_FINISH_LOGIN).await?;

    Ok((
        azalea_auth::game_profile::GameProfile::new(uuid, name),
        eagler,
    ))
}

fn client_version(username: &str) -> eyre::Result<Vec<u8>> {
    let mut out = vec![CLIENT_VERSION];
    out.push(2);
    out.extend_from_slice(&(HANDSHAKE_VERSIONS.len() as u16).to_be_bytes());
    for version in HANDSHAKE_VERSIONS {
        out.extend_from_slice(&version.to_be_bytes());
    }
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

fn request_login(username: &str, eagler: u16) -> eyre::Result<Vec<u8>> {
    let mut out = vec![CLIENT_REQUEST_LOGIN];
    put_str(&mut out, username)?;
    out.push(0);
    out.push(0);
    out.push(0);
    out.push(0);
    if eagler < 5 {
        return Ok(out);
    }
    out.push(0);
    out.push(0);
    Ok(out)
}

fn profile_data(key: &str, data: &[u8]) -> eyre::Result<Vec<u8>> {
    let len = u16::try_from(data.len()).map_err(|_| eyre::eyre!("profile data too long"))?;
    let mut out = vec![CLIENT_PROFILE_DATA, 1, key.len() as u8];
    out.extend_from_slice(key.as_bytes());
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(data);
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
        return Err(describe(got, &mut reader));
    }
}

#[derive(Debug)]
pub(crate) struct Refused {
    pub(crate) reason: Vec<crate::text::Span>,
    plain: String,
}

impl Refused {
    fn new(message: &str) -> Self {
        let reason = match serde_json::from_str::<azalea_chat::FormattedText>(message) {
            Ok(component) => crate::client::chat_text::to_spans(&component),
            Err(_) => crate::text::parse_formatted(message),
        };
        let plain = reason.iter().map(|span| span.text.as_str()).collect();
        Self { reason, plain }
    }
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.plain)
    }
}

impl std::error::Error for Refused {}

fn describe(opcode: u8, reader: &mut Reader) -> eyre::Report {
    match opcode {
        SERVER_DENY_LOGIN => match reader.long_str() {
            Ok(message) => Refused::new(&message).into(),
            Err(_) => eyre::eyre!("the server refused the login"),
        },
        SERVER_ERROR => {
            let code = reader.u8().unwrap_or(0);
            match reader.long_str() {
                Ok(message) => {
                    log_info!("net", "eagler: the server sent error code {code}");
                    Refused::new(&message).into()
                }
                Err(_) => eyre::eyre!("the server reported error code {code}"),
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
            eyre::eyre!(
                "the server does not support eagler handshake versions {HANDSHAKE_VERSIONS:?}; \
                 it accepts {allowed:?}. Only 5 and 4 are implemented, so this server cannot \
                 be joined; see src/eagler/handshake.rs."
            )
        }
        _ => eyre::eyre!("the server sent an unexpected handshake packet 0x{opcode:02x}"),
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
