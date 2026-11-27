use std::{fmt, pin::Pin, sync::OnceLock};

pub struct Request {
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub struct RawResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub type TransportFuture = Pin<Box<dyn Future<Output = Result<RawResponse, String>> + Send>>;

pub type Transport = fn(Request) -> TransportFuture;

static TRANSPORT: OnceLock<Transport> = OnceLock::new();

pub fn set_transport(transport: Transport) {
    let _ = TRANSPORT.set(transport);
}

#[derive(Clone, Debug, Default)]
pub struct Client;

impl Client {
    pub fn new() -> Self {
        Self
    }

    pub fn get(&self, url: impl AsRef<str>) -> RequestBuilder {
        RequestBuilder::new("GET", url.as_ref())
    }

    pub fn post(&self, url: impl AsRef<str>) -> RequestBuilder {
        RequestBuilder::new("POST", url.as_ref())
    }
}

#[must_use]
pub struct RequestBuilder {
    request: Request,
    error: Option<Error>,
}

const CONTENT_TYPE: &str = "Content-Type";

impl RequestBuilder {
    fn new(method: &'static str, url: &str) -> Self {
        Self {
            request: Request {
                method,
                url: url.to_owned(),
                headers: Vec::new(),
                body: Vec::new(),
            },
            error: None,
        }
    }

    pub fn header(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        self.request
            .headers
            .push((name.as_ref().to_owned(), value.as_ref().to_owned()));
        self
    }

    pub fn bearer_auth(self, token: impl fmt::Display) -> Self {
        self.header("Authorization", format!("Bearer {token}"))
    }

    pub fn form(mut self, pairs: &[(&str, &str)]) -> Self {
        self.request.body = encode_form(pairs).into_bytes();
        self.default_content_type("application/x-www-form-urlencoded")
    }

    pub fn json<T: serde::Serialize + ?Sized>(mut self, value: &T) -> Self {
        match serde_json::to_vec(value) {
            Ok(body) => self.request.body = body,
            Err(e) => self.error = Some(Error::new(Kind::Encode(e.to_string()))),
        }
        self.default_content_type("application/json")
    }

    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.request.body = body.into();
        self
    }

    fn default_content_type(mut self, value: &str) -> Self {
        let present = self
            .request
            .headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(CONTENT_TYPE));
        if !present {
            self.request
                .headers
                .push((CONTENT_TYPE.to_owned(), value.to_owned()));
        }
        self
    }

    pub async fn send(self) -> Result<Response, Error> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let Some(transport) = TRANSPORT.get().copied() else {
            return Err(Error::new(Kind::NoTransport));
        };
        let raw = transport(self.request)
            .await
            .map_err(|reason| Error::new(Kind::Transport(reason)))?;
        Ok(Response {
            status: StatusCode(raw.status),
            headers: raw.headers,
            body: raw.body,
        })
    }
}

pub struct Response {
    status: StatusCode,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    pub fn error_for_status(self) -> Result<Response, Error> {
        if (400..600).contains(&self.status.0) {
            Err(Error::new(Kind::Status(self.status)))
        } else {
            Ok(self)
        }
    }

    pub async fn text(self) -> Result<String, Error> {
        Ok(match String::from_utf8(self.body) {
            Ok(text) => text,
            Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
        })
    }

    pub async fn json<T: serde::de::DeserializeOwned>(self) -> Result<T, Error> {
        serde_json::from_slice(&self.body).map_err(|e| Error::new(Kind::Decode(e.to_string())))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StatusCode(u16);

impl StatusCode {
    pub const OK: StatusCode = StatusCode(200);
    pub const NO_CONTENT: StatusCode = StatusCode(204);
    pub const FORBIDDEN: StatusCode = StatusCode(403);
    pub const TOO_MANY_REQUESTS: StatusCode = StatusCode(429);

    pub fn as_u16(&self) -> u16 {
        self.0
    }
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub struct Error {
    kind: Kind,
}

enum Kind {
    NoTransport,
    Transport(String),
    Status(StatusCode),
    Encode(String),
    Decode(String),
}

impl Error {
    fn new(kind: Kind) -> Self {
        Self { kind }
    }

    pub fn status(&self) -> Option<StatusCode> {
        match self.kind {
            Kind::Status(status) => Some(status),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            Kind::NoTransport => write!(f, "no HTTP transport has been set"),
            Kind::Transport(reason) => write!(f, "HTTP transport failed: {reason}"),
            Kind::Status(status) => write!(f, "HTTP status {status}"),
            Kind::Encode(reason) => write!(f, "couldn't encode the request body: {reason}"),
            Kind::Decode(reason) => write!(f, "couldn't decode the response body: {reason}"),
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "http::Error({self})")
    }
}

impl std::error::Error for Error {}

fn encode_form(pairs: &[(&str, &str)]) -> String {
    let mut out = String::new();
    for (i, (name, value)) in pairs.iter().enumerate() {
        if i > 0 {
            out.push('&');
        }
        encode_component(&mut out, name);
        out.push('=');
        encode_component(&mut out, value);
    }
    out
}

fn encode_component(out: &mut String, s: &str) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(HEX[usize::from(b >> 4)] as char);
                out.push(HEX[usize::from(b & 0x0f)] as char);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_encodes_reserved_bytes() {
        assert_eq!(
            encode_form(&[
                ("scope", "service::user.auth.xboxlive.com::MBI_SSL"),
                ("client_id", "00000000441cc96b"),
            ]),
            "scope=service%3A%3Auser.auth.xboxlive.com%3A%3AMBI_SSL&client_id=00000000441cc96b"
        );
        assert_eq!(
            encode_form(&[("k y", "a+b/c=d&e f*-._~")]),
            "k+y=a%2Bb%2Fc%3Dd%26e+f*-._%7E"
        );
        assert_eq!(
            encode_form(&[(
                "grant_type",
                "urn:ietf:params:oauth:grant-type:device_code"
            )]),
            "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code"
        );
    }

    #[test]
    fn form_encodes_non_ascii_as_utf8_bytes() {
        assert_eq!(encode_form(&[("n", "é")]), "n=%C3%A9");
        assert_eq!(encode_form(&[]), "");
        assert_eq!(encode_form(&[("", "")]), "=");
    }

    fn header<'a>(request: &'a Request, name: &str) -> Vec<&'a str> {
        request
            .headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
            .collect()
    }

    #[test]
    fn builder_sets_body_and_content_type() {
        let client = Client::new();

        let b = client
            .post("https://example.com/token")
            .form(&[("a", "1 2")]);
        assert_eq!(b.request.method, "POST");
        assert_eq!(b.request.url, "https://example.com/token");
        assert_eq!(b.request.body, b"a=1+2");
        assert_eq!(
            header(&b.request, "content-type"),
            ["application/x-www-form-urlencoded"]
        );

        let b = client
            .post("https://example.com/")
            .json(&serde_json::json!({ "k": "v" }));
        assert_eq!(b.request.body, br#"{"k":"v"}"#);
        assert_eq!(header(&b.request, "Content-Type"), ["application/json"]);

        let b = client
            .post("https://example.com/")
            .header("content-type", "text/plain")
            .json(&serde_json::json!(1));
        assert_eq!(header(&b.request, "Content-Type"), ["text/plain"]);

        let b = client
            .get("https://example.com/")
            .bearer_auth("tok")
            .header("Accept", "application/json")
            .body("raw");
        assert_eq!(b.request.method, "GET");
        assert_eq!(header(&b.request, "Authorization"), ["Bearer tok"]);
        assert_eq!(header(&b.request, "Accept"), ["application/json"]);
        assert!(header(&b.request, "Content-Type").is_empty());
        assert_eq!(b.request.body, b"raw");
    }

    #[test]
    fn response_status_and_bodies() {
        let ok = || Response {
            status: StatusCode(200),
            headers: Vec::new(),
            body: br#"{"n":5}"#.to_vec(),
        };
        assert_eq!(ok().status(), StatusCode::OK);
        assert!(ok().error_for_status().is_ok());

        let missing = Response {
            status: StatusCode(404),
            headers: Vec::new(),
            body: Vec::new(),
        };
        let err = missing.error_for_status().err().unwrap();
        assert_eq!(err.status(), Some(StatusCode(404)));
        assert_eq!(err.to_string(), "HTTP status 404");

        #[derive(serde::Deserialize)]
        struct N {
            n: u32,
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(async {
            assert_eq!(ok().json::<N>().await.unwrap().n, 5);
            assert_eq!(ok().text().await.unwrap(), r#"{"n":5}"#);
            assert!(ok().json::<Vec<u8>>().await.is_err());
        });
    }

    fn echo(request: Request) -> TransportFuture {
        Box::pin(async move {
            Ok(RawResponse {
                status: 201,
                headers: vec![("X-Method".to_owned(), request.method.to_owned())],
                body: request.body,
            })
        })
    }

    #[test]
    fn send_without_then_with_transport() {
        fn assert_send<T: Send>(_: &T) {}

        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(async {
            let future = Client::new().post("https://example.com/").send();
            assert_send(&future);
            let err = future.await.err().unwrap();
            assert_eq!(err.to_string(), "no HTTP transport has been set");

            set_transport(echo);
            let res = Client::new()
                .post("https://example.com/")
                .body("hi")
                .send()
                .await
                .unwrap();
            assert_eq!(res.status().as_u16(), 201);
            assert_eq!(res.headers(), [("X-Method".to_owned(), "POST".to_owned())]);
            assert_eq!(res.text().await.unwrap(), "hi");
        });
    }
}
