use azalea_registry::builtin::EntityKind;
use bevy::math::{Quat, Vec3};
use bevy::prelude::Transform;

use crate::entities::TexturePath;
use crate::entities::models::objects::minecart;
use crate::entities::registry::Registry;
use crate::entities::render::objects::{hook_for, mirror};
use crate::entities::state::EntityState;
use crate::entities::{BlockRef, RenderSpec, RootPose};

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
    registry.add_many(
        MINECARTS,
        RenderSpec::block("minecart_display", display_block)
            .with_root(display_root)
            .with_fuse_flash(tnt_lit),
    );
}

fn display_block(st: &EntityState) -> Option<BlockRef> {
    st.extras.display_block.map(BlockRef::State)
}

fn texture(_st: &EntityState) -> TexturePath {
    "entity/minecart/minecart".into()
}

fn root(st: &EntityState) -> RootPose {
    RootPose {
        hook: hook_for(mirror()),
        ..cart_pose(st)
    }
}

fn cart_pose(st: &EntityState) -> RootPose {
    let [jitter_x, jitter_y, jitter_z] = jitter(st.id);

    let extra_rotation = Quat::from_rotation_z((-st.x_rot).to_radians()) * super::hurt_wobble(st);

    RootPose {
        world_offset: Vec3::new(jitter_x, jitter_y + 0.375, jitter_z),
        rotation: Quat::from_rotation_y((180.0 - st.body_rot).to_radians()),
        scale: 1.0,
        extra_rotation,
        extra_offset: Vec3::ZERO,
        hook: Transform::IDENTITY,
    }
}

fn display_root(st: &EntityState) -> RootPose {
    const SCALE: f32 = 0.75;
    let offset = Vec3::new(-0.5, (st.extras.display_offset - 8) as f32 / 16.0, 0.5);
    let hook = Transform {
        translation: SCALE * offset,
        rotation: Quat::from_rotation_y(90f32.to_radians()),
        scale: Vec3::splat(SCALE),
    };
    RootPose {
        hook: hook.mul_transform(tnt_swell(st)),
        ..cart_pose(st)
    }
}

fn tnt_swell(st: &EntityState) -> Transform {
    let fuse = st.extras.minecart_fuse;
    if st.kind != EntityKind::TntMinecart || fuse <= -1.0 || fuse >= 10.0 {
        return Transform::IDENTITY;
    }
    let swell = super::blocks::swell_amount(fuse);
    Transform {
        translation: Vec3::new(-swell * 0.5, 0.0, -swell * 0.5),
        rotation: Quat::IDENTITY,
        scale: Vec3::splat(1.0 + swell),
    }
}

fn tnt_lit(st: &EntityState) -> bool {
    st.kind == EntityKind::TntMinecart && super::blocks::is_lit(st.extras.minecart_fuse)
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
