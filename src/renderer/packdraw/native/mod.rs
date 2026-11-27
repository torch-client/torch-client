mod bind;
mod cascade;
mod geometry;
mod install;
mod node;
mod overlay;
mod plans;
mod render;
mod shadow;
mod takeover;
mod uniform;
mod walk;
mod world;

use bevy::core_pipeline::{Core3d, Core3dSystems};
use bevy::prelude::*;
use bevy::render::renderer::RenderAdapter;
use bevy::render::{ExtractSchedule, Render, RenderApp, RenderSystems};

pub(crate) use install::NativePack;
pub(super) use render::NativeRender;

pub(super) fn build(app: &mut App) {
    app.add_systems(
        Update,
        (
            install::sync_native_pack.before(super::install::sync_pack_vertices),
            install::sync_depth_usage,
        )
            .chain(),
    );
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .init_resource::<render::NativeRender>()
        .add_systems(
            ExtractSchedule,
            render::extract_native
                .after(super::frame::extract_pack)
                .before(super::entities::extract_entity_draws),
        )
        .add_systems(
            Render,
            (
                render::drop_failed_native
                    .in_set(RenderSystems::PrepareAssets)
                    .before(super::drop_failed_pack),
                render::prepare_native
                    .in_set(RenderSystems::PrepareResources)
                    .after(super::frame::prepare_uniforms)
                    .before(crate::renderer::terrain_pool::cull::prepare_cull),
            ),
        )
        .add_systems(
            Core3d,
            node::native_frame
                .after(Core3dSystems::MainPass)
                .before(Core3dSystems::EarlyPostProcess),
        );
}

pub(super) fn finish(app: &mut App) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    let formats = render::renderable_formats(render_app.world().resource::<RenderAdapter>());
    app.insert_resource(install::Renderable(formats.into()));
}
