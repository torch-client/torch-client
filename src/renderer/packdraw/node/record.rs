use bevy::ecs::world::World;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::TrackedRenderPass;
use bevy::render::render_resource::{
    BindGroup, LoadOp, Operations, RenderPassColorAttachment, RenderPassDepthStencilAttachment,
    RenderPassDescriptor, StoreOp, Texture, TextureView,
};
use bevy::render::renderer::RenderContext;

use super::super::compile::{PACK_QUADS_SET, PACK_SET, Segment};
use super::super::entities::{
    ENTITY_INSTANCE_SIZE, EntityFrame, EntityPasses, entity_layout_matches,
};
use super::super::plan::Attachment;
use crate::renderer::terrain_pool::draw::StreamSources;

pub(super) fn clear_op([r, g, b, a]: [f32; 4]) -> LoadOp<wgpu::Color> {
    LoadOp::Clear(wgpu::Color {
        r: f64::from(r),
        g: f64::from(g),
        b: f64::from(b),
        a: f64::from(a),
    })
}

pub(in crate::renderer::packdraw) fn color_attachment(
    view: &TextureView,
    load: LoadOp<wgpu::Color>,
) -> Option<RenderPassColorAttachment<'_>> {
    Some(RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: Operations {
            load,
            store: StoreOp::Store,
        },
    })
}

pub(in crate::renderer::packdraw) fn load_attachments(
    attachments: &[Attachment],
) -> Vec<Option<RenderPassColorAttachment<'_>>> {
    attachments
        .iter()
        .map(|a| color_attachment(&a.view, LoadOp::Load))
        .collect()
}

pub(in crate::renderer::packdraw) fn depth_attachment(
    view: &TextureView,
    load: LoadOp<f32>,
) -> RenderPassDepthStencilAttachment<'_> {
    RenderPassDepthStencilAttachment {
        view,
        depth_ops: Some(Operations {
            load,
            store: StoreOp::Store,
        }),
        stencil_ops: None,
    }
}

pub(super) fn copy_whole(encoder: &mut wgpu::CommandEncoder, from: &Texture, to: &Texture) {
    encoder.copy_texture_to_texture(from.as_image_copy(), to.as_image_copy(), from.size());
}

pub(in crate::renderer::packdraw) fn copy_depth(
    render_context: &mut bevy::render::renderer::RenderContext,
    from: &super::super::targets::Target,
    to: &super::super::targets::Target,
) {
    #[cfg(not(all(target_arch = "wasm32", not(feature = "webgpu"))))]
    copy_whole(render_context.command_encoder(), &from.texture, &to.texture);
    #[cfg(all(target_arch = "wasm32", not(feature = "webgpu")))]
    {
        let device = render_context.render_device().clone();
        super::super::rawviews::copy_depth(
            &device,
            render_context.command_encoder(),
            &from.view,
            &to.view,
        );
    }
}

pub(in crate::renderer::packdraw) struct SegmentTargets<'a> {
    pub(in crate::renderer::packdraw) depth: &'a TextureView,
    pub(in crate::renderer::packdraw) color: Option<&'a [TextureView]>,
    pub(in crate::renderer::packdraw) draw_group: Option<(u32, &'a BindGroup)>,
}

pub(in crate::renderer::packdraw) fn entity_segment(
    render_context: &mut RenderContext,
    world: &World,
    frame: &EntityFrame,
    passes: &EntityPasses,
    segment: Segment,
    main_depth: &TextureView,
    own: Option<&SegmentTargets<'_>>,
) {
    let Some(instances) = frame.entity_buffer.as_ref() else {
        return;
    };
    let start = frame
        .entity_batches
        .partition_point(|b| b.segment < segment);
    let end = start + frame.entity_batches[start..].partition_point(|b| b.segment == segment);
    let meshes = world.resource::<RenderAssets<bevy::render::mesh::RenderMesh>>();
    let allocator = world.resource::<bevy::render::mesh::allocator::MeshAllocator>();
    let mut at = start;
    while at < end {
        let kind = frame.entity_batches[at].pass;
        let run = at
            + frame.entity_batches[at..end]
                .iter()
                .take_while(|b| b.pass == kind)
                .count();
        let batches = &frame.entity_batches[at..run];
        at = run;
        let Some(entities) = &passes[kind.index()] else {
            continue;
        };
        let (opaque, blend) = (&entities.opaque, &entities.blend);
        let own = own.filter(|_| matches!(segment, Segment::Shadow | Segment::Display));
        let attachments = match own.and_then(|s| s.color) {
            Some(views) => views
                .iter()
                .map(|view| color_attachment(view, LoadOp::Load))
                .collect(),
            None => load_attachments(&entities.attachments),
        };
        let depth_view = match segment {
            Segment::Sky => None,
            Segment::Shadow | Segment::Display => own.map(|s| s.depth),
            _ => Some(main_depth),
        };
        let depth = depth_view.map(|view| depth_attachment(view, LoadOp::Load));
        let mut encoder = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some(gpu_label!("entities")),
            color_attachments: &attachments,
            depth_stencil_attachment: depth,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if let Some((index, group)) = own.and_then(|s| s.draw_group) {
            encoder.set_bind_group(index as usize, group, &[]);
        }
        for batch in batches {
            let (Some(mesh), Some(vertices), Some(textured)) = (
                meshes.get(batch.mesh),
                allocator.mesh_vertex_slice(&batch.mesh),
                entities.groups.get(&batch.texture),
            ) else {
                continue;
            };
            if !entity_layout_matches(mesh.layout.0.layout()) {
                continue;
            }
            encoder.set_render_pipeline(if batch.blend { blend } else { opaque });
            encoder.set_bind_group(PACK_SET as usize, textured, &[]);
            encoder.set_vertex_buffer(0, vertices.buffer.slice(..));
            let from = u64::from(batch.first) * ENTITY_INSTANCE_SIZE;
            encoder.set_vertex_buffer(
                1,
                instances.slice(from..from + u64::from(batch.count) * ENTITY_INSTANCE_SIZE),
            );
            let base = vertices.range.start;
            match &mesh.buffer_info {
                bevy::render::mesh::RenderMeshBufferInfo::Indexed {
                    count,
                    index_format,
                } => {
                    let Some(indices) = allocator.mesh_index_slice(&batch.mesh) else {
                        continue;
                    };
                    encoder.set_index_buffer(indices.buffer.slice(..), *index_format);
                    let first = indices.range.start;
                    encoder.draw_indexed(first..first + count, base as i32, 0..batch.count);
                }
                bevy::render::mesh::RenderMeshBufferInfo::NonIndexed => {
                    encoder.draw(base..base + mesh.vertex_count, 0..batch.count);
                }
            }
        }
    }
}

pub(in crate::renderer::packdraw) fn draw_pools<'w>(
    encoder: &mut TrackedRenderPass<'w>,
    sources: &StreamSources<'w>,
    mut draw: impl FnMut(&mut TrackedRenderPass<'w>, u32),
) {
    for (pool, quads) in sources
        .pools
        .pools
        .iter()
        .enumerate()
        .filter_map(|(at, p)| Some((at, p.pack_quads.as_ref()?)))
    {
        encoder.set_bind_group(PACK_QUADS_SET as usize, &quads.group, &[]);
        draw(encoder, pool as u32);
    }
}
