use azalea_registry::builtin::{BlockEntityKind, EntityKind};
use simdnbt::owned::{Nbt, NbtCompound};

use super::TickCtx;
use crate::blockentities::feed::BlockEntityData;
use crate::util::mth;

#[derive(Clone, Debug, PartialEq)]
pub struct Data {
    pub display: Option<EntityKind>,
    pub delay: i32,
    pub min_delay: i32,
    pub range: i32,
    pub trial: bool,
}

impl Default for Data {
    fn default() -> Self {
        Data {
            display: None,
            delay: 20,
            min_delay: 200,
            range: 16,
            trial: false,
        }
    }
}

pub fn decode(kind: BlockEntityKind, tag: &Nbt) -> Data {
    if kind == BlockEntityKind::TrialSpawner {
        return Data {
            display: spawn_data(tag, "spawn_data"),
            trial: true,
            ..Data::default()
        };
    }
    let defaults = Data::default();
    Data {
        display: spawn_data(tag, "SpawnData"),
        delay: tag.short("Delay").map_or(defaults.delay, i32::from),
        min_delay: tag
            .short("MinSpawnDelay")
            .map_or(defaults.min_delay, i32::from),
        range: tag
            .short("RequiredPlayerRange")
            .map_or(defaults.range, i32::from),
        trial: false,
    }
}

fn spawn_data(tag: &NbtCompound, key: &str) -> Option<EntityKind> {
    tag.compound(key)?
        .compound("entity")?
        .string("id")?
        .to_str()
        .parse()
        .ok()
}

pub fn trial_spin_speed(state: &str) -> Option<f64> {
    match state {
        "waiting_for_players" => Some(200.0),
        "active" => Some(1000.0),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spin {
    o_spin: f64,
    spin: f64,
    delay: i32,
    min_delay: i32,
}

impl Spin {
    pub fn on_data(&mut self, data: &BlockEntityData) {
        if let BlockEntityData::Spawner(data) = data {
            self.delay = data.delay;
            self.min_delay = data.min_delay;
        }
    }

    pub fn event(&mut self, action: u8) {
        if action == 1 {
            self.delay = self.min_delay;
        }
    }

    pub fn tick(&mut self, ctx: &TickCtx) {
        let BlockEntityData::Spawner(data) = ctx.data else {
            return;
        };
        if data.trial {
            let state = ctx.state.prop("trial_spawner_state");
            if let Some(speed) = trial_spin_speed(state) {
                self.advance(speed / 200.0);
            }
            if matches!(state, "waiting_for_players" | "active") {
                let event = if ctx.state.prop("ominous") == "true" {
                    "block.trial_spawner.ambient_ominous"
                } else {
                    "block.trial_spawner.ambient"
                };
                super::idle_sound(ctx, event);
            }
        } else if !ctx.player_within(ctx.center(), f64::from(data.range)) {
            self.o_spin = self.spin;
        } else if data.display.is_some() {
            self.delay = (self.delay - 1).max(0);
            self.advance(f64::from(1000.0_f32 / (self.delay as f32 + 200.0)));
        }
    }

    fn advance(&mut self, step: f64) {
        self.o_spin = self.spin;
        self.spin = (self.spin + step) % 360.0;
    }

    pub fn angle(&self, partial: f32) -> f32 {
        mth::lerp_f64(partial, self.o_spin, self.spin) as f32 * 10.0
    }
}

#[cfg(test)]
mod tests {
    use simdnbt::owned::NbtTag;

    use super::*;
    use crate::blockentities::sim::test_nbt::{compound, root, string};

    fn with_spawn_data(key: &'static str, id: &str, mut rest: Vec<(&'static str, NbtTag)>) -> Nbt {
        let entity = compound(vec![("id", string(id))]);
        rest.push((
            key,
            NbtTag::Compound(compound(vec![("entity", NbtTag::Compound(entity))])),
        ));
        root(rest)
    }

    #[test]
    fn a_mob_spawner_reads_its_display_entity_and_timers() {
        let tag = with_spawn_data(
            "SpawnData",
            "minecraft:zombie",
            vec![
                ("Delay", NbtTag::Short(77)),
                ("MinSpawnDelay", NbtTag::Short(100)),
                ("RequiredPlayerRange", NbtTag::Short(8)),
            ],
        );
        let data = decode(BlockEntityKind::MobSpawner, &tag);
        assert_eq!(data.display, Some(EntityKind::Zombie));
        assert_eq!((data.delay, data.min_delay, data.range), (77, 100, 8));
        assert!(!data.trial);
    }

    #[test]
    fn a_spawner_without_spawn_data_has_vanilla_defaults() {
        let data = decode(BlockEntityKind::MobSpawner, &root(vec![]));
        assert_eq!(data, Data::default());
        assert_eq!((data.delay, data.min_delay, data.range), (20, 200, 16));
    }

    #[test]
    fn a_trial_spawner_reads_spawn_data() {
        let tag = with_spawn_data("spawn_data", "minecraft:breeze", vec![]);
        let data = decode(BlockEntityKind::TrialSpawner, &tag);
        assert_eq!(data.display, Some(EntityKind::Breeze));
        assert!(data.trial);
    }

    #[test]
    fn only_two_trial_states_spin() {
        assert_eq!(trial_spin_speed("active"), Some(1000.0));
        assert_eq!(trial_spin_speed("waiting_for_players"), Some(200.0));
        assert_eq!(trial_spin_speed("cooldown"), None);
    }

    #[test]
    fn the_spin_wraps_and_lerps_in_tens_of_degrees() {
        let mut spin = Spin::default();
        spin.spin = 359.0;
        spin.advance(2.0);
        assert!((spin.spin - 1.0).abs() < 1e-9);
        let mut spin = Spin::default();
        spin.advance(4.0);
        assert!((spin.angle(0.5) - 20.0).abs() < 1e-4);
    }

    #[test]
    fn event_one_restarts_the_delay() {
        let mut spin = Spin {
            delay: 3,
            min_delay: 200,
            ..Spin::default()
        };
        spin.event(2);
        assert_eq!(spin.delay, 3);
        spin.event(1);
        assert_eq!(spin.delay, 200);
    }
}
