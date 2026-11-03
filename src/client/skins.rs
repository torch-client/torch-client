use std::sync::{Arc, Mutex, OnceLock};

use image::RgbaImage;

use crate::util::global;
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

global! {
fn profiles() -> Mutex<std::collections::HashMap<u128, SkinRef>>
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
    #[cfg(eagler_skins)]
    if let Some(refs) =
        crate::gui::skinpick::worn_by(uuid).or_else(|| crate::eagler::skins::worn_by(uuid))
    {
        return refs;
    }
    profiles()
        .lock()
        .unwrap()
        .get(&uuid)
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn profile_skin(
    profile: &azalea_inventory::components::Profile,
) -> Option<crate::renderer::anim::ProfileSkin> {
    use crate::renderer::anim::ProfileSkin;

    if let Some(body) = &profile.skin_patch.body {
        return Some(ProfileSkin::Resource(Arc::from(body.id.path())));
    }
    Some(match resolvable(profile) {
        Resolvable::Static { textures, id } => {
            match textures.and_then(|t| parse_textures(t).body) {
                Some(url) => {
                    request(&url, Kind::Body);
                    ProfileSkin::Url(url)
                }
                None => ProfileSkin::Default(crate::client::tablist::default_skin(id)),
            }
        }
        Resolvable::Dynamic(key) => dynamic_skin(key),
    })
}

#[derive(Debug, PartialEq)]
enum Resolvable<'a> {
    Static { textures: Option<&'a str>, id: u128 },
    Dynamic(crate::renderer::anim::ProfileKey),
}

fn resolvable(profile: &azalea_inventory::components::Profile) -> Resolvable<'_> {
    use crate::renderer::anim::ProfileKey;
    use azalea_inventory::components::PartialOrFullProfile as P;

    match &*profile.unpack {
        P::Full(g) => Resolvable::Static {
            textures: g.properties.map.get("textures").map(|p| p.value.as_str()),
            id: g.uuid.as_u128(),
        },
        P::Partial(p) => match (&p.name, p.id) {
            (Some(name), None) if p.properties.map.is_empty() => {
                Resolvable::Dynamic(ProfileKey::Name(Arc::from(name.as_str())))
            }
            (None, Some(id)) if p.properties.map.is_empty() => {
                Resolvable::Dynamic(ProfileKey::Id(id.as_u128()))
            }
            (name, id) => Resolvable::Static {
                textures: p.properties.map.get("textures").map(|v| v.value.as_str()),
                id: id
                    .map(|id| id.as_u128())
                    .or_else(|| name.as_deref().map(offline_uuid))
                    .unwrap_or(0),
            },
        },
    }
}

fn offline_uuid(name: &str) -> u128 {
    azalea_crypto::offline::generate_uuid(name).as_u128()
}

fn is_valid_player_name(name: &str) -> bool {
    name.len() <= 16 && name.bytes().all(|c| c > 32 && c < 127)
}

fn dynamic_skin(key: crate::renderer::anim::ProfileKey) -> crate::renderer::anim::ProfileSkin {
    use crate::client::tablist;
    use crate::renderer::anim::{ProfileKey, ProfileSkin};

    let (listed, fallback, askable) = match &key {
        ProfileKey::Name(name) => (
            tablist::uuid_by_name(name),
            tablist::default_skin(offline_uuid(name)),
            is_valid_player_name(name) && !name.is_empty(),
        ),
        ProfileKey::Id(id) => {
            if let Some(url) = of(*id).body {
                return ProfileSkin::Url(url);
            }
            (
                tablist::has_player(*id).then_some(*id),
                tablist::default_skin(*id),
                uuid::Uuid::from_u128(*id).get_version_num() != 3,
            )
        }
    };
    if let Some(uuid) = listed {
        return match of(uuid).body {
            Some(url) => ProfileSkin::Url(url),
            None => ProfileSkin::Default(tablist::default_skin(uuid)),
        };
    }
    if !askable {
        return ProfileSkin::Default(fallback);
    }
    lookup::ask(key, fallback)
}

#[cfg(not(target_arch = "wasm32"))]
mod lookup {
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::sync::{Arc, Condvar, Mutex, OnceLock, PoisonError};
    use std::time::Duration;

    use crate::renderer::anim::{ProfileKey, ProfileSkin};

    const CAP: usize = 256;
    const SPACING: Duration = Duration::from_millis(500);
    const BACKOFF: Duration = Duration::from_secs(30);

    #[derive(Default)]
    struct State {
        done: HashMap<ProfileKey, Option<Arc<str>>>,
        pending: HashSet<ProfileKey>,
        queue: VecDeque<ProfileKey>,
    }

    static STATE: Mutex<Option<State>> = Mutex::new(None);
    static WAKE: Condvar = Condvar::new();

    fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
        f(STATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert_with(State::default))
    }

    pub(super) fn ask(key: ProfileKey, fallback: u8) -> ProfileSkin {
        let answer = with(|s| {
            if let Some(done) = s.done.get(&key) {
                return Some(done.clone());
            }
            if !s.pending.contains(&key) {
                if s.done.len() + s.pending.len() >= CAP {
                    return Some(None);
                }
                s.pending.insert(key.clone());
                s.queue.push_back(key.clone());
                start();
                WAKE.notify_one();
            }
            None
        });
        match answer {
            Some(Some(url)) => {
                super::request(&url, super::Kind::Body);
                ProfileSkin::Url(url)
            }
            Some(None) => ProfileSkin::Default(fallback),
            None => ProfileSkin::Lookup { key, fallback },
        }
    }

    pub(super) fn looked_up(key: &ProfileKey) -> Option<Arc<str>> {
        with(|s| s.done.get(key).cloned().flatten())
    }

    fn start() {
        static STARTED: OnceLock<()> = OnceLock::new();
        STARTED.get_or_init(|| {
            if let Err(e) = std::thread::Builder::new()
                .name("profile-lookup".into())
                .spawn(worker)
            {
                crate::log_warn!("skins", "no profile lookup thread: {e}");
            }
        });
    }

    fn worker() {
        loop {
            let key = {
                let mut guard = STATE.lock().unwrap_or_else(PoisonError::into_inner);
                loop {
                    if let Some(key) = guard.as_mut().and_then(|s| s.queue.pop_front()) {
                        break key;
                    }
                    guard = WAKE.wait(guard).unwrap_or_else(PoisonError::into_inner);
                }
            };
            match resolve(&key) {
                Some(url) => {
                    crate::log_debug!(
                        "skins",
                        "profile {key:?}: {}",
                        url.as_deref().unwrap_or("no skin")
                    );
                    with(|s| {
                        s.pending.remove(&key);
                        s.done.insert(key, url.clone());
                    });
                    if let Some(url) = url {
                        super::request(&url, super::Kind::Body);
                    }
                    std::thread::sleep(SPACING);
                }
                None => {
                    crate::log_debug!("skins", "profile {key:?}: rate limited, retrying");
                    with(|s| s.queue.push_back(key));
                    std::thread::sleep(BACKOFF);
                }
            }
        }
    }

    fn resolve(key: &ProfileKey) -> Option<Option<Arc<str>>> {
        let id = match key {
            ProfileKey::Id(id) => *id,
            ProfileKey::Name(name) => {
                let json = get(&format!(
                    "https://api.minecraftservices.com/minecraft/profile/lookup/name/{}",
                    encode(name)
                ))?;
                let id = json
                    .as_ref()
                    .and_then(|v| v["id"].as_str())
                    .and_then(|id| uuid::Uuid::parse_str(id).ok());
                let Some(id) = id else { return Some(None) };
                std::thread::sleep(SPACING);
                id.as_u128()
            }
        };
        let json = get(&format!(
            "https://sessionserver.mojang.com/session/minecraft/profile/{}",
            uuid::Uuid::from_u128(id).simple()
        ))?;
        let textures = json.as_ref().and_then(|v| {
            v["properties"]
                .as_array()?
                .iter()
                .find(|p| p["name"] == "textures")?["value"]
                .as_str()
                .map(super::parse_textures)
        });
        Some(textures.and_then(|t| t.body))
    }

    fn get(url: &str) -> Option<Option<serde_json::Value>> {
        match crate::platform::http::get_status_blocking(url) {
            Ok((429, _)) => None,
            Ok((200, body)) => Some(serde_json::from_slice(&body).ok()),
            Ok((status, _)) => {
                crate::log_debug!("skins", "{url}: HTTP {status}");
                Some(None)
            }
            Err(e) => {
                crate::log_debug!("skins", "{url}: {e}");
                Some(None)
            }
        }
    }

    fn encode(name: &str) -> String {
        let mut out = String::with_capacity(name.len());
        for b in name.bytes() {
            if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.' {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        out
    }
}

#[cfg(target_arch = "wasm32")]
mod lookup {
    use crate::renderer::anim::{ProfileKey, ProfileSkin};

    pub(super) fn ask(_key: ProfileKey, fallback: u8) -> ProfileSkin {
        ProfileSkin::Default(fallback)
    }

    pub(super) fn looked_up(_key: &ProfileKey) -> Option<std::sync::Arc<str>> {
        None
    }
}

pub(crate) fn looked_up(key: &crate::renderer::anim::ProfileKey) -> Option<Arc<str>> {
    lookup::looked_up(key)
}

pub(crate) fn reset() {
    profiles().lock().unwrap().clear();
    #[cfg(eagler_skins)]
    crate::eagler::skins::forget();
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

#[cfg(eagler_skins)]
pub(crate) fn provide(url: Arc<str>, image: RgbaImage) {
    READY.lock().unwrap().push((url, Kind::Body, image));
}

#[cfg(not(target_arch = "wasm32"))]
fn submit(url: Arc<str>, kind: Kind) {
    use std::sync::mpsc::{Sender, channel};

    static QUEUE: OnceLock<Sender<(Arc<str>, Kind)>> = OnceLock::new();
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
        tx
    });
    let _ = queue.send((url, kind));
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

pub(crate) fn process_legacy_skin(mut image: RgbaImage, url: &str) -> Option<RgbaImage> {
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

    fn partial(name: Option<&str>, id: Option<u128>) -> azalea_inventory::components::Profile {
        use azalea_inventory::components::{PartialOrFullProfile, PartialProfile, Profile};
        Profile {
            unpack: Box::new(PartialOrFullProfile::Partial(PartialProfile {
                name: name.map(str::to_owned),
                id: id.map(uuid::Uuid::from_u128),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    #[test]
    fn a_lone_name_or_id_is_dynamic() {
        use crate::renderer::anim::ProfileKey;
        assert_eq!(
            resolvable(&partial(Some("Notch"), None)),
            Resolvable::Dynamic(ProfileKey::Name(Arc::from("Notch")))
        );
        assert_eq!(
            resolvable(&partial(None, Some(7))),
            Resolvable::Dynamic(ProfileKey::Id(7))
        );
    }

    #[test]
    fn both_or_neither_is_static() {
        assert_eq!(
            resolvable(&partial(Some("Notch"), Some(7))),
            Resolvable::Static {
                textures: None,
                id: 7
            }
        );
        assert_eq!(
            resolvable(&partial(None, None)),
            Resolvable::Static {
                textures: None,
                id: 0
            }
        );
    }

    #[test]
    fn offline_uuid_is_version_3() {
        assert_eq!(
            uuid::Uuid::from_u128(offline_uuid("Steve")).get_version_num(),
            3
        );
    }

    #[test]
    fn validates_player_names_as_string_util_does() {
        assert!(is_valid_player_name("Notch"));
        assert!(is_valid_player_name("a_b-c.d/e"));
        assert!(is_valid_player_name("sixteen_chars_xx"));
        assert!(!is_valid_player_name("seventeen_chars_x"));
        assert!(!is_valid_player_name("has space"));
        assert!(!is_valid_player_name("tab\there"));
        assert!(!is_valid_player_name("caf\u{e9}"));
        assert!(!is_valid_player_name("del\u{7f}"));
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
