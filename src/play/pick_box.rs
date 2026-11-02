use azalea::ecs::prelude::*;
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::metadata::Health;
use azalea::entity::{EntityKindComponent, LocalEntity, Position};
use azalea::interact::pick::PickBoundingBox;
use azalea_core::position::Vec3;

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
        ),
        Without<LocalEntity>,
    >,
) {
    for (entity, position, dimensions, kind, living, state, pick_box) in &mut query {
        let target = [position.x, position.y, position.z];
        let pos = match state {
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
                state.handler.interpolate(Target {
                    pos: &mut state.pos,
                    y_rot: &mut y_rot,
                    x_rot: &mut x_rot,
                });
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
        let aabb = dimensions.make_bounding_box(Vec3::new(pos[0], pos[1], pos[2]));
        match pick_box {
            Some(mut pick_box) => pick_box.0 = aabb,
            None => {
                commands.entity(entity).insert(PickBoundingBox(aabb));
            }
        }
    }
}
