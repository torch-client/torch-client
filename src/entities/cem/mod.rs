pub mod cube;
mod graft;
mod jem;
mod names;

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;

use crate::entities::geom::LayerDef;
use crate::entities::{EntityAssets, EntityImages, EntityRigs, TexturePath};

const ASSET_PREFIX: &str = "cem_asset/";

const STACK_PREFIX: &str = "cem_stack/";

const MAX_SIDE: u32 = 8192;

struct Loaded {
    model: jem::Model,
    slots: Vec<String>,
    role: names::Role,
}

static LOADED: parking_lot::Mutex<Option<HashMap<&'static str, Option<Arc<Loaded>>>>> =
    parking_lot::Mutex::new(None);

fn loaded(spec: &'static str) -> Option<Arc<Loaded>> {
    if let Some(found) = LOADED.lock().get_or_insert_with(HashMap::new).get(spec) {
        return found.clone();
    }
    let found = load(spec).map(Arc::new);
    LOADED
        .lock()
        .get_or_insert_with(HashMap::new)
        .entry(spec)
        .or_insert(found)
        .clone()
}

fn cached(spec: &str) -> Option<Arc<Loaded>> {
    LOADED.lock().as_ref()?.get(spec).cloned().flatten()
}

pub(super) struct Dir {
    pub folder: String,
    pub file: String,
}

fn load(spec: &'static str) -> Option<Loaded> {
    let entry = names::entry(spec)?;
    let mut found: Vec<(usize, Dir, Vec<u8>)> =
        entry.files.iter().filter_map(|name| locate(name)).collect();
    found.sort_by_key(|(rank, ..)| *rank);
    let (model, dir) =
        found
            .into_iter()
            .find_map(|(_, dir, bytes)| match jem::parse(&bytes, &dir) {
                Ok(model) => Some((model, dir)),
                Err(e) => {
                    crate::log_warn!("cem", "{}: {e}", dir.file);
                    None
                }
            })?;
    let mut slots = Vec::new();
    if entry.role == names::Role::Main {
        fn collect(part: &jem::Part, slots: &mut Vec<String>) {
            if let Some(texture) = &part.texture
                && !slots.contains(texture)
            {
                slots.push(texture.clone());
            }
            for child in &part.children {
                collect(child, slots);
            }
        }
        for part in &model.parts {
            collect(part, &mut slots);
        }
    }
    crate::log_info!("cem", "{spec}: custom model {}", dir.file);
    Some(Loaded {
        model,
        slots,
        role: entry.role,
    })
}

fn locate(name: &str) -> Option<(usize, Dir, Vec<u8>)> {
    let places = [
        format!("emf/cem/{name}/"),
        "emf/cem/".to_owned(),
        format!("optifine/cem/{name}/"),
        "optifine/cem/".to_owned(),
    ];
    let mut best: Option<(usize, Dir, Vec<u8>)> = None;
    for folder in places {
        let file = format!("{folder}{name}.jem");
        if let Some((rank, bytes)) = crate::platform::assets::read_pack_ranked(asset_path(&file))
            && best
                .as_ref()
                .is_none_or(|(best_rank, ..)| rank < *best_rank)
        {
            best = Some((rank, Dir { folder, file }, bytes));
        }
    }
    best
}

fn asset_path(relative: &str) -> std::path::PathBuf {
    crate::assets_root().join(relative)
}

fn read(relative: &str) -> Option<Vec<u8>> {
    crate::platform::assets::read(asset_path(relative)).map(|b| b.into_owned())
}

fn resolve(written: &str, ext: &str, dir: &Dir) -> Option<String> {
    let mut path = written.trim().to_owned();
    if path.is_empty() {
        return None;
    }
    if !path.ends_with(&format!(".{ext}")) {
        path = format!("{path}.{ext}");
    }
    let path = match path.split_once(':') {
        Some(("minecraft", rest)) => rest.to_owned(),
        Some((namespace, _)) => {
            crate::log_warn!(
                "cem",
                "{}: {written} is in namespace {namespace}, which is not read",
                dir.file
            );
            return None;
        }
        None if !path.contains('/') => format!("{}{path}", dir.folder),
        None => match (path.strip_prefix("./"), path.strip_prefix("~/")) {
            (Some(rest), _) => format!("{}{rest}", dir.folder),
            (_, Some(rest)) => format!("optifine/{rest}"),
            _ => path,
        },
    };
    if !crate::platform::assets::exists(asset_path(&path)) {
        crate::log_warn!("cem", "{}: {written} not found as {path}", dir.file);
        return None;
    }
    Some(path)
}

pub(crate) fn layer(spec: &'static str, layer: LayerDef) -> LayerDef {
    let (Some(entry), Some(loaded)) = (names::entry(spec), loaded(spec)) else {
        return layer;
    };
    graft::apply(layer, &loaded.model, entry, loaded.slots.len() + 1, spec)
}

pub(crate) fn texture(spec: &'static str, base: TexturePath) -> TexturePath {
    let Some(loaded) = loaded(spec) else {
        return base;
    };
    if loaded.role != names::Role::Main {
        return base;
    }
    let base = match &loaded.model.texture {
        Some(texture) => TexturePath::Owned(format!("{ASSET_PREFIX}{texture}")),
        None => base,
    };
    if loaded.slots.is_empty() {
        base
    } else {
        TexturePath::Owned(format!("{STACK_PREFIX}{spec}|{base}"))
    }
}

pub(crate) fn decode(path: &str) -> Option<Option<image::RgbaImage>> {
    if let Some(rest) = path.strip_prefix(ASSET_PREFIX) {
        return Some(open(rest));
    }
    let (spec, base) = path.strip_prefix(STACK_PREFIX)?.split_once('|')?;
    Some(stack(spec, base))
}

fn open(relative: &str) -> Option<image::RgbaImage> {
    match crate::platform::assets::open_image(asset_path(relative)) {
        Ok(image) => Some(image.to_rgba8()),
        Err(e) => {
            crate::log_warn!("cem", "texture {relative}: {e}");
            None
        }
    }
}

fn stack(spec: &str, base: &str) -> Option<image::RgbaImage> {
    let base = super::decode_entity_texture(base)?;
    let Some(loaded) = cached(spec) else {
        return Some(base);
    };
    let mut sheets = vec![base];
    for slot in &loaded.slots {
        let sheet = open(slot).unwrap_or_else(|| image::RgbaImage::new(1, 1));
        sheets.push(sheet);
    }
    let mut width = sheets.iter().map(|s| s.width()).max().unwrap_or(1);
    let mut height = sheets.iter().map(|s| s.height()).max().unwrap_or(1);
    let bands = sheets.len() as u32;
    let fit = (MAX_SIDE as f32 / width as f32).min(MAX_SIDE as f32 / (height * bands) as f32);
    if fit < 1.0 {
        crate::log_warn!(
            "cem",
            "{spec}: stacked sheet scaled by {fit:.2} to fit {MAX_SIDE}px"
        );
        width = ((width as f32 * fit) as u32).max(1);
        height = ((height as f32 * fit) as u32).max(1);
    }
    let mut out = image::RgbaImage::new(width, height * sheets.len() as u32);
    for (k, sheet) in sheets.iter().enumerate() {
        let sheet = if sheet.dimensions() == (width, height) {
            std::borrow::Cow::Borrowed(sheet)
        } else {
            std::borrow::Cow::Owned(image::imageops::resize(
                sheet,
                width,
                height,
                image::imageops::FilterType::Nearest,
            ))
        };
        image::imageops::replace(&mut out, &*sheet, 0, (height * k as u32) as i64);
    }
    Some(out)
}

fn is_ours(path: &str) -> bool {
    path.starts_with(ASSET_PREFIX) || path.starts_with(STACK_PREFIX)
}

pub(crate) fn forget(
    mut commands: Commands,
    mut rigs: ResMut<EntityRigs>,
    mut assets: ResMut<EntityAssets>,
    mut images: ResMut<EntityImages>,
) {
    rigs.despawn_all(&mut commands);
    assets.forget_models();
    images.forget(is_ours);
    *LOADED.lock() = None;
}
