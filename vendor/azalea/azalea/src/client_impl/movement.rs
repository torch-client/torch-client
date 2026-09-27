use azalea_client::{
    ClientMovementState, SprintDirection, StartSprintEvent, StartWalkEvent, WalkDirection,
};
use azalea_entity::{Jumping, LookDirection};

use crate::{Client, client_impl::error::AzaleaResult};

impl Client {
    pub fn set_jumping(&self, jumping: bool) -> AzaleaResult<()> {
        self.query_self::<&mut Jumping, _>(|mut j| **j = jumping)
    }

    pub fn jumping(&self) -> bool {
        self.component::<Jumping>().map(|j| **j).unwrap_or_default()
    }

    pub fn set_crouching(&self, crouching: bool) -> AzaleaResult<()> {
        self.query_self::<&mut ClientMovementState, _>(|mut p| p.trying_to_crouch = crouching)
    }

    pub fn crouching(&self) -> bool {
        self.query_self::<&ClientMovementState, _>(|p| p.trying_to_crouch)
            .unwrap_or(false)
    }

    pub fn set_direction(&self, y_rot: f32, x_rot: f32) -> AzaleaResult<()> {
        self.query_self::<&mut LookDirection, _>(|mut ld| {
            ld.update(LookDirection::new(y_rot, x_rot));
        })
    }

    pub fn direction(&self) -> AzaleaResult<LookDirection> {
        Ok(*self.component::<LookDirection>()?)
    }

    pub fn walk(&self, direction: WalkDirection) {
        let mut ecs = self.ecs.write();
        ecs.write_message(StartWalkEvent {
            entity: self.entity,
            direction,
        });
    }

    pub fn movement_state(&self) -> AzaleaResult<ClientMovementState> {
        Ok(self.component::<ClientMovementState>()?.clone())
    }

    pub fn sprint(&self, direction: SprintDirection) {
        let mut ecs = self.ecs.write();
        ecs.write_message(StartSprintEvent {
            entity: self.entity,
            direction,
        });
    }
}
