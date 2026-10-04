use azalea_registry::builtin::EntityKind;
use bevy::prelude::*;

use crate::entities::TexturePath;
use crate::entities::models::aquatic::fish;
use crate::entities::registry::Registry;
use crate::entities::render::aquatic::root::fish_root;
use crate::entities::state::EntityState;
use crate::entities::{RenderSpec, RootPose, setup_rotations};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Cod,
        RenderSpec::new("cod", fish::cod_layer, cod_texture, fish::cod_setup).with_root(cod_root),
    );

    registry.add(
        EntityKind::Salmon,
        RenderSpec::new(
            "salmon_small",
            fish::salmon_small_layer,
            salmon_texture,
            fish::salmon_setup,
        )
        .with_root(salmon_root)
        .with_visible(is_small_salmon),
    );
    registry.add(
        EntityKind::Salmon,
        RenderSpec::new(
            "salmon",
            fish::salmon_layer,
            salmon_texture,
            fish::salmon_setup,
        )
        .with_root(salmon_root)
        .with_visible(is_medium_salmon),
    );
    registry.add(
        EntityKind::Salmon,
        RenderSpec::new(
            "salmon_large",
            fish::salmon_large_layer,
            salmon_texture,
            fish::salmon_setup,
        )
        .with_root(salmon_root)
        .with_visible(is_large_salmon),
    );

    registry.add(
        EntityKind::TropicalFish,
        RenderSpec::new(
            "tropical_fish_small",
            fish::tropical_small_layer,
            tropical_body_texture,
            fish::tropical_setup,
        )
        .with_root(tropical_root)
        .with_visible(is_small_tropical)
        .with_tint(tropical_base_tint),
    );
    registry.add(
        EntityKind::TropicalFish,
        RenderSpec::new(
            "tropical_fish_small_pattern",
            fish::tropical_small_pattern_layer,
            tropical_pattern_texture,
            fish::tropical_setup,
        )
        .with_root(tropical_root)
        .with_visible(is_small_tropical)
        .with_tint(tropical_pattern_tint),
    );
    registry.add(
        EntityKind::TropicalFish,
        RenderSpec::new(
            "tropical_fish_large",
            fish::tropical_large_layer,
            tropical_body_texture,
            fish::tropical_setup,
        )
        .with_root(tropical_root)
        .with_visible(is_large_tropical)
        .with_tint(tropical_base_tint),
    );
    registry.add(
        EntityKind::TropicalFish,
        RenderSpec::new(
            "tropical_fish_large_pattern",
            fish::tropical_large_pattern_layer,
            tropical_pattern_texture,
            fish::tropical_setup,
        )
        .with_root(tropical_root)
        .with_visible(is_large_tropical)
        .with_tint(tropical_pattern_tint),
    );

    registry.add(
        EntityKind::Pufferfish,
        RenderSpec::new(
            "pufferfish_small",
            fish::pufferfish_small_layer,
            pufferfish_texture,
            fish::pufferfish_small_setup,
        )
        .with_root(pufferfish_root)
        .with_visible(is_deflated),
    );
    registry.add(
        EntityKind::Pufferfish,
        RenderSpec::new(
            "pufferfish_mid",
            fish::pufferfish_mid_layer,
            pufferfish_texture,
            fish::pufferfish_blue_fin_setup,
        )
        .with_root(pufferfish_root)
        .with_visible(is_half_puffed),
    );
    registry.add(
        EntityKind::Pufferfish,
        RenderSpec::new(
            "pufferfish_big",
            fish::pufferfish_big_layer,
            pufferfish_texture,
            fish::pufferfish_blue_fin_setup,
        )
        .with_root(pufferfish_root)
        .with_visible(is_fully_puffed),
    );
}

fn cod_texture(_st: &EntityState) -> TexturePath {
    "entity/fish/cod".into()
}

fn salmon_texture(_st: &EntityState) -> TexturePath {
    "entity/fish/salmon".into()
}

fn pufferfish_texture(_st: &EntityState) -> TexturePath {
    "entity/fish/pufferfish".into()
}

fn cod_root(st: &EntityState) -> RootPose {
    fish_root(st, 1.0, 1.0, Vec3::new(0.1, 0.1, -0.1))
}

fn salmon_root(st: &EntityState) -> RootPose {
    let (amplitude, angle) = if st.is_in_water {
        (1.0, 1.0)
    } else {
        (1.3, 1.7)
    };
    fish_root(st, amplitude, angle, Vec3::new(0.2, 0.1, 0.0))
}

fn tropical_root(st: &EntityState) -> RootPose {
    fish_root(st, 1.0, 1.0, Vec3::new(0.2, 0.1, 0.0))
}

fn pufferfish_root(st: &EntityState) -> RootPose {
    let mut pose = setup_rotations(st, 90.0);
    pose.world_offset += Vec3::new(0.0, (st.age_ticks * 0.05).cos() * 0.08 * st.scale, 0.0);
    pose
}

fn is_small_salmon(st: &EntityState) -> bool {
    st.extras.variant_id <= 0
}

fn is_medium_salmon(st: &EntityState) -> bool {
    st.extras.variant_id == 1
}

fn is_large_salmon(st: &EntityState) -> bool {
    st.extras.variant_id >= 2
}

fn tropical_pattern(st: &EntityState) -> (i32, i32) {
    let packed = st.extras.variant_id & 0xffff;
    let (base, index) = (packed & 0xff, packed >> 8 & 0xff);
    if (0..2).contains(&base) && (0..6).contains(&index) {
        (base, index)
    } else {
        (0, 0)
    }
}

fn is_small_tropical(st: &EntityState) -> bool {
    tropical_pattern(st).0 == 0
}

fn is_large_tropical(st: &EntityState) -> bool {
    tropical_pattern(st).0 == 1
}

fn tropical_body_texture(st: &EntityState) -> TexturePath {
    if is_small_tropical(st) {
        "entity/fish/tropical_a".into()
    } else {
        "entity/fish/tropical_b".into()
    }
}

fn tropical_pattern_texture(st: &EntityState) -> TexturePath {
    let (base, index) = tropical_pattern(st);
    let letter = if base == 0 { 'a' } else { 'b' };
    let n = index + 1;
    format!("entity/fish/tropical_{letter}_pattern_{n}").into()
}

const DYE_TEXTURE_DIFFUSE: [u32; 16] = [
    0x00f9_fffe,
    0x00f9_801d,
    0x00c7_4ebd,
    0x003a_b3da,
    0x00fe_d83d,
    0x0080_c71f,
    0x00f3_8baa,
    0x0047_4f52,
    0x009d_9d97,
    0x0016_9c9c,
    0x0089_50c9,
    0x003c_44aa,
    0x0083_5432,
    0x005e_7c16,
    0x00b0_2e26,
    0x001d_1d21,
];

fn dye_tint(ordinal: i32) -> [f32; 4] {
    let rgb = DYE_TEXTURE_DIFFUSE[(ordinal.clamp(0, 15)) as usize];
    [
        crate::util::mth::srgb_byte_to_linear((rgb >> 16 & 0xff) as u8),
        crate::util::mth::srgb_byte_to_linear((rgb >> 8 & 0xff) as u8),
        crate::util::mth::srgb_byte_to_linear((rgb & 0xff) as u8),
        1.0,
    ]
}

fn tropical_base_tint(st: &EntityState) -> [f32; 4] {
    dye_tint(st.extras.variant_id >> 16 & 0xff)
}

fn tropical_pattern_tint(st: &EntityState) -> [f32; 4] {
    dye_tint(st.extras.variant_id >> 24 & 0xff)
}

fn is_deflated(st: &EntityState) -> bool {
    st.extras.puff_state == 0
}

fn is_half_puffed(st: &EntityState) -> bool {
    st.extras.puff_state == 1
}

fn is_fully_puffed(st: &EntityState) -> bool {
    !is_deflated(st) && !is_half_puffed(st)
}
