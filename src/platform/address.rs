pub(crate) const WEBSOCKET: [(&str, u16); 3] = [("wss://", 443), ("ws://", 80), (JAVA_TUNNEL, 443)];

pub(crate) const JAVA_TUNNEL: &str = "java://";

pub(crate) const EAGLER: bool = cfg!(any(feature = "eagler", target_arch = "wasm32"));

pub(crate) fn websocket(address: &str) -> Option<(&'static str, u16)> {
    let address = address.trim();
    WEBSOCKET
        .iter()
        .find(|(scheme, _)| address.starts_with(scheme))
        .map(|(scheme, port)| (*scheme, *port))
}

pub(crate) fn normalize(address: &str) -> String {
    let address = address.trim();
    if cfg!(target_arch = "wasm32") && !is_websocket(address) {
        return format!("wss://{address}");
    }
    address.to_string()
}

pub(crate) fn is_websocket(address: &str) -> bool {
    websocket(address).is_some()
}

#[cfg(any(feature = "eagler", target_arch = "wasm32", test))]
pub(crate) fn is_java_tunnel(address: &str) -> bool {
    websocket(address).is_some_and(|(scheme, _)| scheme == JAVA_TUNNEL)
}

pub(crate) fn check(address: &str) -> Result<(), String> {
    match websocket(address) {
        Some((scheme, _)) if !EAGLER => Err(format!(
            "`{scheme}` addresses go over a WebSocket, and this build cannot open one. Rebuild \
             with `cargo build --release --features eagler`."
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{EAGLER, check, is_java_tunnel, is_websocket, websocket};

    #[test]
    fn the_scheme_carries_the_web_default_port() {
        assert_eq!(websocket("wss://play.example"), Some(("wss://", 443)));
        assert_eq!(websocket("ws://play.example"), Some(("ws://", 80)));
        assert_eq!(websocket("java://relay.example"), Some(("java://", 443)));
    }

    #[test]
    fn only_the_java_scheme_is_a_tunnel() {
        assert!(is_java_tunnel(" java://relay.example/t/tcp/mc.example "));
        assert!(!is_java_tunnel("wss://relay.example/t/tcp/mc.example"));
        assert!(!is_java_tunnel("mc.example:25565"));
        assert_eq!(
            super::normalize("java://relay.example"),
            "java://relay.example"
        );
    }

    #[test]
    fn an_ordinary_address_is_not_a_websocket_one() {
        assert_eq!(websocket("play.example:25565"), None);
        assert!(!is_websocket("localhost"));
        assert!(check("play.example:25565").is_ok());
    }

    #[test]
    fn surrounding_space_does_not_hide_the_scheme() {
        assert!(is_websocket("  wss://play.example  "));
    }

    #[test]
    fn a_bare_address_is_a_websocket_one_only_where_there_are_no_sockets() {
        let bare = super::normalize("play.example:25565");
        if cfg!(target_arch = "wasm32") {
            assert_eq!(bare, "wss://play.example:25565");
        } else {
            assert_eq!(bare, "play.example:25565");
        }
        assert_eq!(super::normalize("wss://play.example"), "wss://play.example");
        assert_eq!(super::normalize(" ws://play.example "), "ws://play.example");
    }

    #[test]
    fn a_websocket_address_is_allowed_exactly_when_the_feature_is_on() {
        assert_eq!(check("wss://play.example").is_ok(), EAGLER);
    }
}
