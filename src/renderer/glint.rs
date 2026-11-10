use bevy::prelude::*;
use bevy::shader::ShaderDefVal;

use crate::items::glint::{ARMOR_SCALE, ITEM_SPAN, ROTATION, STRENGTH, periods_secs};

const ITEM_TEXELS: f32 = 16.0;

#[derive(Resource)]
pub(crate) struct GlintTexture(pub Handle<Image>);

#[derive(Resource)]
pub(crate) struct ArmorGlintTexture(pub Handle<Image>);

pub(crate) const ARMOR: Vec4 = Vec4::new(ARMOR_SCALE, ARMOR_SCALE, STRENGTH, 1.0);

pub(crate) fn armor_scale(glint: bool) -> Vec4 {
    if glint { ARMOR } else { Vec4::ZERO }
}

const ENTITY_SCALE: f32 = 0.5;

pub(crate) const ENTITY: Vec4 = Vec4::new(ENTITY_SCALE, ENTITY_SCALE, STRENGTH, 0.0);

pub(crate) fn is_armor(glint: Vec4) -> bool {
    glint.w > 0.0
}

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct GlintLoad;

pub(crate) fn build(app: &mut App) {
    bevy::shader::load_shader_library!(app, "glint.wgsl");
    app.add_systems(
        Update,
        load.in_set(GlintLoad)
            .run_if(resource_added::<crate::renderer::systems::AssetsReady>),
    );
}

fn load(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    if let Some(handle) = load_texture(
        &mut images,
        "textures/misc/enchanted_glint_item.png",
        "item glint",
    ) {
        commands.insert_resource(GlintTexture(handle));
    }
    if let Some(handle) = load_texture(
        &mut images,
        "textures/misc/enchanted_glint_armor.png",
        "armor glint",
    ) {
        commands.insert_resource(ArmorGlintTexture(handle));
    }
}

pub(crate) fn shader_defs() -> [ShaderDefVal; 3] {
    let bits = |name: &str, value: f32| ShaderDefVal::UInt(name.into(), value.to_bits());
    let [period_u, period_v] = periods_secs();
    [
        bits("GLINT_ROTATION_BITS", ROTATION),
        bits("GLINT_PERIOD_U_BITS", period_u),
        bits("GLINT_PERIOD_V_BITS", period_v),
    ]
}

fn load_texture(images: &mut Assets<Image>, path: &str, who: &str) -> Option<Handle<Image>> {
    use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};

    let handle = super::skin::load_texture(images, path, who)?;
    if let Some(mut image) = images.get_mut(&handle) {
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            ..ImageSamplerDescriptor::linear()
        });
    }
    Some(handle)
}

pub(crate) fn scale(glint: bool, texture: Option<&Handle<Image>>, images: &Assets<Image>) -> Vec4 {
    if !glint {
        return Vec4::ZERO;
    }
    let Some(image) = texture.and_then(|t| images.get(t)) else {
        return Vec4::ZERO;
    };
    scale_for_size(image.size_f32())
}

fn scale_for_size(size: Vec2) -> Vec4 {
    let scale = size * (ITEM_SPAN / ITEM_TEXELS);
    Vec4::new(scale.x, scale.y, STRENGTH, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_item_edge_spans_item_span_whatever_the_texture_size() {
        let small = scale_for_size(Vec2::splat(16.0));
        assert_eq!(small.truncate().truncate(), Vec2::splat(ITEM_SPAN));
        assert_eq!(small.z, STRENGTH);
        let large = scale_for_size(Vec2::splat(64.0));
        assert_eq!(large.x, 4.0 * ITEM_SPAN);
    }
}
