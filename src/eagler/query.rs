use std::time::Duration;

use super::socket::{Frame, Socket};
use crate::log_debug;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

const ICON_TIMEOUT: Duration = Duration::from_millis(1500);

const ICON_BYTES: usize = 64 * 64 * 4;

const REPLY_TIMEOUT: Duration = Duration::from_secs(3);

pub(crate) struct Status {
    pub lines: Vec<String>,
    pub online: i32,
    pub max: i32,
    pub version: String,
    pub icon: Option<Vec<u8>>,
    pub protocol: i32,
}

pub(crate) async fn status(url: &str) -> eyre::Result<Status> {
    let (motd, icon) = ask(url, "motd", true).await?;
    let data = motd
        .get("data")
        .ok_or_else(|| eyre::eyre!("the server's motd reply carried no data"))?;

    let lines = data
        .get("motd")
        .and_then(|m| m.as_array())
        .map(|lines| {
            lines
                .iter()
                .filter_map(|l| l.as_str())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    let protocol = match ask(url, "version", false).await {
        Ok((version, _)) => protocol_for(&version),
        Err(e) => {
            log_debug!("net", "eagler: no version query from {url}: {e}");
            azalea_protocol::packets::PROTOCOL_VERSION
        }
    };

    Ok(Status {
        lines,
        online: data.get("online").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        max: data.get("max").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        version: motd
            .get("vers")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned(),
        icon,
        protocol,
    })
}

fn protocol_for(version: &serde_json::Value) -> i32 {
    let ours = azalea_protocol::packets::PROTOCOL_VERSION;
    let Some(data) = version.get("data") else {
        return ours;
    };

    let handshakes = data.get("handshakeVersions").and_then(|v| v.as_array());
    let speaks_v5 = handshakes.is_none_or(|list| {
        list.iter()
            .filter_map(|v| v.as_i64())
            .any(|v| v == super::handshake::HANDSHAKE_VERSION as i64)
    });

    let range = data.get("protocolVersions");
    let min = range.and_then(|r| r.get("min")).and_then(|v| v.as_i64());
    let max = range.and_then(|r| r.get("max")).and_then(|v| v.as_i64());

    #[cfg(feature = "multiversion")]
    let joined = match (min, max) {
        (Some(min), Some(max)) => (min..=max)
            .rev()
            .find(|p| crate::protocol::joinable(*p as i32))
            .map(|p| p as i32),
        _ => Some(ours),
    };
    #[cfg(not(feature = "multiversion"))]
    let joined = match (min, max) {
        (Some(min), Some(max)) => (min..=max).contains(&(ours as i64)).then_some(ours),
        _ => Some(ours),
    };

    match joined {
        Some(protocol) if speaks_v5 => protocol,
        _ => max.unwrap_or(-1) as i32,
    }
}

async fn ask(
    url: &str,
    name: &str,
    want_icon: bool,
) -> eyre::Result<(serde_json::Value, Option<Vec<u8>>)> {
    let mut socket =
        match crate::platform::time::timeout(CONNECT_TIMEOUT, Socket::connect(url)).await {
            Ok(socket) => socket?,
            Err(()) => eyre::bail!(
                "no websocket handshake from {url} within {} seconds",
                CONNECT_TIMEOUT.as_secs()
            ),
        };
    socket.send_text(format!("accept:{name}")).await?;

    let json =
        match crate::platform::time::timeout(REPLY_TIMEOUT, reply(&mut socket, url, name)).await {
            Ok(reply) => reply?,
            Err(()) => eyre::bail!(
                "{url} did not answer `{name}` within {} seconds. A server ignores a query type it \
             does not serve rather than refusing it, and holds the socket open; see REPLY_TIMEOUT.",
                REPLY_TIMEOUT.as_secs()
            ),
        };

    let promised = json
        .get("data")
        .and_then(|data| data.get("icon"))
        .and_then(|icon| icon.as_bool())
        .unwrap_or(false);
    if !want_icon || !promised {
        return Ok((json, None));
    }

    let icon = match crate::platform::time::timeout(ICON_TIMEOUT, icon_frame(&mut socket)).await {
        Ok(Ok(icon)) => Some(icon),
        Ok(Err(e)) => {
            log_debug!("net", "eagler: no icon from {url}: {e}");
            None
        }
        Err(()) => {
            log_debug!("net", "eagler: {url} promised an icon and did not send one");
            None
        }
    };
    Ok((json, icon))
}

async fn icon_frame(socket: &mut Socket) -> eyre::Result<Vec<u8>> {
    loop {
        let Some(message) = socket.recv().await? else {
            eyre::bail!("the server closed the connection before sending the icon");
        };
        match message {
            Frame::Binary(payload) if payload.len() == ICON_BYTES => return Ok(payload),
            Frame::Binary(payload) => {
                eyre::bail!(
                    "the icon frame was {} bytes, not the {ICON_BYTES} a 64x64 image is",
                    payload.len()
                )
            }
            Frame::Close(_) => eyre::bail!("the server closed the connection before the icon"),
            Frame::Text(_) | Frame::Other => {}
        }
    }
}

async fn reply(socket: &mut Socket, url: &str, name: &str) -> eyre::Result<serde_json::Value> {
    let mut ignored = 0usize;
    loop {
        let Some(message) = socket.recv().await? else {
            eyre::bail!(
                "the server closed the connection without answering `{name}` (after {ignored} \
                 other frame(s))"
            );
        };
        match message {
            Frame::Text(text) => return Ok(serde_json::from_str(&text)?),
            Frame::Binary(payload) => {
                if let Ok(text) = std::str::from_utf8(&payload)
                    && let Ok(value) = serde_json::from_str(text)
                {
                    log_debug!(
                        "net",
                        "eagler: {url} answered `{name}` in a binary frame, not a text one"
                    );
                    return Ok(value);
                }
                ignored += 1;
            }
            Frame::Close(reason) => match reason {
                Some(reason) => eyre::bail!(
                    "the server closed the connection without answering `{name}`: {reason}"
                ),
                None => {
                    eyre::bail!("the server closed the connection without answering `{name}`")
                }
            },
            Frame::Other => ignored += 1,
        }
    }
}
