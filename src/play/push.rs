use azalea::app::{App, Plugin};
use azalea::ecs::prelude::*;
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::metadata::AbstractLiving;
use azalea::entity::{
    Dead, EntityKindComponent, EntityUuid, HasClientLoaded, LocalEntity, OnClimbable, Physics,
    Pose, Position,
};
use azalea::interact::pick::PickBoundingBox;
use azalea::physics::{PhysicsSystems, travel::travel};
use azalea::player::GameProfileComponent;
use azalea::world::WorldName;
use azalea_core::entity_id::MinecraftEntityId;
use azalea_core::game_type::GameMode;
use azalea_core::position::Vec3;
use azalea_core::tick::{Frozen, GameTick};
use azalea_registry::builtin::EntityKind;

const MIN_DISTANCE: f64 = 0.009999999776482582;
const STRENGTH: f64 = 0.05000000074505806;
const BOAT_INFLATE: Vec3 = Vec3 {
    x: 0.20000000298023224,
    y: -0.009999999776482582,
    z: 0.20000000298023224,
};

pub struct PushPlugin;

impl Plugin for PushPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            GameTick,
            shove
                .in_set(PhysicsSystems)
                .after(travel)
                .after(crate::play::pick_box::step_pick_boxes),
        );
    }
}

struct Pusher {
    uuid: u128,
    player: bool,
    name: Option<String>,
    x: f64,
    z: f64,
}

#[allow(clippy::type_complexity)]
fn shove(
    mut local: Query<
        (
            &mut Physics,
            &Position,
            &EntityDimensions,
            &MinecraftEntityId,
            &WorldName,
            &OnClimbable,
            Option<&GameMode>,
            Option<&Pose>,
        ),
        (With<LocalEntity>, With<HasClientLoaded>, Without<Dead>),
    >,
    others: Query<
        (
            &Position,
            &EntityDimensions,
            &WorldName,
            &EntityKindComponent,
            &EntityUuid,
            Option<&GameProfileComponent>,
            Option<&PickBoundingBox>,
            Has<AbstractLiving>,
            &MinecraftEntityId,
        ),
        Without<LocalEntity>,
    >,
    frozen: Res<Frozen>,
) {
    for (mut physics, position, dimensions, id, world_name, climbing, game_mode, pose) in &mut local
    {
        if game_mode == Some(&GameMode::Spectator) || **climbing || pose == Some(&Pose::Sleeping) {
            continue;
        }
        if crate::client::tracking::local_vehicle_id().is_some() {
            continue;
        }
        let bounds = dimensions.make_bounding_box(**position);

        let mut found: Vec<Pusher> = Vec::new();
        for (
            other_pos,
            other_dims,
            other_world,
            kind,
            entity_uuid,
            profile,
            pick,
            living,
            other_id,
        ) in &others
        {
            let fallback;
            let other_box = match pick {
                Some(pick) => &pick.0,
                None => {
                    fallback = other_dims.make_bounding_box(**other_pos);
                    &fallback
                }
            };
            let boat = is_boat(kind.0);
            let touching = if boat {
                other_box.inflate(BOAT_INFLATE).intersects_aabb(&bounds)
                    && bounds.min.y <= other_box.min.y
            } else {
                other_box.intersects_aabb(&bounds)
            };
            if !touching || other_world != world_name {
                continue;
            }
            if !boat && (!living || never_pushes(kind.0)) {
                continue;
            }
            if !frozen.entity_ticks(other_id.0) {
                continue;
            }
            let center = other_box.get_center();
            found.push(Pusher {
                uuid: entity_uuid.as_u128(),
                player: kind.0 == EntityKind::Player,
                name: profile.map(|p| p.0.name.clone()),
                x: center.x,
                z: center.z,
            });
        }
        if found.is_empty() {
            continue;
        }

        let carrying = crate::client::tracking::passengers()
            .lock()
            .unwrap()
            .get(&id.0)
            .is_some_and(|p| !p.is_empty());
        if carrying {
            continue;
        }

        let me = crate::client::bot::username();
        let mut buf = [0u8; uuid::fmt::Hyphenated::LENGTH];
        let mut delta = Vec3::new(0.0, 0.0, 0.0);
        crate::client::tablist::with_lookup(|tab| {
            for pusher in &found {
                if pusher.player
                    && tab.gamemode(pusher.uuid) == Some(crate::session::Gamemode::Spectator)
                {
                    continue;
                }
                let name: &str = if pusher.player {
                    pusher
                        .name
                        .as_deref()
                        .or_else(|| tab.name(pusher.uuid))
                        .unwrap_or("")
                } else {
                    uuid::Uuid::from_u128(pusher.uuid)
                        .hyphenated()
                        .encode_lower(&mut buf)
                };
                if !tab.pushes(name, &me) {
                    continue;
                }
                if let Some((dx, dz)) = away(position.x - pusher.x, position.z - pusher.z) {
                    delta.x += dx;
                    delta.z += dz;
                }
            }
        });
        physics.velocity.x += delta.x;
        physics.velocity.z += delta.z;
    }
}

fn away(dx: f64, dz: f64) -> Option<(f64, f64)> {
    let dd = dx.abs().max(dz.abs());
    if dd < MIN_DISTANCE {
        return None;
    }
    let dd = dd.sqrt();
    let pow = (1.0 / dd).min(1.0);
    Some((dx / dd * pow * STRENGTH, dz / dd * pow * STRENGTH))
}

fn is_boat(kind: EntityKind) -> bool {
    use EntityKind::*;
    matches!(
        kind,
        OakBoat
            | OakChestBoat
            | SpruceBoat
            | SpruceChestBoat
            | BirchBoat
            | BirchChestBoat
            | JungleBoat
            | JungleChestBoat
            | AcaciaBoat
            | AcaciaChestBoat
            | CherryBoat
            | CherryChestBoat
            | DarkOakBoat
            | DarkOakChestBoat
            | PaleOakBoat
            | PaleOakChestBoat
            | MangroveBoat
            | MangroveChestBoat
            | BambooRaft
            | BambooChestRaft
    )
}

fn never_pushes(kind: EntityKind) -> bool {
    use EntityKind::*;
    matches!(kind, Bat | ArmorStand | Parrot | Vex | EnderDragon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn far_pairs_cap_at_the_unit_push() {
        let (x, z) = away(0.25, 0.0).unwrap();
        assert!((x - 0.025).abs() < 1e-12 && z == 0.0);
        let (x, _) = away(4.0, 0.0).unwrap();
        assert!((x - 0.05).abs() < 1e-12);
        assert!(away(0.001, -0.001).is_none());
    }
}
