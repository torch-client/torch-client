mod record;

use bevy::ecs::query::QueryItem;
use bevy::pbr::DistanceFog;
use bevy::prelude::*;
use bevy::render::diagnostic::RecordDiagnostics;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderLabel, ViewNode};
use bevy::render::render_resource::{LoadOp, RenderPassColorAttachment, RenderPassDescriptor};
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::view::{ExtractedView, ViewTarget};

use super::compile::{PACK_SET, ScreenStage, Segment, TERRAIN_SET};
use super::entities::{ENTITY_INSTANCE_SIZE, entity_layout_matches};
use super::frame::{PackRender, Sized};
use super::plan::{Attachment, ComputePass, GeometryPass, ScreenPass, StepPass};
use super::targets::zero_layout;
use crate::renderer::terrain_pool::cull::{
    CullBuffers, DirectLists, TerrainViews, pack_shadow_view,
};
use crate::renderer::terrain_pool::draw::{StreamSources, TerrainBindGroup, draw_region};
use crate::renderer::terrain_pool::pools::TerrainPools;
use crate::renderer::terrain_pool::{
    STREAM_CUTOUT, STREAM_SOLID, STREAM_WATER, TerrainTier, TerrainView,
};
use record::{
    clear_op, color_attachment, copy_whole, depth_attachment, draw_pools, load_attachments,
};

#[derive(RenderLabel, Debug, Hash, PartialEq, Eq, Clone)]
pub(crate) struct PackFrameLabel;

#[derive(Default)]
pub(crate) struct PackFrameNode {
    frames: std::sync::atomic::AtomicUsize,
}

impl ViewNode for PackFrameNode {
    type ViewQuery = (
        &'static ExtractedView,
        &'static ViewTarget,
        Has<TerrainView>,
        Option<&'static DistanceFog>,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (view, view_target, is_terrain, fog): QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        if !is_terrain {
            return Ok(());
        }
        let Some(render) = world.get_resource::<PackRender>() else {
            return Ok(());
        };
        let Some(last) = render.drawable(view_target.main_texture_format()) else {
            return Ok(());
        };
        let (Some(install), Some(process)) = (render.install.as_deref(), render.process.as_ref())
        else {
            return Ok(());
        };
        let Some(Sized {
            targets,
            plans,
            setup_done,
            fully_cleared,
            ..
        }) = install.stage.sized()
        else {
            return Ok(());
        };
        let (installed, gpu, frame) = (&*install.installed, &install.gpu, &install.frame);
        let Some(terrain_group) = world.resource::<TerrainBindGroup>().group.as_ref() else {
            return Ok(());
        };
        let plan = &plans[self
            .frames
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            % plans.len()];
        let sources = StreamSources {
            tier: world.resource::<TerrainTier>(),
            pools: world.resource::<TerrainPools>(),
            cull: world.resource::<CullBuffers>(),
            views: world.resource::<TerrainViews>(),
            lists: world.resource::<DirectLists>(),
        };

        let fog = fog.map_or([0.0, 0.0, 0.0, 1.0], |f| {
            let c = f.color.to_srgba();
            [c.red, c.green, c.blue, 1.0]
        });
        let recorder = render_context.diagnostic_recorder();
        let span = recorder.time_span(render_context.command_encoder(), "shaderpack/clears");

        for (image, target) in installed.custom_images.iter().zip(&gpu.custom_images) {
            if !image.clear {
                continue;
            }
            let device = world.resource::<RenderDevice>();
            if device.features().contains(wgpu::Features::CLEAR_TEXTURE) {
                render_context
                    .command_encoder()
                    .clear_texture(&target.texture, &wgpu::ImageSubresourceRange::default());
            } else if let Some(zeros) = &gpu.zeros {
                let size = target.texture.size();
                let (bytes_per_row, _) = zero_layout(&target.texture);
                render_context.command_encoder().copy_buffer_to_texture(
                    wgpu::TexelCopyBufferInfo {
                        buffer: zeros,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(bytes_per_row),
                            rows_per_image: Some(size.height),
                        },
                    },
                    target.texture.as_image_copy(),
                    size,
                );
            }
        }

        let parity = (world.resource::<bevy::diagnostic::FrameCount>().0 & 1) as usize;
        let dispatch = |render_context: &mut RenderContext<'w>, pass: &ComputePass| {
            let (variant, groups) = (&pass.variants[parity], pass.groups);
            let pipeline = &variant.pipeline;
            for (index, snapshot) in gpu.image_snapshots.iter().enumerate() {
                if let Some(snapshot) = snapshot
                    && variant.snapshots & (1 << index) != 0
                {
                    copy_whole(
                        render_context.command_encoder(),
                        &gpu.custom_images[index].texture,
                        &snapshot.texture,
                    );
                }
            }
            let mut compute =
                render_context
                    .command_encoder()
                    .begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some(gpu_label!("compute")),
                        timestamp_writes: None,
                    });
            compute.set_pipeline(pipeline);
            compute.set_bind_group(PACK_SET, &*variant.bind_group, &[]);
            let [x, y, z] = groups;
            compute.dispatch_workgroups(x, y, z);
        };
        let screen = |render_context: &mut RenderContext<'w>, pass: &ScreenPass| {
            plan.record_mipmaps(render_context.command_encoder(), &pass.mipmaps);
            let attachments = load_attachments(&pass.attachments);
            let mut encoder = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some(gpu_label!("pass")),
                color_attachments: &attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            encoder.set_render_pipeline(&pass.pipeline);
            encoder.set_bind_group(PACK_SET as usize, &pass.bind_group, &[]);
            encoder.draw(0..3, 0..1);
        };
        let steps = |render_context: &mut RenderContext<'w>, steps: &[StepPass]| {
            for step in steps {
                match step {
                    StepPass::Compute(pass) => dispatch(render_context, pass),
                    StepPass::Screen(pass) => screen(render_context, pass),
                }
            }
        };

        if !setup_done.swap(true, std::sync::atomic::Ordering::Relaxed) {
            for pass in &plan.setup {
                dispatch(render_context, pass);
            }
        }

        let clear_to = |target: u8| {
            clear_op(installed.targets.settings[target as usize].clear_color(target as usize, fog))
        };
        let first = !fully_cleared.swap(true, std::sync::atomic::Ordering::Relaxed);
        for clear in plan
            .full_clears
            .iter()
            .filter(|_| first)
            .chain(&plan.clears)
        {
            let attachments: Vec<Option<RenderPassColorAttachment>> = clear
                .attachments
                .iter()
                .map(|Attachment { target, view }| color_attachment(view, clear_to(*target)))
                .collect();
            drop(
                render_context.begin_tracked_render_pass(RenderPassDescriptor {
                    label: Some(gpu_label!("clear")),
                    color_attachments: &attachments,
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                }),
            );
        }

        span.end(render_context.command_encoder());
        let span = recorder.time_span(render_context.command_encoder(), "shaderpack/begin");
        steps(render_context, &plan.stages[ScreenStage::Begin.index()]);
        span.end(render_context.command_encoder());

        let entity_segment = |render_context: &mut RenderContext<'w>, segment: Segment| {
            let Some(instances) = frame.entity_buffer.as_ref() else {
                return;
            };
            let start = frame
                .entity_batches
                .partition_point(|b| b.segment < segment);
            let end =
                start + frame.entity_batches[start..].partition_point(|b| b.segment == segment);
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
                let Some(entities) = &plan.entities[kind.index()] else {
                    continue;
                };
                let (opaque, blend) = (&entities.opaque, &entities.blend);
                let attachments = load_attachments(&entities.attachments);
                let depth_view = match segment {
                    Segment::Sky => None,
                    Segment::Shadow => gpu.shadow_targets.as_ref().map(|t| &t.depth.view),
                    _ => Some(&targets.depth.view),
                };
                let depth = depth_view.map(|view| depth_attachment(view, LoadOp::Load));
                let mut encoder = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                    label: Some(gpu_label!("entities")),
                    color_attachments: &attachments,
                    depth_stencil_attachment: depth,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
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
        };
        let span = recorder.time_span(render_context.command_encoder(), "shaderpack/shadow");
        if let (Some(shadow), Some(shadow_targets)) = (&plan.shadow, gpu.shadow_targets.as_ref()) {
            let filter = |render_context: &mut RenderContext<'w>, n: usize| {
                if let (Some(copy), Some(source), Some(target)) = (
                    &process.depth_copy,
                    &shadow.depth_source,
                    &shadow_targets.filtered[n],
                ) {
                    copy.record(render_context.command_encoder(), source, &target.view);
                }
            };
            let key = pack_shadow_view(view.retained_view_entity);
            let streams = [
                (STREAM_SOLID, &shadow.pipeline, true),
                (STREAM_CUTOUT, &shadow.cutout, false),
                (STREAM_WATER, &shadow.pipeline, false),
            ];
            for (stream, pipeline, first) in streams {
                let water = stream == STREAM_WATER;
                if water {
                    filter(render_context, 1);
                }
                if water && shadow.copy_depth {
                    copy_whole(
                        render_context.command_encoder(),
                        &shadow_targets.depth.texture,
                        &shadow_targets.depth_opaque.texture,
                    );
                }
                let attachments: Vec<Option<RenderPassColorAttachment>> = shadow
                    .attachments
                    .iter()
                    .map(|Attachment { target, view }| {
                        let settings = &installed.targets.shadow[usize::from(*target)];
                        let load = if first && settings.clear {
                            clear_op(settings.shadow_clear_color())
                        } else {
                            LoadOp::Load
                        };
                        color_attachment(view, load)
                    })
                    .collect();
                let depth_load = if first {
                    LoadOp::Clear(1.0)
                } else {
                    LoadOp::Load
                };
                let mut encoder = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                    label: Some(gpu_label!("shadow")),
                    color_attachments: &attachments,
                    depth_stencil_attachment: Some(depth_attachment(
                        &shadow_targets.depth.view,
                        depth_load,
                    )),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                encoder.set_render_pipeline(pipeline);
                encoder.set_bind_group(PACK_SET as usize, &shadow.bind_group, &[]);
                encoder.set_bind_group(TERRAIN_SET as usize, terrain_group, &[]);
                draw_pools(&mut encoder, &sources, |encoder, pool| {
                    let _ = draw_region(encoder, &sources, key, pool, stream);
                });
                drop(encoder);
                if stream == STREAM_CUTOUT {
                    entity_segment(render_context, Segment::Shadow);
                }
            }
            filter(render_context, 0);

            for pass in &plan.shadowcomp {
                dispatch(render_context, pass);
            }
        }

        span.end(render_context.command_encoder());
        let span = recorder.time_span(render_context.command_encoder(), "shaderpack/prepare");
        steps(render_context, &plan.stages[ScreenStage::Prepare.index()]);
        span.end(render_context.command_encoder());

        let span = recorder.time_span(render_context.command_encoder(), "shaderpack/sky");
        entity_segment(render_context, Segment::Sky);
        span.end(render_context.command_encoder());

        let clear_depth = std::cell::Cell::new(true);
        let geometry = |render_context: &mut RenderContext<'w>, pass: &GeometryPass| {
            let mut attachments = load_attachments(&pass.attachments);
            for (attachment, Attachment { target, .. }) in
                attachments.iter_mut().zip(&pass.attachments)
            {
                if let Some(attachment) = attachment
                    && pass.clears & (1 << target) != 0
                {
                    attachment.ops.load = clear_to(*target);
                }
            }
            let depth_load = if clear_depth.replace(false) {
                LoadOp::Clear(1.0)
            } else {
                LoadOp::Load
            };
            let mut encoder = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some(gpu_label!("geometry")),
                color_attachments: &attachments,
                depth_stencil_attachment: Some(depth_attachment(&targets.depth.view, depth_load)),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            encoder.set_render_pipeline(&pass.pipeline);
            encoder.set_bind_group(PACK_SET as usize, &pass.bind_group, &[]);
            encoder.set_bind_group(TERRAIN_SET as usize, terrain_group, &[]);
            draw_pools(&mut encoder, &sources, |encoder, pool| {
                let _ = draw_region(
                    encoder,
                    &sources,
                    view.retained_view_entity,
                    pool,
                    pass.stream,
                );
            });
        };
        let span = recorder.time_span(render_context.command_encoder(), "shaderpack/opaque");
        for pass in &plan.opaque {
            geometry(render_context, pass);
        }
        if clear_depth.replace(false) {
            drop(
                render_context.begin_tracked_render_pass(RenderPassDescriptor {
                    label: Some(gpu_label!("depth clear")),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(depth_attachment(
                        &targets.depth.view,
                        LoadOp::Clear(1.0),
                    )),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                }),
            );
        }
        span.end(render_context.command_encoder());
        let span = recorder.time_span(render_context.command_encoder(), "shaderpack/entities");
        entity_segment(render_context, Segment::World);
        if let Some(no_hand) = &targets.depth_no_hand {
            copy_whole(
                render_context.command_encoder(),
                &targets.depth.texture,
                &no_hand.texture,
            );
        }
        entity_segment(render_context, Segment::HandSolid);
        if plan.copy_depth {
            copy_whole(
                render_context.command_encoder(),
                &targets.depth.texture,
                &targets.depth_opaque.texture,
            );
        }

        span.end(render_context.command_encoder());
        let span = recorder.time_span(render_context.command_encoder(), "shaderpack/deferred");
        steps(render_context, &plan.stages[ScreenStage::Deferred.index()]);
        span.end(render_context.command_encoder());
        let span = recorder.time_span(render_context.command_encoder(), "shaderpack/translucent");
        for pass in &plan.translucent {
            geometry(render_context, pass);
        }
        entity_segment(render_context, Segment::WorldTranslucent);
        entity_segment(render_context, Segment::AfterTranslucent);
        entity_segment(render_context, Segment::HandTranslucent);
        span.end(render_context.command_encoder());
        let span = recorder.time_span(render_context.command_encoder(), "shaderpack/composite");
        steps(render_context, &plan.stages[ScreenStage::Composite.index()]);
        for pass in &plan.final_computes {
            dispatch(render_context, pass);
        }
        span.end(render_context.command_encoder());

        let (group, label) = match (&plan.final_pass, &plan.blit) {
            (Some(final_pass), _) => {
                plan.record_mipmaps(render_context.command_encoder(), &final_pass.mipmaps);
                (&final_pass.bind_group, gpu_label!("final"))
            }
            (None, Some(blit)) => (blit, gpu_label!("blit")),
            (None, None) => return Ok(()),
        };
        let attachment = [color_attachment(
            view_target.main_texture_view(),
            LoadOp::Clear(wgpu::Color::BLACK),
        )];
        let span = recorder.time_span(render_context.command_encoder(), "shaderpack/final");
        let mut encoder = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some(label),
            color_attachments: &attachment,
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        encoder.set_render_pipeline(last);
        encoder.set_bind_group(PACK_SET as usize, group, &[]);
        encoder.draw(0..3, 0..1);
        drop(encoder);
        span.end(render_context.command_encoder());

        Ok(())
    }
}
