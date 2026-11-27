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

pub(crate) fn transfer_target(host: &str, port: u32) -> Option<(String, u16)> {
    const MAX_HOST: usize = 254;
    const MAX_LABEL: usize = 63;

    let port = u16::try_from(port).ok()?;
    let bare = match host.strip_prefix('[') {
        Some(rest) => rest.strip_suffix(']')?,
        None => host,
    };
    if bare.contains(':') {
        let ip: std::net::Ipv6Addr = bare.parse().ok()?;
        return Some((ip.to_string(), port));
    }
    if bare.len() != host.len() || host.len() > MAX_HOST {
        return None;
    }
    let dotted = host.replace(['\u{3002}', '\u{FF0E}', '\u{FF61}'], ".");
    let name = dotted.strip_suffix('.').unwrap_or(&dotted);
    let mut labels = Vec::new();
    for label in name.split('.') {
        let label = if label.is_ascii() {
            label.to_owned()
        } else {
            punycode(&label.to_lowercase())
        };
        let known = |b: u8| b.is_ascii_alphanumeric() || b == b'-' || b == b'_';
        if label.is_empty() || label.len() > MAX_LABEL || !label.bytes().all(known) {
            return None;
        }
        labels.push(label);
    }
    let name = labels.join(".");
    (name.len() < MAX_HOST).then_some((name, port))
}

pub(crate) fn transfer_name(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn punycode(label: &str) -> String {
    const BASE: u32 = 36;
    const TMIN: u32 = 1;
    const TMAX: u32 = 26;
    const SKEW: u32 = 38;
    const DAMP: u32 = 700;

    let digit = |d: u32| {
        char::from(if d < 26 {
            b'a' + d as u8
        } else {
            b'0' + (d - 26) as u8
        })
    };
    let mut out: String = label.chars().filter(char::is_ascii).collect();
    let basic = out.len() as u32;
    let total = label.chars().count() as u32;
    if basic > 0 {
        out.push('-');
    }
    let (mut n, mut delta, mut bias, mut handled) = (0x80u32, 0u32, 72u32, basic);
    while handled < total {
        let Some(next) = label.chars().map(u32::from).filter(|c| *c >= n).min() else {
            break;
        };
        delta += (next - n) * (handled + 1);
        n = next;
        for c in label.chars().map(u32::from) {
            if c < n {
                delta += 1;
            }
            if c != n {
                continue;
            }
            let mut q = delta;
            let mut k = BASE;
            loop {
                let t = k.saturating_sub(bias).clamp(TMIN, TMAX);
                if q < t {
                    break;
                }
                out.push(digit(t + (q - t) % (BASE - t)));
                q = (q - t) / (BASE - t);
                k += BASE;
            }
            out.push(digit(q));
            delta = if handled == basic {
                delta / DAMP
            } else {
                delta / 2
            };
            handled += 1;
            delta += delta / handled;
            let mut steps = 0;
            while delta > (BASE - TMIN) * TMAX / 2 {
                delta /= BASE - TMIN;
                steps += BASE;
            }
            bias = steps + (BASE - TMIN + 1) * delta / (delta + SKEW);
            delta = 0;
        }
        delta += 1;
        n += 1;
    }
    format!("xn--{out}")
}

#[cfg(test)]
mod tests {
    use super::{
        EAGLER, check, is_java_tunnel, is_websocket, transfer_name, transfer_target, websocket,
    };

    #[test]
    fn a_transfer_target_is_a_host_and_a_port_and_nothing_else() {
        let host = |host: &str| transfer_target(host, 25565).map(|(host, _)| host);
        assert_eq!(host("play.example").as_deref(), Some("play.example"));
        assert_eq!(host("play.example.").as_deref(), Some("play.example"));
        assert_eq!(host("10.0.0.1").as_deref(), Some("10.0.0.1"));
        assert_eq!(host("::1").as_deref(), Some("::1"));
        assert_eq!(host("[2001:db8::1]").as_deref(), Some("2001:db8::1"));
        assert_eq!(
            host("B\u{fc}cher.example").as_deref(),
            Some("xn--bcher-kva.example")
        );
        assert_eq!(host("m\u{fc}nchen").as_deref(), Some("xn--mnchen-3ya"));
        for bad in [
            "",
            ".",
            "a..b",
            "wss://play.example",
            "java://play.example",
            "play.example:1",
            "play.example/x",
            "play example",
            "[play.example]",
            "[::1",
            "a\nb",
        ] {
            assert_eq!(host(bad), None, "{bad:?}");
        }
        assert_eq!(host(&"a".repeat(64)), None);
        assert_eq!(host(&"a.".repeat(200)), None);
    }

    #[test]
    fn a_transfer_port_fits_sixteen_bits() {
        assert_eq!(transfer_target("a", 65535), Some(("a".into(), 65535)));
        assert_eq!(transfer_target("a", 65536), None);
        assert_eq!(transfer_target("a", u32::MAX), None);
    }

    #[test]
    fn a_transfer_target_reads_back_as_an_address() {
        assert_eq!(transfer_name("play.example", 25565), "play.example:25565");
        assert_eq!(transfer_name("::1", 25566), "[::1]:25566");
    }

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
