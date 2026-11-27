#[cfg(feature = "online-mode")]
mod auth;
#[cfg(feature = "online-mode")]
pub mod cache;
#[cfg(feature = "online-mode")]
pub mod certs;
#[cfg(all(feature = "online-mode", any(test, target_arch = "wasm32")))]
pub mod http;
#[cfg(feature = "online-mode")]
pub mod sessionserver;
#[cfg(feature = "online-mode")]
pub use auth::*;

pub mod game_profile;
pub mod redact;

#[cfg(all(feature = "online-mode", not(target_arch = "wasm32")))]
pub type HttpClient = reqwest::Client;
#[cfg(all(feature = "online-mode", target_arch = "wasm32"))]
pub type HttpClient = http::Client;

#[cfg(all(feature = "online-mode", not(target_arch = "wasm32")))]
pub type HttpError = reqwest::Error;
#[cfg(all(feature = "online-mode", target_arch = "wasm32"))]
pub type HttpError = http::Error;

#[cfg(all(feature = "online-mode", not(target_arch = "wasm32")))]
pub type HttpProxy = reqwest::Proxy;
#[cfg(all(feature = "online-mode", target_arch = "wasm32"))]
pub type HttpProxy = std::convert::Infallible;

#[cfg(all(feature = "online-mode", not(target_arch = "wasm32")))]
static HTTP_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

#[cfg(all(feature = "online-mode", not(target_arch = "wasm32")))]
pub fn set_http_client(client: reqwest::Client) {
    let _ = HTTP_CLIENT.set(client);
}

#[cfg(all(feature = "online-mode", not(target_arch = "wasm32")))]
pub(crate) fn http_client() -> Option<reqwest::Client> {
    HTTP_CLIENT.get().cloned()
}
