use bevy::prelude::*;
use bevy::render::render_resource::{FilterMode, Sampler, SamplerDescriptor};
use bevy::render::renderer::RenderDevice;
use bevy::render::{Render, RenderApp, RenderSystems};

use super::draw::GuiImageViews;
use super::painter::ATLAS_TEXTURE;
use super::pool::GuiPool;
use crate::renderer::mod_capture::{self, ModImages};

#[derive(Resource, Default)]
struct ModImageSampler(Option<Sampler>);

fn bind_mod_images(
    pool: Res<GuiPool>,
    device: Res<RenderDevice>,
    images: Res<ModImages>,
    mut sampler: ResMut<ModImageSampler>,
    mut views: ResMut<GuiImageViews>,
) {
    views.0.retain(|&key, _| images.has(key));
    let sampler = sampler
        .0
        .get_or_insert_with(|| {
            device.create_sampler(&SamplerDescriptor {
                label: Some("mod_image"),
                mag_filter: FilterMode::Nearest,
                min_filter: FilterMode::Nearest,
                ..Default::default()
            })
        })
        .clone();
    for range in pool.ranges.iter().filter(|r| r.texture != ATLAS_TEXTURE) {
        let Some(view) = images.view(range.texture) else {
            continue;
        };
        if views
            .0
            .get(&range.texture)
            .is_some_and(|(v, _)| v.id() == view.id())
        {
            continue;
        }
        views
            .0
            .insert(range.texture, (view.clone(), sampler.clone()));
    }
}

pub fn build(app: &mut App) {
    mod_capture::build(app);
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app.init_resource::<ModImageSampler>().add_systems(
        Render,
        bind_mod_images
            .in_set(RenderSystems::PrepareBindGroups)
            .after(mod_capture::prepare_mod_images)
            .before(super::draw::prepare_gui_image_groups),
    );
}
