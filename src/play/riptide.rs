use azalea::Client;
use azalea::app::{App, Plugin};
use azalea::ecs::prelude::*;
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::metadata::{AbstractLiving, Sprinting};
use azalea::entity::{
    Attributes, EntityUuid, HasClientLoaded, Jumping, LocalEntity, LookDirection, OnClimbable,
    Physics, Pose, Position,
};
use azalea::local_player::TabList;
use azalea::physics::collision::entity_collisions::{AabbQuery, CollidableEntityQuery};
use azalea::physics::collision::{MoveCtx, MoverType, move_colliding};
use azalea::physics::travel::travel;
use azalea::physics::{PhysicsSystems, ai_step};
use azalea::world::{WorldBorder, WorldName, Worlds};
use azalea_core::aabb::Aabb;
use azalea_core::game_type::GameMode;
use azalea_core::math;
use azalea_core::position::Vec3;
use azalea_core::registry_holder::RegistryHolder;
use azalea_core::tick::GameTick;
use azalea_inventory::ItemStack;
use azalea_inventory::components::Enchantments;

use crate::play::elytra::next_damage_will_break;
use crate::session::InteractionHand;

const MIN_HOLD_TICKS: i32 = 10;
const SPIN_TICKS: i32 = 20;
const GROUND_LIFT: f64 = 1.1999999284744263;

#[derive(Component)]
struct PendingRiptide(f32);

#[derive(Resource, Default)]
struct Spin {
    ticks: i32,
    old_box: Aabb,
}

pub struct RiptidePlugin;

impl Plugin for RiptidePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Spin>().add_systems(
            GameTick,
            (
                (launch, snapshot_box)
                    .chain()
                    .in_set(PhysicsSystems)
                    .before(ai_step),
                check_auto_spin_attack.in_set(PhysicsSystems).after(travel),
            ),
        );
    }
}

fn spin_strength(level: i32) -> f32 {
    if level <= 0 {
        return 0.0;
    }
    1.5 + 0.75 * (level - 1) as f32
}

fn stack_strength(stack: &ItemStack, registries: &RegistryHolder) -> f32 {
    let level = stack
        .get_component::<Enchantments>()
        .map_or(0, |enchantments| {
            enchantments
                .levels
                .iter()
                .find(|(e, _)| {
                    crate::play::inventory_bridge::enchantment_name(Some(registries), e)
                        == "riptide"
                })
                .map_or(0, |(_, level)| *level)
        });
    spin_strength(level)
}

fn may_start(breaks: bool, strength: f32, wet: bool) -> bool {
    !breaks && !(strength > 0.0 && !wet)
}

fn may_launch(held_ticks: i32, strength: f32, wet: bool, riding: bool, breaks: bool) -> bool {
    held_ticks >= MIN_HOLD_TICKS && strength > 0.0 && wet && !riding && !breaks
}

fn push_for(y_rot: f32, x_rot: f32, strength: f32) -> Vec3 {
    const DEG_TO_RAD: f32 = std::f32::consts::PI / 180.0;
    let x = -math::sin(y_rot * DEG_TO_RAD) * math::cos(x_rot * DEG_TO_RAD);
    let y = -math::sin(x_rot * DEG_TO_RAD);
    let z = math::cos(y_rot * DEG_TO_RAD) * math::cos(x_rot * DEG_TO_RAD);
    let scale = strength / (x * x + y * y + z * z).sqrt();
    Vec3::new(
        f64::from(x * scale),
        f64::from(y * scale),
        f64::from(z * scale),
    )
}

fn in_water_or_rain(bot: &Client) -> bool {
    let Ok((in_water, top)) = bot
        .component::<Physics>()
        .map(|p| (p.is_in_water(), p.bounding_box.max.y.floor() as i32))
    else {
        return false;
    };
    if in_water {
        return true;
    }
    let (Ok(world), Ok(feet)) = (bot.world(), bot.position()) else {
        return false;
    };
    let (x, y, z) = (
        feet.x.floor() as i32,
        feet.y.floor() as i32,
        feet.z.floor() as i32,
    );
    let world = crate::client::tracking::read_lock(&world, "riptide_wet");
    crate::renderer::weather::raining_at(&world, &[[x, y, z], [x, top, z]])
}

fn hand_trident(bot: &Client, hand: InteractionHand) -> Option<(f32, bool)> {
    let stack = bot
        .component::<azalea::entity::inventory::Inventory>()
        .ok()?
        .get_equipment(hand.equipment())?
        .clone();
    let world = bot.world().ok()?;
    let world = crate::client::tracking::read_lock(&world, "riptide_strength");
    Some((
        stack_strength(&stack, &world.registries),
        next_damage_will_break(&stack),
    ))
}

pub(crate) fn can_start_use(bot: &Client, hand: InteractionHand) -> bool {
    let Some((strength, breaks)) = hand_trident(bot, hand) else {
        return false;
    };
    may_start(breaks, strength, strength <= 0.0 || in_water_or_rain(bot))
}

pub(crate) fn release(bot: &Client, hand: InteractionHand, held_ticks: i32) {
    let Some((strength, breaks)) = hand_trident(bot, hand) else {
        return;
    };
    let riding = crate::client::tracking::local_vehicle_id().is_some();
    if !may_launch(
        held_ticks,
        strength,
        strength > 0.0 && in_water_or_rain(bot),
        riding,
        breaks,
    ) {
        return;
    }
    let mut ecs = bot.ecs.write();
    ecs.entity_mut(bot.entity).insert(PendingRiptide(strength));
}

#[allow(clippy::type_complexity)]
fn launch(
    mut query: Query<
        (
            Entity,
            &PendingRiptide,
            &mut Physics,
            &mut Position,
            &LookDirection,
            &EntityDimensions,
            &Attributes,
            &WorldName,
            &Sprinting,
            &Jumping,
            &OnClimbable,
            &Pose,
            Option<&WorldBorder>,
        ),
        (With<LocalEntity>, With<HasClientLoaded>),
    >,
    worlds: Res<Worlds>,
    aabb_query: AabbQuery,
    collidable_entity_query: CollidableEntityQuery,
    mut spin: ResMut<Spin>,
    mut commands: Commands,
) {
    for (
        entity,
        pending,
        mut physics,
        mut position,
        direction,
        dimensions,
        attributes,
        world_name,
        sprinting,
        jumping,
        on_climbable,
        pose,
        border,
    ) in &mut query
    {
        commands.entity(entity).remove::<PendingRiptide>();
        let push = push_for(direction.y_rot(), direction.x_rot(), pending.0);
        physics.velocity += push;
        physics.has_impulse = true;
        spin.ticks = SPIN_TICKS;
        if !physics.on_ground() {
            continue;
        }
        let Some(world_lock) = worlds.get(world_name) else {
            continue;
        };
        let world = world_lock.read();
        let mut ctx = MoveCtx {
            mover_type: MoverType::Own,
            world: &world,
            position: position.reborrow(),
            physics: &mut physics,
            source_entity: entity,
            aabb_query: &aabb_query,
            collidable_entity_query: &collidable_entity_query,
            vehicle_collisions: None,
            physics_state: None,
            attributes,
            abilities: None,
            active_effects: None,
            powder_snow_walkable: false,
            stands_on_lava: false,
            fall_flying: false,
            border,
            direction: *direction,
            sprinting: *sprinting,
            on_climbable: *on_climbable,
            pose: Some(*pose),
            jumping: *jumping,
            no_physics: false,
        };
        move_colliding(&mut ctx, Vec3::new(0.0, GROUND_LIFT, 0.0));
        physics.bounding_box = dimensions.make_bounding_box(**position);
    }
}

fn snapshot_box(
    query: Query<(&Position, &EntityDimensions), (With<LocalEntity>, With<HasClientLoaded>)>,
    mut spin: ResMut<Spin>,
) {
    if spin.ticks <= 0 {
        return;
    }
    if let Ok((position, dimensions)) = query.single() {
        spin.old_box = dimensions.make_bounding_box(**position);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SpinTouch {
    None,
    Stop,
    Bounce,
}

fn spin_touch(hit_living: bool, hit_any: bool, horizontal_collision: bool) -> SpinTouch {
    if hit_living {
        SpinTouch::Bounce
    } else if !hit_any && horizontal_collision {
        SpinTouch::Stop
    } else {
        SpinTouch::None
    }
}

#[allow(clippy::type_complexity)]
fn check_auto_spin_attack(
    mut players: Query<
        (
            &mut Physics,
            &Position,
            &EntityDimensions,
            &WorldName,
            Option<&TabList>,
        ),
        (With<LocalEntity>, With<HasClientLoaded>),
    >,
    others: Query<
        (
            &Position,
            &EntityDimensions,
            &WorldName,
            Has<AbstractLiving>,
            Option<&EntityUuid>,
        ),
        Without<LocalEntity>,
    >,
    mut spin: ResMut<Spin>,
) {
    if spin.ticks <= 0 {
        return;
    }
    spin.ticks -= 1;
    let Ok((mut physics, position, dimensions, world_name, tab_list)) = players.single_mut() else {
        return;
    };
    let swept = spin
        .old_box
        .minmax(&dimensions.make_bounding_box(**position));
    let spectator = |uuid: Option<&EntityUuid>| {
        uuid.zip(tab_list)
            .and_then(|(uuid, tab_list)| tab_list.get(&**uuid))
            .is_some_and(|info| info.gamemode == GameMode::Spectator)
    };
    let (mut hit_any, mut hit_living) = (false, false);
    for (other, other_dimensions, other_world, living, uuid) in &others {
        if other_world == world_name
            && other_dimensions
                .make_bounding_box(**other)
                .intersects_aabb(&swept)
            && !spectator(uuid)
        {
            hit_any = true;
            hit_living |= living;
        }
    }
    match spin_touch(hit_living, hit_any, physics.horizontal_collision) {
        SpinTouch::None => {}
        SpinTouch::Stop => spin.ticks = 0,
        SpinTouch::Bounce => {
            spin.ticks = 0;
            physics.velocity = physics.velocity * -0.2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strength_climbs_by_three_quarters_a_level() {
        assert_eq!(spin_strength(0), 0.0);
        assert_eq!(spin_strength(1), 1.5);
        assert_eq!(spin_strength(2), 2.25);
        assert_eq!(spin_strength(3), 3.0);
    }

    #[test]
    fn yaw_zero_pushes_south() {
        let push = push_for(0.0, 0.0, 3.0);
        assert!(push.x.abs() < 1e-5 && push.y.abs() < 1e-5);
        assert!((push.z - 3.0).abs() < 1e-5);
    }

    #[test]
    fn looking_straight_up_pushes_up() {
        let push = push_for(0.0, -90.0, 2.25);
        assert!(push.x.abs() < 1e-3 && push.z.abs() < 1e-3);
        assert!((push.y - 2.25).abs() < 1e-3);
    }

    #[test]
    fn use_gate() {
        for (breaks, strength, wet, want) in [
            (false, 0.0, false, true),
            (false, 0.0, true, true),
            (false, 1.5, true, true),
            (false, 1.5, false, false),
            (true, 0.0, true, false),
            (true, 1.5, true, false),
        ] {
            assert_eq!(
                may_start(breaks, strength, wet),
                want,
                "{breaks} {strength} {wet}"
            );
        }
    }

    #[test]
    fn launch_gate() {
        for (held, strength, wet, riding, breaks, want) in [
            (10, 1.5, true, false, false, true),
            (9, 1.5, true, false, false, false),
            (10, 0.0, true, false, false, false),
            (10, 1.5, false, false, false, false),
            (10, 1.5, true, true, false, false),
            (10, 1.5, true, false, true, false),
        ] {
            assert_eq!(
                may_launch(held, strength, wet, riding, breaks),
                want,
                "{held} {strength}"
            );
        }
    }

    #[test]
    fn a_wall_only_stops_a_spin_with_no_entity_in_the_box() {
        assert_eq!(spin_touch(true, true, false), SpinTouch::Bounce);
        assert_eq!(spin_touch(true, true, true), SpinTouch::Bounce);
        assert_eq!(spin_touch(false, false, true), SpinTouch::Stop);
        assert_eq!(spin_touch(false, true, true), SpinTouch::None);
        assert_eq!(spin_touch(false, false, false), SpinTouch::None);
    }
}
