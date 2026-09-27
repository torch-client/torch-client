use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;

use super::{ATLAS_COLS, TILE_PX};

static OPAQUE_TEXTURES: OnceLock<HashSet<String>> = OnceLock::new();

pub fn texture_is_opaque(stem: &str) -> bool {
    OPAQUE_TEXTURES.get().is_some_and(|set| set.contains(stem))
}

fn is_fully_opaque(rgba: &image::RgbaImage) -> bool {
    rgba.pixels().all(|p| p[3] == 255)
}

pub struct Textures {
    pub block: BTreeMap<String, image::RgbaImage>,
    pub item: BTreeMap<String, image::RgbaImage>,
}

impl Textures {
    pub fn load(textures_dir: &str) -> Textures {
        Textures {
            block: decode_dir(&format!("{textures_dir}/block")),
            item: decode_dir(&format!("{textures_dir}/item")),
        }
    }

    pub fn blocks(textures_dir: &str) -> Textures {
        Textures {
            block: decode_dir(&format!("{textures_dir}/block")),
            item: BTreeMap::new(),
        }
    }
}

fn decode_dir(dir: &str) -> BTreeMap<String, image::RgbaImage> {
    let paths: Vec<std::path::PathBuf> = crate::platform::assets::read_dir(dir)
        .into_iter()
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("png"))
        .collect();

    let decode = |path: &std::path::Path| -> Option<(String, image::RgbaImage)> {
        let stem = path.file_stem()?.to_str()?.to_owned();
        let bytes = crate::platform::assets::read(path)?;
        let decoded = image::load_from_memory(&bytes).ok()?;
        Some((stem, decoded.to_rgba8()))
    };

    #[cfg(target_arch = "wasm32")]
    {
        paths
            .iter()
            .filter_map(|p| decode(p))
            .collect::<BTreeMap<String, image::RgbaImage>>()
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
                            .collect::<BTreeMap<String, image::RgbaImage>>()
                    })
                })
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok()).fold(
                BTreeMap::<String, image::RgbaImage>::new(),
                |mut acc, part| {
                    acc.extend(part);
                    acc
                },
            )
        })
    }
}

pub fn build_item_ui_atlas(textures: &Textures) -> (Image, HashMap<String, u32>) {
    let mut entries: Vec<(&str, &image::RgbaImage)> =
        Vec::with_capacity(textures.item.len() + textures.block.len());
    entries.extend(textures.item.iter().map(|(k, v)| (k.as_str(), v)));
    entries.extend(
        textures
            .block
            .iter()
            .filter(|(k, _)| !textures.item.contains_key(*k))
            .map(|(k, v)| (k.as_str(), v)),
    );

    let (atlas_data, atlas_w, atlas_h, tile_map) = pack_tiles(&entries);

    let image = super::rgba_image(atlas_w, atlas_h, atlas_data, RenderAssetUsages::default());
    println!(
        "[ItemAtlas] {} item/block UI textures loaded",
        entries.len()
    );
    (image, tile_map)
}

pub fn build_block_atlas(textures: &Textures, textures_dir: &str) -> (Image, HashMap<String, u32>) {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let colormap_dir = format!("{}/colormap", textures_dir);

    crate::util::biome_color::init(&colormap_dir);
    let grass_tint = crate::util::biome_color::plains_grass();
    let foliage_tint = crate::util::biome_color::plains_foliage();
    let dry_foliage_tint = crate::util::biome_color::plains_dry_foliage();

    if textures.block.is_empty() {
        eprintln!("Cannot read block texture dir {textures_dir}/block");
        return (placeholder_atlas(), HashMap::new());
    }
    let mut opaque: HashSet<String> = HashSet::new();
    let mut recoloured: BTreeMap<&str, image::RgbaImage> = BTreeMap::new();
    for (stem, rgba) in &textures.block {
        if stem == "grass_block_side_overlay" {
            continue;
        }
        if let Some(new) = tinted(
            rgba,
            stem,
            grass_tint,
            foliage_tint,
            dry_foliage_tint,
            &textures.block,
        ) {
            recoloured.insert(stem.as_str(), new);
        }
    }
    let mut entries: Vec<(&str, &image::RgbaImage)> = Vec::with_capacity(textures.block.len());
    for (stem, rgba) in &textures.block {
        if stem == "grass_block_side_overlay" {
            continue;
        }
        let rgba = recoloured.get(stem.as_str()).unwrap_or(rgba);
        if is_fully_opaque(rgba) {
            opaque.insert(stem.clone());
        }
        entries.push((stem.as_str(), rgba));
    }
    let opaque_count = opaque.len();
    OPAQUE_TEXTURES.set(opaque).ok();

    let (atlas_data, atlas_w, atlas_h, tex_to_tile) = pack_tiles(&entries);

    let n_mips = TILE_PX.trailing_zeros() + 1;
    let mip_data = if n_mips > 1 {
        build_mip_chain(&atlas_data, atlas_w, atlas_h, n_mips)
    } else {
        atlas_data
    };

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
    image.data = Some(mip_data);
    image.sampler = block_atlas_sampler();
    println!(
        "[Atlas] {} block textures loaded ({} fully opaque), atlas {}×{}, {} mip levels",
        entries.len(),
        opaque_count,
        atlas_w,
        atlas_h,
        n_mips
    );
    (image, tex_to_tile)
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

fn build_mip_chain(level0: &[u8], atlas_w: u32, atlas_h: u32, n_mips: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(level0.len() * 2);
    out.extend_from_slice(level0);

    for level in 1..n_mips {
        let ts = TILE_PX >> level;
        let f = TILE_PX / ts;
        let lw = atlas_w >> level;
        let lh = atlas_h >> level;
        let tiles_x = ATLAS_COLS;
        let tiles_y = lh / ts;
        let mut buf = vec![0u8; (lw * lh * 4) as usize];

        for ty in 0..tiles_y {
            for tx in 0..tiles_x {
                let s_ox = tx * TILE_PX;
                let s_oy = ty * TILE_PX;
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
            }
        }
        out.extend_from_slice(&buf);
    }
    out
}

fn tinted(
    src: &image::RgbaImage,
    stem: &str,
    grass_tint: [u8; 4],
    foliage_tint: [u8; 4],
    dry_foliage_tint: [u8; 4],
    block: &BTreeMap<String, image::RgbaImage>,
) -> Option<image::RgbaImage> {
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
    const FIXED_TINTED: &[(&str, [u8; 4])] = &[
        ("spruce_leaves", [0x61, 0x99, 0x61, 255]),
        ("birch_leaves", [0x80, 0xa7, 0x55, 255]),
        ("lily_pad", [0x20, 0x80, 0x30, 255]),
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
        return Some(rgba);
    }
    if stem == "grass_block_side" {
        let overlay = block.get("grass_block_side_overlay")?;
        let mut overlay = overlay.clone();
        tint_rgba(&mut overlay, grass_tint);
        let mut rgba = src.clone();
        composite_over(&mut rgba, &overlay);
        return Some(rgba);
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
    let (w, h) = (
        base.width().min(overlay.width()),
        base.height().min(overlay.height()),
    );
    for y in 0..h {
        for x in 0..w {
            let s = overlay.get_pixel(x, y);
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

fn blit_tile(atlas: &mut [u8], tile: u32, atlas_w: u32, rgba: &image::RgbaImage) {
    let col = tile % ATLAS_COLS;
    let row = tile / ATLAS_COLS;
    let (ox, oy) = (col * TILE_PX, row * TILE_PX);
    for py in 0..TILE_PX {
        for px in 0..TILE_PX {
            let sx = px.min(rgba.width() - 1);
            let sy = py.min(rgba.height() - 1);
            let p = rgba.get_pixel(sx, sy);
            let dst = (((oy + py) * atlas_w + (ox + px)) * 4) as usize;
            atlas[dst..dst + 4].copy_from_slice(&p.0);
        }
    }
}

fn blit_gray_placeholder(atlas: &mut [u8], tile: u32, atlas_w: u32) {
    let col = tile % ATLAS_COLS;
    let row = tile / ATLAS_COLS;
    let (ox, oy) = (col * TILE_PX, row * TILE_PX);
    for py in 0..TILE_PX {
        for px in 0..TILE_PX {
            let edge = px == 0 || py == 0 || px == TILE_PX - 1 || py == TILE_PX - 1;
            let checker = ((px / 4) + (py / 4)) % 2 == 0;
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
    blit_gray_placeholder(&mut data, 0, TILE_PX);
    super::rgba_image(TILE_PX, TILE_PX, data, RenderAssetUsages::RENDER_WORLD)
}

fn pack_tiles(entries: &[(&str, &image::RgbaImage)]) -> (Vec<u8>, u32, u32, HashMap<String, u32>) {
    let n_tiles = 1 + entries.len() as u32;
    let atlas_w = ATLAS_COLS * TILE_PX;
    let atlas_h = n_tiles.div_ceil(ATLAS_COLS) * TILE_PX;

    let mut data = vec![0u8; (atlas_w * atlas_h * 4) as usize];
    blit_gray_placeholder(&mut data, 0, atlas_w);

    let mut tiles: HashMap<String, u32> = HashMap::new();
    for (idx, (stem, rgba)) in entries.iter().enumerate() {
        let tile = idx as u32 + 1;
        blit_tile(&mut data, tile, atlas_w, rgba);
        tiles.insert((*stem).to_owned(), tile);
    }

    (data, atlas_w, atlas_h, tiles)
}
