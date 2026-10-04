pub(crate) const USER_AGENT: &str = concat!("torch-client/", env!("CARGO_PKG_VERSION"));

pub(crate) enum Fetched {
    Body(Vec<u8>),
    Cancelled,
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::{Fetched, USER_AGENT};
    use std::io::Read;
    use std::sync::OnceLock;

    const CHUNK: usize = 64 * 1024;

    fn client() -> &'static reqwest::blocking::Client {
        static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
        CLIENT.get_or_init(|| {
            crate::install_crypto_provider();
            let builder = reqwest::blocking::Client::builder().user_agent(USER_AGENT);
            #[cfg(target_os = "android")]
            let builder = match crate::android_tls_config() {
                Some(tls) => builder.tls_backend_preconfigured(tls),
                None => builder,
            };
            builder
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new())
        })
    }

    pub(super) async fn get_string(url: &str) -> Result<String, String> {
        client()
            .get(url)
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.text())
            .map_err(|e| e.to_string())
    }

    pub(super) async fn get_bytes(
        url: &str,
        expected: u64,
        progress: &mut dyn FnMut(u64, u64) -> bool,
    ) -> Result<Fetched, String> {
        let mut resp = client()
            .get(url)
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(|e| e.to_string())?;
        let total = if expected > 0 {
            expected
        } else {
            resp.content_length().unwrap_or(0)
        };

        let mut out = Vec::with_capacity(total as usize);
        let mut buf = vec![0u8; CHUNK];
        loop {
            let n = resp.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
            if !progress(out.len() as u64, total) {
                return Ok(Fetched::Cancelled);
            }
        }
        Ok(Fetched::Body(out))
    }

    #[cfg(feature = "audio")]
    pub(super) fn get_bytes_blocking(url: &str) -> Result<Vec<u8>, String> {
        client()
            .get(url)
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.bytes())
            .map(|b| b.to_vec())
            .map_err(|e| e.to_string())
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::Fetched;
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;

    fn err(what: &str, e: JsValue) -> String {
        format!("{what}: {e:?}")
    }

    async fn response(url: &str) -> Result<web_sys::Response, String> {
        let window = web_sys::window().ok_or("no window")?;
        let value = JsFuture::from(window.fetch_with_str(url))
            .await
            .map_err(|e| err("fetch failed", e))?;
        let resp: web_sys::Response = value.dyn_into().map_err(|e| err("not a response", e))?;
        if !resp.ok() {
            return Err(format!("http {}", resp.status()));
        }
        Ok(resp)
    }

    pub(super) async fn get_string(url: &str) -> Result<String, String> {
        let resp = response(url).await?;
        let text = JsFuture::from(resp.text().map_err(|e| err("no body", e))?)
            .await
            .map_err(|e| err("body failed", e))?;
        text.as_string().ok_or_else(|| "body is not text".into())
    }

    pub(super) async fn get_bytes(
        url: &str,
        expected: u64,
        progress: &mut dyn FnMut(u64, u64) -> bool,
    ) -> Result<Fetched, String> {
        let resp = response(url).await?;
        let total = if expected > 0 {
            expected
        } else {
            resp.headers()
                .get("content-length")
                .ok()
                .flatten()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0)
        };

        let body = resp.body().ok_or("response has no body")?;
        let reader: web_sys::ReadableStreamDefaultReader = body
            .get_reader()
            .dyn_into()
            .map_err(|e| err("not a byte reader", e.into()))?;

        let mut out: Vec<u8> = Vec::with_capacity(total as usize);
        loop {
            let chunk = JsFuture::from(reader.read())
                .await
                .map_err(|e| err("read failed", e))?;
            let done = js_sys::Reflect::get(&chunk, &JsValue::from_str("done"))
                .map(|v| v.as_bool().unwrap_or(false))
                .unwrap_or(false);
            if done {
                break;
            }
            let value = js_sys::Reflect::get(&chunk, &JsValue::from_str("value"))
                .map_err(|e| err("chunk has no value", e))?;
            let array = js_sys::Uint8Array::new(&value);
            let at = out.len();
            out.resize(at + array.length() as usize, 0);
            array.copy_to(&mut out[at..]);
            if !progress(out.len() as u64, total) {
                let _ = reader.cancel();
                return Ok(Fetched::Cancelled);
            }
        }
        Ok(Fetched::Body(out))
    }
}

pub(crate) async fn get_string(url: &str) -> Result<String, String> {
    imp::get_string(url).await
}

pub(crate) async fn get_bytes(
    url: &str,
    expected: u64,
    progress: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<Fetched, String> {
    imp::get_bytes(url, expected, progress).await
}

#[cfg(all(not(target_arch = "wasm32"), feature = "audio"))]
pub(crate) fn get_bytes_blocking(url: &str) -> Result<Vec<u8>, String> {
    imp::get_bytes_blocking(url)
}
