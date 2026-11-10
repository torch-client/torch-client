use std::collections::{BTreeMap, HashMap, HashSet};

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;

use super::TILE_PX;
#[cfg(feature = "animated_textures")]
use super::atlas_anim::{self, AnimatedSprite, AtlasFrames};
use super::atlas_meta::{self, FrameRect, SpriteLayout, SpriteMeta};

pub struct Sprites {
    tiles: HashMap<String, u32>,
    opaque: HashSet<String>,
    masks: HashMap<String, OpaqueMask>,
    grid: AtlasGrid,
    cells: CellSpans,
    #[cfg(feature = "render_lod")]
    tile_colors: std::sync::Arc<crate::renderer::tile_colors::TileColors>,
}

struct OpaqueMask {
    w: u32,
    h: u32,
    opaque: Vec<bool>,
}

impl OpaqueMask {
    fn of(rgba: &image::RgbaImage, layout: &SpriteLayout) -> Option<Self> {
        let first = layout.first_rect()?;
        let mut mask = Self {
            w: first.w,
            h: first.h,
            opaque: vec![true; (first.w * first.h) as usize],
        };
        let mut fold = |rect: FrameRect| {
            for y in 0..rect.h.min(mask.h) {
                for x in 0..rect.w.min(mask.w) {
                    if rgba.get_pixel(rect.x + x, rect.y + y)[3] != 255 {
                        mask.opaque[(y * mask.w + x) as usize] = false;
                    }
                }
            }
        };
        match layout {
            SpriteLayout::Animated(anim) => {
                anim.steps.iter().for_each(|step| fold(anim.rect(step.0)))
            }
            _ => fold(first),
        }
        Some(mask)
    }

    fn region_is_opaque(&self, [u0, v0, u1, v1]: [f32; 4]) -> bool {
        let span = |lo: f32, hi: f32, n: u32| {
            let (lo, hi) = (lo.min(hi), lo.max(hi));
            let a = ((lo / 16.0 * n as f32).floor().max(0.0) as u32).min(n.saturating_sub(1));
            let b = ((hi / 16.0 * n as f32).ceil().max(0.0) as u32).clamp(a + 1, n);
            a..b
        };
        let (xs, ys) = (span(u0, u1, self.w), span(v0, v1, self.h));
        ys.into_iter()
            .all(|y| xs.clone().all(|x| self.opaque[(y * self.w + x) as usize]))
    }
}

impl Default for Sprites {
    fn default() -> Self {
        Self {
            tiles: HashMap::new(),
            opaque: HashSet::new(),
            masks: HashMap::new(),
            grid: AtlasGrid { cols: 1, rows: 1 },
            cells: CellSpans::default(),
            #[cfg(feature = "render_lod")]
            tile_colors: Default::default(),
        }
    }
}

impl Sprites {
    pub fn tile(&self, stem: &str) -> Option<u32> {
        self.tiles.get(stem).copied()
    }

    pub fn sprite(&self, stem: &str) -> Option<SpriteTile> {
        self.tile(stem).map(|tile| SpriteTile {
            tile,
            span: self.span(tile),
        })
    }

    pub fn span(&self, tile: u32) -> u32 {
        self.cells.span(tile)
    }

    pub fn uv_rect(&self, tile: u32) -> [f32; 4] {
        crate::blocks::bake::tile_uv_rect(tile, self.span(tile), self.grid)
    }

    #[cfg_attr(not(feature = "render_lod"), allow(dead_code))]
    pub fn grid(&self) -> AtlasGrid {
        self.grid
    }

    pub fn is_opaque(&self, stem: &str) -> bool {
        self.opaque.contains(stem)
    }

    pub fn region_is_opaque(&self, stem: &str, uv: [f32; 4]) -> bool {
        if self.opaque.contains(stem) {
            return true;
        }
        self.masks.get(stem).is_some_and(|m| m.region_is_opaque(uv))
    }

    #[cfg(feature = "render_lod")]
    pub fn tile_colors(&self) -> &std::sync::Arc<crate::renderer::tile_colors::TileColors> {
        &self.tile_colors
    }

    pub fn mesh_tiles(&self) -> MeshTiles {
        MeshTiles {
            grid: self.grid,
            water_still: self.sprite("water_still").unwrap_or_default(),
            water_flow: self.sprite("water_flow").unwrap_or_default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtlasGrid {
    pub cols: u32,
    pub rows: u32,
}

impl AtlasGrid {
    pub fn cell(self, tile: u32) -> (u32, u32) {
        (tile % self.cols.max(1), tile / self.cols.max(1))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpriteTile {
    pub tile: u32,
    pub span: u32,
}

impl SpriteTile {
    pub const PLACEHOLDER: SpriteTile = SpriteTile { tile: 0, span: 1 };

    pub fn uv_rect(self, grid: AtlasGrid) -> [f32; 4] {
        crate::blocks::bake::tile_uv_rect(self.tile, self.span, grid)
    }
}

impl Default for SpriteTile {
    fn default() -> Self {
        Self::PLACEHOLDER
    }
}

#[derive(Clone, Copy)]
pub struct MeshTiles {
    pub grid: AtlasGrid,
    pub water_still: SpriteTile,
    pub water_flow: SpriteTile,
}

const MAX_SPAN: u32 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CellSpans {
    cols: u32,
    spans: Vec<u8>,
}

impl Default for CellSpans {
    fn default() -> Self {
        Self {
            cols: 1,
            spans: Vec::new(),
        }
    }
}

impl CellSpans {
    pub(crate) fn span(&self, cell: u32) -> u32 {
        self.spans
            .get(cell as usize)
            .map_or(1, |&n| u32::from(n.max(1)))
    }

    pub(crate) fn starts(&self, cell: u32) -> u32 {
        self.spans.get(cell as usize).map_or(1, |&n| u32::from(n))
    }

    pub(crate) fn cells(&self) -> u32 {
        self.spans.len() as u32
    }

    pub(crate) fn cols(&self) -> u32 {
        self.cols
    }

    pub(crate) fn rows(&self) -> u32 {
        self.cells() / self.cols
    }

    pub(crate) fn grid(&self) -> AtlasGrid {
        AtlasGrid {
            cols: self.cols,
            rows: self.rows().max(1),
        }
    }
}

#[cfg_attr(
    not(any(
        feature = "shader_support",
        feature = "animated_textures",
        feature = "render_lod",
        test
    )),
    allow(dead_code)
)]
pub(crate) fn block_cells(origin: u32, span: u32, cols: u32) -> impl Iterator<Item = u32> {
    (0..span * span).map(move |i| origin + (i / span) * cols + i % span)
}

pub(super) fn grid_cols(spans: &[u32]) -> u32 {
    let area: u32 = spans.iter().map(|&n| n.clamp(1, MAX_SPAN).pow(2)).sum();
    let widest = spans.iter().copied().max().unwrap_or(1).clamp(1, MAX_SPAN);
    let mut cols = 1;
    while cols * cols < area {
        cols += 1;
    }
    cols.max(widest)
}

pub(super) fn place_blocks(spans: &[u32], cols: u32) -> (Vec<u32>, CellSpans) {
    let cols = cols.max(1) as usize;
    let mut taken: Vec<bool> = Vec::new();
    let mut cover: Vec<u8> = Vec::new();
    let mut first_free = 0usize;
    let mut origins = Vec::with_capacity(spans.len());
    for &span in spans {
        let n = span.clamp(1, cols as u32) as usize;
        let free = |taken: &[bool], at: usize| {
            let (col, row) = (at % cols, at / cols);
            col + n <= cols
                && (0..n).all(|dy| {
                    (0..n).all(|dx| {
                        !taken
                            .get((row + dy) * cols + col + dx)
                            .copied()
                            .unwrap_or(false)
                    })
                })
        };
        let mut at = first_free;
        while !free(&taken, at) {
            at += 1;
        }
        let (col, row) = (at % cols, at / cols);
        let end = (row + n) * cols;
        if taken.len() < end {
            taken.resize(end, false);
            cover.resize(end, 1);
        }
        for dy in 0..n {
            for dx in 0..n {
                let cell = (row + dy) * cols + col + dx;
                taken[cell] = true;
                cover[cell] = 0;
            }
        }
        cover[at] = n as u8;
        origins.push(at as u32);
        while taken.get(first_free) == Some(&true) {
            first_free += 1;
        }
    }
    cover.resize(cover.len().max(cols), 1);
    (
        origins,
        CellSpans {
            cols: cols as u32,
            spans: cover,
        },
    )
}

fn span_of(rect: Option<FrameRect>, tile_px: u32) -> u32 {
    let Some(rect) = rect else { return 1 };
    let n = rect.w / tile_px.max(1);
    let fits = rect.w == rect.h && rect.w % tile_px.max(1) == 0 && rect.w <= MAX_TILE_PX;
    if fits && (2..=MAX_SPAN).contains(&n) {
        n
    } else {
        1
    }
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
pub(crate) struct UntintedTiles {
    pub cols: u32,
    pub tiles: Vec<(u32, Vec<Vec<u8>>)>,
}

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
    let Packed {
        data: level0,
        width: atlas_w,
        height: atlas_h,
        cells,
        ..
    } = pack_tiles(entries);
    let cols = cells.cols();
    let chain = if n_mips > 1 {
        build_mip_chain(&level0, atlas_w, atlas_h, n_mips, &cells)
    } else {
        level0
    };
    let mut out = UntintedTiles {
        cols,
        tiles: Vec::with_capacity(biome_tinted.len()),
    };
    for stem in biome_tinted {
        let Some(&tile) = tiles.get(*stem) else {
            continue;
        };
        for cell in block_cells(tile, cells.span(tile), cols) {
            out.tiles.push((
                cell,
                tile_levels(&chain, atlas_w, atlas_h, cols, cell, n_mips),
            ));
        }
    }
    out
}

#[cfg(any(feature = "shader_support", feature = "animated_textures", test))]
pub(super) fn tile_levels(
    chain: &[u8],
    atlas_w: u32,
    atlas_h: u32,
    cols: u32,
    tile: u32,
    n_mips: u32,
) -> Vec<Vec<u8>> {
    let tile_px = atlas_w / cols;
    let (tx, ty) = (tile % cols, tile / cols);
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
    let mut masks: HashMap<String, OpaqueMask> = HashMap::new();
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
        if !opaque.contains(stem) {
            if let Some(mask) = OpaqueMask::of(rgba, &layout) {
                masks.insert(stem.clone(), mask);
            }
        }
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

    let Packed {
        data: atlas_data,
        width: atlas_w,
        height: atlas_h,
        tile_px,
        tiles: tex_to_tile,
        cells,
    } = pack_tiles(&entries);
    #[cfg(feature = "render_lod")]
    let tile_colors = std::sync::Arc::new(crate::renderer::tile_colors::TileColors::from_atlas(
        &atlas_data,
        atlas_w,
        atlas_h,
        &cells,
    ));
    #[cfg(feature = "shader_support")]
    let untinted_entries: Vec<PackEntry> = entries
        .iter()
        .map(|(stem, rgba, rect)| match textures.block.get(*stem) {
            Some(source) if biome_tinted.contains(stem) => (*stem, source, *rect),
            _ => (*stem, *rgba, *rect),
        })
        .collect();

    let n_mips = tile_px.trailing_zeros() + 1;
    let mip_data = if n_mips > 1 {
        build_mip_chain(&atlas_data, atlas_w, atlas_h, n_mips, &cells)
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
                let atlas_tile = *tex_to_tile.get(*stem)?;
                Some(AnimatedSprite {
                    atlas_tile,
                    span: cells.span(atlas_tile),
                    image: rgba,
                    pack_image,
                    layout,
                })
            })
            .collect();
        atlas_anim::build_frames(&sprites, (atlas_w, atlas_h), cells.cols(), n_mips)
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
        "{} block textures loaded ({} fully opaque), atlas {}×{} ({}×{} cells of {} px), {} mip levels",
        entries.len(),
        opaque_count,
        atlas_w,
        atlas_h,
        cells.cols(),
        cells.rows(),
        tile_px,
        n_mips
    );
    #[cfg(any(target_arch = "wasm32", target_os = "android"))]
    if atlas_w.max(atlas_h) > SMALL_DEVICE_MAX_TEXTURE {
        crate::log_warn!(
            "atlas",
            "atlas {atlas_w}×{atlas_h} is over {SMALL_DEVICE_MAX_TEXTURE}, the 2D texture size a small device may offer"
        );
    }
    let grid = cells.grid();
    BlockAtlas {
        image,
        sprites: Sprites {
            tiles: tex_to_tile,
            opaque,
            masks,
            grid,
            cells,
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

pub(super) fn build_mip_chain(
    level0: &[u8],
    atlas_w: u32,
    atlas_h: u32,
    n_mips: u32,
    cells: &CellSpans,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(level0.len() * 2);
    out.extend_from_slice(level0);

    let cols = cells.cols();
    let tile_px = atlas_w / cols;
    let tiles_y0 = atlas_h / tile_px;
    let cutout_coverage: Vec<Option<f32>> = (0..tiles_y0 * cols)
        .map(|t| {
            let span = cells.starts(t);
            if span == 0 {
                return None;
            }
            let side = span * tile_px;
            let (s_ox, s_oy) = ((t % cols) * tile_px, (t / cols) * tile_px);
            let alpha: Vec<u8> = (0..side * side)
                .map(|i| level0[(((s_oy + i / side) * atlas_w + s_ox + i % side) * 4 + 3) as usize])
                .collect();
            alpha
                .contains(&0)
                .then(|| alpha_test_coverage(&alpha, side, MIP_ALPHA_CUTOFF, 1.0))
        })
        .collect();
    for level in 1..n_mips {
        let ts = tile_px >> level;
        let f = tile_px / ts;
        let lw = atlas_w >> level;
        let lh = atlas_h >> level;
        let tiles_x = cols;
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
            }
        }
        for (t, coverage) in cutout_coverage.iter().enumerate() {
            let Some(coverage) = coverage else { continue };
            let t = t as u32;
            let origin = ((t % tiles_x) * ts, (t / tiles_x) * ts);
            scale_tile_alpha_to_coverage(&mut buf, lw, origin, cells.span(t) * ts, *coverage);
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
    span: u32,
    (atlas_w, cols): (u32, u32),
    rgba: &image::RgbaImage,
    rect: FrameRect,
) {
    let tile_px = atlas_w / cols;
    let side = span * tile_px;
    let (col, row) = (tile % cols, tile / cols);
    let (ox, oy) = (col * tile_px, row * tile_px);
    for py in 0..side {
        for px in 0..side {
            let sx = (rect.x + px * rect.w / side).min(rgba.width() - 1);
            let sy = (rect.y + py * rect.h / side).min(rgba.height() - 1);
            let p = rgba.get_pixel(sx, sy);
            let dst = (((oy + py) * atlas_w + (ox + px)) * 4) as usize;
            atlas[dst..dst + 4].copy_from_slice(&p.0);
        }
    }
}

fn blit_gray_placeholder(atlas: &mut [u8], tile: u32, atlas_w: u32, cols: u32) {
    let tile_px = atlas_w / cols;
    let (col, row) = (tile % cols, tile / cols);
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
    blit_gray_placeholder(&mut data, 0, TILE_PX, 1);
    #[cfg_attr(not(feature = "shader_support"), allow(unused_mut))]
    let mut image = super::rgba_image(TILE_PX, TILE_PX, data, RenderAssetUsages::RENDER_WORLD);
    #[cfg(feature = "shader_support")]
    super::packdraw::allow_display_view(&mut image);
    image
}

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
const SMALL_DEVICE_MAX_TEXTURE: u32 = 2048;

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

struct Packed {
    data: Vec<u8>,
    width: u32,
    height: u32,
    tile_px: u32,
    tiles: HashMap<String, u32>,
    cells: CellSpans,
}

fn pack_tiles(entries: &[PackEntry]) -> Packed {
    let tile_px = tile_px(entries);
    let spans: Vec<u32> = std::iter::once(1)
        .chain(entries.iter().map(|(_, _, rect)| span_of(*rect, tile_px)))
        .collect();
    let (origins, cells) = place_blocks(&spans, grid_cols(&spans));
    let cols = cells.cols();
    let atlas_w = cols * tile_px;
    let atlas_h = cells.rows() * tile_px;

    let mut data = vec![0u8; (atlas_w * atlas_h * 4) as usize];
    blit_gray_placeholder(&mut data, origins[0], atlas_w, cols);

    let mut tiles: HashMap<String, u32> = HashMap::new();
    for ((stem, rgba, rect), (&tile, &span)) in
        entries.iter().zip(origins.iter().zip(&spans).skip(1))
    {
        match rect {
            Some(rect) => blit_tile(&mut data, tile, span, (atlas_w, cols), rgba, *rect),
            None => blit_gray_placeholder(&mut data, tile, atlas_w, cols),
        }
        tiles.insert((*stem).to_owned(), tile);
    }

    Packed {
        data,
        width: atlas_w,
        height: atlas_h,
        tile_px,
        tiles,
        cells,
    }
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
            atlas.sprites.grid().cols,
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

    fn solid(side: u32, rgba: [u8; 4]) -> image::RgbaImage {
        image::RgbaImage::from_pixel(side, side, image::Rgba(rgba))
    }

    fn big() -> image::RgbaImage {
        image::RgbaImage::from_fn(32, 32, |x, y| {
            let alpha = if x < 16 && y < 16 { 0 } else { 255 };
            image::Rgba([x as u8 * 8, y as u8 * 8, 200, alpha])
        })
    }

    const BLUE: [u8; 4] = [0, 0, 255, 255];

    fn with_big() -> BlockAtlas {
        let mut sprites = vec![("big", big(), None)];
        let names: Vec<String> = (0..20).map(|i| format!("a{i:02}")).collect();
        sprites.extend(
            names
                .iter()
                .map(|name| (name.as_str(), solid(16, BLUE), None)),
        );
        build_block_atlas(&textures(sprites), "/nonexistent", false)
    }

    #[test]
    fn single_spans_pack_in_row_major_order() {
        let (origins, cells) = place_blocks(&[1; 40], 16);
        assert_eq!(origins, (0..40).collect::<Vec<u32>>());
        assert_eq!(cells.rows(), 3);
    }

    #[test]
    fn blocks_take_the_first_square_that_fits_and_leave_no_hole() {
        let mut spans = vec![1, 2];
        spans.extend([1; 20]);
        let (origins, cells) = place_blocks(&spans, 16);
        assert_eq!(&origins[..4], &[0, 1, 3, 4]);
        assert_eq!(origins[14], 15);
        assert_eq!(&origins[15..17], &[16, 19]);
        assert_eq!(
            (
                cells.starts(1),
                cells.starts(2),
                cells.starts(17),
                cells.starts(18)
            ),
            (2, 0, 0, 0)
        );
        assert_eq!(cells.rows(), 2);

        let mut spans = vec![1; 15];
        spans.extend([2, 1]);
        let (origins, _) = place_blocks(&spans, 16);
        assert_eq!(&origins[15..], &[16, 15]);
    }

    #[test]
    fn a_32px_sprite_gets_a_2x2_block_at_full_resolution() {
        let atlas = with_big();
        let tile = atlas.sprites.tile("big").unwrap();
        assert_eq!(atlas.sprites.span(tile), 2);
        assert_eq!(
            atlas.sprites.sprite("big"),
            Some(SpriteTile { tile, span: 2 })
        );
        let source = big();
        for (i, cell) in block_cells(tile, 2, atlas.sprites.grid().cols).enumerate() {
            let level0 = &atlas_levels(&atlas, cell)[0];
            let (ox, oy) = ((i as u32 % 2) * 16, (i as u32 / 2) * 16);
            for y in 0..16 {
                for x in 0..16 {
                    let at = ((y * 16 + x) * 4) as usize;
                    assert_eq!(
                        &level0[at..at + 4],
                        &source.get_pixel(ox + x, oy + y).0,
                        "cell {cell} texel {x},{y}"
                    );
                }
            }
        }
        let block: Vec<u32> = block_cells(tile, 2, atlas.sprites.grid().cols).collect();
        for i in 0..20 {
            let other = atlas.sprites.tile(&format!("a{i:02}")).unwrap();
            assert!(!block.contains(&other), "a{i:02} at {other}");
        }
    }

    #[test]
    fn a_blocks_uv_rect_covers_exactly_the_block() {
        let atlas = with_big();
        let AtlasGrid { cols, rows } = atlas.sprites.grid();
        let tile = atlas.sprites.tile("big").unwrap();
        let [u0, v0, u1, v1] = atlas.sprites.uv_rect(tile);
        let (col, row) = (tile % cols, tile / cols);
        let cell = |u: f32, n: u32| u * n as f32;
        assert_eq!(cell(u0, cols).round() as u32, col);
        assert_eq!(cell(u1, cols).round() as u32, col + 2);
        assert_eq!(cell(v0, rows).round() as u32, row);
        assert_eq!(cell(v1, rows).round() as u32, row + 2);
        assert!(cell(u0, cols) > col as f32 && cell(u1, cols) < (col + 2) as f32);
        assert!(cell(v0, rows) > row as f32 && cell(v1, rows) < (row + 2) as f32);
    }

    #[test]
    fn sixteen_px_tiles_are_unchanged() {
        let names: Vec<String> = (0..20).map(|i| format!("a{i:02}")).collect();
        let plain: Vec<_> = names
            .iter()
            .map(|name| (name.as_str(), solid(16, BLUE), None))
            .collect();
        let atlas = build_block_atlas(&textures(plain), "/nonexistent", false);
        for (i, name) in names.iter().enumerate() {
            let tile = atlas.sprites.tile(name).unwrap();
            assert_eq!(tile, i as u32 + 1);
            assert_eq!(atlas.sprites.span(tile), 1);
            assert_eq!(
                atlas.sprites.uv_rect(tile),
                crate::blocks::bake::tile_uv_rect(tile, 1, atlas.sprites.grid())
            );
        }
        assert_eq!(atlas.sprites.grid(), AtlasGrid { cols: 5, rows: 5 });
        assert_eq!(atlas.image.texture_descriptor.size.width, 5 * TILE_PX);
        assert_eq!(atlas.image.texture_descriptor.size.height, 5 * TILE_PX);

        let atlas = with_big();
        for i in 0..20 {
            let tile = atlas.sprites.tile(&format!("a{i:02}")).unwrap();
            assert_eq!(atlas.sprites.span(tile), 1);
            for level in atlas_levels(&atlas, tile) {
                assert!(level.chunks_exact(4).all(|p| p == BLUE), "a{i:02}");
            }
        }
    }

    #[test]
    fn mips_stay_within_the_block() {
        let atlas = with_big();
        let tile = atlas.sprites.tile("big").unwrap();
        let levels: Vec<Vec<Vec<u8>>> = block_cells(tile, 2, atlas.sprites.grid().cols)
            .map(|cell| atlas_levels(&atlas, cell))
            .collect();
        assert_eq!(levels[0].len(), 5);
        for (cell, cell_levels) in levels.iter().enumerate() {
            for (level, bytes) in cell_levels.iter().enumerate() {
                assert!(
                    bytes.chunks_exact(4).all(|p| p[2] == 200 || p[2] == 0),
                    "cell {cell} level {level}"
                );
            }
        }
        for level in 0..5 {
            let alphas: Vec<u8> = levels[1..]
                .iter()
                .flat_map(|cell| cell[level].chunks_exact(4).map(|p| p[3]))
                .collect();
            assert!(alphas.iter().all(|a| *a == alphas[0]), "level {level}");
        }
    }

    #[cfg(feature = "animated_textures")]
    #[test]
    fn a_32px_animated_sprite_animates_its_whole_block() {
        let textures = textures(vec![
            (
                "a",
                strip(32, 3),
                Some(r#"{"animation":{"frames":[1,0,2]}}"#),
            ),
            ("b", strip(16, 1), None),
            ("c", strip(16, 1), None),
        ]);
        let atlas = build_block_atlas(&textures, "/nonexistent", true);
        let tile = atlas.sprites.tile("a").unwrap();
        assert_eq!(atlas.sprites.span(tile), 2);
        let frames = atlas.tables.frames.as_ref().expect("one animated sprite");
        let animation = &frames.animations[0];
        assert_eq!(animation.span, 2);
        let first = animation.frames[0].tile;
        for (i, cell) in block_cells(tile, 2, atlas.sprites.grid().cols).enumerate() {
            assert_eq!(
                frames.block_levels(first + i as u32),
                atlas_levels(&atlas, cell),
                "cell {cell}"
            );
        }
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

    fn gradient(side: u32) -> image::RgbaImage {
        image::RgbaImage::from_fn(side, side, |x, y| {
            let alpha = if x < side / 2 && y < side / 2 { 0 } else { 255 };
            image::Rgba([x as u8 * 3, y as u8 * 3, 200, alpha])
        })
    }

    fn rect(side: u32) -> Option<FrameRect> {
        Some(FrameRect {
            x: 0,
            y: 0,
            w: side,
            h: side,
        })
    }

    #[test]
    fn span_follows_the_size_ratio_up_to_the_cap() {
        assert_eq!(span_of(None, 16), 1);
        assert_eq!(span_of(rect(16), 16), 1);
        assert_eq!(span_of(rect(32), 16), 2);
        assert_eq!(span_of(rect(64), 16), 4);
        assert_eq!(span_of(rect(64), 32), 2);
        assert_eq!(span_of(rect(128), 16), 1);
        assert_eq!(
            span_of(
                Some(FrameRect {
                    x: 0,
                    y: 0,
                    w: 64,
                    h: 32
                }),
                16
            ),
            1
        );
    }

    #[test]
    fn a_64px_sprite_gets_a_4x4_block_at_full_resolution() {
        let source = gradient(64);
        let names: Vec<String> = (0..20).map(|i| format!("a{i:02}")).collect();
        let mut sprites = vec![("big", source.clone(), None)];
        sprites.extend(
            names
                .iter()
                .map(|name| (name.as_str(), solid(16, BLUE), None)),
        );
        let atlas = build_block_atlas(&textures(sprites), "/nonexistent", false);
        let tile = atlas.sprites.tile("big").unwrap();
        let cols = atlas.sprites.grid().cols;
        assert_eq!(atlas.sprites.span(tile), 4);
        assert_eq!(
            atlas.sprites.sprite("big"),
            Some(SpriteTile { tile, span: 4 })
        );
        let block: Vec<u32> = block_cells(tile, 4, cols).collect();
        assert_eq!(block.len(), 16);
        for (i, &cell) in block.iter().enumerate() {
            let levels = atlas_levels(&atlas, cell);
            assert_eq!(levels.len(), 5);
            let (ox, oy) = ((i as u32 % 4) * 16, (i as u32 / 4) * 16);
            for y in 0..16 {
                for x in 0..16 {
                    let at = ((y * 16 + x) * 4) as usize;
                    assert_eq!(
                        &levels[0][at..at + 4],
                        &source.get_pixel(ox + x, oy + y).0,
                        "cell {cell} texel {x},{y}"
                    );
                }
            }
        }
        for name in &names {
            let other = atlas.sprites.tile(name).unwrap();
            assert!(!block.contains(&other), "{name} at {other}");
            assert_eq!(atlas.sprites.span(other), 1);
        }
        for level in 0..5 {
            let alphas: Vec<u8> = block[12..]
                .iter()
                .flat_map(|&cell| {
                    atlas_levels(&atlas, cell)[level]
                        .chunks_exact(4)
                        .map(|p| p[3])
                        .collect::<Vec<_>>()
                })
                .collect();
            assert!(alphas.iter().all(|a| *a == alphas[0]), "level {level}");
        }
    }

    #[test]
    fn the_grid_grows_square_and_keeps_every_cell() {
        for count in [1usize, 20, 100, 1_000, 1_300, 5_000] {
            let spans = vec![1; count];
            let cols = grid_cols(&spans);
            let (origins, cells) = place_blocks(&spans, cols);
            assert_eq!(cells.cols(), cols);
            assert_eq!(origins.last(), Some(&(count as u32 - 1)));
            assert!(
                cells.rows() <= cols,
                "{count} cells: {cols} x {}",
                cells.rows()
            );
            assert!(cells.rows() * cols >= count as u32);
            assert!(
                cols - cells.rows() <= 1 + cols / 8 || count < 4,
                "{count} cells: {cols} x {}",
                cells.rows()
            );
        }
        let mut spans = vec![1; 300];
        spans.extend([4; 40]);
        let cols = grid_cols(&spans);
        let (origins, cells) = place_blocks(&spans, cols);
        assert_eq!(cols, 31);
        assert!(cells.rows() <= cols + cols / 4, "{} rows", cells.rows());
        assert_eq!(grid_cols(&[4]), 4);
        assert_eq!(place_blocks(&spans, cols).0, origins);
    }

    #[test]
    fn a_pack_atlas_is_square_and_its_mips_follow_the_cell() {
        let names: Vec<String> = (0..399).map(|i| format!("s{i:03}")).collect();
        let sprites: Vec<_> = names
            .iter()
            .map(|name| (name.as_str(), solid(32, BLUE), None))
            .collect();
        let atlas = build_block_atlas(&textures(sprites), "/nonexistent", false);
        let grid = atlas.sprites.grid();
        let size = atlas.image.texture_descriptor.size;
        assert_eq!(grid, AtlasGrid { cols: 20, rows: 20 });
        assert_eq!((size.width, size.height), (640, 640));
        assert_eq!(atlas.image.texture_descriptor.mip_level_count, 6);
        for stem in ["s000", "s398"] {
            let tile = atlas.sprites.tile(stem).unwrap();
            assert!(
                atlas_levels(&atlas, tile)
                    .iter()
                    .all(|l| l.chunks_exact(4).all(|p| p == BLUE)),
                "{stem}"
            );
        }
    }

    #[test]
    fn a_tile_round_trips_through_its_uv_for_any_column_count() {
        let grid = AtlasGrid { cols: 37, rows: 9 };
        for tile in 0..grid.cols * grid.rows {
            let [u0, v0, u1, v1] = SpriteTile { tile, span: 1 }.uv_rect(grid);
            let (col, row) = grid.cell(tile);
            assert_eq!((col, row), (tile % 37, tile / 37));
            let centre = [[(u0 + u1) / 2.0, (v0 + v1) / 2.0]; 4];
            assert_eq!(
                crate::renderer::tile_colors::tile_of_uv(&centre, grid),
                tile
            );
        }
        let [u0, v0, u1, v1] = SpriteTile {
            tile: 37 + 5,
            span: 3,
        }
        .uv_rect(grid);
        assert_eq!(((u1 - u0) * 37.0).round(), 3.0);
        assert_eq!(((v1 - v0) * 9.0).round(), 3.0);
    }
}
