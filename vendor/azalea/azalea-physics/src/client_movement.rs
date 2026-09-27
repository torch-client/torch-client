use azalea_core::position::Vec2;
use bevy_ecs::component::Component;

#[derive(Clone, Component, Default)]
pub struct ClientMovementState {
    pub position_remainder: u32,
    pub was_sprinting: bool,
    pub trying_to_sprint: bool,

    pub trying_to_crouch: bool,

    pub move_direction: WalkDirection,
    pub move_vector: Vec2,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WalkDirection {
    #[default]
    None,
    Forward,
    Backward,
    Left,
    Right,
    ForwardRight,
    ForwardLeft,
    BackwardRight,
    BackwardLeft,
}
impl WalkDirection {
    pub fn forward(self) -> bool {
        DirectionStates::from(self).forward
    }
    pub fn backward(self) -> bool {
        DirectionStates::from(self).backward
    }
    pub fn left(self) -> bool {
        DirectionStates::from(self).left
    }
    pub fn right(self) -> bool {
        DirectionStates::from(self).right
    }

    pub fn set_forward(&mut self, value: bool) {
        let mut d = DirectionStates::from(*self);
        d.forward = value;
        *self = d.into();
    }
    pub fn set_backward(&mut self, value: bool) {
        let mut d = DirectionStates::from(*self);
        d.backward = value;
        *self = d.into();
    }
    pub fn set_left(&mut self, value: bool) {
        let mut d = DirectionStates::from(*self);
        d.left = value;
        *self = d.into();
    }
    pub fn set_right(&mut self, value: bool) {
        let mut d = DirectionStates::from(*self);
        d.right = value;
        *self = d.into();
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Forward => Self::Backward,
            Self::Backward => Self::Forward,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::ForwardRight => Self::BackwardLeft,
            Self::ForwardLeft => Self::BackwardRight,
            Self::BackwardRight => Self::ForwardLeft,
            Self::BackwardLeft => Self::ForwardRight,
        }
    }
}
#[derive(Default)]
pub struct DirectionStates {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
}
impl From<WalkDirection> for DirectionStates {
    fn from(d: WalkDirection) -> Self {
        let mut s = Self::default();
        match d {
            WalkDirection::None => {}
            WalkDirection::Forward => s.forward = true,
            WalkDirection::Backward => s.backward = true,
            WalkDirection::Left => s.left = true,
            WalkDirection::Right => s.right = true,
            WalkDirection::ForwardRight => {
                s.forward = true;
                s.right = true
            }
            WalkDirection::ForwardLeft => {
                s.forward = true;
                s.left = true
            }
            WalkDirection::BackwardRight => {
                s.backward = true;
                s.right = true
            }
            WalkDirection::BackwardLeft => {
                s.forward = true;
                s.left = true
            }
        };
        s
    }
}
impl From<DirectionStates> for WalkDirection {
    fn from(d: DirectionStates) -> Self {
        let left = d.left && !d.right;
        let right = d.right && !d.left;

        if d.forward && !d.backward {
            if right {
                return Self::ForwardRight;
            } else if left {
                return Self::ForwardLeft;
            }
            return Self::Forward;
        } else if d.backward && !d.forward {
            if right {
                return Self::BackwardRight;
            } else if left {
                return Self::BackwardLeft;
            }
            return Self::Backward;
        }
        if right {
            return Self::Right;
        } else if left {
            return Self::Left;
        }
        Self::None
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SprintDirection {
    Forward,
    ForwardRight,
    ForwardLeft,
}

impl From<SprintDirection> for WalkDirection {
    fn from(d: SprintDirection) -> Self {
        match d {
            SprintDirection::Forward => WalkDirection::Forward,
            SprintDirection::ForwardRight => WalkDirection::ForwardRight,
            SprintDirection::ForwardLeft => WalkDirection::ForwardLeft,
        }
    }
}
