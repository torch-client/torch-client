use std::sync::atomic::Ordering;

use bevy::prelude::*;
use bevy::render::render_resource::{LoadOp, RenderPassDescriptor};
use bevy::render::renderer::RenderContext;
use bevy::render::view::{ExtractedView, ViewTarget};

use super::super::compile::Segment;
use super::super::node::record::{
    SegmentTargets, copy_depth, depth_attachment, draw_pools, entity_segment, load_attachments,
};
use super::bind::Bindables;
use super::node::{DRAWING, atlas_and_lightmap, clear_targets, note, screen_pass};
use super::render::Ready;
use super::shadow::{ShadowFrame, record_shadow};
use super::walk::{Step, Walk, steps};
use crate::renderer::terrain_pool::TerrainTier;
use crate::renderer::terrain_pool::cull::{
    CullBuffers, DirectLists, TerrainViews, pack_shadow_view,
};
use crate::renderer::terrain_pool::draw::{StreamSources, TerrainBindGroup, draw_region};
use crate::renderer::terrain_pool::pools::TerrainPools;
use crate::shaderpack::native::api::{GLOBALS_GROUP, TERRAIN_GROUP};

const FAR: f32 = 0.0;

pub(super) fn world_frame(
    world: &World,
    view: &ExtractedView,
    view_target: &ViewTarget,
    ready: Ready<'_>,
    render_context: &mut RenderContext,
) {
    let Ready {
        install,
        pipelines,
        last,
        sized,
    } = ready;
    let (Some(geometry), Some(world_sized)) = (install.geometry.as_ref(), sized.world.as_ref())
    else {
        return;
    };
    let Some(plans) = world_sized.plans.as_ref() else {
        return;
    };
    let start = sized.front.load(Ordering::Relaxed);
    let Some(which) = plans.find(start) else {
        note("the targets are in a state no plan starts from");
        return;
    };
    let (plan, entity_passes) = (&plans.plans[which], &plans.entity_passes[which]);
    let Some(terrain_group) = world.resource::<TerrainBindGroup>().group.as_ref() else {
        note("the terrain's bind group has not been made");
        return;
    };
    let Some((atlas, lightmap)) = atlas_and_lightmap(world) else {
        note("the atlas or the lightmap is not uploaded");
        return;
    };
    note(DRAWING);
    let built = &install.installed.built;
    let sources = StreamSources {
        tier: world.resource::<TerrainTier>(),
        pools: world.resource::<TerrainPools>(),
        cull: world.resource::<CullBuffers>(),
        views: world.resource::<TerrainViews>(),
        lists: world.resource::<DirectLists>(),
    };
    let depth = &world_sized.depth;
    let mut walk = Walk::new(start);

    clear_targets(render_context, install, sized, &mut walk);

    let segment = |render_context: &mut RenderContext, segment: Segment| {
        entity_segment(
            render_context,
            world,
            &geometry.entity_frame,
            entity_passes,
            segment,
            &depth.view,
            None,
        );
    };
    let clear_depth = |render_context: &mut RenderContext| {
        drop(
            render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some(gpu_label!("native depth clear")),
                color_attachments: &[],
                depth_stencil_attachment: Some(depth_attachment(&depth.view, LoadOp::Clear(FAR))),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            }),
        );
    };
    let draw_group = |render_context: &mut RenderContext, index: usize, first: bool| {
        if geometry.reads_depth[index]
            && let Some(copy) = &world_sized.depth_copy
        {
            copy_depth(render_context, depth, copy);
        }
        let group = &plan.groups[index];
        let attachments = load_attachments(&group.attachments);
        let terrain = &pipelines.groups[index].terrain;
        if first && terrain.is_empty() {
            clear_depth(render_context);
        }
        for (at, (stream, pipeline)) in terrain.iter().enumerate() {
            let load = if first && at == 0 {
                LoadOp::Clear(FAR)
            } else {
                LoadOp::Load
            };
            let mut encoder = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some(gpu_label!("native geometry")),
                color_attachments: &attachments,
                depth_stencil_attachment: Some(depth_attachment(&depth.view, load)),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            encoder.set_render_pipeline(pipeline);
            encoder.set_bind_group(GLOBALS_GROUP as usize, &group.bind_group, &[]);
            encoder.set_bind_group(TERRAIN_GROUP as usize, terrain_group, &[]);
            draw_pools(&mut encoder, &sources, |encoder, pool| {
                let _ = draw_region(encoder, &sources, view.retained_view_entity, pool, *stream);
            });
        }
    };

    for step in steps(built) {
        match step {
            Step::Pass(index) => {
                let bindables = Bindables {
                    install,
                    targets: &sized.targets,
                    walk,
                    scene_color: None,
                    scene_depth: Some(&depth.view),
                    atlas: Some(atlas),
                    lightmap: Some(lightmap),
                };
                let output = view_target.main_texture_view();
                screen_pass(
                    world,
                    render_context,
                    install,
                    pipelines,
                    sized,
                    &bindables,
                    &mut walk,
                    index,
                    last,
                    output,
                );
            }
            Step::Shadow => {
                if let (Some(pass), Some(shadow_pipelines), Some(globals)) =
                    (&install.shadow.pass, &pipelines.shadow, &plan.shadow)
                {
                    let frame = ShadowFrame {
                        world,
                        globals,
                        terrain_group,
                        sources: &sources,
                        key: pack_shadow_view(view.retained_view_entity),
                        entity_frame: &geometry.entity_frame,
                        entity_passes,
                    };
                    record_shadow(render_context, pass, shadow_pipelines, &frame);
                }
            }
            Step::Opaque(group) => {
                segment(render_context, Segment::Sky);
                match group {
                    Some(index) => {
                        draw_group(render_context, index, true);
                        let writes = walk.group_writes(&built.targets, &built.geometry[index]);
                        walk.commit(&writes);
                    }
                    None => clear_depth(render_context),
                }
                segment(render_context, Segment::World);
                segment(render_context, Segment::HandSolid);
            }
            Step::Translucent(group) => {
                if let Some(index) = group {
                    draw_group(render_context, index, false);
                    let writes = walk.group_writes(&built.targets, &built.geometry[index]);
                    walk.commit(&writes);
                }
                segment(render_context, Segment::WorldTranslucent);
                segment(render_context, Segment::AfterTranslucent);
                segment(render_context, Segment::HandTranslucent);
            }
        }
    }
    let display = SegmentTargets {
        depth: &depth.view,
        color: Some(std::slice::from_ref(view_target.main_texture_view())),
        draw_group: None,
    };
    entity_segment(
        render_context,
        world,
        &geometry.entity_frame,
        entity_passes,
        Segment::Display,
        &depth.view,
        Some(&display),
    );
    sized.front.store(walk.front, Ordering::Relaxed);
}
