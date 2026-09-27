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
    META_LIVE, META_WATER, ORIGIN_ROW, SLOTS_INITIAL, SlotMeta, TerrainOp, TerrainOpQueue,
    TerrainOps, TerrainStats, TerrainTier,
};

const ORIGIN_TEXEL: usize = 16;

#[cfg(not(target_arch = "wasm32"))]
const POOL_VERTICES: u32 = 3_200_000;
#[cfg(target_arch = "wasm32")]
const POOL_VERTICES: u32 = 1_200_000;

#[cfg(not(target_arch = "wasm32"))]
const POOL_INDICES: u32 = 6_400_000;
#[cfg(target_arch = "wasm32")]
const POOL_INDICES: u32 = 2_400_000;

pub struct Pool {
    pub vertices: Buffer,
    pub indices: Buffer,
    valloc: Allocator,
    ialloc: Allocator,
    pub vertex_capacity: u32,
    pub index_capacity: u32,
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
            valloc: Allocator::new(vertex_capacity),
            ialloc: Allocator::new(index_capacity),
            vertex_capacity,
            index_capacity,
        }
    }

    fn byte_size(&self) -> u64 {
        self.vertex_capacity as u64 * 20 + self.index_capacity as u64 * 4
    }
}

#[derive(Clone, Copy)]
struct SlotState {
    pool: u32,
    vertex: Allocation,
    index: Allocation,
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
    index: Allocation,
}

#[derive(Resource)]
pub struct TerrainPools {
    pub pools: Vec<Pool>,
    pub slot_cap: u32,
    pub meta_cpu: Vec<SlotMeta>,
    pub meta: Buffer,
    pub meta_stride: u32,
    pub vis_cpu: Vec<u32>,
    pub vis: Buffer,
    pub tables_generation: u64,
    pub origins_view: Option<TextureView>,

    origins: Option<Texture>,
    slots: Vec<Option<SlotState>>,
    live_slots: usize,
    used_bytes: u64,
    retired_this_frame: Vec<Retired>,
    retired_last_frame: Vec<Retired>,
    pool_vertex_capacity: u32,
    pool_index_capacity: u32,
    tier: TerrainTier,
    stats: TerrainStats,
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

    pub fn meta_offset(&self, slot: u32) -> u32 {
        slot * self.meta_stride
    }

    fn new(device: &RenderDevice, tier: TerrainTier, stats: TerrainStats) -> Self {
        let limits = device.limits();
        let slot_cap = SLOTS_INITIAL;

        let pool_vertex_capacity = clamp_units(POOL_VERTICES, 20, limits.max_buffer_size);
        let pool_index_capacity = clamp_units(POOL_INDICES, 4, limits.max_buffer_size);

        let meta_cpu = vec![SlotMeta::default(); slot_cap as usize];
        let vis_cpu = vec![u32::MAX; slot_cap.div_ceil(32) as usize];

        let (meta, meta_stride) = if tier.indirect {
            let meta = device.create_buffer(&BufferDescriptor {
                label: Some("terrain_meta"),
                size: slot_cap as u64 * 64,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            (meta, 64)
        } else {
            let stride = limits.min_uniform_buffer_offset_alignment.max(64);
            let meta = device.create_buffer(&BufferDescriptor {
                label: Some("terrain_meta"),
                size: slot_cap as u64 * stride as u64,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            (meta, stride)
        };

        let vis = if tier.indirect {
            device.create_buffer(&BufferDescriptor {
                label: Some("terrain_vis"),
                size: vis_cpu.len() as u64 * 4,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        } else {
            device.create_buffer(&BufferDescriptor {
                label: Some("terrain_vis_unused"),
                size: 4,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };

        let (origins, origins_view) = match tier.indirect {
            true => (None, None),
            false => {
                let (t, v) = new_origins(device, slot_cap);
                (Some(t), Some(v))
            }
        };

        Self {
            pools: Vec::new(),
            slot_cap,
            meta_cpu,
            meta,
            meta_stride,
            vis_cpu,
            vis,
            tables_generation: 0,
            origins_view,
            origins,
            slots: vec![None; slot_cap as usize],
            live_slots: 0,
            used_bytes: 0,
            retired_this_frame: Vec::new(),
            retired_last_frame: Vec::new(),
            pool_vertex_capacity,
            pool_index_capacity,
            tier,
            stats,
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

        self.meta = if self.tier.indirect {
            device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("terrain_meta"),
                contents: bytemuck::cast_slice(&self.meta_cpu),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            })
        } else {
            let stride = self.meta_stride as usize;
            let mut padded = vec![0u8; self.meta_cpu.len() * stride];
            for (i, m) in self.meta_cpu.iter().enumerate() {
                padded[i * stride..i * stride + 64].copy_from_slice(bytemuck::bytes_of(m));
            }
            device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("terrain_meta"),
                contents: &padded,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            })
        };

        if self.tier.indirect {
            self.vis = device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("terrain_vis"),
                contents: bytemuck::cast_slice(&self.vis_cpu),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            });
        }

        if self.origins.is_some() {
            let (texture, view) = new_origins(device, self.slot_cap);
            self.origins = Some(texture);
            self.origins_view = Some(view);
            self.write_origins_all(queue);
        }

        self.tables_generation += 1;
    }

    fn write_origins_all(&self, queue: &RenderQueue) {
        let Some(texture) = self.origins.as_ref() else {
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
        let Some(texture) = self.origins.as_ref() else {
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
        if !self.tier.indirect {
            self.write_origin_slot(queue, slot);
            return;
        }

        queue.write_buffer(
            &self.meta,
            self.meta_offset(slot) as u64,
            bytemuck::bytes_of(&self.meta_cpu[slot as usize]),
        );
    }

    fn write_vis_word(&self, queue: &RenderQueue, slot: u32) {
        if !self.tier.indirect {
            return;
        }
        let word = (slot / 32) as usize;
        queue.write_buffer(
            &self.vis,
            word as u64 * 4,
            bytemuck::bytes_of(&self.vis_cpu[word]),
        );
    }

    fn retire_slot(&mut self, slot: u32) {
        if let Some(state) = self.slots[slot as usize].take() {
            self.retired_this_frame.push(Retired {
                pool: state.pool,
                vertex: state.vertex,
                index: state.index,
            });
            self.live_slots -= 1;
            self.used_bytes -= state.bytes();
        }
    }

    fn allocate(
        &mut self,
        device: &RenderDevice,
        vertex_count: u32,
        index_count: u32,
    ) -> Option<(u32, Allocation, Allocation)> {
        for (i, pool) in self.pools.iter_mut().enumerate() {
            if let Some(v) = pool.valloc.allocate(vertex_count) {
                if let Some(idx) = pool.ialloc.allocate(index_count) {
                    return Some((i as u32, v, idx));
                }
                pool.valloc.free(v);
            }
        }
        if vertex_count > self.pool_vertex_capacity || index_count > self.pool_index_capacity {
            return None;
        }
        let index = self.pools.len() as u32;
        let mut pool = Pool::new(device, self.pool_vertex_capacity, self.pool_index_capacity);
        let v = pool.valloc.allocate(vertex_count)?;
        let idx = pool.ialloc.allocate(index_count)?;
        self.pools.push(pool);
        Some((index, v, idx))
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
            pool.ialloc.free(r.index);
        }
    }
    pools.retired_last_frame = std::mem::take(&mut pools.retired_this_frame);

    let ops = std::mem::take(&mut queue.0);
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
                let Some((pool_index, vertex, index)) =
                    pools.allocate(&device, vertex_count, index_count)
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

                let base_vertex = if pools.tier.base_vertex {
                    vertex.offset
                } else {
                    for i in mesh.idx.iter_mut().chain(mesh.cutout_idx.iter_mut()) {
                        *i += vertex.offset;
                    }
                    0
                };

                if !pools.tier.indirect {
                    for v in mesh.verts.iter_mut() {
                        v.pos[3] = slot as u16 as i16;
                    }
                }

                let pool = &pools.pools[pool_index as usize];
                render_queue.write_buffer(
                    &pool.vertices,
                    vertex.offset as u64 * 20,
                    bytemuck::cast_slice(&mesh.verts),
                );
                let first_index = index.offset;
                render_queue.write_buffer(
                    &pool.indices,
                    first_index as u64 * 4,
                    bytemuck::cast_slice(&mesh.idx),
                );
                render_queue.write_buffer(
                    &pool.indices,
                    (first_index as u64 + mesh.idx.len() as u64) * 4,
                    bytemuck::cast_slice(&mesh.cutout_idx),
                );

                let (min, max) = mesh.bounds();
                pools.meta_cpu[slot as usize] = SlotMeta {
                    min,
                    first_index,
                    max,
                    index_count,
                    origin,
                    base_vertex,
                    solid_count: mesh.idx.len() as u32,
                    flags: META_LIVE | if water { META_WATER } else { 0 },
                    pool: pool_index,
                    _pad: 0,
                };
                let state = SlotState {
                    pool: pool_index,
                    vertex,
                    index,
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
                if pools.tier.indirect {
                    let bytes = pools.meta_cpu.len() * pools.meta_stride as usize;
                    render_queue.write_buffer(&pools.meta, 0, &vec![0u8; bytes]);
                }
                pools.write_origins_all(&render_queue);
            }
            TerrainOp::Visibility(bits) => {
                let n = bits.len().min(pools.vis_cpu.len());
                pools.vis_cpu[..n].copy_from_slice(&bits[..n]);
                for w in pools.vis_cpu[n..].iter_mut() {
                    *w = u32::MAX;
                }
                if pools.tier.indirect {
                    render_queue.write_buffer(&pools.vis, 0, bytemuck::cast_slice(&pools.vis_cpu));
                }
            }
        }
    }

    pools.publish_stats();
}
