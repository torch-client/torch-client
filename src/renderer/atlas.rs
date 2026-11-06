use std::collections::{BTreeMap, HashMap, HashSet};

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;

#[cfg(feature = "animated_textures")]
use super::atlas_anim::{self, AnimatedSprite, AtlasFrames};
use super::atlas_meta::{self, FrameRect, SpriteLayout, SpriteMeta};
use super::{ATLAS_COLS, TILE_PX};

pub struct Sprites {
    tiles: HashMap<String, u32>,
    opaque: HashSet<String>,
    rows: u32,
    #[cfg(feature = "render_lod")]
    tile_colors: std::sync::Arc<crate::renderer::tile_colors::TileColors>,
}

impl Default for Sprites {
    fn default() -> Self {
        Self {
            tiles: HashMap::new(),
            opaque: HashSet::new(),
            rows: 1,
            #[cfg(feature = "render_lod")]
            tile_colors: Default::default(),
        }
    }
}

impl Sprites {
    pub fn tile(&self, stem: &str) -> Option<u32> {
        self.tiles.get(stem).copied()
    }

    pub fn rows(&self) -> u32 {
        self.rows
    }

    pub fn is_opaque(&self, stem: &str) -> bool {
        self.opaque.contains(stem)
    }

    #[cfg(feature = "render_lod")]
    pub fn tile_colors(&self) -> &std::sync::Arc<crate::renderer::tile_colors::TileColors> {
        &self.tile_colors
    }

    pub fn mesh_tiles(&self) -> MeshTiles {
        MeshTiles {
            rows: self.rows,
            water_still: self.tile("water_still").unwrap_or(0),
            water_flow: self.tile("water_flow").unwrap_or(0),
        }
    }
}

#[derive(Clone, Copy)]
pub struct MeshTiles {
    pub rows: u32,
    pub water_still: u32,
    pub water_flow: u32,
}

pub struct BlockAtlas {
    pub image: Image,
    pub sprites: Sprites,
    pub tables: AtlasTables,
}

pub struct AtlasTables {
    tints: Option<crate::util::biome_color::Tints>,
    #[cfg(feature = "shader_support")]
    untinted: Option<std::sync::Arc<UntintedTiles>>,
    #[cfg(feature = "animated_textures")]
    frames: Option<AtlasFrames>,
}

impl AtlasTables {
    pub fn publish(self) {
        crate::util::biome_color::publish(self.tints);
        #[cfg(feature = "shader_support")]
        {
            *UNTINTED.write() = self.untinted;
        }
        #[cfg(feature = "animated_textures")]
        atlas_anim::publish(self.frames);
    }
}

fn is_fully_opaque(rgba: &image::RgbaImage) -> bool {
    rgba.pixels().all(|p| p[3] == 255)
}

pub struct Textures {
    pub block: BTreeMap<String, image::RgbaImage>,
    pub meta: BTreeMap<String, SpriteMeta>,
}

impl Textures {
    pub fn blocks(textures_dir: &str) -> Textures {
        let mut textures = Textures {
            block: BTreeMap::new(),
            meta: BTreeMap::new(),
        };
        for (stem, rgba, meta) in decode_dir(&format!("{textures_dir}/block")) {
            if let Some(meta) = meta {
                textures.meta.insert(stem.clone(), meta);
            }
            textures.block.insert(stem, rgba);
        }
        textures
    }
}

type Decoded = (String, image::RgbaImage, Option<SpriteMeta>);

fn decode_dir(dir: &str) -> Vec<Decoded> {
    let paths: Vec<std::path::PathBuf> = crate::platform::assets::walk(dir)
        .into_iter()
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("png"))
        .collect();

    let decode = |path: &std::path::Path| -> Option<Decoded> {
        let stem = path
            .strip_prefix(dir)
            .ok()?
            .with_extension("")
            .to_str()?
            .replace('\\', "/");
        let bytes = crate::platform::assets::read(path)?;
        let decoded = image::load_from_memory(&bytes).ok()?;
        let meta = crate::platform::assets::read_metadata(path)
            .and_then(|bytes| atlas_meta::parse_meta(&bytes));
        Some((stem, decoded.to_rgba8(), meta))
    };

    #[cfg(target_arch = "wasm32")]
    {
        paths.iter().filter_map(|p| decode(p)).collect()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .clamp(1, 12);
        let chunk = paths.len().div_ceil(threads).max(1);
        let decode = &decode;
        std::thread::scope(|scope| {
            let handles: Vec<_> = paths
                .chunks(chunk)
                .map(|slice| {
                    scope.spawn(move || {
                        slice
                            .iter()
                            .filter_map(|p| decode(p))
                            .collect::<Vec<Decoded>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .filter_map(|h| h.join().ok())
                .flatten()
                .collect()
        })
    }
}

#[cfg(feature = "shader_support")]
pub(crate) type UntintedTiles = Vec<(u32, Vec<Vec<u8>>)>;

#[cfg(feature = "shader_support")]
static UNTINTED: parking_lot::RwLock<Option<std::sync::Arc<UntintedTiles>>> =
    parking_lot::RwLock::new(None);

#[cfg(feature = "shader_support")]
pub(crate) fn untinted_tiles() -> Option<std::sync::Arc<UntintedTiles>> {
    UNTINTED.read().clone()
}

#[cfg(feature = "shader_support")]
fn collect_untinted(
    entries: &[PackEntry],
    biome_tinted: &[&str],
    tiles: &HashMap<String, u32>,
    n_mips: u32,
) -> UntintedTiles {
    let (level0, atlas_w, atlas_h, _) = pack_tiles(entries);
    let chain = if n_mips > 1 {
        build_mip_chain(&level0, atlas_w, atlas_h, n_mips)
    } else {
        level0
    };
    let mut out: UntintedTiles = Vec::with_capacity(biome_tinted.len());
    for stem in biome_tinted {
        let Some(&tile) = tiles.get(*stem) else {
            continue;
        };
        out.push((tile, tile_levels(&chain, atlas_w, atlas_h, tile, n_mips)));
    }
    out
}

#[cfg(any(feature = "shader_support", feature = "animated_textures", test))]
pub(super) fn tile_levels(
    chain: &[u8],
    atlas_w: u32,
    atlas_h: u32,
    tile: u32,
    n_mips: u32,
) -> Vec<Vec<u8>> {
    let tile_px = atlas_w / ATLAS_COLS;
    let (tx, ty) = (tile % ATLAS_COLS, tile / ATLAS_COLS);
    let mut offset = 0usize;
    let mut levels = Vec::with_capacity(n_mips as usize);
    for level in 0..n_mips {
        let (lw, lh, ts) = (atlas_w >> level, atlas_h >> level, tile_px >> level);
        let mut bytes = Vec::with_capacity((ts * ts * 4) as usize);
        for row in 0..ts {
            let start = offset + (((ty * ts + row) * lw + tx * ts) * 4) as usize;
            bytes.extend_from_slice(&chain[start..start + (ts * 4) as usize]);
        }
        levels.push(bytes);
        offset += (lw * lh * 4) as usize;
    }
    levels
}

pub fn build_block_atlas(textures: &Textures, textures_dir: &str, with_frames: bool) -> BlockAtlas {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    #[cfg(not(feature = "animated_textures"))]
    let _ = with_frames;

    let tints = crate::util::biome_color::Tints::load(&format!("{textures_dir}/colormap"));
    let plains = crate::util::biome_color::Tints::plains(tints.as_ref());
    let (grass_tint, foliage_tint, dry_foliage_tint) =
        (plains.grass, plains.foliage, plains.dry_foliage);

    if textures.block.is_empty() {
        crate::log_warn!(
            "atlas",
            "cannot read block texture dir {textures_dir}/block"
        );
        return BlockAtlas {
            image: placeholder_atlas(),
            sprites: Sprites::default(),
            tables: AtlasTables {
                tints,
                #[cfg(feature = "shader_support")]
                untinted: None,
                #[cfg(feature = "animated_textures")]
                frames: None,
            },
        };
    }
    let mut opaque: HashSet<String> = HashSet::new();
    let mut recoloured: BTreeMap<&str, image::RgbaImage> = BTreeMap::new();
    #[cfg(feature = "shader_support")]
    let mut biome_tinted: Vec<&str> = Vec::new();
    for (stem, rgba) in &textures.block {
        if stem == "grass_block_side_overlay" {
            continue;
        }
        if let Some((new, biome)) = tinted(
            rgba,
            stem,
            grass_tint,
            foliage_tint,
            dry_foliage_tint,
            &textures.block,
        ) {
            recoloured.insert(stem.as_str(), new);
            #[cfg(feature = "shader_support")]
            if biome {
                biome_tinted.push(stem.as_str());
            }
            #[cfg(not(feature = "shader_support"))]
            let _ = biome;
        }
    }
    let mut entries: Vec<PackEntry> = Vec::with_capacity(textures.block.len());
    #[cfg(feature = "animated_textures")]
    let mut layouts: Vec<SpriteLayout> = Vec::with_capacity(textures.block.len());
    for (stem, rgba) in &textures.block {
        if stem == "grass_block_side_overlay" {
            continue;
        }
        let rgba = recoloured.get(stem.as_str()).unwrap_or(rgba);
        if is_fully_opaque(rgba) {
            opaque.insert(stem.clone());
        }
        let layout = SpriteLayout::of(textures.meta.get(stem), rgba.width(), rgba.height());
        if layout == SpriteLayout::Broken {
            crate::log_warn!(
                "atlas",
                "{stem} has bad animation metadata, drawing the placeholder"
            );
        }
        entries.push((stem.as_str(), rgba, layout.first_rect()));
        #[cfg(feature = "animated_textures")]
        layouts.push(layout);
    }
    let opaque_count = opaque.len();

    let (atlas_data, atlas_w, atlas_h, tex_to_tile) = pack_tiles(&entries);
    #[cfg(feature = "render_lod")]
    let tile_colors = std::sync::Arc::new(crate::renderer::tile_colors::TileColors::from_atlas(
        &atlas_data,
        atlas_w,
        atlas_h,
    ));
    #[cfg(feature = "shader_support")]
    let untinted_entries: Vec<PackEntry> = entries
        .iter()
        .map(|(stem, rgba, rect)| match textures.block.get(*stem) {
            Some(source) if biome_tinted.contains(stem) => (*stem, source, *rect),
            _ => (*stem, *rgba, *rect),
        })
        .collect();

    let n_mips = (atlas_w / ATLAS_COLS).trailing_zeros() + 1;
    let mip_data = if n_mips > 1 {
        build_mip_chain(&atlas_data, atlas_w, atlas_h, n_mips)
    } else {
        atlas_data
    };
    #[cfg(feature = "shader_support")]
    let untinted = std::sync::Arc::new(collect_untinted(
        &untinted_entries,
        &biome_tinted,
        &tex_to_tile,
        n_mips,
    ));
    #[cfg(feature = "animated_textures")]
    let frames = with_frames.then(|| {
        let sprites: Vec<AnimatedSprite> = entries
            .iter()
            .enumerate()
            .zip(&layouts)
            .filter_map(|((at, (stem, rgba, _)), layout)| {
                let SpriteLayout::Animated(layout) = layout else {
                    return None;
                };
                #[cfg(feature = "shader_support")]
                let pack_image = biome_tinted.contains(stem).then(|| untinted_entries[at].1);
                #[cfg(not(feature = "shader_support"))]
                let (pack_image, _) = (None, at);
                Some(AnimatedSprite {
                    atlas_tile: *tex_to_tile.get(*stem)?,
                    image: rgba,
                    pack_image,
                    layout,
                })
            })
            .collect();
        atlas_anim::build_frames(&sprites, (atlas_w, atlas_h), n_mips)
    });

    let mut image = Image::new_uninit(
        Extent3d {
            width: atlas_w,
            height: atlas_h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.mip_level_count = n_mips;
    #[cfg(all(
        feature = "shader_support",
        target_arch = "wasm32",
        not(feature = "webgpu")
    ))]
    super::packdraw::rawviews::keep_atlas(atlas_w, atlas_h, n_mips, &mip_data);
    image.data = Some(mip_data);
    image.sampler = block_atlas_sampler();
    #[cfg(feature = "shader_support")]
    super::packdraw::allow_display_view(&mut image);
    crate::log_info!(
        "atlas",
        "{} block textures loaded ({} fully opaque), atlas {}×{}, {} mip levels",
        entries.len(),
        opaque_count,
        atlas_w,
        atlas_h,
        n_mips
    );
    let rows = super::atlas_rows(&image);
    BlockAtlas {
        image,
        sprites: Sprites {
            tiles: tex_to_tile,
            opaque,
            rows,
            #[cfg(feature = "render_lod")]
            tile_colors,
        },
        tables: AtlasTables {
            tints,
            #[cfg(feature = "shader_support")]
            untinted: Some(untinted),
            #[cfg(feature = "animated_textures")]
            frames: frames.flatten(),
        },
    }
}

fn block_atlas_sampler() -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        label: Some("block_atlas".into()),
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Nearest,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    })
}

pub(super) fn build_mip_chain(level0: &[u8], atlas_w: u32, atlas_h: u32, n_mips: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(level0.len() * 2);
    out.extend_from_slice(level0);

    let tile_px = atlas_w / ATLAS_COLS;
    let tiles_y0 = atlas_h / tile_px;
    let mut tile_alpha = vec![0u8; (tile_px * tile_px) as usize];
    let cutout_coverage: Vec<Option<f32>> = (0..tiles_y0 * ATLAS_COLS)
        .map(|t| {
            let (s_ox, s_oy) = ((t % ATLAS_COLS) * tile_px, (t / ATLAS_COLS) * tile_px);
            for y in 0..tile_px {
                for x in 0..tile_px {
                    let si = (((s_oy + y) * atlas_w + s_ox + x) * 4 + 3) as usize;
                    tile_alpha[(y * tile_px + x) as usize] = level0[si];
                }
            }
            tile_alpha
                .contains(&0)
                .then(|| alpha_test_coverage(&tile_alpha, tile_px, MIP_ALPHA_CUTOFF, 1.0))
        })
        .collect();
    for level in 1..n_mips {
        let ts = tile_px >> level;
        let f = tile_px / ts;
        let lw = atlas_w >> level;
        let lh = atlas_h >> level;
        let tiles_x = ATLAS_COLS;
        let tiles_y = lh / ts;
        let mut buf = vec![0u8; (lw * lh * 4) as usize];

        for ty in 0..tiles_y {
            for tx in 0..tiles_x {
                let s_ox = tx * tile_px;
                let s_oy = ty * tile_px;
                let d_ox = tx * ts;
                let d_oy = ty * ts;
                for py in 0..ts {
                    for px in 0..ts {
                        let (mut sr, mut sg, mut sb, mut sa) = (0u32, 0u32, 0u32, 0u32);
                        for sy in 0..f {
                            for sx in 0..f {
                                let ssx = s_ox + px * f + sx;
                                let ssy = s_oy + py * f + sy;
                                let si = ((ssy * atlas_w + ssx) * 4) as usize;
                                let a = level0[si + 3] as u32;
                                sr += level0[si] as u32 * a;
                                sg += level0[si + 1] as u32 * a;
                                sb += level0[si + 2] as u32 * a;
                                sa += a;
                            }
                        }
                        let n = (f * f) as u32;
                        let (r, g, b) = if sa > 0 {
                            (sr / sa, sg / sa, sb / sa)
                        } else {
                            (0, 0, 0)
                        };
                        let di = (((d_oy + py) * lw + (d_ox + px)) * 4) as usize;
                        buf[di] = r as u8;
                        buf[di + 1] = g as u8;
                        buf[di + 2] = b as u8;
                        buf[di + 3] = (sa / n) as u8;
                    }
                }
                if let Some(Some(coverage)) = cutout_coverage.get((ty * tiles_x + tx) as usize) {
                    scale_tile_alpha_to_coverage(&mut buf, lw, (d_ox, d_oy), ts, *coverage);
                }
            }
        }
        out.extend_from_slice(&buf);
    }
    out
}

const MIP_ALPHA_CUTOFF: f32 = 0.5;

fn alpha_test_coverage(alpha: &[u8], size: u32, alpha_ref: f32, scale: f32) -> f32 {
    if size < 2 {
        return 0.0;
    }
    let at = |x: u32, y: u32| {
        (f32::from(alpha[(y * size + x) as usize]) / 255.0 * scale).clamp(0.0, 1.0)
    };
    let mut coverage = 0.0;
    for y in 0..size - 1 {
        for x in 0..size - 1 {
            let (a00, a10, a01, a11) = (at(x, y), at(x + 1, y), at(x, y + 1), at(x + 1, y + 1));
            let mut texel = 0.0;
            for sy in 0..4 {
                let fy = (sy as f32 + 0.5) / 4.0;
                for sx in 0..4 {
                    let fx = (sx as f32 + 0.5) / 4.0;
                    let a = a00 * (1.0 - fx) * (1.0 - fy)
                        + a10 * fx * (1.0 - fy)
                        + a01 * (1.0 - fx) * fy
                        + a11 * fx * fy;
                    if a > alpha_ref {
                        texel += 1.0;
                    }
                }
            }
            coverage += texel / 16.0;
        }
    }
    coverage / ((size - 1) * (size - 1)) as f32
}

fn scale_tile_alpha_to_coverage(
    buf: &mut [u8],
    level_w: u32,
    origin: (u32, u32),
    size: u32,
    desired: f32,
) {
    let index = |x: u32, y: u32| (((origin.1 + y) * level_w + origin.0 + x) * 4 + 3) as usize;
    let alpha: Vec<u8> = (0..size * size)
        .map(|i| buf[index(i % size, i / size)])
        .collect();
    let (mut min, mut max, mut scale) = (0.0f32, 4.0f32, 1.0f32);
    let (mut best_scale, mut best_error) = (1.0f32, f32::MAX);
    for _ in 0..5 {
        let current = alpha_test_coverage(&alpha, size, MIP_ALPHA_CUTOFF, scale);
        let error = (current - desired).abs();
        if error < best_error {
            best_error = error;
            best_scale = scale;
        }
        if current < desired {
            min = scale;
        } else if current > desired {
            max = scale;
        } else {
            break;
        }
        scale = (min + max) * 0.5;
    }
    for (i, a) in alpha.iter().enumerate() {
        let i = i as u32;
        let scaled = (f32::from(*a) / 255.0 * best_scale + 0.025).clamp(0.0, 1.0);
        buf[index(i % size, i / size)] = (scaled * 255.0) as u8;
    }
}

const FIXED_TINTED: &[(&str, [u8; 4])] = &[
    ("spruce_leaves", [0x61, 0x99, 0x61, 255]),
    ("birch_leaves", [0x80, 0xa7, 0x55, 255]),
    ("lily_pad", [0x20, 0x80, 0x30, 255]),
];

fn tinted(
    src: &image::RgbaImage,
    stem: &str,
    grass_tint: [u8; 4],
    foliage_tint: [u8; 4],
    dry_foliage_tint: [u8; 4],
    block: &BTreeMap<String, image::RgbaImage>,
) -> Option<(image::RgbaImage, bool)> {
    const GRASS_TINTED: &[&str] = &[
        "grass_block_top",
        "short_grass",
        "fern",
        "tall_grass",
        "tall_grass_top",
        "tall_grass_bottom",
        "large_fern",
        "large_fern_top",
        "large_fern_bottom",
        "sugar_cane",
        "bush",
        "pink_petals_stem",
        "wildflowers_stem",
    ];
    const FOLIAGE_TINTED: &[&str] = &[
        "oak_leaves",
        "jungle_leaves",
        "acacia_leaves",
        "dark_oak_leaves",
        "mangrove_leaves",
        "vine",
    ];
    const UNTINTED_LEAVES: &[&str] = &[
        "pale_oak_leaves",
        "cherry_leaves",
        "azalea_leaves",
        "flowering_azalea_leaves",
    ];

    let flat = if stem == "leaf_litter" {
        Some(dry_foliage_tint)
    } else if let Some((_, fixed)) = FIXED_TINTED.iter().find(|(s, _)| *s == stem) {
        Some(*fixed)
    } else if UNTINTED_LEAVES.contains(&stem) {
        None
    } else if GRASS_TINTED.contains(&stem) {
        Some(grass_tint)
    } else if stem == "water_still" || stem == "water_flow" {
        Some([0x3F, 0x76, 0xE4, 0xFF])
    } else if FOLIAGE_TINTED.contains(&stem) || stem.ends_with("_leaves") {
        Some(foliage_tint)
    } else {
        None
    };
    if let Some(tint) = flat {
        let mut rgba = src.clone();
        tint_rgba(&mut rgba, tint);
        let biome = !FIXED_TINTED.iter().any(|(s, _)| *s == stem);
        return Some((rgba, biome));
    }
    if stem == "grass_block_side" {
        let overlay = block.get("grass_block_side_overlay")?;
        let mut overlay = overlay.clone();
        tint_rgba(&mut overlay, grass_tint);
        let mut rgba = src.clone();
        composite_over(&mut rgba, &overlay);
        return Some((rgba, false));
    }

    None
}

fn tint_rgba(img: &mut image::RgbaImage, tint: [u8; 4]) {
    let (tr, tg, tb) = (tint[0] as u32, tint[1] as u32, tint[2] as u32);
    for px in img.pixels_mut() {
        if px[3] > 0 {
            px[0] = ((px[0] as u32 * tr) / 255) as u8;
            px[1] = ((px[1] as u32 * tg) / 255) as u8;
            px[2] = ((px[2] as u32 * tb) / 255) as u8;
        }
    }
}

fn composite_over(base: &mut image::RgbaImage, overlay: &image::RgbaImage) {
    let (w, h) = base.dimensions();
    let (ow, oh) = overlay.dimensions();
    if ow == 0 || oh == 0 {
        return;
    }
    for y in 0..h {
        for x in 0..w {
            let s = overlay.get_pixel(x * ow / w, y * oh / h);
            if s[3] == 0 {
                continue;
            }
            let d = base.get_pixel_mut(x, y);
            let sa = s[3] as u32;
            let da = 255 - sa;
            d[0] = ((s[0] as u32 * sa + d[0] as u32 * da) / 255) as u8;
            d[1] = ((s[1] as u32 * sa + d[1] as u32 * da) / 255) as u8;
            d[2] = ((s[2] as u32 * sa + d[2] as u32 * da) / 255) as u8;
            d[3] = (sa + d[3] as u32 * da / 255).min(255) as u8;
        }
    }
}

pub(super) fn blit_tile(
    atlas: &mut [u8],
    tile: u32,
    atlas_w: u32,
    rgba: &image::RgbaImage,
    rect: FrameRect,
) {
    let tile_px = atlas_w / ATLAS_COLS;
    let col = tile % ATLAS_COLS;
    let row = tile / ATLAS_COLS;
    let (ox, oy) = (col * tile_px, row * tile_px);
    for py in 0..tile_px {
        for px in 0..tile_px {
            let sx = (rect.x + px * rect.w / tile_px).min(rgba.width() - 1);
            let sy = (rect.y + py * rect.h / tile_px).min(rgba.height() - 1);
            let p = rgba.get_pixel(sx, sy);
            let dst = (((oy + py) * atlas_w + (ox + px)) * 4) as usize;
            atlas[dst..dst + 4].copy_from_slice(&p.0);
        }
    }
}

fn blit_gray_placeholder(atlas: &mut [u8], tile: u32, atlas_w: u32, tile_px: u32) {
    let col = tile % ATLAS_COLS;
    let row = tile / ATLAS_COLS;
    let (ox, oy) = (col * tile_px, row * tile_px);
    let texel = tile_px / TILE_PX;
    for py in 0..tile_px {
        for px in 0..tile_px {
            let edge = px < texel || py < texel || px >= tile_px - texel || py >= tile_px - texel;
            let checker = ((px / (4 * texel)) + (py / (4 * texel))) % 2 == 0;
            let v: u8 = if edge {
                90
            } else if checker {
                150
            } else {
                165
            };
            let dst = (((oy + py) * atlas_w + (ox + px)) * 4) as usize;
            atlas[dst] = v;
            atlas[dst + 1] = v;
            atlas[dst + 2] = v;
            atlas[dst + 3] = 255;
        }
    }
}

pub fn placeholder_atlas() -> Image {
    let mut data = vec![0u8; (TILE_PX * TILE_PX * 4) as usize];
    blit_gray_placeholder(&mut data, 0, TILE_PX, TILE_PX);
    #[cfg_attr(not(feature = "shader_support"), allow(unused_mut))]
    let mut image = super::rgba_image(TILE_PX, TILE_PX, data, RenderAssetUsages::RENDER_WORLD);
    #[cfg(feature = "shader_support")]
    super::packdraw::allow_display_view(&mut image);
    image
}

const MAX_TILE_PX: u32 = 64;

type PackEntry<'a> = (&'a str, &'a image::RgbaImage, Option<FrameRect>);

fn tile_px(entries: &[PackEntry]) -> u32 {
    let mut counts: HashMap<u32, u32> = HashMap::new();
    for (_, rgba, rect) in entries {
        let side = rect
            .map_or(rgba.width().min(rgba.height()), |rect| rect.w.min(rect.h))
            .next_power_of_two();
        *counts.entry(side.clamp(TILE_PX, MAX_TILE_PX)).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|&(side, count)| (count, side))
        .map_or(TILE_PX, |(side, _)| side)
}

fn pack_tiles(entries: &[PackEntry]) -> (Vec<u8>, u32, u32, HashMap<String, u32>) {
    let tile_px = tile_px(entries);
    let n_tiles = 1 + entries.len() as u32;
    let atlas_w = ATLAS_COLS * tile_px;
    let atlas_h = n_tiles.div_ceil(ATLAS_COLS) * tile_px;

    let mut data = vec![0u8; (atlas_w * atlas_h * 4) as usize];
    blit_gray_placeholder(&mut data, 0, atlas_w, tile_px);

    let mut tiles: HashMap<String, u32> = HashMap::new();
    for (idx, (stem, rgba, rect)) in entries.iter().enumerate() {
        let tile = idx as u32 + 1;
        match rect {
            Some(rect) => blit_tile(&mut data, tile, atlas_w, rgba, *rect),
            None => blit_gray_placeholder(&mut data, tile, atlas_w, tile_px),
        }
        tiles.insert((*stem).to_owned(), tile);
    }

    (data, atlas_w, atlas_h, tiles)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip(side: u32, count: u32) -> image::RgbaImage {
        image::RgbaImage::from_fn(side, side * count, |x, y| {
            let frame = y / side;
            let alpha = if frame == 1 && x < side / 2 && y % side < side / 2 {
                0
            } else {
                255
            };
            image::Rgba([
                40 * frame as u8 + 10,
                (x * 7) as u8,
                (y % side * 9) as u8,
                alpha,
            ])
        })
    }

    fn textures(sprites: Vec<(&str, image::RgbaImage, Option<&str>)>) -> Textures {
        let mut textures = Textures {
            block: BTreeMap::new(),
            meta: BTreeMap::new(),
        };
        for (stem, rgba, meta) in sprites {
            textures.block.insert(stem.to_owned(), rgba);
            if let Some(meta) = meta.and_then(|json| atlas_meta::parse_meta(json.as_bytes())) {
                textures.meta.insert(stem.to_owned(), meta);
            }
        }
        textures
    }

    fn atlas_levels(atlas: &BlockAtlas, tile: u32) -> Vec<Vec<u8>> {
        let image = &atlas.image;
        let chain = image
            .data
            .as_ref()
            .expect("the atlas keeps its bytes until it is uploaded");
        let size = image.texture_descriptor.size;
        tile_levels(
            chain,
            size.width,
            size.height,
            tile,
            image.texture_descriptor.mip_level_count,
        )
    }

    #[test]
    fn a_tile_starts_on_its_first_listed_frame() {
        let textures = textures(vec![(
            "fire",
            strip(16, 4),
            Some(r#"{"animation":{"frames":[2,3,0,1]}}"#),
        )]);
        let atlas = build_block_atlas(&textures, "/nonexistent", true);
        let tile = atlas.sprites.tile("fire").unwrap();
        assert_eq!(atlas_levels(&atlas, tile)[0][0], 90);
    }

    #[cfg(feature = "animated_textures")]
    #[test]
    fn every_level_of_a_frame_matches_the_tile_it_would_be() {
        for side in [16, 64] {
            let meta = r#"{"animation":{"frames":[1,0,2]}}"#;
            let textures_now = textures(vec![
                ("a", strip(side, 3), Some(meta)),
                ("filler_a", strip(side, 1), None),
                ("filler_b", strip(side, 1), None),
            ]);
            let atlas = build_block_atlas(&textures_now, "/nonexistent", true);
            let tile = atlas.sprites.tile("a").unwrap();
            let frames = atlas.tables.frames.as_ref().expect("one animated sprite");
            let first = frames.animations[0].frames[0].tile;
            assert_eq!(
                frames.block_levels(first),
                atlas_levels(&atlas, tile),
                "side {side}"
            );
        }
    }

    #[test]
    fn frames_change_neither_the_atlas_size_nor_its_tiles() {
        let sprites = || {
            vec![
                ("a", strip(16, 8), Some(r#"{"animation":{"frametime":2}}"#)),
                ("b", strip(16, 1), None),
                ("c", strip(16, 2), None),
            ]
        };
        let plain: Vec<_> = sprites()
            .into_iter()
            .map(|(stem, rgba, _)| (stem, rgba, None))
            .collect();
        let (with, without) = (
            build_block_atlas(&textures(sprites()), "/nonexistent", true),
            build_block_atlas(&textures(plain), "/nonexistent", true),
        );
        assert_eq!(
            with.image.texture_descriptor.size,
            without.image.texture_descriptor.size
        );
        assert_eq!(
            with.image.texture_descriptor.mip_level_count,
            without.image.texture_descriptor.mip_level_count
        );
        for stem in ["a", "b", "c"] {
            assert_eq!(with.sprites.tile(stem), without.sprites.tile(stem));
        }
        assert_eq!(with.image.data, without.image.data);
    }

    #[test]
    fn broken_metadata_draws_the_placeholder_in_its_own_tile() {
        let textures = textures(vec![
            ("bad", strip(16, 2), Some("not json")),
            ("good", strip(16, 1), None),
        ]);
        let atlas = build_block_atlas(&textures, "/nonexistent", true);
        assert_eq!(atlas.sprites.tile("bad"), Some(1));
        assert_eq!(atlas.sprites.tile("good"), Some(2));
        assert_eq!(atlas_levels(&atlas, 1), atlas_levels(&atlas, 0));
        #[cfg(feature = "animated_textures")]
        assert!(atlas.tables.frames.is_none());
    }

    #[cfg(feature = "animated_textures")]
    #[test]
    fn the_worker_builds_no_frames() {
        let textures = textures(vec![("a", strip(16, 2), Some(r#"{"animation":{}}"#))]);
        assert!(
            build_block_atlas(&textures, "/nonexistent", false)
                .tables
                .frames
                .is_none()
        );
        assert!(
            build_block_atlas(&textures, "/nonexistent", true)
                .tables
                .frames
                .is_some()
        );
    }
}
