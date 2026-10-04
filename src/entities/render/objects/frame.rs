use azalea_registry::builtin::EntityKind;
use bevy::math::{Quat, Vec3};
use bevy::prelude::{Mesh, Transform};

use crate::entities::TexturePath;
use crate::entities::registry::Registry;
use crate::entities::state::{Direction, EntityState};
use crate::entities::{BlockRef, FrameModel, LightMode, RenderSpec, RootPose};
use crate::renderer::maps::MAP_TEXTURE_PREFIX;

const OFFSET_TO_WALL: f32 = 0.46875;

const MAP_EDGE: f32 = crate::client::tracking::MAP_EDGE as f32;

pub fn register(registry: &mut Registry) {
    registry.add_many(
        &[EntityKind::ItemFrame, EntityKind::GlowItemFrame],
        RenderSpec::block("item_frame", frame_model).with_root(frame_model_root),
    );
    registry.add_many(
        &[EntityKind::ItemFrame, EntityKind::GlowItemFrame],
        RenderSpec::built("item_frame_map", map_key, map_mesh, map_texture)
            .with_root(frame_map_root)
            .with_visible(has_map)
            .with_light(LightMode::Text)
            .ignoring_invisibility(),
    );
}

fn frame_model(st: &EntityState) -> Option<BlockRef> {
    Some(BlockRef::Model(
        match (st.kind, st.extras.item.map_id.is_some()) {
            (EntityKind::GlowItemFrame, false) => FrameModel::GlowFrame,
            (EntityKind::GlowItemFrame, true) => FrameModel::GlowFrameMap,
            (_, false) => FrameModel::Frame,
            (_, true) => FrameModel::FrameMap,
        },
    ))
}

pub fn has_map(st: &EntityState) -> bool {
    st.extras.item.map_id.is_some()
}

fn facing(st: &EntityState) -> (Vec3, f32, f32) {
    let dir = st.extras.item_frame_direction;
    let step = Vec3::from(dir.normal());
    match dir {
        Direction::Up => (step, -90.0, 180.0),
        Direction::Down => (step, 90.0, 180.0),
        _ => (step, 0.0, 180.0 - dir.to_y_rot()),
    }
}

pub fn frame_root(st: &EntityState) -> RootPose {
    let (step, x_rot, y_rot) = facing(st);
    RootPose {
        scale: 1.0,
        hook: Transform {
            translation: step * OFFSET_TO_WALL,
            rotation: Quat::from_rotation_x(x_rot.to_radians())
                * Quat::from_rotation_y(y_rot.to_radians()),
            scale: Vec3::ONE,
        },
        ..RootPose::default()
    }
}

fn frame_model_root(st: &EntityState) -> RootPose {
    let mut pose = frame_root(st);
    pose.hook = pose
        .hook
        .mul_transform(Transform::from_xyz(-0.5, -0.5, -0.5));
    pose
}

pub fn contents_depth(st: &EntityState) -> f32 {
    if st.is_invisible { 0.5 } else { 0.4375 }
}

fn map_mesh(_st: &EntityState) -> Mesh {
    const Z: f32 = -0.01;
    let mut out = crate::entities::quads::QuadMesh::new();
    out.quad(
        [
            [0.0, MAP_EDGE, Z],
            [MAP_EDGE, MAP_EDGE, Z],
            [MAP_EDGE, 0.0, Z],
            [0.0, 0.0, Z],
        ],
        [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        [0.0, 0.0, 1.0],
    );
    out.finish()
}

fn map_texture(st: &EntityState) -> Option<TexturePath> {
    st.extras
        .item
        .map_id
        .map(|id| format!("{MAP_TEXTURE_PREFIX}{id}").into())
}

fn map_key(st: &EntityState) -> TexturePath {
    map_texture(st).unwrap_or_default()
}

fn frame_map_root(st: &EntityState) -> RootPose {
    let quarter_turns = (st.extras.item_rotation.rem_euclid(4) * 2) as f32;
    let mut pose = frame_root(st);
    pose.hook = pose
        .hook
        .mul_transform(Transform::from_xyz(0.0, 0.0, contents_depth(st)))
        .mul_transform(Transform::from_rotation(Quat::from_rotation_z(
            (quarter_turns * 45.0 + 180.0).to_radians(),
        )))
        .mul_transform(Transform {
            translation: Vec3::new(-64.0, -64.0, -1.0) / MAP_EDGE,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(1.0 / MAP_EDGE),
        });
    pose
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::state::{Extras, ExtrasShared};

    fn state(dir: Direction) -> EntityState {
        let mut st = EntityState::new(0, EntityKind::ItemFrame);
        st.extras = Extras::with_shared(std::sync::Arc::new(ExtrasShared {
            item_frame_direction: dir,
            ..ExtrasShared::default()
        }));
        st
    }

    #[test]
    fn horizontal_directions_give_their_yaw() {
        for (dir, step, y_rot) in [
            (Direction::South, Vec3::new(0.0, 0.0, 1.0), 180.0),
            (Direction::West, Vec3::new(-1.0, 0.0, 0.0), 90.0),
            (Direction::North, Vec3::new(0.0, 0.0, -1.0), 0.0),
            (Direction::East, Vec3::new(1.0, 0.0, 0.0), -90.0),
        ] {
            let (got_step, x_rot, got_y_rot) = facing(&state(dir));
            assert!((got_step - step).length() < 1e-5, "{dir:?}: {got_step:?}");
            assert_eq!(x_rot, 0.0, "{dir:?}");
            assert_eq!(got_y_rot, y_rot, "{dir:?}");
        }
    }

    #[test]
    fn vertical_directions_pitch_instead() {
        let (up, up_x_rot, up_y_rot) = facing(&state(Direction::Up));
        assert_eq!(up, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!((up_x_rot, up_y_rot), (-90.0, 180.0));
        let (down, down_x_rot, down_y_rot) = facing(&state(Direction::Down));
        assert_eq!(down, Vec3::new(0.0, -1.0, 0.0));
        assert_eq!((down_x_rot, down_y_rot), (90.0, 180.0));
    }

    #[test]
    fn the_model_hangs_on_the_wall_behind_the_frame() {
        for dir in Direction::ALL {
            let pose = frame_model_root(&state(dir));
            let back = pose.hook.transform_point(Vec3::new(0.5, 0.5, 1.0));
            let want = Vec3::from(dir.normal()) * (OFFSET_TO_WALL - 0.5);
            assert!((back - want).length() < 1e-5, "{dir:?}: {back:?}");
        }
    }
}
