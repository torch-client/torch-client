mod extract;

use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{
    BufferDescriptor, BufferUsages, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages, TextureViewDescriptor,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::texture::GpuImage;

use super::GPU_LABEL;
use super::compile::{EntityKind, Segment};
use super::frame::{Install, PackRender, Stage};
use super::pipelines::Building;
use super::targets::extent;

pub(crate) use extract::{extract_entity_draws, extract_standard_draws};

pub(crate) struct EntityDraw {
    mesh: AssetId<Mesh>,
    texture: Option<AssetId<Image>>,
    model: Mat4,
    tint: Vec4,
    light: Vec2,
    blend: bool,
    pass: EntityKind,
    overlay: Vec4,
    entity_id: i32,
    item_id: i32,
    stage: i32,
    order: u8,
    relative: bool,
    flags: u32,
}

impl EntityDraw {
    fn segment(&self) -> Segment {
        self.pass.segment()
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct EntityInstance {
    model: [[f32; 4]; 3],
    tint: [f32; 4],
    overlay: [u8; 4],
    light: [f32; 2],
    ids: [i32; 4],
    flags: u32,
}

const ENTITY_SHADE_IN_ALPHA: u32 = 1;

pub(super) const ENTITY_INSTANCE_SIZE: u64 = std::mem::size_of::<EntityInstance>() as u64;
const _: () = assert!(ENTITY_INSTANCE_SIZE == 96);

pub(super) fn entity_instance_layout() -> bevy::mesh::VertexBufferLayout {
    use bevy::render::render_resource::{VertexAttribute, VertexFormat, VertexStepMode};
    let attributes = [
        (VertexFormat::Float32x4, 0),
        (VertexFormat::Float32x4, 16),
        (VertexFormat::Float32x4, 32),
        (VertexFormat::Float32x4, 48),
        (VertexFormat::Unorm8x4, 64),
        (VertexFormat::Float32x2, 68),
        (VertexFormat::Sint32, 76),
        (VertexFormat::Sint32, 80),
        (VertexFormat::Sint32, 84),
        (VertexFormat::Sint32, 88),
        (VertexFormat::Uint32, 92),
    ];
    bevy::mesh::VertexBufferLayout {
        array_stride: ENTITY_INSTANCE_SIZE,
        step_mode: VertexStepMode::Instance,
        attributes: attributes
            .into_iter()
            .zip(4..)
            .map(|((format, offset), shader_location)| VertexAttribute {
                format,
                offset,
                shader_location,
            })
            .collect(),
    }
}

fn blended(mode: AlphaMode) -> bool {
    matches!(
        mode,
        AlphaMode::Blend | AlphaMode::Premultiplied | AlphaMode::Add
    )
}

pub(super) struct EntityBatch {
    pub(super) segment: Segment,
    pub(super) pass: EntityKind,
    pub(super) blend: bool,
    pub(super) texture: Option<AssetId<Image>>,
    pub(super) mesh: AssetId<Mesh>,
    pub(super) first: u32,
    pub(super) count: u32,
}

const ENTITY_TEXTURE_FRAMES: u32 = 600;

pub(super) fn entity_vertex_layout() -> bevy::mesh::VertexBufferLayout {
    use bevy::render::render_resource::{VertexAttribute, VertexStepMode};
    bevy::mesh::VertexBufferLayout {
        array_stride: ENTITY_STRIDE,
        step_mode: VertexStepMode::Vertex,
        attributes: ENTITY_ATTRIBUTES
            .into_iter()
            .enumerate()
            .map(|(location, (format, offset))| VertexAttribute {
                format,
                offset,
                shader_location: location as u32,
            })
            .collect(),
    }
}

pub(super) fn entity_layout_matches(layout: &bevy::mesh::VertexBufferLayout) -> bool {
    layout.array_stride == ENTITY_STRIDE
        && layout.attributes.len() == ENTITY_ATTRIBUTES.len()
        && layout
            .attributes
            .iter()
            .zip(ENTITY_ATTRIBUTES)
            .all(|(a, (format, offset))| a.format == format && a.offset == offset)
}

const ENTITY_STRIDE: u64 = 48;
const ENTITY_ATTRIBUTES: [(bevy::render::render_resource::VertexFormat, u64); 4] = [
    (bevy::render::render_resource::VertexFormat::Float32x3, 0),
    (bevy::render::render_resource::VertexFormat::Float32x3, 12),
    (bevy::render::render_resource::VertexFormat::Float32x2, 24),
    (bevy::render::render_resource::VertexFormat::Float32x4, 32),
];

pub(crate) fn prepare_entity_draws(
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    images: Res<RenderAssets<GpuImage>>,
    render: Option<ResMut<PackRender>>,
) {
    let Some(mut render) = render else { return };
    let Some(install) = render.install.as_deref_mut() else {
        return;
    };
    let Install {
        stage:
            Stage::Started {
                pipelines: Building::Ready(_),
                sized: Some(sized),
            },
        frame,
        entity_textures,
        ..
    } = install
    else {
        return;
    };
    frame.entity_instances.clear();
    frame.entity_batches.clear();
    if frame.entity_draws.is_empty()
        || sized
            .plans
            .iter()
            .all(|p| p.entities.iter().all(Option::is_none))
    {
        return;
    }
    frame
        .entity_draws
        .sort_unstable_by_key(|d| (d.segment(), d.order, d.pass, d.blend, d.texture, d.mesh));
    frame.entity_frame = frame.entity_frame.wrapping_add(1);
    let now = frame.entity_frame;
    let camera = frame.camera.as_dvec3();
    for draw in &frame.entity_draws {
        let mut model = draw.model;
        if !draw.relative {
            let offset = model.w_axis.truncate().as_dvec3() - camera;
            model.w_axis = offset.as_vec3().extend(1.0);
        }
        let rows = model.transpose();
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        frame.entity_instances.push(EntityInstance {
            model: [
                rows.x_axis.to_array(),
                rows.y_axis.to_array(),
                rows.z_axis.to_array(),
            ],
            tint: draw.tint.to_array(),
            overlay: draw.overlay.to_array().map(byte),
            light: draw.light.to_array(),
            ids: [draw.entity_id, -1, draw.item_id, draw.stage],
            flags: draw.flags,
        });
        let at = frame.entity_instances.len() as u32 - 1;
        match frame.entity_batches.last_mut() {
            Some(batch)
                if (batch.pass, batch.blend, batch.texture, batch.mesh)
                    == (draw.pass, draw.blend, draw.texture, draw.mesh) =>
            {
                batch.count += 1;
            }
            _ => {
                frame.entity_batches.push(EntityBatch {
                    segment: draw.segment(),
                    pass: draw.pass,
                    blend: draw.blend,
                    texture: draw.texture,
                    mesh: draw.mesh,
                    first: at,
                    count: 1,
                });
                frame.entity_texture_use.insert(draw.texture, now);
            }
        }
    }
    let bytes: &[u8] = bytemuck::cast_slice(&frame.entity_instances);
    if frame
        .entity_buffer
        .as_ref()
        .is_none_or(|b| b.size() < bytes.len() as u64)
    {
        frame.entity_buffer = Some(device.create_buffer(&BufferDescriptor {
            label: Some(&format!("{GPU_LABEL} entity instances")),
            size: (bytes.len() as u64).next_power_of_two(),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
    }
    if let Some(buffer) = &frame.entity_buffer {
        queue.write_buffer(buffer, 0, bytes);
    }

    if now.is_multiple_of(ENTITY_TEXTURE_FRAMES) {
        frame.entity_texture_use.retain(|texture, last| {
            let keep = now.wrapping_sub(*last) < ENTITY_TEXTURE_FRAMES;
            if !keep {
                if let Some(id) = texture {
                    entity_textures.remove(id);
                }
                for pass in sized
                    .plans
                    .iter_mut()
                    .flat_map(|p| p.entities.iter_mut().flatten())
                {
                    pass.groups.remove(texture);
                }
            }
            keep
        });
    }

    let mut encoder: Option<wgpu::CommandEncoder> = None;
    let mut remade: Vec<AssetId<Image>> = Vec::new();
    for batch in &frame.entity_batches {
        let Some(id) = batch.texture else { continue };
        let Some(image) = images.get(id) else {
            continue;
        };
        if image.texture_format != TextureFormat::Rgba8UnormSrgb
            || entity_textures
                .get(&id)
                .is_some_and(|(from, _)| *from == image.texture_view.id())
        {
            continue;
        }
        let size = extent(image.size.width, image.size.height);
        let copy = device.create_texture(&TextureDescriptor {
            label: Some(&format!("{GPU_LABEL} entity texture")),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        encoder
            .get_or_insert_with(|| {
                device
                    .wgpu_device()
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some(gpu_label!("entity textures")),
                    })
            })
            .copy_texture_to_texture(image.texture.as_image_copy(), copy.as_image_copy(), size);
        let view = copy.create_view(&TextureViewDescriptor::default());
        entity_textures.insert(id, (image.texture_view.id(), view));
        remade.push(id);
    }
    if let Some(encoder) = encoder {
        queue.submit([encoder.finish()]);
    }

    for plan in &mut sized.plans {
        for pass in plan.entities.iter_mut().flatten() {
            for id in &remade {
                pass.groups.remove(&Some(*id));
            }
        }
        for batch in &frame.entity_batches {
            let Some(pass) = plan.entities[batch.pass.index()].as_mut() else {
                continue;
            };
            if pass.groups.contains_key(&batch.texture) {
                continue;
            }
            let view = match batch.texture {
                Some(id) => match (entity_textures.get(&id), images.get(id)) {
                    (Some((_, copy)), _) => Some(copy.clone()),
                    (None, Some(image)) => Some(image.texture_view.clone()),
                    (None, None) => continue,
                },
                None => None,
            };
            let group = pass.template.create(&device, view.as_ref());
            pass.groups.insert(batch.texture, group);
        }
    }
}
