use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use bevy_ecs::{
    resource::Resource,
    schedule::{ScheduleLabel, SystemSet},
};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, ScheduleLabel)]
pub struct GameTick;

#[derive(Clone, Debug, Eq, Hash, PartialEq, SystemSet)]
pub struct TickRateSystems;

#[derive(Clone, Debug, Resource)]
pub struct TickRateManager {
    tickrate: f32,
    frozen_ticks_to_run: i32,
    run_game_elements: bool,
    is_frozen: bool,
}

impl Default for TickRateManager {
    fn default() -> Self {
        TickRateManager {
            tickrate: 20.0,
            frozen_ticks_to_run: 0,
            run_game_elements: true,
            is_frozen: false,
        }
    }
}

impl TickRateManager {
    pub fn set_tick_rate(&mut self, rate: f32) {
        self.tickrate = rate.max(1.0);
    }

    pub fn tickrate(&self) -> f32 {
        self.tickrate
    }

    pub fn runs_normally(&self) -> bool {
        self.run_game_elements
    }

    pub fn set_frozen_ticks_to_run(&mut self, ticks: i32) {
        self.frozen_ticks_to_run = ticks;
    }

    pub fn set_frozen(&mut self, frozen: bool) {
        self.is_frozen = frozen;
    }

    pub fn is_frozen(&self) -> bool {
        self.is_frozen
    }

    pub fn runs_next(&self) -> bool {
        !self.is_frozen || self.frozen_ticks_to_run > 0
    }

    pub fn tick(&mut self) {
        self.run_game_elements = self.runs_next();
        if self.frozen_ticks_to_run > 0 {
            self.frozen_ticks_to_run -= 1;
        }
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub struct Frozen(pub Option<Arc<FrozenEntities>>);

impl Frozen {
    pub fn entity_ticks(&self, id: i32) -> bool {
        self.0.as_ref().is_none_or(|frozen| frozen.ticks(id))
    }
}

#[derive(Clone, Debug, Default)]
pub struct FrozenEntities {
    moving: HashSet<i32>,
    vehicle_of: HashMap<i32, i32>,
}

const MAX_CHAIN: usize = 64;

impl FrozenEntities {
    pub fn new(vehicle_of: HashMap<i32, i32>, players: impl IntoIterator<Item = i32>) -> Self {
        let mut moving = HashSet::new();
        for player in players {
            let mut id = player;
            moving.insert(id);
            for _ in 0..MAX_CHAIN {
                let Some(&vehicle) = vehicle_of.get(&id) else {
                    break;
                };
                moving.insert(vehicle);
                id = vehicle;
            }
        }
        FrozenEntities { moving, vehicle_of }
    }

    pub fn is_frozen(&self, id: i32) -> bool {
        !self.moving.contains(&id)
    }

    pub fn ticks(&self, id: i32) -> bool {
        let mut root = id;
        for _ in 0..MAX_CHAIN {
            let Some(&vehicle) = self.vehicle_of.get(&root) else {
                break;
            };
            root = vehicle;
        }
        !self.is_frozen(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_runs_its_ticks_and_freezes_again() {
        let mut rate = TickRateManager::default();
        rate.tick();
        assert!(rate.runs_normally());
        rate.set_frozen(true);
        rate.tick();
        assert!(!rate.runs_normally());
        rate.set_frozen_ticks_to_run(2);
        let ran: Vec<bool> = (0..4)
            .map(|_| {
                rate.tick();
                rate.runs_normally()
            })
            .collect();
        assert_eq!(ran, [true, true, false, false]);
    }

    #[test]
    fn a_mob_ticks_with_the_player_it_shares_a_vehicle_with() {
        let vehicles = HashMap::from([(1, 10), (2, 10), (3, 11), (11, 12), (4, 12)]);
        let frozen = FrozenEntities::new(vehicles, [1, 4, 5]);
        assert!(frozen.ticks(2) && frozen.is_frozen(2));
        assert!(frozen.ticks(10) && !frozen.is_frozen(10));
        assert!(frozen.ticks(3) && frozen.is_frozen(11));
        assert!(frozen.ticks(5) && !frozen.is_frozen(5));
        assert!(!frozen.ticks(20) && frozen.is_frozen(20));
    }
}
