use crate::platform::time::Instant;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::gui::atlas::{SERVER_ICON_PX, SERVER_ICON_SLOTS};
use crate::text::Span;

pub type IconPixels = std::sync::Arc<Vec<u8>>;

pub const ICON_BYTES: usize = (SERVER_ICON_PX * SERVER_ICON_PX * 4) as usize;

const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug)]
pub enum PingState {
    Pinging,
    Ok(Box<PingInfo>),
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct PingInfo {
    pub motd: Vec<Span>,
    pub online: i32,
    pub max: i32,
    #[allow(
        dead_code,
        reason = "the status reply's version string, decoded but not drawn yet"
    )]
    pub version: String,
    pub protocol: i32,
    pub latency_ms: u32,
}

impl PingInfo {
    pub fn incompatible(&self) -> bool {
        #[cfg(feature = "multiversion")]
        {
            !crate::protocol::joinable(self.protocol)
        }
        #[cfg(not(feature = "multiversion"))]
        {
            self.protocol != azalea_protocol::packets::PROTOCOL_VERSION
        }
    }

    pub fn bars(&self) -> u32 {
        match self.latency_ms {
            0..150 => 5,
            150..300 => 4,
            300..600 => 3,
            600..1000 => 2,
            _ => 1,
        }
    }
}

pub(crate) struct Status {
    pub motd: Vec<Span>,
    pub online: i32,
    pub max: i32,
    pub version: String,
    pub protocol: i32,
    pub latency_ms: u32,
    #[cfg_attr(
        target_arch = "wasm32",
        allow(
            dead_code,
            reason = "`--ping` is the only reader, and the web build has no command line"
        )
    )]
    pub sample: Vec<String>,
}

fn table() -> &'static Mutex<HashMap<String, PingState>> {
    static TABLE: OnceLock<Mutex<HashMap<String, PingState>>> = OnceLock::new();
    TABLE.get_or_init(Default::default)
}

#[derive(Default)]
struct Icons {
    slots: HashMap<String, usize>,
    pending: Vec<(usize, IconPixels)>,
}

fn icons() -> &'static Mutex<Icons> {
    static ICONS: OnceLock<Mutex<Icons>> = OnceLock::new();
    ICONS.get_or_init(Default::default)
}

fn note_icon(address: &str, pixels: IconPixels) {
    if pixels.len() != ICON_BYTES {
        return;
    }
    let mut icons = icons().lock().unwrap();
    let slot = match icons.slots.get(address) {
        Some(slot) => *slot,
        None => {
            let slot = icons.slots.len();
            if slot >= SERVER_ICON_SLOTS {
                return;
            }
            icons.slots.insert(address.to_string(), slot);
            slot
        }
    };
    icons.pending.push((slot, pixels));
}

pub fn icon_slot(address: &str) -> Option<usize> {
    icons().lock().unwrap().slots.get(address).copied()
}

pub fn take_pending_icons() -> Vec<(usize, IconPixels)> {
    std::mem::take(&mut icons().lock().unwrap().pending)
}

pub fn request(address: &str) {
    {
        let mut pending = table().lock().unwrap();
        if pending.contains_key(address) {
            return;
        }
        pending.insert(address.to_string(), PingState::Pinging);
    }
    let key = address.to_string();
    crate::platform::executor::spawn_detached(async move {
        let state = ping_once(&key).await;
        table().lock().unwrap().insert(key, state);
    });
}

async fn ping_once(address: &str) -> PingState {
    match status_once(address, true).await {
        Ok(status) => {
            #[cfg(feature = "multiversion")]
            crate::protocol::remember(address, status.protocol);
            PingState::Ok(Box::new(PingInfo {
                motd: status.motd,
                online: status.online,
                max: status.max,
                version: status.version,
                protocol: status.protocol,
                latency_ms: status.latency_ms,
            }))
        }
        Err(message) => PingState::Failed(message),
    }
}

pub(crate) async fn status_once(address: &str, icon: bool) -> Result<Status, String> {
    let started = Instant::now();
    let dial = crate::platform::address::normalize(address);

    #[cfg(any(feature = "eagler", target_arch = "wasm32"))]
    if crate::platform::address::is_websocket(&dial) {
        return match crate::platform::time::timeout(TIMEOUT, crate::eagler::query::status(&dial))
            .await
        {
            Ok(Ok(status)) => {
                if icon && let Some(pixels) = status.icon {
                    note_icon(address, IconPixels::new(pixels));
                }
                Ok(Status {
                    motd: status
                        .lines
                        .iter()
                        .map(|line| crate::text::parse_formatted(line))
                        .collect::<Vec<_>>()
                        .join(&crate::text::Span {
                            text: " ".to_string(),
                            style: Default::default(),
                        }),
                    online: status.online,
                    max: status.max,
                    version: status.version,
                    protocol: status.protocol,
                    latency_ms: elapsed_ms(started),
                    sample: Vec::new(),
                })
            }
            Ok(Err(e)) => Err(e.to_string()),
            Err(_) => Err(format!("No answer in {}s", TIMEOUT.as_secs())),
        };
    }

    let result =
        crate::platform::time::timeout(TIMEOUT, azalea::ping::ping_server(dial.clone())).await;
    match result {
        Ok(Ok(status)) => {
            if icon && let Some(pixels) = status.favicon.as_deref().and_then(decode_favicon) {
                note_icon(address, pixels);
            }
            Ok(Status {
                motd: crate::client::chat_text::to_spans(&status.description),
                online: status.players.online,
                max: status.players.max,
                version: status.version.name,
                protocol: status.version.protocol,
                latency_ms: elapsed_ms(started),
                sample: status
                    .players
                    .sample
                    .into_iter()
                    .map(|player| player.name)
                    .collect(),
            })
        }
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err(format!("No answer in {}s", TIMEOUT.as_secs())),
    }
}

fn decode_favicon(favicon: &str) -> Option<IconPixels> {
    use base64::Engine as _;

    let encoded = favicon.trim().strip_prefix("data:image/png;base64,")?;
    let packed: String = encoded.chars().filter(|c| !c.is_whitespace()).collect();
    let png = base64::engine::general_purpose::STANDARD
        .decode(packed)
        .ok()?;
    let image = image::load_from_memory(&png).ok()?.to_rgba8();
    if image.width() != SERVER_ICON_PX || image.height() != SERVER_ICON_PX {
        return None;
    }
    Some(IconPixels::new(image.into_raw()))
}

fn elapsed_ms(started: Instant) -> u32 {
    started.elapsed().as_millis().min(u32::MAX as u128) as u32
}

pub fn get(address: &str) -> Option<PingState> {
    table().lock().unwrap().get(address).cloned()
}

pub fn refresh() {
    table().lock().unwrap().clear();
}

pub fn forget(address: &str) {
    table().lock().unwrap().remove(address);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(latency_ms: u32, protocol: i32) -> PingInfo {
        PingInfo {
            motd: Vec::new(),
            online: 0,
            max: 20,
            version: "test".into(),
            protocol,
            latency_ms,
        }
    }

    #[test]
    fn bars_step_at_the_vanilla_thresholds() {
        assert_eq!(info(0, 0).bars(), 5);
        assert_eq!(info(149, 0).bars(), 5);
        assert_eq!(info(150, 0).bars(), 4);
        assert_eq!(info(299, 0).bars(), 4);
        assert_eq!(info(300, 0).bars(), 3);
        assert_eq!(info(599, 0).bars(), 3);
        assert_eq!(info(600, 0).bars(), 2);
        assert_eq!(info(1000, 0).bars(), 1);
        assert_eq!(info(u32::MAX, 0).bars(), 1);
    }

    #[test]
    fn a_matching_protocol_is_compatible() {
        let current = azalea_protocol::packets::PROTOCOL_VERSION;
        assert!(!info(10, current).incompatible());
        assert!(info(10, -1).incompatible());
    }
}
