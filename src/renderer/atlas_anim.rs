use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use bevy::prelude::*;
use bevy::render::Extract;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::TextureViewId;
use bevy::render::render_resource::{
    Buffer, BufferInitDescriptor, BufferUsages, CommandEncoder, CommandEncoderDescriptor, Extent3d,
    Origin3d, TexelCopyBufferInfo, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture,
    TextureAspect,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::texture::GpuImage;
use parking_lot::Mutex;

use super::atlas::{blit_tile, block_cells, build_mip_chain, grid_cols, place_blocks, tile_levels};
use super::atlas_meta::{AnimLayout, FrameRect};
use super::terrain_pool::TerrainTextures;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameInfo {
    pub tile: u32,
    pub pack_tile: u32,
    pub time: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    pub atlas_tile: u32,
    pub span: u32,
    pub frames: Vec<FrameInfo>,
    pub interpolate: bool,
    pub blend_slot: Option<u32>,
    pub pack_blend_slot: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnimationState {
    frame: usize,
    sub_frame: u32,
    dirty: bool,
}

impl AnimationState {
    fn tick(&mut self, frames: &[FrameInfo]) {
        self.dirty = false;
        self.sub_frame += 1;
        if self.sub_frame >= frames[self.frame].time {
            let old = frames[self.frame].tile;
            self.frame = (self.frame + 1) % frames.len();
            self.sub_frame = 0;
            self.dirty = old != frames[self.frame].tile;
        }
    }

    fn blend(&self, frames: &[FrameInfo]) -> (FrameInfo, FrameInfo, u32) {
        let current = frames[self.frame];
        let next = frames[(self.frame + 1) % frames.len()];
        (
            current,
            next,
            (self.sub_frame as f32 / current.time as f32 * 1000.0) as u32,
        )
    }
}

fn blend_byte(from: u8, to: u8, q: u32) -> u8 {
    ((u32::from(from) * (1000 - q) + u32::from(to) * q + 500) / 1000) as u8
}

const PLACEMENT_ALIGN: u32 = 512;

const ROW_ALIGN: u32 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockLevel {
    side: u32,
    offset: u32,
    pitch: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TileBlocks {
    levels: Vec<BlockLevel>,
    stride: u32,
    atlas_cols: u32,
}

impl TileBlocks {
    fn new(tile_px: u32, n_mips: u32, atlas_cols: u32) -> TileBlocks {
        let mut stride = 0;
        let levels = (0..n_mips)
            .map(|level| {
                let side = tile_px >> level;
                let pitch = (side * 4).next_multiple_of(ROW_ALIGN);
                let at = BlockLevel {
                    side,
                    offset: stride,
                    pitch,
                };
                stride += (pitch * side).next_multiple_of(PLACEMENT_ALIGN);
                at
            })
            .collect();
        TileBlocks {
            levels,
            stride,
            atlas_cols,
        }
    }

    fn write(&self, block: &mut [u8], levels: &[Vec<u8>]) {
        for (at, bytes) in self.levels.iter().zip(levels) {
            let row = (at.side * 4) as usize;
            for (y, source) in bytes.chunks_exact(row).enumerate() {
                let start = at.offset as usize + y * at.pitch as usize;
                block[start..start + row].copy_from_slice(source);
            }
        }
    }

    fn blend(&self, block: &mut [u8], from: &[Vec<u8>], to: &[Vec<u8>], q: u32) {
        for ((at, from), to) in self.levels.iter().zip(from).zip(to) {
            let row = (at.side * 4) as usize;
            for (y, (from, to)) in from.chunks_exact(row).zip(to.chunks_exact(row)).enumerate() {
                let start = at.offset as usize + y * at.pitch as usize;
                for ((out, a), b) in block[start..start + row].iter_mut().zip(from).zip(to) {
                    *out = blend_byte(*a, *b, q);
                }
            }
        }
    }

    pub(crate) fn copy_tile(
        &self,
        encoder: &mut CommandEncoder,
        buffer: &Buffer,
        texture: &Texture,
        tile: u32,
        atlas_tile: u32,
        span: u32,
    ) {
        for (block, cell) in (tile..).zip(block_cells(atlas_tile, span, self.atlas_cols)) {
            let (tx, ty) = (cell % self.atlas_cols, cell / self.atlas_cols);
            for (level, at) in self.levels.iter().enumerate() {
                encoder.copy_buffer_to_texture(
                    TexelCopyBufferInfo {
                        buffer,
                        layout: TexelCopyBufferLayout {
                            offset: u64::from(block) * u64::from(self.stride)
                                + u64::from(at.offset),
                            bytes_per_row: Some(at.pitch),
                            rows_per_image: None,
                        },
                    },
                    TexelCopyTextureInfo {
                        texture,
                        mip_level: level as u32,
                        origin: Origin3d {
                            x: tx * at.side,
                            y: ty * at.side,
                            z: 0,
                        },
                        aspect: TextureAspect::All,
                    },
                    Extent3d {
                        width: at.side,
                        height: at.side,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
    }
}

pub struct AtlasFrames {
    #[cfg_attr(not(feature = "shader_support"), allow(dead_code))]
    pub generation: u64,
    atlas_size: (u32, u32),
    pub animations: Vec<Animation>,
    pub layout: TileBlocks,
    blocks: Vec<u8>,
    first_slot: u32,
    slots: u32,
    kept: HashMap<u32, Vec<Vec<u8>>>,
}

impl AtlasFrames {
    #[cfg(test)]
    pub(super) fn block_levels(&self, tile: u32) -> Vec<Vec<u8>> {
        let block = &self.blocks[(tile * self.layout.stride) as usize..];
        let level = |at: &BlockLevel| {
            let rows = (0..at.side).flat_map(|y| {
                let start = (at.offset + y * at.pitch) as usize;
                block[start..start + (at.side * 4) as usize].iter().copied()
            });
            rows.collect()
        };
        self.layout.levels.iter().map(level).collect()
    }
}

pub(super) struct AnimatedSprite<'a> {
    pub atlas_tile: u32,
    pub span: u32,
    pub image: &'a image::RgbaImage,
    pub pack_image: Option<&'a image::RgbaImage>,
    pub layout: &'a AnimLayout,
}

pub(super) fn build_frames(
    sprites: &[AnimatedSprite],
    atlas_size: (u32, u32),
    atlas_cols: u32,
    n_mips: u32,
) -> Option<AtlasFrames> {
    if sprites.is_empty() {
        return None;
    }
    let tile_px = atlas_size.0 / atlas_cols;
    let mut placed: Vec<(&image::RgbaImage, FrameRect, u32, u32)> = Vec::new();
    let mut next_block = 0u32;
    let mut animations: Vec<Animation> = sprites
        .iter()
        .map(|sprite| {
            let span = sprite.span.max(1);
            let (mut own, mut pack) = (HashMap::new(), HashMap::new());
            let mut place = |image, seen: &mut HashMap<u32, u32>, index| {
                *seen.entry(index).or_insert_with(|| {
                    placed.push((image, sprite.layout.rect(index), span, next_block));
                    next_block += span * span;
                    next_block - span * span
                })
            };
            let frames = sprite
                .layout
                .steps
                .iter()
                .map(|&(index, time)| {
                    let tile = place(sprite.image, &mut own, index);
                    let pack_tile = sprite
                        .pack_image
                        .map_or(tile, |image| place(image, &mut pack, index));
                    FrameInfo {
                        tile,
                        pack_tile,
                        time,
                    }
                })
                .collect();
            Animation {
                atlas_tile: sprite.atlas_tile,
                span,
                frames,
                interpolate: sprite.layout.interpolate,
                blend_slot: None,
                pack_blend_slot: None,
            }
        })
        .collect();

    let first_slot = next_block;
    let mut next_slot = first_slot;
    let mut slot = |span: u32| {
        next_slot += span * span;
        next_slot - span * span
    };
    let mut kept: HashMap<u32, Vec<Vec<u8>>> = HashMap::new();
    for animation in animations
        .iter_mut()
        .filter(|animation| animation.interpolate)
    {
        let span = animation.span;
        animation.blend_slot = Some(slot(span));
        if animation
            .frames
            .iter()
            .any(|frame| frame.pack_tile != frame.tile)
        {
            animation.pack_blend_slot = Some(slot(span));
        }
        for frame in &animation.frames {
            for i in 0..span * span {
                kept.extend([
                    (frame.tile + i, Vec::new()),
                    (frame.pack_tile + i, Vec::new()),
                ]);
            }
        }
    }

    let spans: Vec<u32> = placed.iter().map(|&(_, _, span, _)| span).collect();
    let frames_cols = grid_cols(&spans);
    let (origins, cells) = place_blocks(&spans, frames_cols);
    let (frames_w, frames_h) = (frames_cols * tile_px, cells.rows() * tile_px);
    let mut level0 = vec![0u8; (frames_w * frames_h * 4) as usize];
    for (&(image, rect, span, _), &origin) in placed.iter().zip(&origins) {
        blit_tile(
            &mut level0,
            origin,
            span,
            (frames_w, frames_cols),
            image,
            rect,
        );
    }
    let chain = build_mip_chain(&level0, frames_w, frames_h, n_mips, &cells);

    let layout = TileBlocks::new(tile_px, n_mips, atlas_cols);
    let mut blocks = vec![0u8; layout.stride as usize * next_slot as usize];
    for (&(_, _, span, first), &origin) in placed.iter().zip(&origins) {
        for (tile, cell) in (first..).zip(block_cells(origin, span, frames_cols)) {
            let at = (tile * layout.stride) as usize;
            let levels = tile_levels(&chain, frames_w, frames_h, frames_cols, cell, n_mips);
            layout.write(&mut blocks[at..at + layout.stride as usize], &levels);
            if let Some(keep) = kept.get_mut(&tile) {
                *keep = levels;
            }
        }
    }
    crate::log_info!(
        "atlas",
        "{} animated sprites, {} frame tiles, {} blend slots, {} KiB",
        animations.len(),
        first_slot,
        next_slot - first_slot,
        blocks.len() / 1024
    );
    Some(AtlasFrames {
        generation: 0,
        atlas_size,
        animations,
        layout,
        blocks,
        first_slot,
        slots: next_slot - first_slot,
        kept,
    })
}

static PENDING: Mutex<Option<Option<AtlasFrames>>> = Mutex::new(None);

static GENERATION: AtomicU64 = AtomicU64::new(0);

pub(super) fn publish(mut frames: Option<AtlasFrames>) {
    if let Some(frames) = &mut frames {
        frames.generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    }
    *PENDING.lock() = Some(frames);
}

const TICKS_PER_SECOND: f32 = 20.0;

#[derive(Resource, Default)]
pub struct AtlasClock {
    residual: f32,
    step: bool,
}

impl AtlasClock {
    fn advance(&mut self, seconds: f32) {
        self.residual += seconds * TICKS_PER_SECOND;
        let ticks = self.residual.floor();
        self.residual -= ticks;
        self.step = ticks >= 1.0;
    }
}

pub fn extract_atlas_clock(
    time: Extract<Res<Time<bevy::time::Real>>>,
    mut clock: ResMut<AtlasClock>,
) {
    clock.advance(time.delta_secs());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileCopy {
    pub atlas_tile: u32,
    pub tile: u32,
    #[cfg_attr(not(feature = "shader_support"), allow(dead_code))]
    pub pack_tile: u32,
    pub span: u32,
}

#[derive(Resource, Default)]
pub struct AtlasFrameCopies(pub Vec<TileCopy>);

#[derive(Resource, Default)]
pub struct AtlasAnimator {
    view: Option<TextureViewId>,
    frames: Option<AtlasFrames>,
    buffer: Option<Buffer>,
    states: Vec<AnimationState>,
    tail: Vec<u8>,
}

impl AtlasAnimator {
    fn follow(&mut self, gpu: &GpuImage, device: &RenderDevice) {
        let view = gpu.texture_view.id();
        if self.view == Some(view) {
            return;
        }
        self.view = Some(view);
        let size = (gpu.size.width, gpu.size.height);
        {
            let mut pending = PENDING.lock();
            match pending.as_ref() {
                Some(Some(frames)) if frames.atlas_size != size => self.drop_frames(),
                Some(_) => {
                    self.drop_frames();
                    self.frames = pending.take().flatten();
                }
                None => {}
            }
        }
        if let Some(frames) = &mut self.frames {
            if frames.atlas_size != size {
                self.drop_frames();
                return;
            }
            if !frames.blocks.is_empty() {
                let blocks = std::mem::take(&mut frames.blocks);
                self.buffer = Some(device.create_buffer_with_data(&BufferInitDescriptor {
                    label: Some("atlas_frames"),
                    contents: &blocks,
                    usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
                }));
            }
        }
        self.reset_states();
    }

    #[cfg(feature = "shader_support")]
    pub(crate) fn live(&self, view: TextureViewId) -> Option<(&AtlasFrames, &Buffer)> {
        if self.view != Some(view) {
            return None;
        }
        Some((self.frames.as_ref()?, self.buffer.as_ref()?))
    }

    #[cfg(feature = "shader_support")]
    pub(crate) fn pack_tiles(&self) -> impl Iterator<Item = (u32, u32, u32)> {
        let animations = self.frames.iter().flat_map(|frames| &frames.animations);
        animations.zip(&self.states).map(|(animation, state)| {
            (
                animation.atlas_tile,
                animation.frames[state.frame].pack_tile,
                animation.span,
            )
        })
    }

    fn reset_states(&mut self) {
        let frames = self.frames.as_ref();
        self.states =
            vec![AnimationState::default(); frames.map_or(0, |frames| frames.animations.len())];
        self.tail =
            vec![0; frames.map_or(0, |frames| (frames.slots * frames.layout.stride) as usize)];
    }

    fn drop_frames(&mut self) {
        self.frames = None;
        self.buffer = None;
        self.states.clear();
        self.tail.clear();
    }

    fn tick(&mut self, copies: &mut Vec<TileCopy>) {
        let Some(frames) = &self.frames else { return };
        let stride = frames.layout.stride as usize;
        let mut mix = |slot: u32, from: u32, to: u32, q: u32, span: u32| {
            for i in 0..span * span {
                let at = (slot + i - frames.first_slot) as usize * stride;
                let block = &mut self.tail[at..at + stride];
                if let (Some(from), Some(to)) =
                    (frames.kept.get(&(from + i)), frames.kept.get(&(to + i)))
                {
                    frames.layout.blend(block, from, to, q);
                }
            }
        };
        for (animation, state) in frames.animations.iter().zip(&mut self.states) {
            state.tick(&animation.frames);
            let (atlas_tile, span) = (animation.atlas_tile, animation.span);
            if let Some(slot) = animation.blend_slot {
                let (current, next, q) = state.blend(&animation.frames);
                mix(slot, current.tile, next.tile, q, span);
                let pack_tile = animation.pack_blend_slot.unwrap_or(slot);
                if pack_tile != slot {
                    mix(pack_tile, current.pack_tile, next.pack_tile, q, span);
                }
                copies.push(TileCopy {
                    atlas_tile,
                    tile: slot,
                    pack_tile,
                    span,
                });
            } else if state.dirty {
                let frame = animation.frames[state.frame];
                copies.push(TileCopy {
                    atlas_tile,
                    tile: frame.tile,
                    pack_tile: frame.pack_tile,
                    span,
                });
            }
        }
    }
}

pub fn upload_atlas_frames(
    textures: Option<Res<TerrainTextures>>,
    images: Res<RenderAssets<GpuImage>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    clock: Res<AtlasClock>,
    mut animator: ResMut<AtlasAnimator>,
    mut copies: ResMut<AtlasFrameCopies>,
) {
    crate::prof_span!("render:upload_atlas_frames");
    copies.0.clear();
    let Some(textures) = textures else { return };
    let Some(gpu) = images.get(&textures.atlas) else {
        return;
    };
    animator.follow(gpu, &device);
    if !clock.step {
        return;
    }
    animator.tick(&mut copies.0);
    if copies.0.is_empty() {
        return;
    }
    let (Some(frames), Some(buffer)) = (&animator.frames, &animator.buffer) else {
        return;
    };
    if !animator.tail.is_empty() {
        queue.write_buffer(
            buffer,
            u64::from(frames.first_slot) * u64::from(frames.layout.stride),
            &animator.tail,
        );
    }
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("atlas_frames"),
    });
    for copy in &copies.0 {
        frames.layout.copy_tile(
            &mut encoder,
            buffer,
            &gpu.texture,
            copy.tile,
            copy.atlas_tile,
            copy.span,
        );
    }
    queue.submit([encoder.finish()]);
    if let Some(stats) = super::terrain_pool::stats() {
        let cells: u32 = copies.0.iter().map(|copy| copy.span * copy.span).sum();
        let level_copies = cells as usize * frames.layout.levels.len();
        stats.0.atlas_copies.store(level_copies, Ordering::Relaxed);
        stats.0.atlas_steps.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::atlas_meta::{SpriteLayout, parse_meta};

    fn frames(tiles_and_times: &[(u32, u32)]) -> Vec<FrameInfo> {
        tiles_and_times
            .iter()
            .map(|&(tile, time)| FrameInfo {
                tile,
                pack_tile: tile,
                time,
            })
            .collect()
    }

    #[test]
    fn frame_sequence_follows_each_frames_time() {
        let frames = frames(&[(0, 2), (1, 1), (2, 3)]);
        let mut state = AnimationState::default();
        let mut seen = Vec::new();
        for _ in 0..7 {
            state.tick(&frames);
            seen.push((state.frame, state.sub_frame));
        }
        assert_eq!(
            seen,
            [(0, 1), (1, 0), (2, 0), (2, 1), (2, 2), (0, 0), (0, 1)]
        );
    }

    #[test]
    fn dirty_only_when_the_index_changes() {
        let frames = frames(&[(5, 1), (5, 1), (6, 2)]);
        let mut state = AnimationState::default();
        let dirty: Vec<bool> = (0..5)
            .map(|_| {
                state.tick(&frames);
                state.dirty
            })
            .collect();
        assert_eq!(dirty, [false, true, false, true, false]);
    }

    #[test]
    fn blend_rounds_to_nearest() {
        assert_eq!(blend_byte(10, 200, 0), 10);
        assert_eq!(blend_byte(10, 200, 1000), 200);
        assert_eq!(blend_byte(0, 255, 500), 128);
        assert_eq!(blend_byte(255, 0, 1), 255);
        assert_eq!(blend_byte(0, 255, 1), 0);
        assert_eq!(blend_byte(0, 255, 2), 1);
    }

    #[test]
    fn blend_share_is_thousandths_of_the_frame() {
        let frames = frames(&[(0, 3), (1, 300)]);
        let mut state = AnimationState::default();
        assert_eq!(state.blend(&frames).2, 0);
        state.tick(&frames);
        let (current, next, q) = state.blend(&frames);
        assert_eq!((current.tile, next.tile, q), (0, 1, 333));
        state.tick(&frames);
        assert_eq!(state.blend(&frames).2, 666);
        state.tick(&frames);
        let (current, next, q) = state.blend(&frames);
        assert_eq!((current.tile, next.tile, q), (1, 0, 0));
    }

    fn flat(side: u32, count: u32, red: impl Fn(u32) -> u8) -> image::RgbaImage {
        image::RgbaImage::from_fn(side, side * count, |_, y| {
            image::Rgba([red(y / side), 0, 0, 255])
        })
    }

    fn frames_of(
        json: &str,
        image: &image::RgbaImage,
        pack: Option<&image::RgbaImage>,
    ) -> AtlasFrames {
        let meta = parse_meta(json.as_bytes()).expect("has an animation section");
        let SpriteLayout::Animated(layout) =
            SpriteLayout::of(Some(&meta), image.width(), image.height())
        else {
            panic!("not animated: {json}")
        };
        let sprite = AnimatedSprite {
            atlas_tile: 7,
            span: 1,
            image,
            pack_image: pack,
            layout: &layout,
        };
        build_frames(&[sprite], (256, 256), 16, 5).expect("one sprite")
    }

    fn animator(frames: AtlasFrames) -> AtlasAnimator {
        let mut animator = AtlasAnimator {
            frames: Some(frames),
            ..Default::default()
        };
        animator.reset_states();
        animator
    }

    #[test]
    fn only_interpolated_frames_keep_cpu_bytes() {
        let image = flat(16, 3, |frame| frame as u8);
        assert!(
            frames_of(r#"{"animation":{}}"#, &image, None)
                .kept
                .is_empty()
        );
        let frames = frames_of(r#"{"animation":{"interpolate":true}}"#, &image, None);
        assert_eq!(frames.kept.len(), 3);
        assert_eq!(frames.slots, 1);
        assert_eq!(frames.first_slot, 3);
        assert_eq!(frames.blocks.len(), 4 * 8192);
    }

    #[test]
    fn a_tinted_interpolated_sprite_gets_a_slot_for_the_pack() {
        let (image, pack) = (flat(16, 2, |_| 200), flat(16, 2, |_| 100));
        let frames = frames_of(r#"{"animation":{"interpolate":true}}"#, &image, Some(&pack));
        assert_eq!(frames.slots, 2);
        let animation = &frames.animations[0];
        assert_ne!(animation.blend_slot, animation.pack_blend_slot);
        assert!(
            animation
                .frames
                .iter()
                .all(|frame| frame.tile != frame.pack_tile)
        );
    }

    #[test]
    fn ticking_an_interpolated_sprite_mixes_into_its_slot() {
        let image = flat(16, 2, |frame| if frame == 0 { 0 } else { 200 });
        let frames = frames_of(
            r#"{"animation":{"interpolate":true,"frametime":4}}"#,
            &image,
            None,
        );
        let slot = frames.animations[0].blend_slot.unwrap();
        let mut animator = animator(frames);
        let mut copies = Vec::new();
        animator.tick(&mut copies);
        assert_eq!(
            copies,
            [TileCopy {
                atlas_tile: 7,
                tile: slot,
                pack_tile: slot,
                span: 1
            }]
        );
        let last = animator.frames.as_ref().unwrap().layout.levels[4].offset as usize;
        assert_eq!((animator.tail[0], animator.tail[3]), (50, 255));
        assert_eq!(animator.tail[last], 50);
    }

    #[test]
    fn a_discrete_sprite_copies_only_when_its_frame_changes() {
        let image = flat(16, 2, |frame| frame as u8);
        let frames = frames_of(r#"{"animation":{"frametime":2}}"#, &image, None);
        let tiles = [
            frames.animations[0].frames[0].tile,
            frames.animations[0].frames[1].tile,
        ];
        let mut animator = animator(frames);
        let mut copies = Vec::new();
        animator.tick(&mut copies);
        assert!(copies.is_empty());
        animator.tick(&mut copies);
        assert_eq!(
            copies,
            [TileCopy {
                atlas_tile: 7,
                tile: tiles[1],
                pack_tile: tiles[1],
                span: 1
            }]
        );
    }

    #[cfg(feature = "shader_support")]
    #[test]
    fn a_pack_syncs_to_the_frame_each_sprite_is_on() {
        let (image, pack) = (flat(16, 3, |_| 200), flat(16, 3, |_| 100));
        let frames = frames_of(r#"{"animation":{}}"#, &image, Some(&pack));
        let pack_tiles: Vec<u32> = frames.animations[0]
            .frames
            .iter()
            .map(|frame| frame.pack_tile)
            .collect();
        let mut animator = animator(frames);
        assert_eq!(
            animator.pack_tiles().collect::<Vec<_>>(),
            [(7, pack_tiles[0], 1)]
        );
        animator.tick(&mut Vec::new());
        animator.tick(&mut Vec::new());
        assert_eq!(
            animator.pack_tiles().collect::<Vec<_>>(),
            [(7, pack_tiles[2], 1)]
        );
    }

    #[test]
    fn clock_steps_once_and_never_catches_up() {
        let mut clock = AtlasClock::default();
        clock.advance(0.03);
        assert!(!clock.step);
        clock.advance(0.03);
        assert!(clock.step);
        clock.advance(5.0);
        assert!(clock.step);
        clock.advance(0.001);
        assert!(!clock.step);
    }

    #[test]
    fn block_rows_and_levels_are_aligned() {
        for (tile_px, n_mips) in [(16, 5), (32, 6), (64, 7)] {
            let blocks = TileBlocks::new(tile_px, n_mips, 16);
            assert_eq!(blocks.levels.len(), n_mips as usize);
            assert_eq!(blocks.stride % PLACEMENT_ALIGN, 0);
            for (level, at) in blocks.levels.iter().enumerate() {
                assert_eq!(at.side, tile_px >> level);
                assert_eq!(at.pitch % ROW_ALIGN, 0);
                assert!(at.pitch >= at.side * 4);
                assert_eq!(at.offset % PLACEMENT_ALIGN, 0);
                assert!(at.offset + at.pitch * at.side <= blocks.stride);
            }
        }
        assert_eq!(TileBlocks::new(16, 5, 16).stride, 8192);
    }
}
