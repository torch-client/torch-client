mod handshake;
pub(crate) use handshake::Refused;
pub(crate) mod proxy;
pub(crate) mod query;
#[cfg(eagler_skins)]
pub(crate) mod skins;
mod socket;
mod stream;
mod wisp;

use std::net::{Ipv4Addr, SocketAddr};

use azalea_protocol::address::{ResolvedAddr, ServerAddr};

use crate::log_info;

struct Target {
    url: String,
    host: String,
    port: u16,
    java: bool,
    destination: Option<proxy::Destination>,
}

struct Tunnel {
    socket: socket::Socket,
    server: ServerAddr,
    wisp: Option<wisp::Stream>,
}

fn parse(address: &str) -> Option<Target> {
    let address = address.trim();
    let (scheme, default_port) = crate::platform::address::websocket(address)?;
    let rest = address.strip_prefix(scheme)?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let separator = match authority.rfind(']') {
        Some(bracket) => authority[bracket..].find(':').map(|i| bracket + i),
        None => authority.rfind(':'),
    };
    let (host, port) = match separator {
        Some(i) => match authority[i + 1..].parse() {
            Ok(port) => (&authority[..i], port),
            Err(_) => return None,
        },
        None => (authority, default_port),
    };
    if host.is_empty() {
        return None;
    }
    let java = scheme == crate::platform::address::JAVA_TUNNEL;
    let path = &rest[authority.len()..];
    let destination = (java && path.trim_matches('/').is_empty()).then(|| proxy::Destination {
        host: host.to_string(),
        port: separator.map(|_| port),
    });
    Some(Target {
        url: if java {
            format!("wss://{rest}")
        } else {
            address.to_string()
        },
        host: host.to_string(),
        port,
        java,
        destination,
    })
}

async fn open_tunnel(target: &Target) -> eyre::Result<Tunnel> {
    let Some(destination) = &target.destination else {
        return open_relay(&target.url).await;
    };
    let proxy = proxy::selected();
    log_info!("net", "tunnel: going through the {} proxy", proxy.name);
    match proxy.transport {
        proxy::Transport::Relay(base) => {
            open_relay(&format!("{base}{}", destination.authority())).await
        }
        proxy::Transport::Wisp(url) => {
            let server = proxy::resolve(destination).await;
            let (socket, stream) = wisp::open(url, &server).await?;
            Ok(Tunnel {
                socket,
                server,
                wisp: Some(stream),
            })
        }
    }
}

async fn open_relay(url: &str) -> eyre::Result<Tunnel> {
    let mut socket = socket::Socket::connect(url).await?;
    match socket.recv().await? {
        Some(socket::Frame::Text(text)) => match relayed_address(&text) {
            Some(server) => Ok(Tunnel {
                socket,
                server,
                wisp: None,
            }),
            None => Err(eyre::eyre!(
                "the relay's first message was not a host:port: {text:?}"
            )),
        },
        Some(socket::Frame::Close(Some(reason))) => {
            Err(eyre::eyre!("the relay closed the connection: {reason}"))
        }
        Some(socket::Frame::Close(None)) | None => Err(eyre::eyre!(
            "the relay closed the connection without saying why"
        )),
        Some(_) => Err(eyre::eyre!(
            "the relay sent something other than the address it reached first; is this URL a \
             java:// relay?"
        )),
    }
}

fn relayed_address(text: &str) -> Option<ServerAddr> {
    let (host, port) = text.trim().rsplit_once(':')?;
    let host = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    if host.is_empty() {
        return None;
    }
    Some(ServerAddr {
        host: host.to_string(),
        port: port.parse().ok()?,
    })
}

pub(crate) async fn tunnel_status(
    address: &str,
) -> eyre::Result<azalea_protocol::packets::status::c_status_response::ClientboundStatusResponse> {
    let Some(target) = parse(address).filter(|t| t.java) else {
        eyre::bail!("{address} is not a java:// tunnel");
    };
    let tunnel = open_tunnel(&target).await?;
    let connection = stream::raw_connection(tunnel.socket, tunnel.wisp);
    azalea::ping::ping_server_with_connection(tunnel.server, connection)
        .await
        .map_err(|e| eyre::eyre!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::parse;

    fn split(address: &str) -> Option<(String, u16)> {
        parse(address).map(|t| (t.host, t.port))
    }

    #[test]
    fn scheme_decides_the_default_port() {
        assert_eq!(
            split("wss://play.example"),
            Some(("play.example".into(), 443))
        );
        assert_eq!(
            split("ws://play.example"),
            Some(("play.example".into(), 80))
        );
    }

    #[test]
    fn an_ordinary_address_is_left_alone() {
        assert_eq!(split("play.example:25565"), None);
        assert_eq!(split("localhost"), None);
    }

    #[test]
    fn the_path_is_not_part_of_the_host() {
        assert_eq!(
            split("wss://play.example/backend"),
            Some(("play.example".into(), 443))
        );
        assert_eq!(
            split("wss://play.example:8080/backend?a=1"),
            Some(("play.example".into(), 8080))
        );
    }

    #[test]
    fn an_ipv6_literal_keeps_its_own_colons() {
        assert_eq!(split("wss://[::1]"), Some(("[::1]".into(), 443)));
        assert_eq!(split("wss://[::1]:8080/x"), Some(("[::1]".into(), 8080)));
    }

    #[test]
    fn a_java_tunnel_is_dialled_as_wss() {
        let target = parse("java://relay.example/token/tcp/mc.example").unwrap();
        assert!(target.java);
        assert_eq!(target.url, "wss://relay.example/token/tcp/mc.example");
        assert_eq!((target.host.as_str(), target.port), ("relay.example", 443));
        assert!(
            !parse("wss://relay.example/token/tcp/mc.example")
                .unwrap()
                .java
        );
        assert_eq!(target.destination, None);
    }

    #[test]
    fn a_java_address_with_no_path_names_the_server() {
        let destination = |address: &str| parse(address).unwrap().destination;
        let named = |host: &str, port| {
            Some(super::proxy::Destination {
                host: host.to_string(),
                port,
            })
        };
        assert_eq!(destination("java://mc.example"), named("mc.example", None));
        assert_eq!(destination("java://mc.example/"), named("mc.example", None));
        assert_eq!(
            destination("java://1.2.3.4:25570"),
            named("1.2.3.4", Some(25570))
        );
        assert_eq!(
            destination("java://[2001:db8::1]:25565"),
            named("[2001:db8::1]", Some(25565))
        );
        assert_eq!(destination("wss://mc.example"), None);
    }

    #[test]
    fn the_relays_address_line_parses() {
        let pair = |text: &str| super::relayed_address(text).map(|a| (a.host, a.port));
        assert_eq!(pair("mc.example:25565"), Some(("mc.example".into(), 25565)));
        assert_eq!(pair("1.2.3.4:25570\n"), Some(("1.2.3.4".into(), 25570)));
        assert_eq!(
            pair("[2001:db8::1]:25565"),
            Some(("2001:db8::1".into(), 25565))
        );
        assert_eq!(pair("mc.example"), None);
        assert_eq!(pair(":25565"), None);
        assert_eq!(pair("mc.example:port"), None);
    }

    #[test]
    fn a_port_that_is_not_a_number_is_refused() {
        assert_eq!(split("wss://play.example:http"), None);
        assert_eq!(split("wss://"), None);
    }
}

pub(crate) async fn prepare(address: &str, username: &str) -> eyre::Result<Option<ResolvedAddr>> {
    #[cfg(eagler_skins)]
    crate::gui::skinpick::wear(None, 0);
    #[cfg(eagler_skins)]
    skins::end();
    let Some(target) = parse(address) else {
        return Ok(None);
    };
    if target.java {
        log_info!("net", "tunnel: opening {address}");
        let tunnel = open_tunnel(&target).await?;
        log_info!(
            "net",
            "tunnel: heading for {}:{}",
            tunnel.server.host,
            tunnel.server.port
        );
        stream::prepare_raw(tunnel.socket, tunnel.wisp);
        return Ok(Some(ResolvedAddr {
            server: tunnel.server,
            socket: SocketAddr::from((Ipv4Addr::LOCALHOST, target.port)),
        }));
    }
    log_info!("net", "eagler: opening a websocket to {}", target.url);
    let mut socket = socket::Socket::connect(&target.url).await?;
    #[cfg(eagler_skins)]
    let skin = crate::gui::skinpick::for_account(username);
    #[cfg(eagler_skins)]
    let wire = skin.as_ref().map(|s| s.wire.as_slice());
    #[cfg(not(eagler_skins))]
    let wire = None;
    let (profile, version) = handshake::run(&mut socket, username, wire).await?;
    #[cfg(eagler_skins)]
    crate::gui::skinpick::wear(skin, profile.uuid.as_u128());
    log_info!(
        "net",
        "eagler: the proxy logged us in as {} ({})",
        profile.name,
        profile.uuid
    );

    stream::prepare(socket, profile, version);

    Ok(Some(ResolvedAddr {
        server: ServerAddr {
            host: target.host,
            port: target.port,
        },
        socket: SocketAddr::from((Ipv4Addr::LOCALHOST, target.port)),
    }))
}
