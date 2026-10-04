#[cfg(feature = "online-mode")]
mod auth;
#[cfg(feature = "online-mode")]
pub mod cache;
#[cfg(feature = "online-mode")]
pub mod certs;
#[cfg(feature = "online-mode")]
pub mod sessionserver;
#[cfg(feature = "online-mode")]
pub use auth::*;

pub mod game_profile;

#[cfg(feature = "online-mode")]
static HTTP_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

#[cfg(feature = "online-mode")]
pub fn set_http_client(client: reqwest::Client) {
    let _ = HTTP_CLIENT.set(client);
}

#[cfg(feature = "online-mode")]
pub(crate) fn http_client() -> Option<reqwest::Client> {
    HTTP_CLIENT.get().cloned()
}
