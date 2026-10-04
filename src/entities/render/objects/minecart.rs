use azalea_registry::builtin::EntityKind;
use bevy::math::{Quat, Vec3};

use crate::entities::TexturePath;
use crate::entities::models::objects::minecart;
use crate::entities::registry::Registry;
use crate::entities::render::objects::{hook_for, mirror};
use crate::entities::state::EntityState;
use crate::entities::{RenderSpec, RootPose};

const MINECARTS: &[EntityKind] = &[
    EntityKind::Minecart,
    EntityKind::ChestMinecart,
    EntityKind::FurnaceMinecart,
    EntityKind::TntMinecart,
    EntityKind::HopperMinecart,
    EntityKind::SpawnerMinecart,
    EntityKind::CommandBlockMinecart,
];

pub fn register(registry: &mut Registry) {
    registry.add_many(
        MINECARTS,
        RenderSpec::new("minecart", minecart::layer, texture, minecart::setup_anim).with_root(root),
    );
}

fn texture(_st: &EntityState) -> TexturePath {
    "entity/minecart/minecart".into()
}

fn root(st: &EntityState) -> RootPose {
    let [jitter_x, jitter_y, jitter_z] = jitter(st.id);

    let mut extra_rotation = Quat::from_rotation_z((-st.x_rot).to_radians());
    let hurt = st.extras.hurt_time;
    if hurt > 0.0 {
        let degrees = hurt.sin() * hurt * st.extras.damage / 10.0 * st.extras.hurt_dir;
        extra_rotation *= Quat::from_rotation_x(degrees.to_radians());
    }

    RootPose {
        world_offset: Vec3::new(jitter_x, jitter_y + 0.375, jitter_z),
        rotation: Quat::from_rotation_y((180.0 - st.body_rot).to_radians()),
        scale: 1.0,
        extra_rotation,
        extra_offset: Vec3::ZERO,
        hook: hook_for(mirror()),
    }
}

fn jitter(id: i32) -> [f32; 3] {
    let seed = (id as i64).wrapping_mul(493_286_711);
    let seed = seed
        .wrapping_mul(seed)
        .wrapping_mul(4_392_167_121)
        .wrapping_add(seed.wrapping_mul(98_761));
    let axis = |shift: u32| (((seed >> shift) & 7) as f32 + 0.5) / 8.0 - 0.5;
    [axis(16) * 0.004, axis(20) * 0.004, axis(24) * 0.004]
}
