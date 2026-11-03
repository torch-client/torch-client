use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use image::RgbaImage;
use tokio::sync::mpsc::UnboundedSender;

use crate::client::skins::SkinRef;
use crate::util::wire::{Reader, write_varint};
use crate::{log_debug, log_info};

pub(crate) const V4_CHANNEL: &str = "eagler:1-8";

const V5_GET_OTHER_SKIN: u8 = 0x01;
const V5_SKIN_PRESET: u8 = 0x01;
const V5_SKIN_CUSTOM: u8 = 0x02;
const V4_GET_OTHER_SKIN: u8 = 0x01;
const V4_SKIN_PRESET: u8 = 0x02;
const V4_SKIN_CUSTOM: u8 = 0x03;

const MULTI: u8 = 0xFF;
const INJECTED: u8 = 0xEE;

const CUSTOM_LEN: usize = 64 * 64 * 3;

struct Session {
    version: u16,
    socket: UnboundedSender<Vec<u8>>,
    outbox: Vec<Vec<u8>>,
    next_id: i32,
    pending: HashMap<i32, u128>,
    asked: HashSet<u128>,
    worn: HashMap<u128, SkinRef>,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);

static PROVIDED: Mutex<Option<HashSet<u8>>> = Mutex::new(None);

pub(super) fn begin(version: u16, socket: UnboundedSender<Vec<u8>>) {
    *SESSION.lock().unwrap() = Some(Session {
        version,
        socket,
        outbox: Vec::new(),
        next_id: 0,
        pending: HashMap::new(),
        asked: HashSet::new(),
        worn: HashMap::new(),
    });
}

pub(super) fn end() {
    *SESSION.lock().unwrap() = None;
}

pub(crate) fn forget() {
    if let Some(s) = SESSION.lock().unwrap().as_mut() {
        s.outbox.clear();
        s.pending.clear();
        s.asked.clear();
        s.worn.clear();
    }
}

pub(crate) fn worn_by(uuid: u128) -> Option<SkinRef> {
    SESSION.lock().unwrap().as_ref()?.worn.get(&uuid).cloned()
}

pub(crate) fn request(uuid: u128) {
    let mut guard = SESSION.lock().unwrap();
    let Some(s) = guard.as_mut() else { return };
    if !s.asked.insert(uuid) {
        return;
    }
    let (hi, lo) = ((uuid >> 64) as u64, uuid as u64);
    if s.version >= 5 {
        let id = s.next_id;
        s.next_id = s.next_id.wrapping_add(1) & i32::MAX;
        s.pending.insert(id, uuid);
        let mut frame = vec![INJECTED, V5_GET_OTHER_SKIN];
        write_varint(&mut frame, id);
        frame.extend(hi.to_be_bytes());
        frame.extend(lo.to_be_bytes());
        let _ = s.socket.send(frame);
    } else {
        let mut message = vec![V4_GET_OTHER_SKIN];
        message.extend(hi.to_be_bytes());
        message.extend(lo.to_be_bytes());
        s.outbox.push(message);
    }
}

pub(crate) fn take_outbox() -> Vec<Vec<u8>> {
    match SESSION.lock().unwrap().as_mut() {
        Some(s) if !s.outbox.is_empty() => std::mem::take(&mut s.outbox),
        _ => Vec::new(),
    }
}

pub(super) fn receive_injected(frame: &[u8]) {
    let Some(body) = frame.strip_prefix(&[INJECTED]) else {
        return;
    };
    for message in unbatch(body) {
        receive(message, 5);
    }
}

pub(crate) fn receive_plugin(data: &[u8]) {
    for message in unbatch(data) {
        receive(message, 4);
    }
}

fn unbatch(body: &[u8]) -> Vec<&[u8]> {
    let Some(rest) = body.strip_prefix(&[MULTI]) else {
        return vec![body];
    };
    let mut out = Vec::new();
    let mut r = Reader::new(rest);
    let Some(count) = r.varint() else { return out };
    for _ in 0..count as u32 {
        let Some(len) = r.varint() else { break };
        let Some(message) = r.take(len as u32 as usize) else {
            break;
        };
        out.push(message);
    }
    out
}

enum Texture<'a> {
    Preset(i32),
    Custom { slim: bool, pixels: &'a [u8] },
}

fn receive(message: &[u8], version: u16) {
    let mut r = Reader::new(message);
    let Some(id) = r.u8() else { return };
    let parsed = if version >= 5 {
        match id {
            V5_SKIN_PRESET => r
                .varint()
                .and_then(|req| Some((Who::Request(req), Texture::Preset(r.varint()?)))),
            V5_SKIN_CUSTOM => r.varint().and_then(|req| {
                let model = r.u8()?;
                Some((
                    Who::Request(req),
                    Texture::Custom {
                        slim: model == 1,
                        pixels: r.take(CUSTOM_LEN)?,
                    },
                ))
            }),
            _ => return,
        }
    } else {
        match id {
            V4_SKIN_PRESET => r
                .uuid()
                .and_then(|uuid| Some((Who::Player(uuid), Texture::Preset(r.i32()?)))),
            V4_SKIN_CUSTOM => r.uuid().and_then(|uuid| {
                let model = r.u8()?;
                Some((
                    Who::Player(uuid),
                    Texture::Custom {
                        slim: model == 1,
                        pixels: r.take(CUSTOM_LEN)?,
                    },
                ))
            }),
            _ => return,
        }
    };
    let Some((who, texture)) = parsed else {
        log_debug!("net", "eagler: a skin reply (0x{id:02x}) was cut short");
        return;
    };
    apply(who, texture);
}

enum Who {
    Request(i32),
    Player(u128),
}

fn apply(who: Who, texture: Texture) {
    let mut guard = SESSION.lock().unwrap();
    let Some(s) = guard.as_mut() else { return };
    let uuid = match who {
        Who::Request(id) => match s.pending.remove(&id) {
            Some(uuid) => uuid,
            None => return,
        },
        Who::Player(uuid) => uuid,
    };
    let refs = match texture {
        Texture::Preset(id) => {
            let Some(image) = crate::gui::skinpick::preset_image(id) else {
                log_debug!(
                    "net",
                    "eagler: preset skin {id} is not one this client ships"
                );
                return;
            };
            let url: Arc<str> = format!("eagler:preset/{id}").into();
            let fresh = PROVIDED
                .lock()
                .unwrap()
                .get_or_insert_with(HashSet::new)
                .insert(id as u8);
            if fresh {
                crate::client::skins::provide(url.clone(), image);
            }
            SkinRef {
                body: Some(url),
                slim: crate::gui::skinpick::preset_is_slim(id),
                ..SkinRef::default()
            }
        }
        Texture::Custom { slim, pixels } => {
            let url: Arc<str> = format!("eagler:skin/{uuid:032x}").into();
            let Some(image) =
                decode(pixels).and_then(|i| crate::client::skins::process_legacy_skin(i, &url))
            else {
                return;
            };
            crate::client::skins::provide(url.clone(), image);
            SkinRef {
                body: Some(url),
                slim,
                ..SkinRef::default()
            }
        }
    };
    if s.worn.is_empty() {
        log_info!("net", "eagler: the proxy is answering skin requests");
    }
    s.worn.insert(uuid, refs);
}

fn decode(pixels: &[u8]) -> Option<RgbaImage> {
    let rgba = pixels
        .chunks_exact(3)
        .flat_map(|p| {
            [
                (p[2] & 0x7F) << 1,
                p[1],
                p[0],
                if p[2] & 0x80 != 0 { 0xFF } else { 0 },
            ]
        })
        .collect();
    RgbaImage::from_raw(64, 64, rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_batch_comes_apart_into_its_messages() {
        let body = [MULTI, 2, 2, 0xAA, 0xBB, 1, 0xCC];
        assert_eq!(unbatch(&body), vec![&[0xAA, 0xBB][..], &[0xCC][..]]);
        assert_eq!(unbatch(&[0x01, 0x02]), vec![&[0x01, 0x02][..]]);
    }

    #[test]
    fn a_custom_pixel_decodes_as_the_server_encoded_it() {
        let mut pixels = vec![0u8; CUSTOM_LEN];
        pixels[..3].copy_from_slice(&[0x30, 0x20, 0x88]);
        let image = decode(&pixels).unwrap();
        assert_eq!(image.get_pixel(0, 0).0, [0x10, 0x20, 0x30, 0xFF]);
        assert_eq!(image.get_pixel(1, 0).0[3], 0);
    }
}
