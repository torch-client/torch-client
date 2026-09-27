use std::sync::{Arc, Mutex, OnceLock};

use image::RgbaImage;

use crate::{log_debug, log_warn};

const SKIN_W: u32 = 64;
const SKIN_H: u32 = 64;
const LEGACY_SKIN_H: u32 = 32;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SkinRef {
    pub(crate) body: Option<Arc<str>>,
    pub(crate) cape: Option<Arc<str>>,
    pub(crate) elytra: Option<Arc<str>>,
    pub(crate) slim: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SkinState {
    pub(crate) refs: SkinRef,
    pub(crate) default_index: u8,
    pub(crate) parts: u8,
}

impl Default for SkinState {
    fn default() -> Self {
        Self {
            refs: SkinRef::default(),
            default_index: 0,
            parts: ALL_PARTS,
        }
    }
}

pub(crate) const ALL_PARTS: u8 = 0x7F;

impl SkinState {
    pub(crate) fn slim(&self) -> bool {
        if self.refs.body.is_some() {
            self.refs.slim
        } else {
            (self.default_index as usize) < crate::gui::tablist::DEFAULT_SKINS.len() / 2
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Body,
    Cape,
}

const BODY_KEY: &str = "SKIN";
const CAPE_KEY: &str = "CAPE";
const ELYTRA_KEY: &str = "ELYTRA";

pub(crate) fn parse_textures(property: &str) -> SkinRef {
    use base64::Engine as _;

    let Ok(json) = base64::engine::general_purpose::STANDARD.decode(property.trim()) else {
        return SkinRef::default();
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&json) else {
        return SkinRef::default();
    };
    let textures = &value["textures"];
    let body = &textures[BODY_KEY];

    SkinRef {
        body: texture_url(&body["url"]),
        cape: texture_url(&textures[CAPE_KEY]["url"]),
        elytra: texture_url(&textures[ELYTRA_KEY]["url"]),
        slim: body["metadata"]["model"].as_str() == Some("slim"),
    }
}

fn texture_url(value: &serde_json::Value) -> Option<Arc<str>> {
    let url = value.as_str()?;
    if url.len() > 512 || !is_allowed_texture_domain(url) {
        return None;
    }
    Some(Arc::from(url))
}

const ALLOWED_TEXTURE_DOMAINS: [&str; 2] = [".minecraft.net", ".mojang.com"];

fn is_allowed_texture_domain(url: &str) -> bool {
    let rest = match url.split_once("://") {
        Some(("http" | "https", rest)) => rest,
        _ => return false,
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or_default();
    let host = match host.rsplit_once(':') {
        Some((before, after)) if !after.contains(']') => before,
        _ => host,
    };
    ALLOWED_TEXTURE_DOMAINS
        .iter()
        .any(|domain| host.ends_with(domain))
}

static PROFILES: OnceLock<Mutex<std::collections::HashMap<u128, SkinRef>>> = OnceLock::new();

fn profiles() -> &'static Mutex<std::collections::HashMap<u128, SkinRef>> {
    PROFILES.get_or_init(Default::default)
}

pub(crate) fn remember(uuid: u128, refs: SkinRef) {
    if let Some(url) = &refs.body {
        request(url, Kind::Body);
    }
    if let Some(url) = &refs.cape {
        request(url, Kind::Cape);
    }
    if let Some(url) = &refs.elytra {
        request(url, Kind::Cape);
    }
    profiles().lock().unwrap().insert(uuid, refs);
}

pub(crate) fn of(uuid: u128) -> SkinRef {
    profiles()
        .lock()
        .unwrap()
        .get(&uuid)
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn reset() {
    profiles().lock().unwrap().clear();
}

static REQUESTED: OnceLock<Mutex<std::collections::HashSet<Arc<str>>>> = OnceLock::new();

static READY: Mutex<Vec<(Arc<str>, Kind, RgbaImage)>> = Mutex::new(Vec::new());

pub(crate) fn request(url: &Arc<str>, kind: Kind) {
    let first = REQUESTED
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .insert(url.clone());
    if first {
        submit(url.clone(), kind);
    }
}

pub(crate) fn take_ready() -> Vec<(Arc<str>, Kind, RgbaImage)> {
    let mut ready = READY.lock().unwrap();
    if ready.is_empty() {
        return Vec::new();
    }
    std::mem::take(&mut *ready)
}

#[cfg(not(target_arch = "wasm32"))]
fn submit(url: Arc<str>, kind: Kind) {
    use std::sync::mpsc::{Sender, channel};

    static QUEUE: OnceLock<Mutex<Sender<(Arc<str>, Kind)>>> = OnceLock::new();
    let queue = QUEUE.get_or_init(|| {
        let (tx, rx) = channel::<(Arc<str>, Kind)>();
        let spawned = std::thread::Builder::new()
            .name("skins".to_owned())
            .spawn(move || {
                while let Ok((url, kind)) = rx.recv() {
                    if let Some(image) = load(&url, kind) {
                        READY.lock().unwrap().push((url, kind, image));
                    }
                }
            });
        if let Err(e) = &spawned {
            log_warn!("skins", "no worker thread: {e}");
        }
        Mutex::new(tx)
    });
    let _ = queue.lock().unwrap().send((url, kind));
}

#[cfg(target_arch = "wasm32")]
fn submit(_url: Arc<str>, _kind: Kind) {}

#[cfg(not(target_arch = "wasm32"))]
fn load(url: &str, kind: Kind) -> Option<RgbaImage> {
    let path = cache_path(url);
    let cached = path.as_ref().and_then(|p| std::fs::read(p).ok());
    let bytes = match cached {
        Some(bytes) => bytes,
        None => {
            let fetched = fetch(url)?;
            if let Some(path) = &path
                && let Some(parent) = path.parent()
                && std::fs::create_dir_all(parent).is_ok()
            {
                let _ = std::fs::write(path, &fetched);
            }
            fetched
        }
    };

    let image = match image::load_from_memory(&bytes) {
        Ok(image) => image.to_rgba8(),
        Err(e) => {
            log_warn!("skins", "{url}: not an image ({e})");
            return None;
        }
    };
    match kind {
        Kind::Body => process_legacy_skin(image, url),
        Kind::Cape => Some(image),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch(url: &str) -> Option<Vec<u8>> {
    use crate::platform::http::Fetched;

    const MAX_BODY: u64 = 2 * 1024 * 1024;

    log_debug!("skins", "fetching {url}");
    match crate::platform::executor::block_on(crate::platform::http::get_bytes(
        url,
        0,
        &mut |done, _| done <= MAX_BODY,
    )) {
        Ok(Fetched::Body(body)) => Some(body),
        Ok(Fetched::Cancelled) => {
            log_warn!("skins", "{url}: body over {MAX_BODY} bytes, dropped");
            None
        }
        Err(e) => {
            log_warn!("skins", "{url}: {e}");
            None
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn cache_path(url: &str) -> Option<std::path::PathBuf> {
    let segment = url.rsplit('/').next()?;
    if segment.len() < 8
        || segment.len() > 128
        || !segment.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return None;
    }
    Some(
        crate::platform::storage::dir()
            .join("skins")
            .join(&segment[..2])
            .join(segment),
    )
}

fn process_legacy_skin(mut image: RgbaImage, url: &str) -> Option<RgbaImage> {
    let (w, h) = image.dimensions();
    if w != SKIN_W || (h != LEGACY_SKIN_H && h != SKIN_H) {
        log_warn!("skins", "{url}: discarding {w}x{h} skin");
        return None;
    }

    let legacy = h == LEGACY_SKIN_H;
    if legacy {
        let mut grown = RgbaImage::new(SKIN_W, SKIN_H);
        for y in 0..LEGACY_SKIN_H {
            for x in 0..SKIN_W {
                grown.put_pixel(x, y, *image.get_pixel(x, y));
            }
        }
        image = grown;

        for (x, y, dx, dy, w, h) in [
            (4, 16, 16, 32, 4, 4),
            (8, 16, 16, 32, 4, 4),
            (0, 20, 24, 32, 4, 12),
            (4, 20, 16, 32, 4, 12),
            (8, 20, 8, 32, 4, 12),
            (12, 20, 16, 32, 4, 12),
            (44, 16, -8, 32, 4, 4),
            (48, 16, -8, 32, 4, 4),
            (40, 20, 0, 32, 4, 12),
            (44, 20, -8, 32, 4, 12),
            (48, 20, -16, 32, 4, 12),
            (52, 20, -8, 32, 4, 12),
        ] {
            copy_rect(&mut image, x, y, dx, dy, w, h);
        }
    }

    set_no_alpha(&mut image, 0, 0, 32, 16);
    if legacy {
        notch_transparency_hack(&mut image, 32, 0, 64, 32);
    }
    set_no_alpha(&mut image, 0, 16, 64, 32);
    set_no_alpha(&mut image, 16, 48, 48, 64);
    Some(image)
}

fn copy_rect(image: &mut RgbaImage, x: i32, y: i32, dx: i32, dy: i32, w: i32, h: i32) {
    for row in 0..h {
        for col in 0..w {
            let from = ((x + col) as u32, (y + row) as u32);
            let to = ((x + dx + w - 1 - col) as u32, (y + dy + row) as u32);
            let pixel = *image.get_pixel(from.0, from.1);
            image.put_pixel(to.0, to.1, pixel);
        }
    }
}

fn set_no_alpha(image: &mut RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32) {
    for y in y0..y1 {
        for x in x0..x1 {
            image.get_pixel_mut(x, y).0[3] = 0xFF;
        }
    }
}

fn notch_transparency_hack(image: &mut RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32) {
    for y in y0..y1 {
        for x in x0..x1 {
            if image.get_pixel(x, y).0[3] < 128 {
                return;
            }
        }
    }
    for y in y0..y1 {
        for x in x0..x1 {
            image.get_pixel_mut(x, y).0[3] = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn property(json: &str) -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(json)
    }

    #[test]
    fn reads_both_urls_and_the_model() {
        let skin = parse_textures(&property(
            r#"{"textures":{
                "SKIN":{"url":"https://textures.minecraft.net/texture/aaaaaaaa","metadata":{"model":"slim"}},
                "CAPE":{"url":"https://textures.minecraft.net/texture/bbbbbbbb"}}}"#,
        ));
        assert_eq!(
            skin.body.as_deref(),
            Some("https://textures.minecraft.net/texture/aaaaaaaa")
        );
        assert_eq!(
            skin.cape.as_deref(),
            Some("https://textures.minecraft.net/texture/bbbbbbbb")
        );
        assert!(skin.slim);
    }

    #[test]
    fn defaults_to_wide_with_no_cape() {
        let skin = parse_textures(&property(
            r#"{"textures":{"SKIN":{"url":"https://textures.minecraft.net/texture/abcd"}}}"#,
        ));
        assert!(skin.body.is_some());
        assert!(!skin.slim);
        assert!(skin.cape.is_none());
    }

    #[test]
    fn survives_a_broken_property() {
        assert_eq!(parse_textures(&property("not json")), SkinRef::default());
        assert_eq!(parse_textures("not base64 either"), SkinRef::default());
        assert_eq!(parse_textures(""), SkinRef::default());
    }

    #[test]
    fn rejects_a_url_off_the_allowed_domains() {
        let skin = parse_textures(&property(
            r#"{"textures":{"SKIN":{"url":"https://evil.example/a.png"}}}"#,
        ));
        assert!(skin.body.is_none());
    }

    #[test]
    fn allows_mojangs_own_hosts() {
        for url in [
            "http://textures.minecraft.net/texture/abcd",
            "https://textures.minecraft.net/texture/abcd",
            "https://api.mojang.com/x",
            "https://textures.minecraft.net:443/texture/abcd",
        ] {
            assert!(is_allowed_texture_domain(url), "should allow {url}");
        }
    }

    #[test]
    fn rejects_hosts_that_only_look_like_mojangs() {
        for url in [
            "https://textures.minecraft.net@evil.example/a.png",
            "https://textures.minecraft.net:pass@evil.example/a.png",
            "https://evilminecraft.net/a.png",
            "https://minecraft.net.evil.example/a.png",
            "https://mojang.com.evil.example/a.png",
            "https://evil.example/minecraft.net/a.png",
            "file:///etc/passwd",
            "ftp://textures.minecraft.net/a.png",
            "javascript:alert(1)",
            "textures.minecraft.net/a.png",
            "",
        ] {
            assert!(!is_allowed_texture_domain(url), "should reject {url}");
        }
    }

    #[test]
    fn rejects_the_bare_apex_domains_as_vanilla_does() {
        assert!(!is_allowed_texture_domain("https://minecraft.net/x"));
        assert!(!is_allowed_texture_domain("https://mojang.com/x"));
    }

    #[test]
    fn discards_a_wrongly_sized_skin() {
        assert!(process_legacy_skin(RgbaImage::new(32, 32), "test").is_none());
        assert!(process_legacy_skin(RgbaImage::new(128, 128), "test").is_none());
        assert!(process_legacy_skin(RgbaImage::new(64, 64), "test").is_some());
    }

    #[test]
    fn mirrors_a_legacy_sheet() {
        let mut legacy = RgbaImage::new(64, 32);
        legacy.put_pixel(4, 20, Rgba([1, 2, 3, 255]));
        let out = process_legacy_skin(legacy, "test").unwrap();
        assert_eq!(out.dimensions(), (64, 64));
        assert_eq!(*out.get_pixel(23, 52), Rgba([1, 2, 3, 255]));
    }

    #[test]
    fn clears_a_fully_opaque_legacy_hat() {
        let mut legacy = RgbaImage::new(64, 32);
        for y in 0..32 {
            for x in 0..64 {
                legacy.put_pixel(x, y, Rgba([9, 9, 9, 255]));
            }
        }
        let out = process_legacy_skin(legacy, "test").unwrap();
        assert_eq!(out.get_pixel(40, 8).0[3], 0);
        assert_eq!(out.get_pixel(8, 8).0[3], 255);
    }
}
