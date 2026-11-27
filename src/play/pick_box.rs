use azalea::ecs::prelude::*;
use azalea::entity::dimensions::{
    EntityDimensions, RemoteScale, ShulkerPeekAmount, shulker_bounding_box,
};
use azalea::entity::metadata::{AttachFace, Health};
use azalea::entity::{EntityKindComponent, LocalEntity, Position};
use azalea::interact::pick::PickBoundingBox;
use azalea_core::entity_id::MinecraftEntityId;
use azalea_core::position::Vec3;
use azalea_core::tick::Frozen;

use crate::util::interp::{Interpolation, Target};

#[derive(Component)]
pub(crate) struct Interpolated {
    handler: Interpolation,
    pos: [f64; 3],
    target: [f64; 3],
}

#[allow(clippy::type_complexity)]
pub(crate) fn step_pick_boxes(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &Position,
            &EntityDimensions,
            &EntityKindComponent,
            Has<Health>,
            Option<&mut Interpolated>,
            Option<&mut PickBoundingBox>,
            (
                Option<&AttachFace>,
                Option<&ShulkerPeekAmount>,
                Option<&RemoteScale>,
            ),
            &MinecraftEntityId,
        ),
        Without<LocalEntity>,
    >,
    frozen: Res<Frozen>,
) {
    let new_carts = crate::client::tracking::minecart_improvements();
    for (entity, position, dimensions, kind, living, state, pick_box, shulker, id) in &mut query {
        let target = [position.x, position.y, position.z];
        let pos = match state {
            Some(_) if new_carts && crate::entities::feed::is_minecart(**kind) => {
                commands
                    .entity(entity)
                    .remove::<(Interpolated, PickBoundingBox)>();
                continue;
            }
            Some(mut state) => {
                let state = &mut *state;
                let (mut y_rot, mut x_rot) = (0.0, 0.0);
                if state.target != target {
                    state.handler.interpolate_to(
                        Target {
                            pos: &mut state.pos,
                            y_rot: &mut y_rot,
                            x_rot: &mut x_rot,
                        },
                        target,
                        0.0,
                        0.0,
                    );
                    state.target = target;
                }
                if frozen.entity_ticks(id.0) {
                    state.handler.interpolate(Target {
                        pos: &mut state.pos,
                        y_rot: &mut y_rot,
                        x_rot: &mut x_rot,
                    });
                }
                state.pos
            }
            None => {
                let Some(handler) = crate::entities::feed::interpolation_for(**kind, living) else {
                    continue;
                };
                commands.entity(entity).insert(Interpolated {
                    handler,
                    pos: target,
                    target,
                });
                target
            }
        };
        let at = Vec3::new(pos[0], pos[1], pos[2]);
        let aabb = match shulker {
            (Some(face), peek, scale) => shulker_bounding_box(
                scale.map_or(1.0, |s| s.0),
                face.0,
                peek.map_or(0.0, |p| p.0),
                at,
            ),
            _ => dimensions.make_bounding_box(at),
        };
        match pick_box {
            Some(mut pick_box) => pick_box.0 = aabb,
            None => {
                commands.entity(entity).insert(PickBoundingBox(aabb));
            }
        }
    }
}
