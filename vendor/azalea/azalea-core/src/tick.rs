use bevy_ecs::schedule::ScheduleLabel;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, ScheduleLabel)]
pub struct GameTick;
