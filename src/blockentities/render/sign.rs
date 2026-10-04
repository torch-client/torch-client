use std::collections::hash_map::DefaultHasher;
use std::f32::consts::PI;
use std::hash::{Hash, Hasher};

use azalea_registry::builtin::BlockEntityKind;
use bevy::prelude::*;

use crate::blockentities::feed::{BlockEntityData, SignData, SignFace};
use crate::blockentities::models::{self, HangingAttachment};
use crate::blockentities::text;
use crate::blockentities::{BeRegistry, BeSpec, BeState, BuiltTexture, GeomKey};
use crate::entities::Blend;
use crate::entities::geom::{BakedModel, LayerDef, PartState};
use crate::gui::atlas::GuiAtlas;

pub fn register(registry: &mut BeRegistry) {
    registry.add(
        BlockEntityKind::Sign,
        BeSpec::model("sign", plain_key, plain_layer, plain_texture, no_anim)
            .with_transform(plain_body_transform),
    );
    registry.add(
        BlockEntityKind::HangingSign,
        BeSpec::model(
            "hanging_sign",
            hanging_key,
            hanging_layer,
            hanging_texture,
            no_anim,
        )
        .with_transform(hanging_body_transform),
    );
    registry.add_many(
        &[BlockEntityKind::Sign, BlockEntityKind::HangingSign],
        BeSpec::built(
            "sign_text_front",
            front_key,
            build_front,
            BuiltTexture::Font,
        )
        .with_transform(front_text_transform)
        .with_blend(Blend::Translucent)
        .with_full_bright(front_full_bright),
    );
    registry.add_many(
        &[BlockEntityKind::Sign, BlockEntityKind::HangingSign],
        BeSpec::built("sign_text_back", back_key, build_back, BuiltTexture::Font)
            .with_transform(back_text_transform)
            .with_blend(Blend::Translucent)
            .with_full_bright(back_full_bright),
    );
}

fn no_anim(_model: &BakedModel, _parts: &mut [PartState], _st: &BeState) {}

pub const WOODS: &[&str] = &[
    "oak", "spruce", "birch", "acacia", "cherry", "jungle", "dark_oak", "pale_oak", "crimson",
    "warped", "mangrove", "bamboo",
];

pub fn known_wood(block: &str) -> Option<&'static str> {
    let wood = wood_type(block);
    WOODS.iter().copied().find(|w| *w == wood)
}

pub fn wood_type(block: &str) -> &str {
    for suffix in ["_wall_hanging_sign", "_hanging_sign", "_wall_sign", "_sign"] {
        if let Some(wood) = block.strip_suffix(suffix) {
            return wood;
        }
    }
    block
}

pub(crate) fn is_wall(block: &str) -> bool {
    block.ends_with("_wall_sign") || block.ends_with("_wall_hanging_sign")
}

fn segment_degrees(segment: u32) -> f32 {
    segment as f32 * (360.0 / 16.0)
}

fn sign_angle(st: &BeState) -> f32 {
    if is_wall(&st.state.block) {
        super::chest::facing_y_rot(st.state.facing)
    } else {
        segment_degrees(st.state.rotation)
    }
}

fn hanging_attachment(st: &BeState) -> HangingAttachment {
    if is_wall(&st.state.block) {
        HangingAttachment::Wall
    } else if st.state.prop("attached") == "true" {
        HangingAttachment::CeilingMiddle
    } else {
        HangingAttachment::Ceiling
    }
}

fn plain_key(st: &BeState) -> Option<u64> {
    let attachment = if is_wall(&st.state.block) {
        "wall"
    } else {
        "ground"
    };
    Some(
        GeomKey::new()
            .str(wood_type(&st.state.block))
            .str(attachment)
            .finish(),
    )
}

fn plain_layer(st: &BeState) -> LayerDef {
    if is_wall(&st.state.block) {
        models::wall_sign()
    } else {
        models::standing_sign()
    }
}

fn plain_texture(st: &BeState) -> String {
    format!("entity/signs/{}", wood_type(&st.state.block))
}

fn hanging_key(st: &BeState) -> Option<u64> {
    let attachment = match hanging_attachment(st) {
        HangingAttachment::Wall => "wall",
        HangingAttachment::Ceiling => "ceiling",
        HangingAttachment::CeilingMiddle => "ceiling_middle",
    };
    Some(
        GeomKey::new()
            .str(wood_type(&st.state.block))
            .str(attachment)
            .finish(),
    )
}

fn hanging_layer(st: &BeState) -> LayerDef {
    models::hanging_sign(hanging_attachment(st))
}

fn hanging_texture(st: &BeState) -> String {
    format!("entity/signs/hanging/{}", wood_type(&st.state.block))
}

const PLAIN_RENDER_SCALE: f32 = 0.666_666_7;

const PLAIN_TEXT_OFFSET: Vec3 = Vec3::new(0.0, 0.333_333_34, 0.046_666_667);

const PLAIN_TEXT_SCALE: f32 = 0.010_416_667;

const HANGING_TEXT_OFFSET: Vec3 = Vec3::new(0.0, -0.32, 0.073);

const HANGING_TEXT_SCALE: f32 = 0.014_062_5;

fn plain_base(st: &BeState) -> (Vec3, Quat) {
    let rotation = Quat::from_rotation_y(-sign_angle(st).to_radians());
    let mut translation = Vec3::new(0.5, 0.5, 0.5);
    if is_wall(&st.state.block) {
        translation += rotation * Vec3::new(0.0, -0.3125, -0.4375);
    }
    (translation, rotation)
}

fn plain_body_transform(st: &BeState) -> Transform {
    let (translation, rotation) = plain_base(st);
    Transform {
        translation,
        rotation,
        scale: Vec3::new(PLAIN_RENDER_SCALE, -PLAIN_RENDER_SCALE, -PLAIN_RENDER_SCALE),
    }
}

fn hanging_base(st: &BeState) -> (Vec3, Quat) {
    let rotation = Quat::from_rotation_y(-sign_angle(st).to_radians());
    let translation = Vec3::new(0.5, 0.9375, 0.5) + rotation * Vec3::new(0.0, -0.3125, 0.0);
    (translation, rotation)
}

fn hanging_body_transform(st: &BeState) -> Transform {
    let (translation, rotation) = hanging_base(st);
    Transform {
        translation,
        rotation,
        scale: Vec3::new(1.0, -1.0, -1.0),
    }
}

fn sign_data(st: &BeState) -> Option<&SignData> {
    match &*st.data {
        BlockEntityData::Sign(sign) => Some(sign),
        #[cfg(feature = "skins")]
        BlockEntityData::Skull(_) => None,
        BlockEntityData::Banner(_) | BlockEntityData::None => None,
    }
}

fn face<'a>(st: &'a BeState, front: bool) -> Option<&'a SignFace> {
    let sign = sign_data(st)?;
    Some(if front { &sign.front } else { &sign.back })
}

fn text_key(st: &BeState, front: bool) -> Option<u64> {
    let face = face(st, front)?;
    if face
        .lines
        .iter()
        .all(|line| line.iter().all(|span| span.text.trim().is_empty()))
    {
        return None;
    }
    let style = text::text_style(face, st.draw_outline);
    let mut hasher = DefaultHasher::new();
    face.hash(&mut hasher);
    style.hash(&mut hasher);
    Some(GeomKey::new().mix(hasher.finish()).finish())
}

fn front_key(st: &BeState) -> Option<u64> {
    text_key(st, true)
}

fn back_key(st: &BeState) -> Option<u64> {
    text_key(st, false)
}

fn build_text(st: &BeState, atlas: &GuiAtlas, front: bool) -> Option<Mesh> {
    let face = face(st, front)?;
    let metrics = text::SignKind::of(st.kind)?.metrics();
    text::sign_text_mesh(
        atlas,
        face,
        text::text_style(face, st.draw_outline),
        metrics,
    )
}

fn build_front(st: &BeState, atlas: &GuiAtlas) -> Option<Mesh> {
    build_text(st, atlas, true)
}

fn build_back(st: &BeState, atlas: &GuiAtlas) -> Option<Mesh> {
    build_text(st, atlas, false)
}

fn full_bright(st: &BeState, front: bool) -> bool {
    face(st, front).map(|face| face.glowing).unwrap_or(false)
}

fn front_full_bright(st: &BeState) -> bool {
    full_bright(st, true)
}

fn back_full_bright(st: &BeState) -> bool {
    full_bright(st, false)
}

fn text_transform(st: &BeState, front: bool) -> Transform {
    let (base, rotation, offset, scale) = match st.kind {
        BlockEntityKind::HangingSign => {
            let (translation, rotation) = hanging_base(st);
            (
                translation,
                rotation,
                HANGING_TEXT_OFFSET,
                HANGING_TEXT_SCALE,
            )
        }
        _ => {
            let (translation, rotation) = plain_base(st);
            (translation, rotation, PLAIN_TEXT_OFFSET, PLAIN_TEXT_SCALE)
        }
    };
    let rotation = if front {
        rotation
    } else {
        rotation * Quat::from_rotation_y(PI)
    };
    Transform {
        translation: base + rotation * offset,
        rotation,
        scale: Vec3::new(scale, -scale, scale),
    }
}

fn front_text_transform(st: &BeState) -> Transform {
    text_transform(st, true)
}

fn back_text_transform(st: &BeState) -> Transform {
    text_transform(st, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sign_block_names_its_wood() {
        assert_eq!(wood_type("oak_sign"), "oak");
        assert_eq!(wood_type("dark_oak_wall_sign"), "dark_oak");
        assert_eq!(wood_type("pale_oak_hanging_sign"), "pale_oak");
        assert_eq!(wood_type("crimson_wall_hanging_sign"), "crimson");
    }

    #[test]
    fn rotation_segments_are_sixteenths_of_a_turn() {
        assert_eq!(segment_degrees(0), 0.0);
        assert_eq!(segment_degrees(4), 90.0);
        assert_eq!(segment_degrees(15), 337.5);
    }

    #[test]
    fn a_standing_sign_stands_on_the_block_centre() {
        let st = super::super::test_state("oak_sign", &[("rotation", "0")]);
        let transform = plain_body_transform(&st);
        assert!(
            transform
                .translation
                .abs_diff_eq(Vec3::new(0.5, 0.5, 0.5), 1e-6)
        );
        let top = transform.transform_point(Vec3::new(0.0, -14.0 / 16.0, 0.0));
        assert!((top.y - (0.5 + 14.0 / 16.0 * PLAIN_RENDER_SCALE)).abs() < 1e-6);
    }

    #[test]
    fn a_wall_sign_hangs_off_the_block_behind_it() {
        let st = super::super::test_state("oak_wall_sign", &[("facing", "south")]);
        let transform = plain_body_transform(&st);
        assert!(
            transform
                .translation
                .abs_diff_eq(Vec3::new(0.5, 0.1875, 0.0625), 1e-6),
            "{:?}",
            transform.translation
        );
    }

    #[test]
    fn the_back_text_faces_backwards() {
        let st = super::super::test_state("oak_sign", &[("rotation", "0")]);
        let front = text_transform(&st, true);
        let back = text_transform(&st, false);
        assert!(front.translation.z > 0.5);
        assert!(back.translation.z < 0.5);
        let turned = front.rotation * Quat::from_rotation_y(PI);
        assert!(back.rotation.abs_diff_eq(turned, 1e-6));
    }

    #[test]
    fn a_hanging_sign_hangs_from_the_ceiling() {
        let st = super::super::test_state("oak_hanging_sign", &[("rotation", "0")]);
        let transform = hanging_body_transform(&st);
        assert!(
            transform
                .translation
                .abs_diff_eq(Vec3::new(0.5, 0.625, 0.5), 1e-6),
            "{:?}",
            transform.translation
        );
    }
}
