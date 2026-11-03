use std::{
    net::IpAddr,
    sync::atomic::{AtomicUsize, Ordering},
};

use azalea_protocol::address::ServerAddr;

use crate::platform::storage;

const DEFAULT_PORT: u16 = 25565;

pub(crate) struct Proxy {
    pub(crate) name: &'static str,
    pub(super) transport: Transport,
}

pub(super) enum Transport {
    Relay(&'static str),
    Wisp(&'static str),
}

pub(crate) static ALL: [Proxy; 3] = [
    Proxy {
        name: "anura.pro",
        transport: Transport::Wisp("wss://anura.pro/"),
    },
    Proxy {
        name: "xpncvr",
        transport: Transport::Relay("wss://proxier.xpncvr.workers.dev/test/tcp/"),
    },
    Proxy {
        name: "mercurywork",
        transport: Transport::Wisp("wss://wisp.mercurywork.shop/"),
    },
];

const UNREAD: usize = usize::MAX;

static SELECTED: AtomicUsize = AtomicUsize::new(UNREAD);

pub(crate) fn selected() -> &'static Proxy {
    let mut index = SELECTED.load(Ordering::Relaxed);
    if index == UNREAD {
        index = saved();
        SELECTED.store(index, Ordering::Relaxed);
    }
    &ALL[index]
}

pub(crate) fn select(index: usize) {
    let Some(proxy) = ALL.get(index) else {
        return;
    };
    SELECTED.store(index, Ordering::Relaxed);
    storage::PROXY.store(proxy.name);
}

fn saved() -> usize {
    storage::PROXY
        .load()
        .and_then(|name| ALL.iter().position(|p| p.name == name.trim()))
        .unwrap_or(0)
}

#[derive(Debug, PartialEq)]
pub(super) struct Destination {
    pub(super) host: String,
    pub(super) port: Option<u16>,
}

impl Destination {
    pub(super) fn authority(&self) -> String {
        match self.port {
            Some(port) => format!("{}:{port}", self.host),
            None => self.host.clone(),
        }
    }
}

pub(super) async fn resolve(destination: &Destination) -> ServerAddr {
    let host = destination
        .host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(&destination.host);
    let direct = |port| ServerAddr {
        host: host.to_string(),
        port,
    };
    if let Some(port) = destination.port {
        return direct(port);
    }
    if host.parse::<IpAddr>().is_ok() {
        return direct(DEFAULT_PORT);
    }
    srv(host).await.unwrap_or_else(|| direct(DEFAULT_PORT))
}

#[cfg(not(target_arch = "wasm32"))]
async fn srv(host: &str) -> Option<ServerAddr> {
    let address = ServerAddr {
        host: host.to_string(),
        port: DEFAULT_PORT,
    };
    let mut found = azalea_protocol::resolve::resolve_srv_redirect(&address)
        .await
        .ok()?;
    if found.host.ends_with('.') {
        found.host.pop();
    }
    Some(found)
}

#[cfg(target_arch = "wasm32")]
async fn srv(host: &str) -> Option<ServerAddr> {
    if !host
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
    {
        return None;
    }
    let url = format!("https://cloudflare-dns.com/dns-query?name=_minecraft._tcp.{host}&type=SRV");
    let body = crate::platform::http::get_string_accepting(&url, "application/dns-json")
        .await
        .ok()?;
    best_srv(&body)
}

#[cfg(any(test, target_arch = "wasm32"))]
fn best_srv(json: &str) -> Option<ServerAddr> {
    const SRV: u64 = 33;

    let reply: serde_json::Value = serde_json::from_str(json).ok()?;
    let mut best: Option<(u32, ServerAddr)> = None;
    for answer in reply.get("Answer")?.as_array()? {
        if answer.get("type").and_then(|t| t.as_u64()) != Some(SRV) {
            continue;
        }
        let Some(data) = answer.get("data").and_then(|d| d.as_str()) else {
            continue;
        };
        let mut fields = data.split_whitespace();
        let priority = fields.next().and_then(|f| f.parse::<u32>().ok());
        let port = fields.nth(1).and_then(|f| f.parse::<u16>().ok());
        let target = fields.next().map(|t| t.trim_end_matches('.'));
        let (Some(priority), Some(port), Some(target)) = (priority, port, target) else {
            continue;
        };
        if port > 0 && !target.is_empty() && best.as_ref().is_none_or(|(p, _)| priority < *p) {
            let host = target.to_string();
            best = Some((priority, ServerAddr { host, port }));
        }
    }
    best.map(|(_, server)| server)
}

#[cfg(test)]
mod tests {
    use super::{ALL, Destination, Transport, best_srv};

    #[test]
    fn the_default_is_anura() {
        assert_eq!(ALL[0].name, "anura.pro");
        assert!(matches!(ALL[0].transport, Transport::Wisp(_)));
    }

    #[test]
    fn a_relay_gets_the_port_only_when_one_was_typed() {
        let at = |port| Destination {
            host: "mc.example".to_string(),
            port,
        };
        assert_eq!(at(None).authority(), "mc.example");
        assert_eq!(at(Some(25570)).authority(), "mc.example:25570");
    }

    #[test]
    fn the_lowest_priority_srv_answer_wins() {
        let reply = r#"{"Answer":[
            {"type":5,"data":"alias.example."},
            {"type":33,"data":"5 1 25570 far.example."},
            {"type":33,"data":"1 1 25565 java.example."},
            {"type":33,"data":"1 9 25566 tie.example."}
        ]}"#;
        let best = best_srv(reply).map(|s| (s.host, s.port));
        assert_eq!(best, Some(("java.example".to_string(), 25565)));
    }

    #[test]
    fn no_usable_srv_answer_is_none() {
        assert!(best_srv(r#"{"Status":0}"#).is_none());
        assert!(best_srv(r#"{"Answer":[{"type":33,"data":"junk"}]}"#).is_none());
        assert!(best_srv("not json").is_none());
    }
}
