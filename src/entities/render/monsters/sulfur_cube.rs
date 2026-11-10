use std::f32::consts::PI;

use azalea_registry::builtin::EntityKind;
use bevy::prelude::{Mat4, Transform, Vec3};

use crate::entities::models::monsters::{slime, sulfur_cube};
use crate::entities::registry::Registry;
use crate::entities::render::objects::blocks::{is_lit, swell_amount};
use crate::entities::state::EntityState;
use crate::entities::{
    Blend, BlockRef, RenderSpec, RootPose, TexturePath, default_root, hook_scale_then_translate,
    layer_base,
};

const INNER_SORT_BIAS: f32 = -0.25;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::SulfurCube,
        RenderSpec::new(
            "sulfur_cube",
            sulfur_cube::outer_layer,
            outer_texture,
            slime::setup_anim,
        )
        .with_root(cube_root)
        .with_blend(Blend::Translucent)
        .with_visible(|st| !st.extras.is_baby),
    );
    registry.add(
        EntityKind::SulfurCube,
        RenderSpec::new(
            "sulfur_cube_small",
            sulfur_cube::small_outer_layer,
            small_outer_texture,
            slime::setup_anim,
        )
        .with_root(cube_root)
        .with_blend(Blend::Translucent)
        .with_visible(|st| st.extras.is_baby),
    );
    registry.add(
        EntityKind::SulfurCube,
        RenderSpec::new(
            "sulfur_cube_inner",
            sulfur_cube::inner_layer,
            inner_texture,
            slime::setup_anim,
        )
        .with_root(cube_root)
        .with_blend(Blend::Translucent)
        .with_visible(|st| !st.extras.is_baby && shows_inner(st))
        .with_sort_bias(INNER_SORT_BIAS)
        .with_fuse_flash(fuse_lit),
    );
    registry.add(
        EntityKind::SulfurCube,
        RenderSpec::new(
            "sulfur_cube_small_inner",
            sulfur_cube::small_inner_layer,
            small_inner_texture,
            slime::setup_anim,
        )
        .with_root(cube_root)
        .with_blend(Blend::Translucent)
        .with_visible(|st| st.extras.is_baby && shows_inner(st))
        .with_sort_bias(INNER_SORT_BIAS)
        .with_fuse_flash(fuse_lit),
    );
    registry.add(
        EntityKind::SulfurCube,
        RenderSpec::block("sulfur_cube_block", contained_block)
            .with_root(block_root)
            .with_fuse_flash(fuse_lit)
            .ignoring_invisibility(),
    );
}

fn outer_texture(_st: &EntityState) -> TexturePath {
    "entity/sulfur_cube/sulfur_cube_outer".into()
}

fn small_outer_texture(_st: &EntityState) -> TexturePath {
    "entity/sulfur_cube/sulfur_cube_outer_small".into()
}

fn inner_texture(_st: &EntityState) -> TexturePath {
    "entity/sulfur_cube/sulfur_cube_inner".into()
}

fn small_inner_texture(_st: &EntityState) -> TexturePath {
    "entity/sulfur_cube/sulfur_cube_inner_small".into()
}

fn shows_inner(st: &EntityState) -> bool {
    st.extras.block_state.is_none() && !st.is_invisible
}

fn contained_block(st: &EntityState) -> Option<BlockRef> {
    st.extras.block_state.map(BlockRef::State)
}

fn fuse_lit(st: &EntityState) -> bool {
    let fuse = st.extras.fuse_remaining;
    fuse > 0.0 && is_lit(fuse)
}

fn cube_hook(st: &EntityState) -> Transform {
    let size = st.extras.size as f32;
    let ss = if st.extras.block_state.is_some() {
        0.0
    } else {
        st.extras.squish / (size * 0.5 + 1.0)
    };
    let w = 1.0 / (ss + 1.0);
    let mut scale = Vec3::new(w * size, 1.0 / w * size, w * size);
    let fuse = st.extras.fuse_remaining;
    if fuse < 10.0 && fuse > 0.0 {
        scale *= 1.0 + swell_amount(fuse);
    }
    let baby = st.extras.is_baby;
    let v_offset = if baby { 1.24 } else { 0.98 };
    let extra_downscale = if baby { 1.0 } else { 0.5 };
    let one_pixel_up_if_visible = (if st.is_invisible { 0.0 } else { 1.0 }) / 16.0;
    scale *= extra_downscale;

    let mut hook = hook_scale_then_translate(Vec3::splat(0.999), Vec3::new(0.0, 0.001, 0.0));
    hook.scale *= scale;
    hook.translation += hook.scale * Vec3::new(0.0, v_offset - one_pixel_up_if_visible, 0.0);
    hook
}

fn cube_root(st: &EntityState) -> RootPose {
    let mut pose = default_root(st);
    pose.hook = cube_hook(st);
    pose
}

fn block_root(st: &EntityState) -> RootPose {
    let mut pose = cube_root(st);
    let baby_scale = if st.extras.is_baby { 0.5 } else { 1.0 };
    let local = Mat4::from_rotation_x(PI)
        * Mat4::from_scale(Vec3::splat(baby_scale))
        * Mat4::from_translation(Vec3::new(-0.5, -0.518, -0.5));
    pose.hook = Transform::from_matrix(layer_base(&pose) * local);
    pose
}
