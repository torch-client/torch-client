use std::sync::atomic::Ordering;

use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{
    LoadOp, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, TextureUsages,
    TextureView,
};
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::texture::GpuImage;
use bevy::render::view::{ExtractedView, ViewDepthTexture, ViewTarget};

use super::super::node::record::color_attachment;
use super::bind::Bindables;
use super::render::{Install, NativeRender, Pipelines, Ready, Sized};
use super::walk::Walk;
use crate::renderer::terrain_pool::draw::PackTakeover;
use crate::renderer::terrain_pool::{TerrainTextures, TerrainView};
use crate::shaderpack::native::api::{GLOBALS_GROUP, ScreenOutput};
use crate::shaderpack::native::build::Built;

pub(super) const DRAWING: &str = "drawing";

pub(super) fn note(state: &'static str) {
    static LAST: std::sync::Mutex<&'static str> = std::sync::Mutex::new("");
    let Ok(mut last) = LAST.lock() else { return };
    if *last == state {
        return;
    }
    *last = state;
    match state {
        "" => {}
        DRAWING => crate::log_info!("shaders", "native pack: drawing"),
        waiting => crate::log_info!("shaders", "native pack: not drawing, {waiting}"),
    }
}

pub(super) fn atlas_and_lightmap(world: &World) -> Option<(&TextureView, &TextureView)> {
    let textures = world.get_resource::<TerrainTextures>()?;
    let images = world.get_resource::<RenderAssets<GpuImage>>()?;
    Some((
        &images.get(&textures.atlas)?.texture_view,
        &images.get(&textures.lightmap)?.texture_view,
    ))
}

pub(crate) fn native_frame(
    world: &World,
    view: ViewQuery<(
        &ExtractedView,
        &ViewTarget,
        Option<&ViewDepthTexture>,
        Has<TerrainView>,
    )>,
    mut ctx: RenderContext,
) {
    let (extracted, view_target, depth, is_terrain) = view.into_inner();
    if !is_terrain {
        return;
    }
    let Some(render) = world.get_resource::<NativeRender>() else {
        return;
    };
    let format = view_target.main_texture_format();
    let Some(ready) = render.ready(format) else {
        note(render.blocked(format));
        return;
    };
    let main = view_target.main_texture().size();
    if ready.sized.size != UVec2::new(main.width, main.height) {
        note("the targets are sized for another view");
        return;
    }
    let drawable = ready
        .install
        .installed
        .built
        .passes
        .iter()
        .zip(&ready.pipelines.passes)
        .all(|(pass, pipeline)| pass.output == ScreenOutput::Screen || pipeline.is_some());
    if !drawable {
        note("a pass that draws into a target has no pipeline");
        return;
    }
    if ready.install.draws_world() {
        if !world.get_resource::<PackTakeover>().is_some_and(|t| t.0) {
            note("waiting for the view takeover");
            return;
        }
        super::world::world_frame(world, extracted, view_target, ready, &mut ctx);
        return;
    }
    post_only_frame(world, view_target, depth, ready, &mut ctx);
}

fn post_only_frame(
    world: &World,
    view_target: &ViewTarget,
    depth: Option<&ViewDepthTexture>,
    Ready {
        install,
        pipelines,
        last,
        sized,
    }: Ready<'_>,
    render_context: &mut RenderContext,
) {
    let depth = match depth {
        _ if !install.installed.uses_depth => None,
        Some(depth)
            if depth
                .texture
                .usage()
                .contains(TextureUsages::TEXTURE_BINDING) =>
        {
            Some(depth.view())
        }
        _ => {
            note("the view's depth is not sampleable yet");
            return;
        }
    };
    note(DRAWING);

    let post = view_target.post_process_write();
    let textures = atlas_and_lightmap(world);
    let mut walk = Walk::new(sized.front.load(Ordering::Relaxed));
    let bindables = |walk: Walk| Bindables {
        install,
        targets: &sized.targets,
        walk,
        scene_color: Some(post.source),
        scene_depth: depth,
        atlas: textures.map(|t| t.0),
        lightmap: textures.map(|t| t.1),
    };
    clear_targets(render_context, install, sized, &mut walk);
    for index in 0..install.installed.built.passes.len() {
        screen_pass(
            world,
            render_context,
            install,
            pipelines,
            sized,
            &bindables(walk),
            &mut walk,
            index,
            last,
            post.destination,
        );
    }
    sized.front.store(walk.front, Ordering::Relaxed);
}

pub(super) fn clear_targets(
    render_context: &mut RenderContext,
    install: &Install,
    sized: &Sized,
    walk: &mut Walk,
) {
    let built = &install.installed.built;
    let first = !sized.cleared.swap(true, Ordering::Relaxed);
    for (t, which) in walk.clears(&built.targets) {
        let Some([r, g, b, a]) = built.targets[t].clear else {
            continue;
        };
        if !first && !built.targets[t].double() && !drawn_into(built, t) {
            continue;
        }
        let attachment = [color_attachment(
            &sized.targets[t][which].view,
            LoadOp::Clear(wgpu::Color { r, g, b, a }),
        )];
        drop(
            render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some(gpu_label!("native clear")),
                color_attachments: &attachment,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            }),
        );
    }
}

fn drawn_into(built: &Built, t: usize) -> bool {
    built
        .passes
        .iter()
        .any(|pass| matches!(&pass.output, ScreenOutput::Targets(writes) if writes.contains(&t)))
        || built.geometry.iter().any(|group| group.writes.contains(&t))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn screen_pass(
    world: &World,
    render_context: &mut RenderContext,
    install: &Install,
    pipelines: &Pipelines,
    sized: &Sized,
    bindables: &Bindables<'_>,
    walk: &mut Walk,
    index: usize,
    last: &RenderPipeline,
    view: &TextureView,
) {
    let built = &install.installed.built;
    let pass = &built.passes[index];
    let Some(template) = bindables.template(
        gpu_label!("native pass"),
        &install.layouts[index],
        &pass.bindings,
    ) else {
        return;
    };
    let group = template.create(world.resource::<RenderDevice>(), None);
    let writes = walk.pass_writes(&built.targets, pass);
    let (pipeline, attachments): (_, Vec<Option<RenderPassColorAttachment>>) = match &pass.output {
        ScreenOutput::Screen => (
            last,
            vec![color_attachment(view, LoadOp::Clear(wgpu::Color::BLACK))],
        ),
        ScreenOutput::Targets(_) => {
            let Some(pipeline) = pipelines.passes[index].as_ref() else {
                return;
            };
            let attachments = writes
                .iter()
                .map(|&(t, which)| color_attachment(&sized.targets[t][which].view, LoadOp::Load))
                .collect();
            (pipeline, attachments)
        }
    };
    let mut encoder = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some(gpu_label!("native pass")),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    encoder.set_render_pipeline(pipeline);
    encoder.set_bind_group(GLOBALS_GROUP as usize, &group, &[]);
    encoder.draw(0..3, 0..1);
    drop(encoder);
    walk.commit(&writes);
}
