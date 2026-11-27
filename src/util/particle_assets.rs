use std::collections::HashMap;
use std::sync::Arc;

use crate::util::reload::Reloadable;

pub(crate) const PARTICLE_TILE_PX: u32 = 32;

const PARTICLE_ATLAS_COLS: u32 = 16;

pub(crate) struct ParticleAtlasData {
    pub image: bevy::image::Image,
    pub rows: u32,
    pub sheets: HashMap<String, Vec<u32>>,
    pub frametimes: HashMap<String, u32>,
}

pub(crate) fn tile_uv(tile: u32, rows: u32) -> [f32; 4] {
    let cols = PARTICLE_ATLAS_COLS as f32;
    let rows = rows.max(1) as f32;
    let col = (tile % PARTICLE_ATLAS_COLS) as f32;
    let row = (tile / PARTICLE_ATLAS_COLS) as f32;
    [
        col / cols,
        row / rows,
        (col + 1.0) / cols,
        (row + 1.0) / rows,
    ]
}

static SPRITES: Reloadable<HashMap<String, Vec<String>>> = Reloadable::new(load_sprites);

pub(crate) fn reset() {
    SPRITES.reset();
}

fn particle_sprites() -> Arc<HashMap<String, Vec<String>>> {
    SPRITES.get()
}

fn load_sprites() -> HashMap<String, Vec<String>> {
    {
        let dir = crate::assets_root().join("particles");
        let mut out = HashMap::new();
        for path in crate::platform::assets::read_dir(&dir) {
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some(text) = crate::platform::assets::read_to_string(&path) else {
                continue;
            };
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let Some(textures) = json.get("textures").and_then(|v| v.as_array()) else {
                continue;
            };
            let sprites: Vec<String> = textures
                .iter()
                .filter_map(crate::util::datapack::bare_id)
                .map(str::to_owned)
                .collect();
            if !sprites.is_empty() {
                out.insert(id.to_owned(), sprites);
            }
        }
        crate::log_info!("particles", "{} particle sprite sheets loaded", out.len());
        out
    }
}

pub(crate) fn build_atlas() -> ParticleAtlasData {
    let dir = crate::assets_root().join("textures/particle");
    let mut entries: Vec<(String, Vec<image::RgbaImage>, u32)> = Vec::new();
    for path in crate::platform::assets::read_dir(&dir) {
        if path.extension().is_none_or(|ext| ext != "png") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else {
            continue;
        };
        let Some(bytes) = crate::platform::assets::read(&path) else {
            continue;
        };
        let Ok(decoded) = image::load_from_memory(&bytes) else {
            continue;
        };
        let rgba = decoded.to_rgba8();
        let (frames, frametime) = match read_animation(&path) {
            Some(animation) => (split_frames(&rgba, &animation), animation.frametime),
            None => (vec![rgba], 0),
        };
        if frames.is_empty() {
            continue;
        }
        entries.push((stem, frames.into_iter().map(fit_tile).collect(), frametime));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let tile_count: u32 = entries.iter().map(|e| e.1.len() as u32).sum();
    let rows = tile_count.max(1).div_ceil(PARTICLE_ATLAS_COLS);
    let atlas_w = PARTICLE_ATLAS_COLS * PARTICLE_TILE_PX;
    let atlas_h = rows * PARTICLE_TILE_PX;
    let mut data = vec![0u8; (atlas_w * atlas_h * 4) as usize];
    let mut tiles: HashMap<&str, (Vec<u32>, u32)> = HashMap::with_capacity(entries.len());
    let mut next_tile = 0u32;
    for (stem, frames, frametime) in &entries {
        let mut list = Vec::with_capacity(frames.len());
        for frame in frames {
            blit(&mut data, next_tile, atlas_w, frame);
            list.push(next_tile);
            next_tile += 1;
        }
        tiles.insert(stem.as_str(), (list, *frametime));
    }

    let mut sheets: HashMap<String, Vec<u32>> = HashMap::new();
    let mut frametimes: HashMap<String, u32> = HashMap::new();
    for (particle_type, sprites) in particle_sprites().iter() {
        let mut resolved: Vec<u32> = Vec::new();
        let mut frametime = 0;
        for sprite in sprites {
            if let Some((list, time)) = tiles.get(sprite.as_str()) {
                resolved.extend_from_slice(list);
                if *time > 0 {
                    frametime = *time;
                }
            }
        }
        if !resolved.is_empty() {
            if frametime > 0 {
                frametimes.insert(particle_type.clone(), frametime);
            }
            sheets.insert(particle_type.clone(), resolved);
        }
    }
    crate::log_info!(
        "particles",
        "particle atlas {atlas_w}x{atlas_h}, {} sprites, {} sheets",
        entries.len(),
        sheets.len()
    );

    ParticleAtlasData {
        image: crate::renderer::rgba_image(
            atlas_w,
            atlas_h,
            data,
            bevy::asset::RenderAssetUsages::RENDER_WORLD,
        ),
        rows,
        sheets,
        frametimes,
    }
}

struct Animation {
    frametime: u32,
    frames: Option<Vec<u32>>,
    width: Option<u32>,
    height: Option<u32>,
}

fn read_animation(png: &std::path::Path) -> Option<Animation> {
    use serde_json::Value;
    let mut name = png.file_name()?.to_os_string();
    name.push(".mcmeta");
    let text = crate::platform::assets::read_to_string(png.with_file_name(name))?;
    let json = serde_json::from_str::<Value>(&text).ok()?;
    let animation = json.get("animation")?;
    let int = |key: &str| {
        animation
            .get(key)
            .and_then(Value::as_u64)
            .map(|v| v.min(u64::from(u32::MAX)) as u32)
    };
    let frames = animation
        .get("frames")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|f| {
                    f.as_u64()
                        .or_else(|| f.get("index").and_then(Value::as_u64))
                })
                .map(|i| i.min(u64::from(u32::MAX)) as u32)
                .collect()
        });
    Some(Animation {
        frametime: int("frametime").unwrap_or(1).max(1),
        frames,
        width: int("width"),
        height: int("height"),
    })
}

fn split_frames(src: &image::RgbaImage, animation: &Animation) -> Vec<image::RgbaImage> {
    let (w, h) = (src.width(), src.height());
    let side = w.min(h);
    let (fw, fh) = match (animation.width, animation.height) {
        (Some(fw), Some(fh)) => (fw, fh),
        (Some(fw), None) => (fw, h),
        (None, Some(fh)) => (w, fh),
        (None, None) => (side, side),
    };
    if fw == 0 || fh == 0 || fw > w || fh > h {
        return Vec::new();
    }
    let cols = w / fw;
    let count = cols * (h / fh);
    let order: Vec<u32> = animation
        .frames
        .clone()
        .unwrap_or_else(|| (0..count).collect());
    order
        .into_iter()
        .filter(|&i| i < count)
        .map(|i| {
            image::imageops::crop_imm(src, (i % cols) * fw, (i / cols) * fh, fw, fh).to_image()
        })
        .collect()
}

fn fit_tile(src: image::RgbaImage) -> image::RgbaImage {
    let (w, h) = (src.width().max(1), src.height().max(1));
    let size = PARTICLE_TILE_PX;
    let mut out = image::RgbaImage::new(size, size);
    for y in 0..size {
        let sy = (y * h / size).min(h - 1);
        for x in 0..size {
            let sx = (x * w / size).min(w - 1);
            out.put_pixel(x, y, *src.get_pixel(sx, sy));
        }
    }
    out
}

fn blit(atlas: &mut [u8], tile: u32, atlas_w: u32, rgba: &image::RgbaImage) {
    let ox = (tile % PARTICLE_ATLAS_COLS) * PARTICLE_TILE_PX;
    let oy = (tile / PARTICLE_ATLAS_COLS) * PARTICLE_TILE_PX;
    for py in 0..PARTICLE_TILE_PX {
        for px in 0..PARTICLE_TILE_PX {
            let p = rgba.get_pixel(px, py);
            let dst = (((oy + py) * atlas_w + (ox + px)) * 4) as usize;
            atlas[dst..dst + 4].copy_from_slice(&p.0);
        }
    }
}
