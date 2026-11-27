use azalea::block::{BlockState, BlockTrait};
use azalea_core::direction::Direction;
use azalea_registry::builtin::EntityKind;
use bevy::math::{Mat4, Quat, Vec3};
use bevy::prelude::Transform;

use crate::blockentities::feed::ChestHalf;
use crate::blockentities::render::chest;
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
    registry.add_many(
        MINECARTS,
        chest_spec(
            "minecart_chest",
            || chest::layer_for(ChestHalf::Single),
            ChestHalf::Single,
        ),
    );
    registry.add_many(
        MINECARTS,
        chest_spec(
            "minecart_chest_left",
            || chest::layer_for(ChestHalf::Left),
            ChestHalf::Left,
        ),
    );
    registry.add_many(
        MINECARTS,
        chest_spec(
            "minecart_chest_right",
            || chest::layer_for(ChestHalf::Right),
            ChestHalf::Right,
        ),
    );
}

fn chest_spec(
    name: &'static str,
    layer: fn() -> crate::entities::geom::LayerDef,
    half: ChestHalf,
) -> RenderSpec {
    let visible: fn(&EntityState) -> bool = match half {
        ChestHalf::Single => |st| chest_cart(st).is_some_and(|c| c.half == ChestHalf::Single),
        ChestHalf::Left => |st| chest_cart(st).is_some_and(|c| c.half == ChestHalf::Left),
        ChestHalf::Right => |st| chest_cart(st).is_some_and(|c| c.half == ChestHalf::Right),
    };
    RenderSpec::new(name, layer, chest_texture, |_, _, _| {})
        .with_root(chest_root)
        .with_visible(visible)
}

#[derive(Clone, Copy)]
struct ChestCart {
    half: ChestHalf,
    facing: Option<Direction>,
}

fn chest_cart(st: &EntityState) -> Option<ChestCart> {
    use std::sync::atomic::{AtomicU8, Ordering::Relaxed};
    const UNKNOWN: u8 = 0;
    const OTHER: u8 = 1;
    const CHEST: u8 = 2;
    const HALVES: [ChestHalf; 3] = [ChestHalf::Single, ChestHalf::Left, ChestHalf::Right];
    const FACINGS: [Option<Direction>; 5] = [
        None,
        Some(Direction::North),
        Some(Direction::East),
        Some(Direction::South),
        Some(Direction::West),
    ];
    const STATES: usize = BlockState::MAX_STATE as usize + 1;
    static CACHE: [AtomicU8; STATES] = [const { AtomicU8::new(UNKNOWN) }; STATES];

    let id = st.extras.display_block?;
    let slot = CACHE.get(id as usize)?;
    match slot.load(Relaxed) {
        UNKNOWN => {
            let chest = decode_chest(id);
            let code = chest.map_or(OTHER, |chest| {
                let half = HALVES.iter().position(|h| *h == chest.half).unwrap_or(0);
                let facing = FACINGS.iter().position(|f| *f == chest.facing).unwrap_or(0);
                CHEST + (half * FACINGS.len() + facing) as u8
            });
            slot.store(code, Relaxed);
            chest
        }
        OTHER => None,
        code => {
            let index = usize::from(code - CHEST);
            Some(ChestCart {
                half: HALVES[index / FACINGS.len()],
                facing: FACINGS[index % FACINGS.len()],
            })
        }
    }
}

fn chest_name(block: &dyn BlockTrait) -> Option<&'static str> {
    let name = block.id();
    (matches!(name, "chest" | "trapped_chest" | "ender_chest") || name.ends_with("copper_chest"))
        .then_some(name)
}

fn decode_chest(id: u32) -> Option<ChestCart> {
    let block = Box::<dyn BlockTrait>::from(BlockState::try_from(id).ok()?);
    let name = chest_name(&*block)?;
    let half = match block.get_property("type") {
        _ if name == "ender_chest" => ChestHalf::Single,
        Some("left") => ChestHalf::Left,
        Some("right") => ChestHalf::Right,
        _ => ChestHalf::Single,
    };
    let facing = block
        .get_property("facing")
        .and_then(crate::blocks::facing::from_name);
    Some(ChestCart { half, facing })
}

fn chest_texture(st: &EntityState) -> TexturePath {
    let name = chest_cart(st).and_then(|chest| {
        let block =
            Box::<dyn BlockTrait>::from(BlockState::try_from(st.extras.display_block?).ok()?);
        Some((chest_name(&*block)?, chest.half))
    });
    match name {
        Some((name, half)) => chest::texture_of(name, half).into(),
        None => "entity/chest/normal".into(),
    }
}

fn chest_hook(display_offset: i32, facing: Option<Direction>) -> Transform {
    hook_for(display_matrix(display_offset) * chest::transform_for(facing).to_matrix())
}

fn display_matrix(display_offset: i32) -> Mat4 {
    display_hook(display_offset).to_matrix()
}

fn chest_root(st: &EntityState) -> RootPose {
    let facing = chest_cart(st).and_then(|c| c.facing);
    RootPose {
        hook: chest_hook(st.extras.display_offset, facing),
        ..cart_pose(st)
    }
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
    let render_offset = Vec3::from(st.extras.render_offset);

    if let Some(pose) = st.extras.cart_new {
        return RootPose {
            world_offset: render_offset + Vec3::new(jitter_x, jitter_y, jitter_z),
            rotation: Quat::from_rotation_y(pose.y_rot.to_radians())
                * Quat::from_rotation_z((-pose.x_rot).to_radians()),
            scale: 1.0,
            extra_rotation: super::hurt_wobble(st),
            extra_offset: Vec3::new(0.0, 0.375, 0.0),
            hook: Transform::IDENTITY,
        };
    }

    let rail = st.extras.rail;
    let on_rail = rail.map_or(Vec3::ZERO, |r| Vec3::from_array(r.offset));
    let (yaw, pitch) = rail.and_then(|r| r.turn).unwrap_or((st.body_rot, st.x_rot));

    let extra_rotation = Quat::from_rotation_z((-pitch).to_radians()) * super::hurt_wobble(st);

    RootPose {
        world_offset: render_offset + on_rail + Vec3::new(jitter_x, jitter_y + 0.375, jitter_z),
        rotation: Quat::from_rotation_y((180.0 - yaw).to_radians()),
        scale: 1.0,
        extra_rotation,
        extra_offset: Vec3::ZERO,
        hook: Transform::IDENTITY,
    }
}

fn display_root(st: &EntityState) -> RootPose {
    RootPose {
        hook: display_hook(st.extras.display_offset).mul_transform(tnt_swell(st)),
        ..cart_pose(st)
    }
}

fn display_hook(display_offset: i32) -> Transform {
    const SCALE: f32 = 0.75;
    let offset = Vec3::new(-0.5, (display_offset - 8) as f32 / 16.0, 0.5);
    Transform {
        translation: SCALE * offset,
        rotation: Quat::from_rotation_y(90f32.to_radians()),
        scale: Vec3::splat(SCALE),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::MODEL_Y_OFFSET;

    fn placed(hook: Transform, p: Vec3) -> Vec3 {
        (mirror() * hook.to_matrix() * Mat4::from_translation(Vec3::new(0.0, MODEL_Y_OFFSET, 0.0)))
            .transform_point3(p)
    }

    #[test]
    fn the_chest_sits_upright_over_the_cart_centre() {
        for facing in [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
        ] {
            let hook = chest_hook(6, Some(facing));
            let got = placed(hook, Vec3::new(0.5, 0.5, 0.5));
            let want = Vec3::new(0.0, 0.75 * (0.5 + (6 - 8) as f32 / 16.0), 0.0);
            assert!(
                (got - want).length() < 1e-5,
                "{facing:?}: {got:?} vs {want:?}"
            );
            let top = placed(hook, Vec3::new(0.5, 1.0, 0.5));
            assert!(top.y > got.y, "{facing:?} is upside down");
        }
    }

    #[test]
    fn a_south_facing_chest_front_points_along_plus_x() {
        let hook = chest_hook(8, Some(Direction::South));
        let front = placed(hook, Vec3::new(0.5, 0.5, 1.0));
        assert!(
            (front.x - 0.375).abs() < 1e-5 && front.z.abs() < 1e-5,
            "{front:?}"
        );
        let north = chest_hook(8, Some(Direction::North));
        let front = placed(north, Vec3::new(0.5, 0.5, 1.0));
        assert!(
            (front.x + 0.375).abs() < 1e-5 && front.z.abs() < 1e-5,
            "{front:?}"
        );
    }
}
