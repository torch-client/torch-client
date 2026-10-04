use std::sync::OnceLock;

use crate::protocol::version::ProtocolVersion;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum IdSpace {
    Attribute,
    Block,
    BlockEntityType,
    BlockState,
    DataComponentType,
    EntityDataSerializer,
    EntityType,
    GameEvent,
    Item,
    Menu,
    MobEffect,
    ParticleType,
    Potion,
    SoundEvent,
    VillagerProfession,
    VillagerType,
}

impl IdSpace {
    pub(crate) const ALL: [IdSpace; 16] = [
        IdSpace::Attribute,
        IdSpace::Block,
        IdSpace::BlockEntityType,
        IdSpace::BlockState,
        IdSpace::DataComponentType,
        IdSpace::EntityDataSerializer,
        IdSpace::EntityType,
        IdSpace::GameEvent,
        IdSpace::Item,
        IdSpace::Menu,
        IdSpace::MobEffect,
        IdSpace::ParticleType,
        IdSpace::Potion,
        IdSpace::SoundEvent,
        IdSpace::VillagerProfession,
        IdSpace::VillagerType,
    ];

    fn key(self) -> &'static str {
        match self {
            IdSpace::Attribute => "attribute",
            IdSpace::Block => "block",
            IdSpace::BlockEntityType => "block_entity_type",
            IdSpace::BlockState => "block_state",
            IdSpace::DataComponentType => "data_component_type",
            IdSpace::EntityDataSerializer => "entity_data_serializer",
            IdSpace::EntityType => "entity_type",
            IdSpace::GameEvent => "game_event",
            IdSpace::Item => "item",
            IdSpace::Menu => "menu",
            IdSpace::MobEffect => "mob_effect",
            IdSpace::ParticleType => "particle_type",
            IdSpace::Potion => "potion",
            IdSpace::SoundEvent => "sound_event",
            IdSpace::VillagerProfession => "villager_profession",
            IdSpace::VillagerType => "villager_type",
        }
    }
}

struct Runs {
    runs: Vec<(i32, Option<i32>)>,
    count: i32,
}

impl Runs {
    fn get(&self, id: i32) -> Option<i32> {
        if id < 0 || id >= self.count {
            return None;
        }
        let slot = self.runs.partition_point(|(start, _)| *start <= id);
        Some(id + self.runs[slot - 1].1?)
    }

    fn is_identity(&self) -> bool {
        self.runs.iter().all(|(_, offset)| *offset == Some(0))
    }
}

pub(crate) struct Remap {
    from: ProtocolVersion,
    to: ProtocolVersion,
    spaces: Vec<Runs>,
}

impl Remap {
    pub(crate) fn map(&self, space: IdSpace, id: i32) -> Option<i32> {
        self.spaces[space as usize].get(id)
    }

    pub(crate) fn is_identity(&self, space: IdSpace) -> bool {
        self.spaces[space as usize].is_identity()
    }

    pub(crate) fn from(&self) -> ProtocolVersion {
        self.from
    }

    pub(crate) fn to(&self) -> ProtocolVersion {
        self.to
    }

    pub(crate) fn to_native() -> &'static Remap {
        &pair().0
    }

    pub(crate) fn from_native() -> &'static Remap {
        &pair().1
    }

    fn parse(json: &str) -> Result<(Remap, Remap), String> {
        let file: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let version = |key: &str| -> Result<ProtocolVersion, String> {
            let name = file[key]["version"]
                .as_str()
                .ok_or_else(|| format!("{key}.version is missing"))?;
            ProtocolVersion::from_name(name).ok_or_else(|| format!("unknown version {name}"))
        };
        let from = version("from")?;
        let to = version("to")?;

        let direction = |field: &str| -> Result<Vec<Runs>, String> {
            let mut out = Vec::with_capacity(IdSpace::ALL.len());
            for space in IdSpace::ALL {
                let key = space.key();
                let table = &file[field][key];
                let count = table["count"]
                    .as_i64()
                    .and_then(|n| i32::try_from(n).ok())
                    .ok_or_else(|| format!("{field}.{key}.count is missing"))?;
                let pairs = table["runs"]
                    .as_array()
                    .ok_or_else(|| format!("{field}.{key}.runs is missing"))?;

                let mut runs = Vec::with_capacity(pairs.len());
                for pair in pairs {
                    let start = pair[0]
                        .as_i64()
                        .and_then(|n| i32::try_from(n).ok())
                        .ok_or_else(|| format!("{field}.{key} holds a malformed start"))?;
                    let offset = match &pair[1] {
                        serde_json::Value::Null => None,
                        value => Some(
                            value
                                .as_i64()
                                .and_then(|n| i32::try_from(n).ok())
                                .ok_or_else(|| format!("{field}.{key} holds a malformed offset"))?,
                        ),
                    };
                    runs.push((start, offset));
                }
                if runs.first().map(|(start, _)| *start) != Some(0) {
                    return Err(format!("{field}.{key} does not start at id 0"));
                }
                if runs.windows(2).any(|w| w[0].0 >= w[1].0) {
                    return Err(format!("{field}.{key} is not sorted"));
                }
                out.push(Runs { runs, count });
            }
            Ok(out)
        };

        Ok((
            Remap {
                from,
                to,
                spaces: direction("forward")?,
            },
            Remap {
                from: to,
                to: from,
                spaces: direction("back")?,
            },
        ))
    }
}

fn pair() -> &'static (Remap, Remap) {
    static PAIR: OnceLock<(Remap, Remap)> = OnceLock::new();
    PAIR.get_or_init(|| {
        Remap::parse(include_str!("data/remap-774-775.json"))
            .unwrap_or_else(|e| panic!("embedded 774 -> 775 remap: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_774_identities() {
        let r = Remap::to_native();
        for space in [
            IdSpace::Attribute,
            IdSpace::BlockEntityType,
            IdSpace::EntityType,
            IdSpace::GameEvent,
            IdSpace::Menu,
            IdSpace::MobEffect,
            IdSpace::Potion,
            IdSpace::VillagerProfession,
            IdSpace::VillagerType,
        ] {
            assert!(
                r.is_identity(space),
                "{space:?} was expected to be identity"
            );
        }
        for space in [
            IdSpace::Block,
            IdSpace::BlockState,
            IdSpace::DataComponentType,
            IdSpace::EntityDataSerializer,
            IdSpace::Item,
            IdSpace::ParticleType,
            IdSpace::SoundEvent,
        ] {
            assert!(!r.is_identity(space), "{space:?} was expected to shift");
        }
    }

    #[test]
    fn the_block_runs_follow_azaleas_registry() {
        use azalea_registry::Registry as _;
        use azalea_registry::builtin::BlockKind;

        let inserted = [
            BlockKind::GoldenDandelion.to_u32() as i32,
            BlockKind::PottedGoldenDandelion.to_u32() as i32,
        ];
        assert_eq!(inserted, [158, 424], "775 moved a block this remap pins");

        let forward = Remap::to_native();
        let back = Remap::from_native();
        for id in inserted {
            assert_eq!(back.map(IdSpace::Block, id), None);
        }
        for (wire, native) in [(157, 157), (158, 159), (422, 423), (423, 425)] {
            assert_eq!(forward.map(IdSpace::Block, wire), Some(native));
            assert_eq!(back.map(IdSpace::Block, native), Some(wire));
        }
        let last = (0u32..)
            .take_while(|id| BlockKind::is_valid_id(*id))
            .last()
            .expect("the block registry is not empty") as i32;
        assert_eq!(
            back.map(IdSpace::Block, last),
            Some(last - 2),
            "775's last block is 774's last block"
        );
        assert_eq!(
            forward.map(IdSpace::Block, last - 1),
            None,
            "774 has two fewer blocks, so its ids stop two short"
        );
    }

    #[test]
    fn the_endpoints_are_the_two_versions() {
        let r = Remap::to_native();
        assert_eq!(r.from().protocol, 774);
        assert_eq!(r.to().protocol, super::super::version::NATIVE.protocol);
        assert_eq!(Remap::from_native().from().protocol, 775);
        assert_eq!(Remap::from_native().to().protocol, 774);
    }

    #[test]
    fn known_boundaries() {
        let r = Remap::to_native();
        assert_eq!(r.map(IdSpace::BlockState, 0), Some(0));
        assert_eq!(r.map(IdSpace::BlockState, 1380), Some(1380));
        assert_eq!(r.map(IdSpace::BlockState, 1381), Some(1581));
        assert_eq!(r.map(IdSpace::BlockState, 2122), Some(2323));
        assert_eq!(r.map(IdSpace::BlockState, 10441), Some(10643));
        assert_eq!(r.map(IdSpace::Item, 229), Some(229));
        assert_eq!(r.map(IdSpace::Item, 230), Some(231));
        assert_eq!(r.map(IdSpace::EntityDataSerializer, 21), Some(21));
        assert_eq!(r.map(IdSpace::EntityDataSerializer, 22), Some(23));
        assert_eq!(r.map(IdSpace::EntityDataSerializer, 28), Some(32));
        assert_eq!(r.map(IdSpace::DataComponentType, 40), Some(40));
        assert_eq!(r.map(IdSpace::DataComponentType, 41), Some(42));
    }

    #[test]
    fn an_id_out_of_range_does_not_map() {
        let r = Remap::to_native();
        assert_eq!(r.map(IdSpace::BlockState, 29670), Some(29872));
        assert_eq!(r.map(IdSpace::BlockState, 29671), None);
        assert_eq!(r.map(IdSpace::BlockState, -1), None);
        assert_eq!(r.map(IdSpace::Item, 1505), None);
    }

    #[test]
    fn every_id_round_trips() {
        let forward = Remap::to_native();
        let back = Remap::from_native();
        for space in IdSpace::ALL {
            let count = forward.spaces[space as usize].count;
            for id in 0..count {
                let there = forward
                    .map(space, id)
                    .unwrap_or_else(|| panic!("{space:?} id {id} does not map"));
                assert_eq!(
                    back.map(space, there),
                    Some(id),
                    "{space:?} id {id} -> {there} did not come back"
                );
            }
        }
    }

    #[test]
    fn no_two_ids_share_a_target() {
        let r = Remap::to_native();
        for space in IdSpace::ALL {
            let count = r.spaces[space as usize].count;
            let mut taken = std::collections::HashMap::new();
            for id in 0..count {
                let there = r.map(space, id).unwrap();
                if let Some(first) = taken.insert(there, id) {
                    panic!("{space:?}: ids {first} and {id} both land on {there}");
                }
            }
        }
    }

    #[test]
    fn sound_events_were_reordered_not_just_inserted() {
        let r = Remap::to_native();
        assert_eq!(r.map(IdSpace::SoundEvent, 280), Some(291));
        assert_eq!(
            r.map(IdSpace::SoundEvent, 281),
            Some(288),
            "281 lands below 280, which is what makes the map non-monotonic"
        );
        let count = r.spaces[IdSpace::SoundEvent as usize].count;
        let descents = (1..count)
            .filter(|id| r.map(IdSpace::SoundEvent, *id) <= r.map(IdSpace::SoundEvent, id - 1))
            .count();
        assert_eq!(descents, 8);
    }

    #[test]
    fn an_id_775_added_has_nowhere_to_go_back_to() {
        let back = Remap::from_native();
        assert_eq!(back.spaces[IdSpace::SoundEvent as usize].count, 1902);
        assert_eq!(back.spaces[IdSpace::BlockState as usize].count, 29_873);
        assert_eq!(back.map(IdSpace::BlockState, 29_872), Some(29_670));
        assert_eq!(back.map(IdSpace::BlockState, 29_873), None);
        assert_eq!(back.map(IdSpace::Item, 230), None, "the item 775 inserted");
        assert_eq!(back.map(IdSpace::Item, 231), Some(230));
    }

    #[test]
    fn what_775_added_that_774_cannot_receive() {
        let back = Remap::from_native();
        let missing = |space: IdSpace| {
            (0..back.spaces[space as usize].count)
                .filter(|id| back.map(space, *id).is_none())
                .count()
        };
        assert_eq!(missing(IdSpace::SoundEvent), 1902 - 1838);
        assert_eq!(missing(IdSpace::Item), 1);
        assert_eq!(missing(IdSpace::DataComponentType), 110 - 104);
        assert_eq!(missing(IdSpace::EntityDataSerializer), 4);
        assert_eq!(missing(IdSpace::BlockState), 29_873 - 29_671);
        assert_eq!(missing(IdSpace::EntityType), 0, "identity has no holes");
    }
}
