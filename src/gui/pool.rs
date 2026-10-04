use bevy::prelude::*;
use bevy::render::Extract;
use bevy::render::extract_resource::ExtractResource;
use bevy::render::render_resource::{Buffer, BufferDescriptor, BufferUsages};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bytemuck::{Pod, Zeroable};
use parking_lot::Mutex;

#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
pub struct GuiVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

pub const GUI_VERTEX_STRIDE: u64 = 32;
const _: () = assert!(size_of::<GuiVertex>() as u64 == GUI_VERTEX_STRIDE);

#[derive(Clone, Copy)]
pub struct GuiRange {
    pub window: Entity,
    pub first_index: u32,
    pub index_count: u32,
    pub base_vertex: i32,
}

#[derive(Default)]
pub struct GuiFrame {
    pub vertices: Vec<GuiVertex>,
    pub indices: Vec<u32>,
    pub ranges: Vec<GuiRange>,
}

#[derive(Resource, Default)]
pub struct GuiFrames(pub Mutex<GuiFrame>);

impl GuiFrames {
    #[allow(clippy::too_many_arguments, reason = "one range's whole description")]
    pub fn push(
        &self,
        window: Entity,
        positions: &[[f32; 3]],
        uvs: &[[f32; 2]],
        colors: &[[f32; 4]],
        indices: &[u32],
        vw: f32,
        vh: f32,
    ) {
        let mut frame = self.0.lock();
        let base_vertex = frame.vertices.len() as i32;
        let first_index = frame.indices.len() as u32;
        let (sx, sy) = (2.0 / vw, 2.0 / vh);
        frame.vertices.reserve(positions.len());
        for i in 0..positions.len() {
            let p = positions[i];
            frame.vertices.push(GuiVertex {
                pos: [p[0] * sx - 1.0, 1.0 - p[1] * sy],
                uv: uvs[i],
                color: colors[i],
            });
        }
        frame.indices.extend_from_slice(indices);
        frame.ranges.push(GuiRange {
            window,
            first_index,
            index_count: indices.len() as u32,
            base_vertex,
        });
    }
}

#[derive(Resource, Clone, ExtractResource)]
pub struct GuiTextures {
    pub atlas: Handle<Image>,
    pub unihex: Handle<Image>,
}

#[derive(Resource, Default)]
pub struct GuiPool {
    pub vertices: Option<Buffer>,
    pub indices: Option<Buffer>,
    pub vertex_capacity: usize,
    pub index_capacity: usize,
    pub ranges: Vec<GuiRange>,
}

const INITIAL_VERTICES: usize = 16384;
const INITIAL_INDICES: usize = INITIAL_VERTICES / 4 * 6;

impl GuiPool {
    fn ensure(&mut self, device: &RenderDevice, verts: usize, indices: usize) {
        if self.vertices.is_none() || self.vertex_capacity < verts {
            let mut cap = self.vertex_capacity.max(INITIAL_VERTICES);
            while cap < verts {
                cap *= 2;
            }
            self.vertices = Some(device.create_buffer(&BufferDescriptor {
                label: Some("gui_vertices"),
                size: cap as u64 * GUI_VERTEX_STRIDE,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.vertex_capacity = cap;
        }
        if self.indices.is_none() || self.index_capacity < indices {
            let mut cap = self.index_capacity.max(INITIAL_INDICES);
            while cap < indices {
                cap *= 2;
            }
            self.indices = Some(device.create_buffer(&BufferDescriptor {
                label: Some("gui_indices"),
                size: cap as u64 * 4,
                usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.index_capacity = cap;
        }
    }
}

pub fn extract_gui(
    frames: Extract<Res<GuiFrames>>,
    gate: Extract<Option<Res<crate::renderer::systems::FrameGate>>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    mut pool: ResMut<GuiPool>,
) {
    let mut frame = frames.0.lock();
    pool.ranges.clear();
    let render = gate.as_ref().is_none_or(|g| g.render);

    if render && !frame.indices.is_empty() {
        pool.ensure(&device, frame.vertices.len(), frame.indices.len());
        if let (Some(vertices), Some(indices)) = (&pool.vertices, &pool.indices) {
            queue.write_buffer(vertices, 0, bytemuck::cast_slice(&frame.vertices));
            queue.write_buffer(indices, 0, bytemuck::cast_slice(&frame.indices));
            pool.ranges.extend_from_slice(&frame.ranges);
        }
    }

    frame.vertices.clear();
    frame.indices.clear();
    frame.ranges.clear();
}
