use azalea_core::direction::Direction;
use azalea_registry::builtin::BlockEntityKind;
use bevy::prelude::*;

use crate::blockentities::sim::Anim;
use crate::blockentities::{BeRegistry, BeSpec, BeState, GeomKey};
use crate::entities::geom::{BakedModel, LayerDef, PartState};
use crate::entities::models::monsters::shulker;
use crate::util::mth::DEG_TO_RAD;

const SCALE: f32 = 0.9995;
const MAX_LID_HEIGHT: f32 = 0.5;
const MAX_LID_ROTATION: f32 = 270.0;
const SUFFIX: &str = "_shulker_box";

pub fn register(registry: &mut BeRegistry) {
    registry.add(
        BlockEntityKind::ShulkerBox,
        BeSpec::model("shulker_box", key, layer, texture, setup).with_transform(transform),
    );
}

fn color(st: &BeState) -> &str {
    st.state.block.strip_suffix(SUFFIX).unwrap_or_default()
}

fn key(st: &BeState) -> Option<u64> {
    Some(GeomKey::new().str(color(st)).finish())
}

fn layer(_st: &BeState) -> LayerDef {
    shulker::box_layer()
}

fn texture(st: &BeState) -> String {
    match color(st) {
        "" => "entity/shulker/shulker".to_string(),
        dye => format!("entity/shulker/shulker_{dye}"),
    }
}

fn progress(st: &BeState) -> f32 {
    match &st.anim {
        Anim::Shulker(shulker) => shulker.progress(st.partial),
        _ => 0.0,
    }
}

fn setup(model: &BakedModel, parts: &mut [PartState], st: &BeState) {
    lid_pose(&mut parts[model.id("lid")], progress(st));
}

fn lid_pose(lid: &mut PartState, progress: f32) {
    lid.y = 24.0 - progress * MAX_LID_HEIGHT * 16.0;
    lid.y_rot = MAX_LID_ROTATION * progress * DEG_TO_RAD;
}

fn transform(st: &BeState) -> Transform {
    let facing =
        crate::blocks::direction::Direction::from(st.state.facing.unwrap_or(Direction::Up));
    let rotation = facing.rotation() * Quat::from_rotation_x(std::f32::consts::PI);
    Transform {
        translation: Vec3::splat(0.5) + SCALE * (rotation * Vec3::NEG_Y),
        rotation,
        scale: Vec3::splat(SCALE),
    }
}

#[cfg(test)]
pub(super) fn test_states(out: &mut Vec<BeState>) {
    for block in shulkers() {
        for facing in ["down", "up", "north", "south", "west", "east"] {
            out.push(super::test_state(&block, &[("facing", facing)]));
        }
    }
}

#[cfg(test)]
fn shulkers() -> impl Iterator<Item = String> {
    let dyed = crate::util::dye::DYES.map(|dye| format!("{}{SUFFIX}", dye.name));
    std::iter::once("shulker_box".to_string()).chain(dyed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fully_open_lid_rises_eight_pixels_and_turns_270_degrees() {
        let mut lid = PartState::default();
        lid_pose(&mut lid, 1.0);
        assert_eq!(lid.y, 16.0);
        assert!((lid.y_rot - 270.0_f32.to_radians()).abs() < 1e-5);
        lid_pose(&mut lid, 0.0);
        assert_eq!(lid.y, 24.0);
    }

    #[test]
    fn an_upward_box_sits_in_its_block() {
        let st = super::super::test_state("red_shulker_box", &[("facing", "up")]);
        let t = transform(&st);
        let floor = t.transform_point(Vec3::new(0.0, 1.5, 0.0));
        assert!((floor.y - 0.0).abs() < 1e-3, "{floor:?}");
        assert!((floor.x - 0.5).abs() < 1e-3);
    }

    #[test]
    fn textures_are_on_disk() {
        for block in shulkers() {
            let st = super::super::test_state(&block, &[("facing", "up")]);
            let file = crate::assets_root().join(format!("textures/{}.png", texture(&st)));
            assert!(file.exists(), "missing {}", texture(&st));
        }
    }
}
