use std::sync::Mutex;

use azalea::ecs::prelude::*;
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::indexing::EntityIdIndex;
use azalea::entity::metadata::AbstractMinecart;
use azalea::entity::{EntityKindComponent, LocalEntity, LookDirection, Physics, Position};
use azalea_core::entity_id::MinecraftEntityId;
use azalea_core::position::{BlockPos, Vec3};
use azalea_core::tick::Frozen;
use azalea_registry::builtin::EntityKind;

use crate::client::tracking::{cart_lerps, local_vehicle_id, minecart_improvements};
use crate::util::minecart_lerp::{CartRide, Step};

#[derive(Clone, Debug)]
pub(crate) struct LocalCart {
    pub ride: CartRide,
    pub plain: bool,
}

static LOCAL_CART: Mutex<Option<LocalCart>> = Mutex::new(None);

pub(crate) fn local_ride() -> Option<LocalCart> {
    LOCAL_CART.lock().unwrap().clone()
}

fn arr(v: Vec3) -> [f64; 3] {
    [v.x, v.y, v.z]
}

fn vec(a: [f64; 3]) -> Vec3 {
    Vec3::new(a[0], a[1], a[2])
}

#[allow(clippy::type_complexity)]
pub(super) fn tick_cart_lerps(
    local: Query<&EntityIdIndex, With<LocalEntity>>,
    mut carts: Query<
        (
            &EntityKindComponent,
            &mut Position,
            &mut Physics,
            &mut LookDirection,
            &EntityDimensions,
        ),
        (With<AbstractMinecart>, Without<LocalEntity>),
    >,
    frozen: Res<Frozen>,
) {
    let mut ride = None;
    if minecart_improvements()
        && let Ok(index) = local.single()
    {
        let vehicle = local_vehicle_id();
        let mut lerps = cart_lerps().lock().unwrap();
        lerps.retain(|&id, lerp| {
            let Some(entity) = index.get_by_minecraft_entity(MinecraftEntityId(id)) else {
                return false;
            };
            let Ok((kind, mut position, mut physics, mut look, dimensions)) = carts.get_mut(entity)
            else {
                return false;
            };
            if !frozen.entity_ticks(id) {
                return true;
            }
            let before = **position;
            let now = Step {
                pos: arr(before),
                movement: arr(physics.velocity),
                y_rot: look.y_rot(),
                x_rot: look.x_rot(),
                weight: 0.0,
            };
            if let Some(step) = lerp.tick(now) {
                let pos = vec(step.pos);
                **position = pos;
                physics.set_old_pos(Position::new(before));
                physics.velocity = vec(step.movement);
                physics.bounding_box = dimensions.make_bounding_box(pos);
                *look = LookDirection::new(step.y_rot, step.x_rot);
            }
            if vehicle == Some(id) {
                ride = Some(LocalCart {
                    ride: CartRide {
                        view: lerp.view().clone(),
                        pos_old: arr(before),
                        pos: arr(**position),
                    },
                    plain: **kind == EntityKind::Minecart,
                });
            }
            true
        });
    } else if !minecart_improvements() {
        if let Ok(mut lerps) = cart_lerps().try_lock() {
            lerps.clear();
        }
    }
    *LOCAL_CART.lock().unwrap() = ride;
}

#[cfg_attr(not(feature = "audio"), allow(dead_code))]
pub(crate) fn on_rails(level: &azalea_world::World, pos: [f64; 3]) -> bool {
    let rail = |y: f64| {
        let at = BlockPos::new(
            pos[0].floor() as i32,
            y.floor() as i32,
            pos[2].floor() as i32,
        );
        level
            .get_block_state(at)
            .and_then(crate::util::rail::shape_of)
            .is_some()
    };
    rail(pos[1] - 0.1 - 9.999999747378752E-6) || rail(pos[1])
}
