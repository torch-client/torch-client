use std::sync::atomic::Ordering;

use bevy::prelude::*;
use bevy::render::render_resource::{
    Buffer, BufferDescriptor, BufferInitDescriptor, BufferUsages, Extent3d, Origin3d,
    TexelCopyBufferLayout, TexelCopyTextureInfo, Texture, TextureAspect, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::{Extract, ExtractSchedule, Render, RenderApp, RenderSystems};
use offset_allocator::{Allocation, Allocator};

use super::{
    META_LIVE, META_WATER, ORIGIN_ROW, SLOTS_INITIAL, STREAM_CUTOUT, STREAM_SOLID, STREAM_WATER,
    STREAMS, SlotMeta, TerrainOp, TerrainOpQueue, TerrainOps, TerrainStats, TerrainTier,
};

const ORIGIN_TEXEL: usize = 16;

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
const POOL_VERTICES: u32 = 3_200_000;
#[cfg(any(target_arch = "wasm32", target_os = "android"))]
const POOL_VERTICES: u32 = 1_200_000;

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
const POOL_INDICES: u32 = 6_400_000;
#[cfg(any(target_arch = "wasm32", target_os = "android"))]
const POOL_INDICES: u32 = 2_400_000;

const PAGE_INDICES: u32 = 1 << 16;

const PAGE_ALLOCS: u32 = 512;

pub struct Pool {
    pub vertices: Buffer,
    pub indices: Buffer,
    valloc: Allocator,
    ialloc: Allocator,
    pages: [Vec<Page>; STREAMS as usize],
    pub vertex_capacity: u32,
    pub index_capacity: u32,
    #[cfg(feature = "shader_support")]
    pub pack_quads: Option<PackQuads>,
}

#[cfg(feature = "shader_support")]
pub struct PackQuads {
    texture: Texture,
    pub group: bevy::render::render_resource::BindGroup,
}

impl Pool {
    fn new(device: &RenderDevice, vertex_capacity: u32, index_capacity: u32) -> Self {
        let vertices = device.create_buffer(&BufferDescriptor {
            label: Some("terrain_pool_vertices"),
            size: vertex_capacity as u64 * 20,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let indices = device.create_buffer(&BufferDescriptor {
            label: Some("terrain_pool_indices"),
            size: index_capacity as u64 * 4,
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            vertices,
            indices,
            valloc: Allocator::new(vertex_capacity / 4),
            ialloc: Allocator::new(index_capacity),
            pages: Default::default(),
            vertex_capacity,
            index_capacity,
            #[cfg(feature = "shader_support")]
            pack_quads: None,
        }
    }

    fn byte_size(&self) -> u64 {
        #[cfg(feature = "shader_support")]
        let pack = self
            .pack_quads
            .as_ref()
            .map_or(0, |_| self.pack_byte_size());
        #[cfg(not(feature = "shader_support"))]
        let pack = 0;
        self.vertex_capacity as u64 * 20 + self.index_capacity as u64 * 4 + pack
    }

    fn allocate_indices(&mut self, stream: u32, count: u32) -> Option<IndexRange> {
        let pages = &mut self.pages[stream as usize];
        for page in pages.iter_mut() {
            if let Some(sub) = page.sub.allocate(count) {
                page.live += 1;
                return Some(IndexRange {
                    stream,
                    page: page.first,
                    sub,
                    first: page.first + sub.offset,
                });
            }
        }
        let size = page_size(count.max(PAGE_INDICES));
        let range = self.ialloc.allocate(size)?;
        let mut sub_alloc = Allocator::with_max_allocs(size, PAGE_ALLOCS);
        let Some(sub) = sub_alloc.allocate(count) else {
            self.ialloc.free(range);
            return None;
        };
        let first = range.offset;
        pages.push(Page {
            range,
            first,
            sub: sub_alloc,
            live: 1,
        });
        Some(IndexRange {
            stream,
            page: first,
            sub,
            first: first + sub.offset,
        })
    }

    fn free_indices(&mut self, range: IndexRange) {
        let pages = &mut self.pages[range.stream as usize];
        let Some(i) = pages.iter().position(|p| p.first == range.page) else {
            return;
        };
        pages[i].sub.free(range.sub);
        pages[i].live -= 1;
        if pages[i].live == 0 {
            let page = pages.remove(i);
            self.ialloc.free(page.range);
        }
    }

    fn free_slot_indices(&mut self, ranges: [Option<IndexRange>; 2]) {
        for range in ranges.into_iter().flatten() {
            self.free_indices(range);
        }
    }

    fn allocate_slot(
        &mut self,
        vertex_count: u32,
        streams: [(u32, u32); 2],
    ) -> Option<(Allocation, [Option<IndexRange>; 2])> {
        let vertex = self.valloc.allocate(vertex_count.div_ceil(4))?;
        let mut indices = [None; 2];
        for (k, &(stream, count)) in streams.iter().enumerate() {
            if count == 0 {
                continue;
            }
            let Some(range) = self.allocate_indices(stream, count) else {
                self.free_slot_indices(indices);
                self.valloc.free(vertex);
                return None;
            };
            indices[k] = Some(range);
        }
        Some((vertex, indices))
    }

    #[cfg(feature = "shader_support")]
    fn pack_extent(&self) -> Extent3d {
        let quads = self.vertex_capacity / 4;
        Extent3d {
            width: crate::renderer::packvertex::QUAD_ROW,
            height: quads.div_ceil(crate::renderer::packvertex::QUAD_ROW).max(1),
            depth_or_array_layers: 1,
        }
    }

    #[cfg(feature = "shader_support")]
    fn pack_byte_size(&self) -> u64 {
        let extent = self.pack_extent();
        u64::from(extent.width)
            * u64::from(extent.height)
            * std::mem::size_of::<crate::renderer::packvertex::PackQuad>() as u64
    }

    #[cfg(feature = "shader_support")]
    fn pack_quads(
        &mut self,
        device: &RenderDevice,
        layout: &bevy::render::render_resource::BindGroupLayout,
    ) -> &PackQuads {
        let size = self.pack_extent();
        self.pack_quads.get_or_insert_with(|| {
            let texture = device.create_texture(&TextureDescriptor {
                label: Some("shaderpack terrain_pool_pack_quads"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba32Uint,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&TextureViewDescriptor::default());
            let group = device.create_bind_group(
                "shaderpack pack quads",
                layout,
                &bevy::render::render_resource::BindGroupEntries::single(&view),
            );
            PackQuads { texture, group }
        })
    }
}

struct Page {
    range: Allocation,
    first: u32,
    sub: Allocator,
    live: u32,
}

fn page_size(count: u32) -> u32 {
    let unit = 1u32 << (u32::BITS - count.leading_zeros()).saturating_sub(4);
    count.div_ceil(unit) * unit
}

#[derive(Clone, Copy)]
struct IndexRange {
    stream: u32,
    page: u32,
    sub: Allocation,
    first: u32,
}

#[cfg(feature = "shader_support")]
fn write_quads(
    queue: &RenderQueue,
    texture: &Texture,
    first: u32,
    quads: &[crate::renderer::packvertex::PackQuad],
) {
    use crate::renderer::packvertex::{QUAD_ROW, quad_texel};
    let mut at = first;
    let mut rest = quads;
    while !rest.is_empty() {
        let [x, y] = quad_texel(at);
        let left = rest.len() as u32;
        let (width, height) = if x == 0 && left >= QUAD_ROW {
            (QUAD_ROW, left / QUAD_ROW)
        } else {
            ((QUAD_ROW - x).min(left), 1)
        };
        let count = (width * height) as usize;
        queue.write_texture(
            TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: Origin3d { x, y, z: 0 },
                aspect: TextureAspect::All,
            },
            bytemuck::cast_slice(&rest[..count]),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 16),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        at += count as u32;
        rest = &rest[count..];
    }
}

#[derive(Clone, Copy)]
struct SlotState {
    pool: u32,
    vertex: Allocation,
    indices: [Option<IndexRange>; 2],
    vertex_count: u32,
    index_count: u32,
}

impl SlotState {
    fn bytes(&self) -> u64 {
        self.vertex_count as u64 * 20 + self.index_count as u64 * 4
    }
}

struct Retired {
    pool: u32,
    vertex: Allocation,
    indices: [Option<IndexRange>; 2],
}

const META_STRIDE: u64 = 64;
const _: () = assert!(std::mem::size_of::<SlotMeta>() == META_STRIDE as usize);

pub enum SlotTable {
    Indirect {
        meta: Buffer,
        vis: Buffer,
    },
    Direct {
        origins: Texture,
        origins_view: TextureView,
    },
}

#[derive(Resource)]
pub struct TerrainPools {
    pub pools: Vec<Pool>,
    pub slot_cap: u32,
    pub meta_cpu: Vec<SlotMeta>,
    pub vis_cpu: Vec<u32>,
    pub table: SlotTable,
    pub tables_generation: u64,
    pub draw_end: u32,
    pub water_slots: Vec<u32>,

    slots: Vec<Option<SlotState>>,
    live_slots: usize,
    used_bytes: u64,
    retired_this_frame: Vec<Retired>,
    retired_last_frame: Vec<Retired>,
    pool_vertex_capacity: u32,
    pool_index_capacity: u32,
    tier: TerrainTier,
    pub(super) stats: TerrainStats,
    #[cfg(feature = "shader_support")]
    pub quads_layout: bevy::render::render_resource::BindGroupLayout,
}

fn new_origins(device: &RenderDevice, slot_cap: u32) -> (Texture, TextureView) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("terrain_origins"),
        size: Extent3d {
            width: ORIGIN_ROW,
            height: slot_cap.div_ceil(ORIGIN_ROW),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba32Sint,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());

    (texture, view)
}

impl TerrainPools {
    pub fn is_live(&self, slot: u32) -> bool {
        self.meta_cpu
            .get(slot as usize)
            .is_some_and(|m| m.flags & META_LIVE != 0)
    }

    pub fn is_visible(&self, slot: u32) -> bool {
        let word = self
            .vis_cpu
            .get(slot as usize / 32)
            .copied()
            .unwrap_or(u32::MAX);
        (word >> (slot % 32)) & 1 != 0
    }

    fn new(device: &RenderDevice, tier: TerrainTier, stats: TerrainStats) -> Self {
        let limits = device.limits();
        let slot_cap = SLOTS_INITIAL;

        let pool_vertex_capacity = clamp_units(POOL_VERTICES, 20, limits.max_buffer_size);
        let pool_index_capacity = clamp_units(POOL_INDICES, 4, limits.max_buffer_size);

        let meta_cpu = vec![SlotMeta::default(); slot_cap as usize];
        let vis_cpu = vec![u32::MAX; slot_cap.div_ceil(32) as usize];

        let table = if tier.indirect {
            SlotTable::Indirect {
                meta: device.create_buffer(&BufferDescriptor {
                    label: Some("terrain_meta"),
                    size: slot_cap as u64 * META_STRIDE,
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                vis: device.create_buffer(&BufferDescriptor {
                    label: Some("terrain_vis"),
                    size: vis_cpu.len() as u64 * 4,
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
            }
        } else {
            let (origins, origins_view) = new_origins(device, slot_cap);
            SlotTable::Direct {
                origins,
                origins_view,
            }
        };

        Self {
            pools: Vec::new(),
            slot_cap,
            meta_cpu,
            vis_cpu,
            table,
            tables_generation: 0,
            draw_end: 0,
            water_slots: Vec::new(),
            slots: vec![None; slot_cap as usize],
            live_slots: 0,
            used_bytes: 0,
            retired_this_frame: Vec::new(),
            retired_last_frame: Vec::new(),
            pool_vertex_capacity,
            pool_index_capacity,
            tier,
            stats,
            #[cfg(feature = "shader_support")]
            quads_layout: {
                let descriptor = crate::renderer::packvertex::quads_layout();
                device.create_bind_group_layout(descriptor.label.as_ref(), &descriptor.entries)
            },
        }
    }

    fn grow_tables(&mut self, device: &RenderDevice, queue: &RenderQueue, slot: u32) {
        let mut new_cap = self.slot_cap;
        while slot >= new_cap {
            new_cap *= 2;
        }
        self.meta_cpu.resize(new_cap as usize, SlotMeta::default());
        self.vis_cpu.resize(new_cap.div_ceil(32) as usize, u32::MAX);
        self.slots.resize(new_cap as usize, None);
        self.slot_cap = new_cap;

        match &mut self.table {
            SlotTable::Indirect { meta, vis } => {
                *meta = device.create_buffer_with_data(&BufferInitDescriptor {
                    label: Some("terrain_meta"),
                    contents: bytemuck::cast_slice(&self.meta_cpu),
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                });
                *vis = device.create_buffer_with_data(&BufferInitDescriptor {
                    label: Some("terrain_vis"),
                    contents: bytemuck::cast_slice(&self.vis_cpu),
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                });
            }
            SlotTable::Direct {
                origins,
                origins_view,
            } => {
                let (texture, view) = new_origins(device, self.slot_cap);
                *origins = texture;
                *origins_view = view;
            }
        }
        self.write_origins_all(queue);

        self.tables_generation += 1;
    }

    fn write_origins_all(&self, queue: &RenderQueue) {
        let SlotTable::Direct {
            origins: texture, ..
        } = &self.table
        else {
            return;
        };

        let rows = self.slot_cap.div_ceil(ORIGIN_ROW);
        let mut texels = vec![0i32; (rows * ORIGIN_ROW) as usize * 4];
        for (slot, meta) in self.meta_cpu.iter().enumerate() {
            texels[slot * 4..slot * 4 + 3].copy_from_slice(&meta.origin);
        }

        queue.write_texture(
            TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            bytemuck::cast_slice(&texels),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ORIGIN_ROW * ORIGIN_TEXEL as u32),
                rows_per_image: Some(rows),
            },
            Extent3d {
                width: ORIGIN_ROW,
                height: rows,
                depth_or_array_layers: 1,
            },
        );
    }

    fn write_origin_slot(&self, queue: &RenderQueue, slot: u32) {
        let SlotTable::Direct {
            origins: texture, ..
        } = &self.table
        else {
            return;
        };

        let origin = self.meta_cpu[slot as usize].origin;
        let texel = [origin[0], origin[1], origin[2], 0];

        queue.write_texture(
            TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: Origin3d {
                    x: slot % ORIGIN_ROW,
                    y: slot / ORIGIN_ROW,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            bytemuck::cast_slice(&texel),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ORIGIN_TEXEL as u32),
                rows_per_image: Some(1),
            },
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }

    fn write_meta_slot(&self, queue: &RenderQueue, slot: u32) {
        match &self.table {
            SlotTable::Indirect { meta, .. } => queue.write_buffer(
                meta,
                slot as u64 * META_STRIDE,
                bytemuck::bytes_of(&self.meta_cpu[slot as usize]),
            ),
            SlotTable::Direct { .. } => self.write_origin_slot(queue, slot),
        }
    }

    fn write_vis_word(&self, queue: &RenderQueue, slot: u32) {
        let SlotTable::Indirect { vis, .. } = &self.table else {
            return;
        };
        let word = (slot / 32) as usize;
        queue.write_buffer(
            vis,
            word as u64 * 4,
            bytemuck::bytes_of(&self.vis_cpu[word]),
        );
    }

    fn retire_slot(&mut self, slot: u32) {
        if let Some(state) = self.slots[slot as usize].take() {
            self.retired_this_frame.push(Retired {
                pool: state.pool,
                vertex: state.vertex,
                indices: state.indices,
            });
            self.live_slots -= 1;
            self.used_bytes -= state.bytes();
        }
    }

    fn allocate(
        &mut self,
        device: &RenderDevice,
        vertex_count: u32,
        streams: [(u32, u32); 2],
    ) -> Option<(u32, Allocation, [Option<IndexRange>; 2])> {
        for (i, pool) in self.pools.iter_mut().enumerate() {
            if let Some((vertex, indices)) = pool.allocate_slot(vertex_count, streams) {
                return Some((i as u32, vertex, indices));
            }
        }
        let index_count: u32 = streams.iter().map(|&(_, count)| count).sum();
        if vertex_count > self.pool_vertex_capacity || index_count > self.pool_index_capacity {
            return None;
        }
        let index = self.pools.len() as u32;
        let mut pool = Pool::new(device, self.pool_vertex_capacity, self.pool_index_capacity);
        let (vertex, indices) = pool.allocate_slot(vertex_count, streams)?;
        self.pools.push(pool);
        Some((index, vertex, indices))
    }

    fn publish_stats(&self) {
        let capacity: u64 = self.pools.iter().map(Pool::byte_size).sum();
        self.stats
            .0
            .live_slots
            .store(self.live_slots, Ordering::Relaxed);
        self.stats
            .0
            .used_bytes
            .store(self.used_bytes, Ordering::Relaxed);
        self.stats
            .0
            .capacity_bytes
            .store(capacity, Ordering::Relaxed);
        self.stats
            .0
            .pools
            .store(self.pools.len(), Ordering::Relaxed);
        self.stats
            .0
            .indirect
            .store(self.tier.indirect as usize, Ordering::Relaxed);
    }
}

fn clamp_units(unit_capacity: u32, unit_bytes: u64, max_buffer_size: u64) -> u32 {
    let max_units = max_buffer_size / unit_bytes;
    unit_capacity.min(max_units.min(u32::MAX as u64) as u32)
}

pub fn build(app: &mut App, stats: TerrainStats) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .insert_resource(stats)
        .init_resource::<TerrainOpQueue>()
        .add_systems(ExtractSchedule, extract_ops)
        .add_systems(Render, apply_ops.in_set(RenderSystems::PrepareResources));
}

fn extract_ops(main_ops: Extract<Res<TerrainOps>>, mut queue: ResMut<TerrainOpQueue>) {
    queue.0.append(&mut main_ops.0.lock());
}

pub fn finish(app: &mut App) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    let device = render_app.world().resource::<RenderDevice>().clone();
    let tier = *render_app.world().resource::<TerrainTier>();
    let stats = render_app.world().resource::<TerrainStats>().clone();
    let pools = TerrainPools::new(&device, tier, stats);
    render_app.insert_resource(pools);
}

pub fn apply_ops(
    mut pools: ResMut<TerrainPools>,
    mut queue: ResMut<TerrainOpQueue>,
    device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    let due = std::mem::take(&mut pools.retired_last_frame);
    for r in due {
        if let Some(pool) = pools.pools.get_mut(r.pool as usize) {
            pool.valloc.free(r.vertex);
            pool.free_slot_indices(r.indices);
        }
    }
    pools.retired_last_frame = std::mem::take(&mut pools.retired_this_frame);

    #[cfg(feature = "shader_support")]
    let quads_layout = if crate::renderer::packvertex::active() {
        let layout = pools.quads_layout.clone();
        for pool in &mut pools.pools {
            pool.pack_quads(&device, &layout);
        }
        Some(layout)
    } else {
        for pool in &mut pools.pools {
            pool.pack_quads = None;
        }
        None
    };

    let ops = std::mem::take(&mut queue.0);
    let changed = !ops.is_empty();
    let slots_changed = ops.iter().any(|op| !matches!(op, TerrainOp::Visibility(_)));
    for op in ops {
        match op {
            TerrainOp::Upload {
                slot,
                origin,
                water,
                mut mesh,
            } => {
                if slot >= pools.slot_cap {
                    pools.grow_tables(&device, &render_queue, slot);
                }
                pools.retire_slot(slot);

                let unnameable = !pools.tier.indirect && slot > u16::MAX as u32;
                if unnameable {
                    crate::log_warn!(
                        "render",
                        "terrain: slot {slot} past the direct tier's 16-bit id lane; dropped"
                    );
                }

                if mesh.is_empty() || unnameable {
                    pools.meta_cpu[slot as usize] = SlotMeta::default();
                    pools.write_meta_slot(&render_queue, slot);
                    continue;
                }

                let vertex_count = mesh.verts.len() as u32;
                let index_count = mesh.index_count() as u32;
                let solid_count = mesh.idx.len() as u32;
                let streams = if water {
                    [(STREAM_WATER, index_count), (STREAM_WATER, 0)]
                } else {
                    [
                        (STREAM_SOLID, solid_count),
                        (STREAM_CUTOUT, index_count - solid_count),
                    ]
                };
                let Some((pool_index, vertex, indices)) =
                    pools.allocate(&device, vertex_count, streams)
                else {
                    crate::log_warn!(
                        "render",
                        "terrain: section at slot {slot} has {vertex_count} verts / \
                         {index_count} indices, too large for a fresh pool; dropped"
                    );
                    pools.meta_cpu[slot as usize] = SlotMeta::default();
                    pools.write_meta_slot(&render_queue, slot);
                    continue;
                };

                debug_assert!(vertex_count % 4 == 0, "terrain meshes are quads");
                let first_vertex = vertex.offset * 4;

                let base_vertex = if pools.tier.indirect && pools.tier.base_vertex {
                    first_vertex
                } else {
                    for i in mesh.idx.iter_mut().chain(mesh.cutout_idx.iter_mut()) {
                        *i += first_vertex;
                    }
                    0
                };

                if !pools.tier.indirect {
                    for v in mesh.verts.iter_mut() {
                        v.pos[3] = slot as u16 as i16;
                    }
                }

                #[cfg(feature = "shader_support")]
                if let Some(layout) = &quads_layout
                    && !mesh.pack.is_empty()
                    && mesh.pack.len() * 4 == mesh.verts.len()
                {
                    let quads = pools.pools[pool_index as usize].pack_quads(&device, layout);
                    write_quads(&render_queue, &quads.texture, vertex.offset, &mesh.pack);
                }
                let pool = &pools.pools[pool_index as usize];
                render_queue.write_buffer(
                    &pool.vertices,
                    first_vertex as u64 * 20,
                    bytemuck::cast_slice(&mesh.verts),
                );
                let first_index = indices[0].map_or(0, |r| r.first);
                let cutout_first = match indices[1] {
                    Some(r) => r.first,
                    None if water => first_index + solid_count,
                    None => 0,
                };
                for (first, list) in [(first_index, &mesh.idx), (cutout_first, &mesh.cutout_idx)] {
                    if !list.is_empty() {
                        render_queue.write_buffer(
                            &pool.indices,
                            first as u64 * 4,
                            bytemuck::cast_slice(list),
                        );
                    }
                }

                let (min, max) = mesh.bounds();
                pools.meta_cpu[slot as usize] = SlotMeta {
                    min,
                    first_index,
                    max,
                    index_count,
                    origin,
                    base_vertex,
                    solid_count: if water { 0 } else { solid_count },
                    flags: META_LIVE | if water { META_WATER } else { 0 },
                    pool: pool_index,
                    cutout_first: if water { 0 } else { cutout_first },
                };
                let state = SlotState {
                    pool: pool_index,
                    vertex,
                    indices,
                    vertex_count,
                    index_count,
                };
                pools.live_slots += 1;
                pools.used_bytes += state.bytes();
                pools.slots[slot as usize] = Some(state);
                pools.write_meta_slot(&render_queue, slot);

                let word = (slot / 32) as usize;
                pools.vis_cpu[word] |= 1 << (slot % 32);
                pools.write_vis_word(&render_queue, slot);
            }
            TerrainOp::Free { slot } => {
                if slot >= pools.slot_cap {
                    continue;
                }
                pools.retire_slot(slot);
                pools.meta_cpu[slot as usize] = SlotMeta::default();
                pools.write_meta_slot(&render_queue, slot);
            }
            TerrainOp::ClearAll => {
                for slot in 0..pools.slot_cap {
                    pools.retire_slot(slot);
                }
                for m in pools.meta_cpu.iter_mut() {
                    *m = SlotMeta::default();
                }
                if let SlotTable::Indirect { meta, .. } = &pools.table {
                    let bytes = pools.meta_cpu.len() * META_STRIDE as usize;
                    render_queue.write_buffer(meta, 0, &vec![0u8; bytes]);
                }
                pools.write_origins_all(&render_queue);
            }
            TerrainOp::Visibility(bits) => {
                let n = bits.len().min(pools.vis_cpu.len());
                pools.vis_cpu[..n].copy_from_slice(&bits[..n]);
                for w in pools.vis_cpu[n..].iter_mut() {
                    *w = u32::MAX;
                }
                if let SlotTable::Indirect { vis, .. } = &pools.table {
                    render_queue.write_buffer(vis, 0, bytemuck::cast_slice(&pools.vis_cpu));
                }
            }
        }
    }

    if changed {
        pools.draw_end = pools
            .slots
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |i| i as u32 + 1);
    }
    if slots_changed && pools.tier.indirect {
        let pools = &mut *pools;
        let end = (pools.draw_end as usize).min(pools.meta_cpu.len());
        pools.water_slots.clear();
        pools.water_slots.extend(
            (0u32..)
                .zip(&pools.meta_cpu[..end])
                .filter(|(_, meta)| meta.flags & META_WATER != 0)
                .map(|(slot, _)| slot),
        );
    }

    pools.publish_stats();
}
