const ALLOWED_HOSTS: [&str; 5] = [
    "login.live.com",
    "user.auth.xboxlive.com",
    "xsts.auth.xboxlive.com",
    "api.minecraftservices.com",
    "sessionserver.mojang.com",
];

const HTTPS_PORT: u16 = 443;

const MAX_HEAD_BYTES: usize = 64 * 1024;

const MAX_HEADERS: usize = 64;

const MAX_BODY_BYTES: usize = 1024 * 1024;

const OWN_HEADERS: [&str; 5] = [
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
    "accept-encoding",
];

fn allowed(host: &str) -> bool {
    ALLOWED_HOSTS.contains(&host)
}

#[derive(Debug, PartialEq)]
struct Url {
    host: String,
    port: u16,
    target: String,
}

impl Url {
    fn authority(&self) -> String {
        if self.port == HTTPS_PORT {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or("/")
    }
}

fn split_url(url: &str) -> Result<Url, String> {
    let Some(rest) = url.strip_prefix("https://") else {
        let scheme = url
            .split_once("://")
            .map(|(s, _)| s)
            .filter(|s| {
                !s.is_empty()
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
            })
            .unwrap_or("no scheme");
        return Err(format!(
            "only https:// addresses are sent through the proxy, and this one has {scheme}"
        ));
    };
    let rest = rest.split('#').next().unwrap_or(rest);
    let end = rest.find(['/', '?']).unwrap_or(rest.len());
    let (authority, target) = rest.split_at(end);
    if authority.contains(['@', '[', ']']) {
        return Err(format!("{authority} is not a plain host name"));
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => match port.parse::<u16>() {
            Ok(port) if port != 0 => (host, port),
            _ => {
                return Err(format!(
                    "{authority} has a port that is not a number from 1 to 65535"
                ));
            }
        },
        None => (authority, HTTPS_PORT),
    };
    if host.is_empty() {
        return Err("the address has no host".to_string());
    }
    let target = match target {
        "" => "/".to_string(),
        query if query.starts_with('?') => format!("/{query}"),
        path => path.to_string(),
    };
    if target.bytes().any(|b| b <= b' ' || b == 0x7F) {
        return Err(format!(
            "the address for {host} contains a space or a control character"
        ));
    }
    Ok(Url {
        host: host.to_ascii_lowercase(),
        port,
        target,
    })
}

fn is_token(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

fn request_bytes(
    method: &str,
    url: &Url,
    headers: &[(String, String)],
    body: &[u8],
    user_agent: &str,
) -> Result<Vec<u8>, String> {
    if !is_token(method) {
        return Err(format!("{method:?} is not an HTTP method"));
    }
    let mut head = format!(
        "{method} {} HTTP/1.1\r\nHost: {}\r\n",
        url.target,
        url.authority()
    );
    let mut has_agent = false;
    for (name, value) in headers {
        if !is_token(name) {
            return Err(format!("{name:?} is not a valid header name"));
        }
        let lower = name.to_ascii_lowercase();
        if OWN_HEADERS.contains(&lower.as_str()) {
            return Err(format!(
                "the {name} header is written by the transport, and the caller set it too"
            ));
        }
        if value.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0) {
            return Err(format!(
                "the value of the {name} header contains a line break"
            ));
        }
        has_agent |= lower == "user-agent";
        head.push_str(name);
        head.push_str(": ");
        head.push_str(value);
        head.push_str("\r\n");
    }
    if !has_agent {
        head.push_str("User-Agent: ");
        head.push_str(user_agent);
        head.push_str("\r\n");
    }
    if !body.is_empty() || matches!(method, "POST" | "PUT" | "PATCH") {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    head.push_str("Accept-Encoding: identity\r\nConnection: close\r\n\r\n");
    let mut wire = head.into_bytes();
    wire.extend_from_slice(body);
    Ok(wire)
}

#[derive(Debug, PartialEq)]
struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Ending {
    Open,
    Clean,
    Abrupt,
}

#[derive(Debug, PartialEq)]
enum Framing {
    Empty,
    Length(usize),
    Chunked,
    ToEnd,
}

struct Head {
    status: u16,
    headers: Vec<(String, String)>,
    framing: Framing,
    size: usize,
}

fn parse_response(buf: &[u8], ending: Ending) -> Result<Option<Response>, String> {
    let mut at = 0;
    let head = loop {
        match parse_head(&buf[at..])? {
            None if ending == Ending::Open => return Ok(None),
            None => return Err("the connection ended before the response headers did".into()),
            Some(head) if head.status == 101 => {
                return Err("it switched protocols, which nothing asked for".into());
            }
            Some(head) if (100..200).contains(&head.status) => at += head.size,
            Some(head) => break head,
        }
    };
    let body = &buf[at + head.size..];
    let decoded = match head.framing {
        Framing::Empty => Some(Vec::new()),
        Framing::Length(n) if body.len() >= n => Some(body[..n].to_vec()),
        Framing::Length(n) => {
            if ending != Ending::Open {
                return Err(format!(
                    "the connection ended {} bytes into a {n} byte body",
                    body.len()
                ));
            }
            None
        }
        Framing::Chunked => match dechunk(body)? {
            Some(decoded) => Some(decoded),
            None if ending != Ending::Open => {
                return Err("the connection ended before the last chunk of the body".into());
            }
            None if body.len() > MAX_BODY_BYTES => return Err(too_large()),
            None => None,
        },
        Framing::ToEnd => match ending {
            Ending::Open if body.len() > MAX_BODY_BYTES => return Err(too_large()),
            Ending::Open => None,
            Ending::Clean => Some(body.to_vec()),
            Ending::Abrupt => {
                return Err(
                    "the connection ended without a TLS close_notify, and the body has \
                     no length to check it against, so it may have been cut short"
                        .into(),
                );
            }
        },
    };
    Ok(decoded.map(|body| Response {
        status: head.status,
        headers: head.headers,
        body,
    }))
}

fn too_large() -> String {
    format!("its body is larger than {} KiB", MAX_BODY_BYTES / 1024)
}

fn parse_head(buf: &[u8]) -> Result<Option<Head>, String> {
    let mut slots = [httparse::EMPTY_HEADER; MAX_HEADERS];
    let mut response = httparse::Response::new(&mut slots);
    let size = match response.parse(buf) {
        Ok(httparse::Status::Complete(size)) => size,
        Ok(httparse::Status::Partial) if buf.len() > MAX_HEAD_BYTES => {
            return Err(format!(
                "its headers ran past {} KiB",
                MAX_HEAD_BYTES / 1024
            ));
        }
        Ok(httparse::Status::Partial) => return Ok(None),
        Err(httparse::Error::TooManyHeaders) => {
            return Err(format!("it sent more than {MAX_HEADERS} headers"));
        }
        Err(e) => return Err(format!("its headers are not valid HTTP/1.1 ({e})")),
    };
    if size > MAX_HEAD_BYTES {
        return Err(format!(
            "its headers ran past {} KiB",
            MAX_HEAD_BYTES / 1024
        ));
    }
    let status = response.code.unwrap_or(0);
    let headers: Vec<(String, String)> = response
        .headers
        .iter()
        .map(|h| {
            (
                h.name.to_string(),
                String::from_utf8_lossy(h.value).into_owned(),
            )
        })
        .collect();
    let framing = framing(status, &headers)?;
    Ok(Some(Head {
        status,
        headers,
        framing,
        size,
    }))
}

fn framing(status: u16, headers: &[(String, String)]) -> Result<Framing, String> {
    if (100..200).contains(&status) || status == 204 || status == 304 {
        return Ok(Framing::Empty);
    }
    let mut length = None;
    let mut chunked = false;
    for (name, value) in headers {
        let value = value.trim();
        if name.eq_ignore_ascii_case("transfer-encoding") {
            if !value
                .split(',')
                .all(|coding| coding.trim().eq_ignore_ascii_case("chunked"))
            {
                return Err(format!(
                    "it used a transfer coding this client does not read ({value})"
                ));
            }
            chunked = true;
        } else if name.eq_ignore_ascii_case("content-length") {
            let n: usize = value
                .parse()
                .map_err(|_| format!("its Content-Length is not a number ({value})"))?;
            if length.is_some_and(|l| l != n) {
                return Err("it sent two different Content-Length headers".into());
            }
            length = Some(n);
        } else if name.eq_ignore_ascii_case("content-encoding")
            && !value.is_empty()
            && !value.eq_ignore_ascii_case("identity")
        {
            return Err(format!(
                "it compressed the body ({value}) although the request asked for none"
            ));
        }
    }
    if chunked {
        return Ok(Framing::Chunked);
    }
    match length {
        Some(n) if n > MAX_BODY_BYTES => Err(too_large()),
        Some(n) => Ok(Framing::Length(n)),
        None => Ok(Framing::ToEnd),
    }
}

fn dechunk(body: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let mut out = Vec::new();
    let mut at = 0;
    loop {
        let (line, size) = match httparse::parse_chunk_size(&body[at..]) {
            Ok(httparse::Status::Complete(parsed)) => parsed,
            Ok(httparse::Status::Partial) => return Ok(None),
            Err(_) => return Err("a chunk size in its body is not valid".into()),
        };
        at += line;
        if size == 0 {
            loop {
                let Some(end) = body[at..].windows(2).position(|w| w == b"\r\n") else {
                    return Ok(None);
                };
                at += end + 2;
                if end == 0 {
                    return Ok(Some(out));
                }
            }
        }
        let size = usize::try_from(size)
            .ok()
            .filter(|size| out.len().saturating_add(*size) <= MAX_BODY_BYTES)
            .ok_or_else(too_large)?;
        if body.len() - at < size + 2 {
            return Ok(None);
        }
        if &body[at + size..at + size + 2] != b"\r\n" {
            return Err("a chunk in its body runs past its stated size".into());
        }
        out.extend_from_slice(&body[at..at + size]);
        at += size + 2;
    }
}

#[cfg(all(feature = "microsoft_web", target_arch = "wasm32"))]
pub(crate) use web::send;

#[cfg(all(feature = "microsoft_web", target_arch = "wasm32"))]
mod web {
    use std::{
        sync::{Arc, OnceLock},
        time::Duration,
    };

    use azalea_auth::http::{RawResponse, Request, TransportFuture};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_rustls::rustls::{
        self, ClientConfig, RootCertStore,
        pki_types::{ServerName, UnixTime},
        time_provider::TimeProvider,
    };

    use super::{Ending, Response, Url, allowed, parse_response, request_bytes, split_url};
    use crate::eagler::proxy::{self, Proxy};
    use crate::platform::time::{Instant, SystemTime, UNIX_EPOCH};
    use crate::{log_info, log_warn};

    const REQUEST_TIMEOUT: Duration = crate::eagler::handshake::STEP_TIMEOUT;

    const READ_CHUNK: usize = 16 * 1024;

    pub(crate) fn send(request: Request) -> TransportFuture {
        let (reply, answer) = tokio::sync::oneshot::channel();
        crate::platform::executor::spawn_detached(async move {
            let _ = reply.send(perform(request).await);
        });
        Box::pin(async move {
            answer.await.unwrap_or_else(|_| {
                Err("the sign-in request stopped before it had an answer".to_string())
            })
        })
    }

    async fn perform(request: Request) -> Result<RawResponse, String> {
        let url = split_url(&request.url)?;
        if !allowed(&url.host) {
            return Err(format!(
                "{} is not a Microsoft or Mojang sign-in host, and nothing else is sent through \
                 the proxy this way",
                url.host
            ));
        }
        let wire = request_bytes(
            request.method,
            &url,
            &request.headers,
            &request.body,
            crate::platform::http::USER_AGENT,
        )?;
        let via = proxy::selected();
        let started = Instant::now();
        let outcome = match crate::platform::time::timeout(
            REQUEST_TIMEOUT,
            exchange(via, &url, wire),
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(()) => Err(format!(
                "{} did not answer through the {} proxy within {} seconds",
                url.host,
                via.name,
                REQUEST_TIMEOUT.as_secs()
            )),
        };
        let ms = started.elapsed().as_millis();
        match &outcome {
            Ok(response) => log_info!(
                "auth",
                "{} https://{}{} -> {} in {ms} ms through {}",
                request.method,
                url.authority(),
                url.path(),
                response.status,
                via.name
            ),
            Err(e) => log_warn!(
                "auth",
                "{} https://{}{} failed after {ms} ms: {e}",
                request.method,
                url.authority(),
                url.path()
            ),
        }
        outcome.map(|response| RawResponse {
            status: response.status,
            headers: response.headers,
            body: response.body,
        })
    }

    async fn exchange(via: Proxy, url: &Url, wire: Vec<u8>) -> Result<Response, String> {
        let host = url.host.as_str();
        let through = format!("through the {} proxy", via.name);
        let bridge = crate::eagler::open_tcp(via, host, url.port)
            .await
            .map_err(|e| format!("could not reach {host} {through}: {e}"))?;
        let name = ServerName::try_from(url.host.clone())
            .map_err(|_| format!("{host} is not a name a certificate can be checked against"))?;
        let mut tls = tokio_rustls::TlsConnector::from(config()?)
            .connect(name, bridge)
            .await
            .map_err(|e| format!("the secure connection to {host} {through} failed: {e}"))?;
        let broke = |e: std::io::Error| {
            format!("the connection to {host} {through} broke while sending the request: {e}")
        };
        tls.write_all(&wire).await.map_err(broke)?;
        tls.flush().await.map_err(broke)?;
        drop(wire);

        let mut buf = Vec::new();
        let mut chunk = vec![0u8; READ_CHUNK];
        loop {
            let ending = match tls.read(&mut chunk).await {
                Ok(0) => Ending::Clean,
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    Ending::Open
                }
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ending::Abrupt,
                Err(e) => {
                    return Err(format!(
                        "the connection to {host} {through} broke while reading the answer: {e}"
                    ));
                }
            };
            match parse_response(&buf, ending) {
                Ok(Some(response)) => return Ok(response),
                Ok(None) if ending == Ending::Open => {}
                Ok(None) => return Err(format!("{host} {through} ended the answer early")),
                Err(why) => {
                    return Err(format!(
                        "{host} sent an answer this client could not read: {why}"
                    ));
                }
            }
        }
    }

    fn config() -> Result<Arc<ClientConfig>, String> {
        static CONFIG: OnceLock<Result<Arc<ClientConfig>, String>> = OnceLock::new();
        CONFIG
            .get_or_init(|| {
                let roots: RootCertStore = webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect();
                let provider = Arc::new(rustls::crypto::ring::default_provider());
                let mut config = ClientConfig::builder_with_details(provider, Arc::new(PageClock))
                    .with_safe_default_protocol_versions()
                    .map_err(|e| format!("no TLS configuration could be built: {e}"))?
                    .with_root_certificates(roots)
                    .with_no_client_auth();
                config.alpn_protocols = vec![b"http/1.1".to_vec()];
                Ok(Arc::new(config))
            })
            .clone()
    }

    #[derive(Debug)]
    struct PageClock;

    impl TimeProvider for PageClock {
        fn current_time(&self) -> Option<UnixTime> {
            let since = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
            Some(UnixTime::since_unix_epoch(since))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AGENT: &str = "test-agent/1";

    fn url(text: &str) -> Url {
        split_url(text).unwrap()
    }

    #[test]
    fn a_url_splits_into_host_port_and_target() {
        assert_eq!(
            url("https://login.live.com/oauth20_connect.srf"),
            Url {
                host: "login.live.com".into(),
                port: 443,
                target: "/oauth20_connect.srf".into(),
            }
        );
        let with_query = url("https://Api.MinecraftServices.com:8443/a/b?x=1&y=2#frag");
        assert_eq!(with_query.host, "api.minecraftservices.com");
        assert_eq!(with_query.port, 8443);
        assert_eq!(with_query.target, "/a/b?x=1&y=2");
        assert_eq!(with_query.path(), "/a/b");
        assert_eq!(with_query.authority(), "api.minecraftservices.com:8443");
        assert_eq!(url("https://login.live.com").target, "/");
        assert_eq!(url("https://login.live.com?a=b").target, "/?a=b");
    }

    #[test]
    fn only_https_with_a_plain_host_is_accepted() {
        assert!(split_url("http://login.live.com/").is_err());
        assert!(split_url("login.live.com/").is_err());
        assert!(split_url("https:///path").is_err());
        assert!(split_url("https://user@login.live.com/").is_err());
        assert!(split_url("https://[::1]/").is_err());
        assert!(split_url("https://login.live.com:http/").is_err());
        assert!(split_url("https://login.live.com:0/").is_err());
        assert!(split_url("https://login.live.com/a b").is_err());
    }

    #[test]
    fn an_error_does_not_repeat_the_query() {
        let e = split_url("http://login.live.com/x?code=secret").unwrap_err();
        assert!(!e.contains("secret"), "{e}");
    }

    #[test]
    fn the_allowlist_is_exact() {
        for host in ALLOWED_HOSTS {
            assert!(allowed(host));
            assert!(allowed(
                &url(&format!("https://{}/", host.to_uppercase())).host
            ));
        }
        assert!(!allowed("example.com"));
        assert!(!allowed("login.live.com.example.com"));
        assert!(!allowed("evil-login.live.com"));
    }

    #[test]
    fn a_post_is_serialised_with_the_transports_headers() {
        let headers = vec![("Content-Type".to_string(), "application/json".to_string())];
        let wire = request_bytes(
            "POST",
            &url("https://user.auth.xboxlive.com/user/authenticate"),
            &headers,
            b"{}",
            AGENT,
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(wire).unwrap(),
            "POST /user/authenticate HTTP/1.1\r\n\
             Host: user.auth.xboxlive.com\r\n\
             Content-Type: application/json\r\n\
             User-Agent: test-agent/1\r\n\
             Content-Length: 2\r\n\
             Accept-Encoding: identity\r\n\
             Connection: close\r\n\
             \r\n\
             {}"
        );
    }

    #[test]
    fn an_empty_post_says_so_and_a_get_says_nothing() {
        let target = url("https://sessionserver.mojang.com:444/session/minecraft/join");
        let post =
            String::from_utf8(request_bytes("POST", &target, &[], b"", AGENT).unwrap()).unwrap();
        assert!(post.contains("\r\nHost: sessionserver.mojang.com:444\r\n"));
        assert!(post.contains("\r\nContent-Length: 0\r\n"));
        let get =
            String::from_utf8(request_bytes("GET", &target, &[], b"", AGENT).unwrap()).unwrap();
        assert!(!get.contains("Content-Length"));
    }

    #[test]
    fn the_callers_user_agent_wins_and_owned_headers_are_refused() {
        let target = url("https://login.live.com/");
        let agent = vec![("user-agent".to_string(), "mine".to_string())];
        let wire =
            String::from_utf8(request_bytes("GET", &target, &agent, b"", AGENT).unwrap()).unwrap();
        assert!(wire.contains("user-agent: mine\r\n"));
        assert!(!wire.contains(AGENT));

        let host = vec![("Host".to_string(), "elsewhere".to_string())];
        assert!(request_bytes("GET", &target, &host, b"", AGENT).is_err());
        let split = vec![("Authorization".to_string(), "Bearer a\r\nX: y".to_string())];
        let e = request_bytes("GET", &target, &split, b"", AGENT).unwrap_err();
        assert!(!e.contains("Bearer"), "{e}");
    }

    #[test]
    fn a_content_length_body_is_read_to_its_length() {
        let wire =
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 5\r\n\r\nhello";
        assert_eq!(
            parse_response(&wire[..wire.len() - 2], Ending::Open),
            Ok(None)
        );
        let response = parse_response(wire, Ending::Open).unwrap().unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"hello");
        assert_eq!(
            response.headers[0],
            ("Content-Type".to_string(), "application/json".to_string())
        );
        assert!(parse_response(wire, Ending::Abrupt).unwrap().is_some());
    }

    #[test]
    fn a_chunked_body_with_an_extension_and_a_trailer_is_decoded() {
        let wire = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n\
                     4;name=value\r\nWiki\r\n\
                     5\r\npedia\r\n\
                     0\r\nExpires: never\r\n\r\n";
        let response = parse_response(wire, Ending::Open).unwrap().unwrap();
        assert_eq!(response.body, b"Wikipedia");
        for cut in 40..wire.len() {
            assert_eq!(
                parse_response(&wire[..cut], Ending::Open),
                Ok(None),
                "cut at {cut}"
            );
        }
        assert!(parse_response(&wire[..wire.len() - 2], Ending::Clean).is_err());
    }

    #[test]
    fn a_body_with_no_length_runs_to_a_clean_end() {
        let wire = b"HTTP/1.1 200 OK\r\nServer: x\r\n\r\nall of it";
        assert_eq!(parse_response(wire, Ending::Open), Ok(None));
        assert_eq!(
            parse_response(wire, Ending::Clean).unwrap().unwrap().body,
            b"all of it"
        );
        assert!(parse_response(wire, Ending::Abrupt).is_err());
    }

    #[test]
    fn no_content_has_no_body_and_an_interim_response_is_skipped() {
        let wire = b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n";
        let response = parse_response(wire, Ending::Open).unwrap().unwrap();
        assert_eq!((response.status, response.body.len()), (204, 0));
        let wire = b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 204 No Content\r\n\r\n";
        assert_eq!(
            parse_response(wire, Ending::Open).unwrap().unwrap().status,
            204
        );
    }

    #[test]
    fn a_truncated_body_is_an_error_once_the_stream_ends() {
        let wire = b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nshort";
        assert_eq!(parse_response(wire, Ending::Open), Ok(None));
        assert!(parse_response(wire, Ending::Clean).is_err());
        assert!(parse_response(wire, Ending::Abrupt).is_err());
        assert!(parse_response(b"HTTP/1.1 200 OK\r\nContent-", Ending::Clean).is_err());
    }

    #[test]
    fn oversized_headers_and_bodies_are_refused() {
        let mut wire = b"HTTP/1.1 200 OK\r\nX-Long: ".to_vec();
        wire.extend(std::iter::repeat_n(b'a', MAX_HEAD_BYTES));
        assert!(parse_response(&wire, Ending::Open).is_err());

        let mut many = b"HTTP/1.1 200 OK\r\n".to_vec();
        for i in 0..=MAX_HEADERS {
            many.extend_from_slice(format!("X-{i}: v\r\n").as_bytes());
        }
        many.extend_from_slice(b"\r\n");
        assert!(parse_response(&many, Ending::Open).is_err());

        let declared = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY_BYTES + 1
        );
        assert!(parse_response(declared.as_bytes(), Ending::Open).is_err());
    }

    #[test]
    fn a_compressed_or_conflicting_body_is_refused() {
        let gzip = b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 1\r\n\r\nx";
        assert!(parse_response(gzip, Ending::Open).is_err());
        let two = b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\nxy";
        assert!(parse_response(two, Ending::Open).is_err());
        let coded = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip, chunked\r\n\r\n0\r\n\r\n";
        assert!(parse_response(coded, Ending::Open).is_err());
    }
}
