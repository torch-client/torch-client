mod handshake;
pub(crate) mod query;
mod socket;
mod stream;

use std::net::{Ipv4Addr, SocketAddr};

use azalea_protocol::address::{ResolvedAddr, ServerAddr};

use crate::log_info;

struct Target {
    url: String,
    host: String,
    port: u16,
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
    Some(Target {
        url: address.to_string(),
        host: host.to_string(),
        port,
    })
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
    fn a_port_that_is_not_a_number_is_refused() {
        assert_eq!(split("wss://play.example:http"), None);
        assert_eq!(split("wss://"), None);
    }
}

pub(crate) async fn prepare(address: &str, username: &str) -> eyre::Result<Option<ResolvedAddr>> {
    let Some(target) = parse(address) else {
        return Ok(None);
    };
    log_info!("net", "eagler: opening a websocket to {}", target.url);
    let mut socket = socket::Socket::connect(&target.url).await?;
    let profile = handshake::run(&mut socket, username).await?;
    log_info!(
        "net",
        "eagler: the proxy logged us in as {} ({})",
        profile.name,
        profile.uuid
    );

    stream::prepare(socket, profile);

    Ok(Some(ResolvedAddr {
        server: ServerAddr {
            host: target.host,
            port: target.port,
        },
        socket: SocketAddr::from((Ipv4Addr::LOCALHOST, target.port)),
    }))
}
