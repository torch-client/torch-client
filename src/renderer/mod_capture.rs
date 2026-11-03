use std::collections::{HashMap, VecDeque};
use std::num::NonZeroU64;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Duration;

use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::render::render_graph::{
    Node, NodeRunError, RenderGraph, RenderGraphContext, RenderLabel,
};
use bevy::render::render_resource::binding_types::uniform_buffer_sized;
use bevy::render::render_resource::{
    BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BlendState,
    Buffer, BufferBinding, BufferDescriptor, BufferUsages, CachedRenderPipelineId,
    ColorTargetState, ColorWrites, CompareFunction, DepthStencilState, Extent3d, Face,
    FragmentState, IndexFormat, LoadOp, MapMode, MultisampleState, Operations, Origin3d,
    PipelineCache, PrimitiveState, RenderPassColorAttachment, RenderPassDepthStencilAttachment,
    RenderPassDescriptor, RenderPipelineDescriptor, ShaderStages, SpecializedRenderPipeline,
    SpecializedRenderPipelines, StoreOp, TexelCopyBufferInfo, TexelCopyBufferLayout,
    TexelCopyTextureInfo, Texture, TextureAspect, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages, TextureView, TextureViewDescriptor, VertexState,
};
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::{Shader, ShaderDefVal};
use bytemuck::{Pod, Zeroable};

use super::terrain_pool::draw::{
    TerrainBindGroup, TerrainPipeline, draw_slots, packed_vertex_layout,
};
use super::terrain_pool::pools::TerrainPools;
use super::terrain_pool::{
    META_WATER, STREAM_CUTOUT, STREAM_SOLID, STREAM_WATER, STREAMS, TerrainTier,
};
use crate::platform::time::Instant;

pub const CAPTURES_PER_FRAME: usize = 8;
pub const CAPTURE_WAIT_TICKS: u32 = 100;
const TICK: Duration = Duration::from_millis(50);
const MAX_WAIT_COLUMNS: i64 = 4096;
const CAPTURE_DEFER_FRAMES: u32 = 600;
const DEPTH_IDLE_FRAMES: u64 = 300;
const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;
const NOON: f64 = 6000.0;
const DAY_GAMMA: f32 = 0.5;
const LEVELS: usize = 16;
const SKY_FULL: u8 = 15;
const LIGHT_WORLD: u32 = 0;
const LIGHT_DAY: u32 = 1;
const LIGHT_FULL: u32 = 2;
const RGBA_BYTES: usize = 4;
const R16_BYTES: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TexelFormat {
    Rgba8,
    R16,
}

impl TexelFormat {
    pub const fn bytes(self) -> usize {
        match self {
            TexelFormat::Rgba8 => RGBA_BYTES,
            TexelFormat::R16 => R16_BYTES,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            TexelFormat::Rgba8 => "rgba8",
            TexelFormat::R16 => "r16",
        }
    }

    const fn texture(self) -> TextureFormat {
        match self {
            TexelFormat::Rgba8 => TextureFormat::Rgba8UnormSrgb,
            TexelFormat::R16 => TextureFormat::R16Uint,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ticket {
    pub owner: usize,
    pub id: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Light {
    World,
    Day,
    Full,
}

#[derive(Clone, Copy, Debug)]
pub enum Projection {
    Ortho { width: f64, height: f64 },
    Perspective { fov_y: f64 },
}

#[derive(Clone, Copy, Debug)]
pub struct View {
    pub eye: [f64; 3],
    pub forward: [f64; 3],
    pub up: [f64; 3],
    pub near: f64,
    pub far: f64,
    pub projection: Projection,
}

impl View {
    fn corners(&self, aspect: f64) -> [DVec3; 8] {
        let (eye, f, up) = (
            DVec3::from(self.eye),
            DVec3::from(self.forward),
            DVec3::from(self.up),
        );
        let right = up.cross(-f);
        let mut out = [DVec3::ZERO; 8];
        for (i, d) in [self.near, self.far].into_iter().enumerate() {
            let (hw, hh) = match self.projection {
                Projection::Ortho { width, height } => (width / 2.0, height / 2.0),
                Projection::Perspective { fov_y } => {
                    let hh = (fov_y / 2.0).tan() * d;
                    (hh * aspect, hh)
                }
            };
            let centre = eye + f * d;
            for (j, (sx, sy)) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
                .into_iter()
                .enumerate()
            {
                out[i * 4 + j] = centre + right * (sx * hw) + up * (sy * hh);
            }
        }
        out
    }

    fn clip_from_world(&self, origin: DVec3, aspect: f32) -> (Mat4, Vec3) {
        let eye = (DVec3::from(self.eye) - origin).as_vec3();
        let f = DVec3::from(self.forward).as_vec3();
        let up = DVec3::from(self.up).as_vec3();
        let back = -f;
        let right = up.cross(back);
        let world_from_view = Mat4::from_cols(
            right.extend(0.0),
            up.extend(0.0),
            back.extend(0.0),
            eye.extend(1.0),
        );
        let (near, far) = (self.near as f32, self.far as f32);
        let clip_from_view = match self.projection {
            Projection::Ortho { width, height } => {
                let (hw, hh) = ((width / 2.0) as f32, (height / 2.0) as f32);
                Mat4::orthographic_rh(-hw, hw, -hh, hh, far, near)
            }
            Projection::Perspective { fov_y } => {
                Mat4::perspective_rh(fov_y as f32, aspect, far, near)
            }
        };
        (clip_from_view * world_from_view.inverse(), eye)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Capture {
    pub into: u64,
    pub depth: Option<u64>,
    pub rect: [u32; 4],
    pub view: View,
    pub terrain: bool,
    pub fluids: bool,
    pub light: Light,
    pub fog: bool,
    pub wait: bool,
    pub ticket: Ticket,
    pub partial: bool,
}

pub enum ImageOp {
    Create {
        key: u64,
        w: u32,
        h: u32,
        format: TexelFormat,
    },
    Remove {
        key: u64,
    },
    Clear,
    Fill {
        key: u64,
        texel: [u8; 4],
    },
    Set {
        key: u64,
        x: u32,
        y: u32,
        texel: [u8; 4],
    },
    Write {
        key: u64,
        rect: [u32; 4],
        bytes: Box<[u8]>,
    },
    Capture(Capture),
    Readback {
        key: u64,
        version: u64,
        ticket: Option<Ticket>,
    },
}

impl ImageOp {
    fn keys(&self) -> [Option<u64>; 2] {
        match self {
            ImageOp::Create { key, .. }
            | ImageOp::Remove { key }
            | ImageOp::Fill { key, .. }
            | ImageOp::Set { key, .. }
            | ImageOp::Write { key, .. }
            | ImageOp::Readback { key, .. } => [Some(*key), None],
            ImageOp::Capture(c) => [Some(c.into), c.depth],
            ImageOp::Clear => [None, None],
        }
    }

    fn fail(self, why: &str) {
        match self {
            ImageOp::Capture(c) => answer(Done::Capture {
                ticket: c.ticket,
                result: Err(why.to_string()),
            }),
            ImageOp::Readback {
                key,
                version,
                ticket,
            } => answer(Done::Readback {
                key,
                version,
                ticket,
                result: Err(why.to_string()),
            }),
            _ => {}
        }
    }
}

pub enum Done {
    Capture {
        ticket: Ticket,
        result: Result<bool, String>,
    },
    Readback {
        key: u64,
        version: u64,
        ticket: Option<Ticket>,
        result: Result<Box<[u8]>, String>,
    },
}

struct Channel<T> {
    tx: Sender<T>,
    rx: Mutex<Receiver<T>>,
}

impl<T> Channel<T> {
    fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            tx,
            rx: Mutex::new(rx),
        }
    }

    fn drain(&self, mut f: impl FnMut(T)) {
        let rx = self.rx.lock().unwrap_or_else(PoisonError::into_inner);
        while let Ok(item) = rx.try_recv() {
            f(item);
        }
    }
}

static OPS: LazyLock<Channel<ImageOp>> = LazyLock::new(Channel::new);
static ROUTED: LazyLock<Channel<ImageOp>> = LazyLock::new(Channel::new);
static DONE: LazyLock<Channel<Done>> = LazyLock::new(Channel::new);
static MAPPED: LazyLock<Channel<(u64, bool)>> = LazyLock::new(Channel::new);

pub fn send(op: ImageOp) {
    let _ = OPS.tx.send(op);
}

pub fn take_done(f: impl FnMut(Done)) {
    DONE.drain(f);
}

fn answer(done: Done) {
    let _ = DONE.tx.send(done);
}

#[derive(Resource, Default)]
struct Router {
    queue: Vec<(Instant, ImageOp)>,
    spare: Vec<(Instant, ImageOp)>,
    blocked: Vec<u64>,
}

fn covered(capture: &Capture, index: &super::systems::ChunkIndex) -> Option<bool> {
    let aspect = f64::from(capture.rect[2]) / f64::from(capture.rect[3]);
    let corners = capture.view.corners(aspect);
    let (mut lo, mut hi) = (corners[0], corners[0]);
    for c in &corners[1..] {
        lo = lo.min(*c);
        hi = hi.max(*c);
    }
    let column = |v: f64| (v.floor() as i64) >> 4;
    let (x0, x1, z0, z1) = (column(lo.x), column(hi.x), column(lo.z), column(hi.z));
    if (x1 - x0 + 1) * (z1 - z0 + 1) > MAX_WAIT_COLUMNS {
        return None;
    }
    Some((x0..=x1).all(|x| (z0..=z1).all(|z| index.has_column(x as i32, z as i32))))
}

fn route_ops(mut router: ResMut<Router>, index: Option<Res<super::systems::ChunkIndex>>) {
    let now = Instant::now();
    OPS.drain(|op| router.queue.push((now, op)));
    if router.queue.is_empty() {
        return;
    }
    let wait = TICK * CAPTURE_WAIT_TICKS;
    let Router {
        queue,
        spare,
        blocked,
    } = &mut *router;
    blocked.clear();
    let mut budget = CAPTURES_PER_FRAME;
    for (since, mut op) in queue.drain(..) {
        match op {
            ImageOp::Clear => {
                spare.clear();
                blocked.clear();
                let _ = ROUTED.tx.send(op);
                continue;
            }
            ImageOp::Remove { key } => {
                for (_, held) in spare.extract_if(.., |(_, held)| held.keys().contains(&Some(key)))
                {
                    held.fail("the image was removed");
                }
                blocked.retain(|&b| b != key);
                let _ = ROUTED.tx.send(op);
                continue;
            }
            _ => {}
        }
        let keys = op.keys();
        let mut hold = keys.iter().flatten().any(|k| blocked.contains(k));
        if !hold && let ImageOp::Capture(c) = &mut op {
            let ready = index.as_deref().and_then(|i| covered(c, i));
            if budget == 0 || (c.wait && ready == Some(false) && now.duration_since(since) < wait) {
                hold = true;
            } else {
                c.partial = ready != Some(true);
                budget -= 1;
            }
        }
        if hold {
            blocked.extend(keys.iter().flatten());
            spare.push((since, op));
        } else {
            let _ = ROUTED.tx.send(op);
        }
    }
    std::mem::swap(queue, spare);
}

struct ImageTexture {
    texture: Texture,
    view: TextureView,
    w: u32,
    h: u32,
    format: TexelFormat,
}

struct Read {
    key: u64,
    version: u64,
    ticket: Option<Ticket>,
    texture: Texture,
    buffer: Buffer,
    w: u32,
    h: u32,
    bpp: usize,
    padded: usize,
}

struct Planned {
    rect: [u32; 4],
    into: TextureView,
    depth_image: Option<TextureView>,
    scratch: TextureView,
    index: usize,
    clear: CachedRenderPipelineId,
    streams: [Option<CachedRenderPipelineId>; STREAMS as usize],
}

enum Work {
    Capture(Planned),
    Read(usize),
}

struct Scratch {
    view: TextureView,
    used: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CaptureUniform {
    clip_from_world: [[f32; 4]; 4],
    eye: [f32; 4],
    forward: [f32; 4],
    mode: [u32; 4],
    day: [[f32; 4]; LEVELS],
}

const UNIFORM_SIZE: u64 = size_of::<CaptureUniform>() as u64;

struct Uniforms {
    buffer: Buffer,
    group: BindGroup,
    empty: BindGroup,
    stride: u64,
}

#[derive(Resource, Default)]
pub struct ModImages {
    images: HashMap<u64, ImageTexture>,
    pending: VecDeque<(u32, ImageOp)>,
    later: VecDeque<(u32, ImageOp)>,
    work: Vec<Work>,
    lists: Vec<[Vec<u32>; STREAMS as usize]>,
    reads: Vec<Read>,
    mapping: HashMap<u64, Read>,
    next_read: u64,
    answers: Vec<(Ticket, bool)>,
    depth: HashMap<(u32, u32), Scratch>,
    uniforms: Option<Uniforms>,
    uniform_bytes: Vec<u8>,
    fill_bytes: Vec<u8>,
    frame: u64,
}

impl ModImages {
    pub fn view(&self, key: u64) -> Option<&TextureView> {
        self.images
            .get(&key)
            .filter(|t| t.format == TexelFormat::Rgba8)
            .map(|t| &t.view)
    }

    pub fn has(&self, key: u64) -> bool {
        self.images.contains_key(&key)
    }
}

#[derive(Resource)]
pub struct CapturePipeline {
    shader: Handle<Shader>,
    uniform_layout: BindGroupLayoutDescriptor,
    empty_layout: BindGroupLayoutDescriptor,
    terrain_layout: BindGroupLayoutDescriptor,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CaptureKey {
    stream: Option<u32>,
    depth: bool,
    direct: bool,
}

impl SpecializedRenderPipeline for CapturePipeline {
    type Key = CaptureKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let mut defs: Vec<ShaderDefVal> = Vec::new();
        if key.depth {
            defs.push("DEPTH_TARGET".into());
        }
        if key.direct {
            defs.push("META_TEXTURE".into());
        }
        match key.stream {
            Some(STREAM_CUTOUT) => defs.push("ALPHA_CUTOUT".into()),
            Some(STREAM_WATER) => defs.push("WATER".into()),
            _ => {}
        }
        let blend = (key.stream == Some(STREAM_WATER)).then_some(BlendState::ALPHA_BLENDING);
        let mut targets = vec![Some(ColorTargetState {
            format: TexelFormat::Rgba8.texture(),
            blend,
            write_mask: ColorWrites::ALL,
        })];
        if key.depth {
            targets.push(Some(ColorTargetState {
                format: TexelFormat::R16.texture(),
                blend: None,
                write_mask: ColorWrites::ALL,
            }));
        }
        let (vertex, fragment, layout, buffers) = match key.stream {
            Some(_) => (
                "vertex",
                "fragment",
                vec![
                    self.uniform_layout.clone(),
                    self.empty_layout.clone(),
                    self.terrain_layout.clone(),
                ],
                vec![packed_vertex_layout()],
            ),
            None => ("clear_vertex", "clear_fragment", Vec::new(), Vec::new()),
        };
        let drawn = key.stream.is_some();
        RenderPipelineDescriptor {
            label: Some("mod_capture".into()),
            layout,
            vertex: VertexState {
                shader: self.shader.clone(),
                shader_defs: defs.clone(),
                entry_point: Some(vertex.into()),
                buffers,
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs: defs,
                entry_point: Some(fragment.into()),
                targets,
            }),
            primitive: PrimitiveState {
                cull_mode: drawn.then_some(Face::Back),
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: drawn,
                depth_compare: if drawn {
                    CompareFunction::GreaterEqual
                } else {
                    CompareFunction::Always
                },
                stencil: default(),
                bias: default(),
            }),
            multisample: MultisampleState::default(),
            ..default()
        }
    }
}

#[derive(Resource, Clone)]
struct CaptureShader(Handle<Shader>);

fn init_capture_pipeline(
    mut commands: Commands,
    shader: Option<Res<CaptureShader>>,
    terrain: Option<Res<TerrainPipeline>>,
    tier: Option<Res<TerrainTier>>,
) {
    let (Some(shader), Some(terrain), Some(tier)) = (shader, terrain, tier) else {
        return;
    };
    let uniform = BindGroupLayoutEntries::single(
        ShaderStages::VERTEX_FRAGMENT,
        uniform_buffer_sized(true, NonZeroU64::new(UNIFORM_SIZE)),
    );
    commands.insert_resource(CapturePipeline {
        shader: shader.0.clone(),
        uniform_layout: BindGroupLayoutDescriptor::new("mod_capture", &uniform),
        empty_layout: BindGroupLayoutDescriptor::new("mod_capture_empty", &[]),
        terrain_layout: terrain.terrain_layout(!tier.indirect),
    });
}

fn day_row() -> [[f32; 4]; LEVELS] {
    use super::lightmap::{State, light_color};
    let state = State::sample(
        super::dimension::current(),
        &super::timeline::sample(NOON),
        0.0,
        DAY_GAMMA,
    );
    std::array::from_fn(|block| {
        let [r, g, b] = light_color(&state, block as u8, SKY_FULL);
        [r, g, b, 1.0]
    })
}

struct Frame<'a> {
    device: &'a RenderDevice,
    queue: &'a RenderQueue,
    cache: &'a PipelineCache,
    pipeline: Option<&'a CapturePipeline>,
    pools: Option<&'a TerrainPools>,
    ready: bool,
    direct: bool,
    origin: DVec3,
    render_origin: IVec3,
}

#[allow(clippy::too_many_arguments, reason = "one system, flat arguments")]
pub fn prepare_mod_images(
    mut store: ResMut<ModImages>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    cache: Res<PipelineCache>,
    pipeline: Option<Res<CapturePipeline>>,
    mut pipelines: ResMut<SpecializedRenderPipelines<CapturePipeline>>,
    pools: Option<Res<TerrainPools>>,
    tier: Option<Res<TerrainTier>>,
    terrain_group: Res<TerrainBindGroup>,
    origin: Res<super::RenderOrigin>,
) {
    let store = &mut *store;
    store.frame += 1;
    store.work.clear();
    store.reads.clear();
    store.answers.clear();
    finish_reads(store);
    ROUTED.drain(|op| store.pending.push_back((0, op)));
    if !store.pending.is_empty() {
        if store.uniforms.is_none()
            && let Some(pipeline) = pipeline.as_deref()
        {
            store.uniforms = Some(make_uniforms(&device, &cache, pipeline));
        }
        let frame = Frame {
            device: &device,
            queue: &queue,
            cache: &cache,
            pipeline: pipeline.as_deref(),
            pools: pools.as_deref(),
            ready: pools.is_some() && terrain_group.group.is_some() && store.uniforms.is_some(),
            direct: tier.as_deref().is_none_or(|t| !t.indirect),
            origin: origin.world(),
            render_origin: origin.block(),
        };
        take_ops(store, &frame, &mut pipelines);
    }
    let now = store.frame;
    store.depth.retain(|_, s| now - s.used < DEPTH_IDLE_FRAMES);
}

fn make_uniforms(
    device: &RenderDevice,
    cache: &PipelineCache,
    pipeline: &CapturePipeline,
) -> Uniforms {
    let align = u64::from(device.limits().min_uniform_buffer_offset_alignment);
    let stride = UNIFORM_SIZE.div_ceil(align) * align;
    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("mod_capture"),
        size: stride * CAPTURES_PER_FRAME as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(
        "mod_capture",
        &cache.get_bind_group_layout(&pipeline.uniform_layout),
        &BindGroupEntries::single(BufferBinding {
            buffer: &buffer,
            offset: 0,
            size: NonZeroU64::new(UNIFORM_SIZE),
        }),
    );
    let empty = device.create_bind_group(
        "mod_capture_empty",
        &cache.get_bind_group_layout(&pipeline.empty_layout),
        &[],
    );
    Uniforms {
        buffer,
        group,
        empty,
        stride,
    }
}

fn take_ops(
    store: &mut ModImages,
    frame: &Frame,
    pipelines: &mut SpecializedRenderPipelines<CapturePipeline>,
) {
    let max_side = frame.device.limits().max_texture_dimension_2d;
    let mut day = None;
    let mut used: Vec<u64> = Vec::new();
    let mut deferred: Vec<u64> = Vec::new();
    let mut later = std::mem::take(&mut store.later);
    let mut captures = 0usize;
    store.uniform_bytes.clear();
    while let Some((frames, op)) = store.pending.pop_front() {
        let keys = op.keys();
        let touches = |list: &[u64]| keys.iter().flatten().any(|k| list.contains(k));
        if touches(&deferred) {
            later.push_back((frames, op));
            continue;
        }
        match op {
            ImageOp::Create { key, w, h, format } => {
                if w > max_side || h > max_side {
                    crate::log_warn!(
                        "mods",
                        "a torch.image of {w}x{h} is over this GPU's {max_side} a side; it has no texture"
                    );
                    continue;
                }
                let texture = frame.device.create_texture(&TextureDescriptor {
                    label: Some("mod_image"),
                    size: Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: format.texture(),
                    usage: TextureUsages::TEXTURE_BINDING
                        | TextureUsages::COPY_DST
                        | TextureUsages::COPY_SRC
                        | TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                let view = texture.create_view(&TextureViewDescriptor::default());
                store.images.insert(
                    key,
                    ImageTexture {
                        texture,
                        view,
                        w,
                        h,
                        format,
                    },
                );
            }
            ImageOp::Remove { key } => {
                store.images.remove(&key);
            }
            ImageOp::Clear => {
                store.images.clear();
                later.clear();
                deferred.clear();
            }
            ImageOp::Fill { .. } | ImageOp::Set { .. } | ImageOp::Write { .. }
                if touches(&used) =>
            {
                deferred.extend(keys.iter().flatten());
                later.push_back((frames, op));
            }
            ImageOp::Fill { key, texel } => {
                let Some(t) = store.images.get(&key) else {
                    continue;
                };
                let fill = &mut store.fill_bytes;
                fill.clear();
                let texel = &texel[..t.format.bytes()];
                for _ in 0..u64::from(t.w) * u64::from(t.h) {
                    fill.extend_from_slice(texel);
                }
                write(frame.queue, t, [0, 0, t.w, t.h], fill);
            }
            ImageOp::Set { key, x, y, texel } => {
                if let Some(t) = store.images.get(&key) {
                    write(frame.queue, t, [x, y, 1, 1], &texel[..t.format.bytes()]);
                }
            }
            ImageOp::Write { key, rect, bytes } => {
                if let Some(t) = store.images.get(&key) {
                    write(frame.queue, t, rect, &bytes);
                }
            }
            ImageOp::Readback {
                key,
                version,
                ticket,
            } => {
                let Some(t) = store.images.get(&key) else {
                    ImageOp::Readback {
                        key,
                        version,
                        ticket,
                    }
                    .fail("the image has no texture");
                    continue;
                };
                let bpp = t.format.bytes();
                let padded = RenderDevice::align_copy_bytes_per_row(t.w as usize * bpp);
                let buffer = frame.device.create_buffer(&BufferDescriptor {
                    label: Some("mod_image_readback"),
                    size: (padded * t.h as usize) as u64,
                    usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let read = Read {
                    key,
                    version,
                    ticket,
                    texture: t.texture.clone(),
                    buffer,
                    w: t.w,
                    h: t.h,
                    bpp,
                    padded,
                };
                store.reads.push(read);
                store.work.push(Work::Read(store.reads.len() - 1));
                used.push(key);
            }
            ImageOp::Capture(c) => match plan(store, &c, captures, frame, pipelines) {
                Plan::Draw(p) => {
                    let Some(pools) = frame.pools else { continue };
                    let aspect = c.rect[2] as f32 / c.rect[3] as f32;
                    let (clip_from_world, eye) = c.view.clip_from_world(frame.origin, aspect);
                    cull(
                        &mut store.lists[p.index],
                        pools,
                        &clip_from_world,
                        frame.render_origin,
                        &c,
                    );
                    let uniform = CaptureUniform {
                        clip_from_world: clip_from_world.to_cols_array_2d(),
                        eye: eye.extend(c.view.near as f32).to_array(),
                        forward: DVec3::from(c.view.forward)
                            .as_vec3()
                            .extend(c.view.far as f32)
                            .to_array(),
                        mode: [
                            match c.light {
                                Light::World => LIGHT_WORLD,
                                Light::Day => LIGHT_DAY,
                                Light::Full => LIGHT_FULL,
                            },
                            u32::from(c.fog),
                            0,
                            0,
                        ],
                        day: *day.get_or_insert_with(day_row),
                    };
                    let stride =
                        store.uniforms.as_ref().map_or(UNIFORM_SIZE, |u| u.stride) as usize;
                    store.uniform_bytes.resize(p.index * stride, 0);
                    store
                        .uniform_bytes
                        .extend_from_slice(bytemuck::bytes_of(&uniform));
                    used.extend(keys.iter().flatten());
                    store.answers.push((c.ticket, c.partial));
                    store.work.push(Work::Capture(p));
                    captures += 1;
                }
                Plan::Wait if frames < CAPTURE_DEFER_FRAMES => {
                    deferred.extend(keys.iter().flatten());
                    later.push_back((frames + 1, ImageOp::Capture(c)));
                }
                Plan::Wait => ImageOp::Capture(c).fail("the capture pipeline never became ready"),
                Plan::Fail(why) => ImageOp::Capture(c).fail(why),
            },
        }
    }
    std::mem::swap(&mut store.pending, &mut later);
    store.later = later;
    if captures > 0
        && let Some(u) = &store.uniforms
    {
        frame.queue.write_buffer(&u.buffer, 0, &store.uniform_bytes);
    }
}

fn write(queue: &RenderQueue, t: &ImageTexture, rect: [u32; 4], bytes: &[u8]) {
    let [x, y, w, h] = rect;
    queue.write_texture(
        TexelCopyTextureInfo {
            texture: &t.texture,
            mip_level: 0,
            origin: Origin3d { x, y, z: 0 },
            aspect: TextureAspect::All,
        },
        bytes,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * t.format.bytes() as u32),
            rows_per_image: Some(h),
        },
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
}

enum Plan {
    Draw(Planned),
    Wait,
    Fail(&'static str),
}

fn plan(
    store: &mut ModImages,
    c: &Capture,
    captures: usize,
    frame: &Frame,
    pipelines: &mut SpecializedRenderPipelines<CapturePipeline>,
) -> Plan {
    let Some(into) = store.images.get(&c.into) else {
        return Plan::Fail("the image has no texture: removed, or larger than this GPU allows");
    };
    let [x, y, w, h] = c.rect;
    if into.format != TexelFormat::Rgba8 || w == 0 || h == 0 || x + w > into.w || y + h > into.h {
        return Plan::Fail("the capture does not fit its image");
    }
    let size = (into.w, into.h);
    let into_view = into.view.clone();
    let depth_image = match c.depth {
        None => None,
        Some(key) => match store.images.get(&key) {
            Some(d) if d.format == TexelFormat::R16 && (d.w, d.h) == size => Some(d.view.clone()),
            Some(_) => return Plan::Fail("the depth image is not an r16 image of the same size"),
            None => return Plan::Fail("the depth image was removed"),
        },
    };
    let Some(pipeline) = frame
        .pipeline
        .filter(|_| frame.ready && captures < CAPTURES_PER_FRAME)
    else {
        return Plan::Wait;
    };
    let (depth, direct) = (c.depth.is_some(), frame.direct);
    let mut id = |stream| {
        pipelines.specialize(
            frame.cache,
            pipeline,
            CaptureKey {
                stream,
                depth,
                direct,
            },
        )
    };
    let clear = id(None);
    let wanted = [c.terrain, c.terrain, c.fluids];
    let mut streams = [None; STREAMS as usize];
    for (stream, slot) in (0..STREAMS).zip(&mut streams) {
        if wanted[stream as usize] {
            *slot = Some(id(Some(stream)));
        }
    }
    let compiled = |id: CachedRenderPipelineId| frame.cache.get_render_pipeline(id).is_some();
    if !compiled(clear) || streams.iter().flatten().any(|&s| !compiled(s)) {
        return Plan::Wait;
    }
    let now = store.frame;
    let scratch = store.depth.entry(size).or_insert_with(|| {
        let texture = frame.device.create_texture(&TextureDescriptor {
            label: Some("mod_capture_depth"),
            size: Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        Scratch {
            view: texture.create_view(&TextureViewDescriptor::default()),
            used: now,
        }
    });
    scratch.used = now;
    let scratch = scratch.view.clone();
    if store.lists.len() <= captures {
        store.lists.resize_with(captures + 1, Default::default);
    }
    Plan::Draw(Planned {
        rect: c.rect,
        into: into_view,
        depth_image,
        scratch,
        index: captures,
        clear,
        streams,
    })
}

fn cull(
    lists: &mut [Vec<u32>; STREAMS as usize],
    pools: &TerrainPools,
    clip_from_world: &Mat4,
    render_origin: IVec3,
    c: &Capture,
) {
    let shape = super::terrain_pool::cull::frustum_of(clip_from_world);
    for list in lists.iter_mut() {
        list.clear();
    }
    let end = (pools.draw_end as usize).min(pools.meta_cpu.len());
    for (slot, meta) in (0u32..).zip(&pools.meta_cpu[..end]) {
        if !pools.is_live(slot) {
            continue;
        }
        let (lo, hi) = meta.render_box(render_origin);
        if !shape.keeps(lo, hi) {
            continue;
        }
        if meta.flags & META_WATER != 0 {
            if c.fluids {
                lists[STREAM_WATER as usize].push(slot);
            }
            continue;
        }
        if c.terrain {
            if meta.solid_count > 0 {
                lists[STREAM_SOLID as usize].push(slot);
            }
            if meta.index_count > meta.solid_count {
                lists[STREAM_CUTOUT as usize].push(slot);
            }
        }
    }
    for (stream, list) in (0..STREAMS).zip(lists.iter_mut()) {
        list.sort_unstable_by_key(|&slot| {
            let meta = &pools.meta_cpu[slot as usize];
            (meta.pool, meta.stream_range(stream).0)
        });
    }
}

fn finish_reads(store: &mut ModImages) {
    if store.mapping.is_empty() {
        return;
    }
    MAPPED.drain(|(id, ok)| {
        let Some(read) = store.mapping.remove(&id) else {
            return;
        };
        let result = if ok {
            let row = read.w as usize * read.bpp;
            let mut out = Vec::with_capacity(row * read.h as usize);
            {
                let range = read.buffer.slice(..).get_mapped_range();
                for y in 0..read.h as usize {
                    out.extend_from_slice(&range[y * read.padded..y * read.padded + row]);
                }
            }
            read.buffer.unmap();
            Ok(out.into_boxed_slice())
        } else {
            Err("the GPU could not map the readback".to_string())
        };
        answer(Done::Readback {
            key: read.key,
            version: read.version,
            ticket: read.ticket,
            result,
        });
    });
}

fn submit_frame(mut store: ResMut<ModImages>) {
    let store = &mut *store;
    for (ticket, partial) in store.answers.drain(..) {
        answer(Done::Capture {
            ticket,
            result: Ok(partial),
        });
    }
    store.work.clear();
    for read in store.reads.drain(..) {
        let id = store.next_read;
        store.next_read += 1;
        let tx = MAPPED.tx.clone();
        read.buffer
            .slice(..)
            .map_async(MapMode::Read, move |result| {
                let _ = tx.send((id, result.is_ok()));
            });
        store.mapping.insert(id, read);
    }
}

#[derive(RenderLabel, Debug, Hash, PartialEq, Eq, Clone)]
struct CaptureLabel;

struct CaptureNode;

impl Node for CaptureNode {
    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let store = world.resource::<ModImages>();
        for work in &store.work {
            match work {
                Work::Read(i) => {
                    let r = &store.reads[*i];
                    render_context.command_encoder().copy_texture_to_buffer(
                        r.texture.as_image_copy(),
                        TexelCopyBufferInfo {
                            buffer: &r.buffer,
                            layout: TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(r.padded as u32),
                                rows_per_image: Some(r.h),
                            },
                        },
                        Extent3d {
                            width: r.w,
                            height: r.h,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                Work::Capture(p) => draw_capture(render_context, world, store, p),
            }
        }
        Ok(())
    }
}

fn draw_capture<'w>(
    render_context: &mut RenderContext<'w>,
    world: &'w World,
    store: &'w ModImages,
    p: &'w Planned,
) {
    let cache = world.resource::<PipelineCache>();
    let (Some(terrain), Some(u), Some(pools), Some(tier)) = (
        world.resource::<TerrainBindGroup>().group.as_ref(),
        store.uniforms.as_ref(),
        world.get_resource::<TerrainPools>(),
        world.get_resource::<TerrainTier>(),
    ) else {
        return;
    };
    let target = |view: &'w TextureView| {
        Some(RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Load,
                store: StoreOp::Store,
            },
        })
    };
    let colors = [target(&p.into), p.depth_image.as_ref().and_then(target)];
    let count = if p.depth_image.is_some() { 2 } else { 1 };
    let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("mod_capture"),
        color_attachments: &colors[..count],
        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
            view: &p.scratch,
            depth_ops: Some(Operations {
                load: LoadOp::Clear(0.0),
                store: StoreOp::Discard,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    let [x, y, w, h] = p.rect;
    pass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0.0, 1.0);
    pass.set_scissor_rect(x, y, w, h);
    if let Some(clear) = cache.get_render_pipeline(p.clear) {
        pass.set_render_pipeline(clear);
        pass.draw(0..3, 0..1);
    }
    let offset = (p.index as u64 * u.stride) as u32;
    for (stream, id) in (0..STREAMS).zip(p.streams) {
        let slots = &store.lists[p.index][stream as usize];
        let Some(pipeline) = id
            .and_then(|id| cache.get_render_pipeline(id))
            .filter(|_| !slots.is_empty())
        else {
            continue;
        };
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, &u.group, &[offset]);
        pass.set_bind_group(1, &u.empty, &[]);
        pass.set_bind_group(super::terrain_pool::draw::TERRAIN_GROUP, terrain, &[]);
        for (pool_index, pool) in (0u32..).zip(&pools.pools) {
            pass.set_vertex_buffer(0, pool.vertices.slice(..));
            pass.set_index_buffer(pool.indices.slice(..), IndexFormat::Uint32);
            draw_slots(&mut pass, pools, slots, pool_index, stream, tier.indirect);
        }
    }
}

pub fn build(app: &mut App) {
    bevy::asset::embedded_asset!(app, "mod_capture.wgsl");
    app.init_resource::<Router>().add_systems(Update, route_ops);
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .init_resource::<ModImages>()
        .init_resource::<SpecializedRenderPipelines<CapturePipeline>>()
        .add_systems(
            RenderStartup,
            init_capture_pipeline.after(super::terrain_pool::draw::init_terrain_pipeline),
        )
        .add_systems(
            Render,
            (
                prepare_mod_images
                    .in_set(RenderSystems::PrepareBindGroups)
                    .after(super::terrain_pool::draw::prepare_terrain_bind_group),
                submit_frame
                    .in_set(RenderSystems::Render)
                    .after(bevy::render::renderer::render_system),
            ),
        );
    let mut graph = render_app.world_mut().resource_mut::<RenderGraph>();
    graph.add_node(CaptureLabel, CaptureNode);
    graph.add_node_edge(bevy::render::graph::CameraDriverLabel, CaptureLabel);
    graph.add_node_edge(CaptureLabel, crate::gui::draw::GuiLabel);
}

pub fn finish(app: &mut App) {
    let shader = app
        .world()
        .resource::<AssetServer>()
        .load(super::embedded_path("renderer/mod_capture.wgsl"));
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app.insert_resource(CaptureShader(shader));
    }
}
