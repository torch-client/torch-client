use std::collections::HashMap;
use std::sync::OnceLock;

pub(crate) const PARTICLE_TILE_PX: u32 = 32;

const PARTICLE_ATLAS_COLS: u32 = 16;

pub(crate) struct ParticleAtlasData {
    pub image: bevy::image::Image,
    pub rows: u32,
    pub sheets: HashMap<String, Vec<u32>>,
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

fn particle_sprites() -> &'static HashMap<String, Vec<String>> {
    static SPRITES: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    SPRITES.get_or_init(|| {
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
    })
}

pub(crate) fn build_atlas() -> ParticleAtlasData {
    let dir = crate::assets_root().join("textures/particle");
    let mut entries: Vec<(String, image::RgbaImage)> = Vec::new();
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
        entries.push((stem, fit_tile(decoded.to_rgba8())));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let rows = (entries.len() as u32).max(1).div_ceil(PARTICLE_ATLAS_COLS);
    let atlas_w = PARTICLE_ATLAS_COLS * PARTICLE_TILE_PX;
    let atlas_h = rows * PARTICLE_TILE_PX;
    let mut data = vec![0u8; (atlas_w * atlas_h * 4) as usize];
    let mut tiles: HashMap<&str, u32> = HashMap::with_capacity(entries.len());
    for (index, (stem, rgba)) in entries.iter().enumerate() {
        let tile = index as u32;
        blit(&mut data, tile, atlas_w, rgba);
        tiles.insert(stem.as_str(), tile);
    }

    let mut sheets: HashMap<String, Vec<u32>> = HashMap::new();
    for (particle_type, sprites) in particle_sprites() {
        let resolved: Vec<u32> = sprites
            .iter()
            .filter_map(|sprite| tiles.get(sprite.as_str()).copied())
            .collect();
        if !resolved.is_empty() {
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
    }
}

fn fit_tile(src: image::RgbaImage) -> image::RgbaImage {
    let (w, h) = (src.width().max(1), src.height().max(1));
    let (w, h) = if h > w && h % w == 0 { (w, w) } else { (w, h) };
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
