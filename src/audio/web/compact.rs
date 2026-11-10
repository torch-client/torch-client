use wasm_bindgen_futures::JsFuture;

use super::js;

pub(crate) const BITRATE: u32 = 24_000;

pub(crate) async fn compact(key: &str, bytes: &[u8]) -> Option<Vec<u8>> {
    match JsFuture::from(js::mc_audio_compact(bytes, BITRATE)).await {
        Ok(value) if value.is_null() || value.is_undefined() => None,
        Ok(value) => Some(js_sys::Uint8Array::new(&value).to_vec()),
        Err(e) => {
            crate::log_warn!("audio", "{key} kept as it came: {e:?}");
            None
        }
    }
}
