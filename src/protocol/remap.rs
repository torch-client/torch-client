use std::sync::OnceLock;

use crate::protocol::version::{NATIVE, ProtocolVersion};

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

    pub(crate) fn of_registry(path: &str) -> Option<IdSpace> {
        IdSpace::ALL
            .into_iter()
            .filter(|space| !matches!(space, IdSpace::BlockState | IdSpace::EntityDataSerializer))
            .find(|space| space.key() == path)
    }

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

    fn end(&self, i: usize) -> i32 {
        self.runs.get(i + 1).map_or(self.count, |(start, _)| *start)
    }

    fn run_at(&self, id: i32) -> (Option<i32>, i32) {
        if id < 0 {
            return (None, 0);
        }
        if id >= self.count {
            return (None, i32::MAX);
        }
        let slot = self.runs.partition_point(|(start, _)| *start <= id) - 1;
        (self.runs[slot].1, self.end(slot))
    }

    fn then(&self, next: &Runs) -> Runs {
        let mut runs: Vec<(i32, Option<i32>)> =
            Vec::with_capacity(self.runs.len() + next.runs.len());
        let mut push = |start: i32, offset: Option<i32>| {
            if runs.last().map(|(_, last)| *last) != Some(offset) {
                runs.push((start, offset));
            }
        };
        for (i, &(start, offset)) in self.runs.iter().enumerate() {
            let end = self.end(i);
            let Some(offset) = offset else {
                push(start, None);
                continue;
            };
            let mut id = start;
            while id < end {
                let (middle, middle_end) = next.run_at(id + offset);
                let stop = middle_end.saturating_sub(offset).min(end);
                push(id, middle.map(|o| offset + o));
                id = stop;
            }
        }
        Runs {
            runs,
            count: self.count,
        }
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

    pub(crate) fn compose(a: &Remap, b: &Remap) -> Remap {
        assert_eq!(
            a.to.protocol, b.from.protocol,
            "composing {} -> {} with {} -> {}",
            a.from.name, a.to.name, b.from.name, b.to.name
        );
        Remap {
            from: a.from,
            to: b.to,
            spaces: a
                .spaces
                .iter()
                .zip(&b.spaces)
                .map(|(first, second)| first.then(second))
                .collect(),
        }
    }

    pub(crate) fn between(from: i32, to: i32) -> Option<&'static Remap> {
        static FROM_774: OnceLock<Remap> = OnceLock::new();
        static TO_774: OnceLock<Remap> = OnceLock::new();
        let adjacent = |from, to| Remap::between(from, to).expect("an embedded pair");
        match (from, to) {
            (774, 775) => Some(&pair_774_775().0),
            (775, 774) => Some(&pair_774_775().1),
            (775, 776) => Some(&pair_775_776().0),
            (776, 775) => Some(&pair_775_776().1),
            (774, 776) => Some(
                FROM_774.get_or_init(|| Remap::compose(adjacent(774, 775), adjacent(775, 776))),
            ),
            (776, 774) => {
                Some(TO_774.get_or_init(|| Remap::compose(adjacent(776, 775), adjacent(775, 774))))
            }
            _ => None,
        }
    }

    pub(crate) fn to_native(wire: i32) -> Option<&'static Remap> {
        Remap::between(wire, NATIVE.protocol)
    }

    pub(crate) fn from_native(wire: i32) -> Option<&'static Remap> {
        Remap::between(NATIVE.protocol, wire)
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

fn pair_774_775() -> &'static (Remap, Remap) {
    static PAIR: OnceLock<(Remap, Remap)> = OnceLock::new();
    PAIR.get_or_init(|| {
        Remap::parse(include_str!("data/remap-774-775.json"))
            .unwrap_or_else(|e| panic!("embedded 774 -> 775 remap: {e}"))
    })
}

fn pair_775_776() -> &'static (Remap, Remap) {
    static PAIR: OnceLock<(Remap, Remap)> = OnceLock::new();
    PAIR.get_or_init(|| {
        Remap::parse(include_str!("data/remap-775-776.json"))
            .unwrap_or_else(|e| panic!("embedded 775 -> 776 remap: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(from: i32, to: i32) -> &'static Remap {
        Remap::between(from, to).unwrap_or_else(|| panic!("no {from} -> {to} table"))
    }

    fn count(r: &Remap, space: IdSpace) -> i32 {
        r.spaces[space as usize].count
    }

    #[test]
    fn the_774_identities() {
        let r = pair(774, 775);
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
    fn the_775_identities() {
        let r = pair(775, 776);
        for space in [
            IdSpace::EntityDataSerializer,
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
            IdSpace::Attribute,
            IdSpace::Block,
            IdSpace::BlockEntityType,
            IdSpace::BlockState,
            IdSpace::DataComponentType,
            IdSpace::EntityType,
            IdSpace::GameEvent,
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
        assert_eq!(
            inserted,
            [158, 424],
            "a version moved a block this remap pins"
        );

        let forward = pair(774, 775);
        let back = pair(775, 774);
        for id in inserted {
            assert_eq!(back.map(IdSpace::Block, id), None);
        }
        for (wire, native) in [(157, 157), (158, 159), (422, 423), (423, 425)] {
            assert_eq!(forward.map(IdSpace::Block, wire), Some(native));
            assert_eq!(back.map(IdSpace::Block, native), Some(wire));
        }
        let last = count(back, IdSpace::Block) - 1;
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
    fn the_26_2_block_insertion_follows_azaleas_registry() {
        use azalea_registry::Registry as _;
        use azalea_registry::builtin::BlockKind;

        let tuff = BlockKind::ChiseledTuffBricks.to_u32() as i32;
        let sulfur = BlockKind::Sulfur.to_u32() as i32;
        assert_eq!((tuff, sulfur), (997, 998));

        let forward = pair(775, 776);
        let back = pair(776, 775);
        assert_eq!(
            forward.map(IdSpace::Block, tuff),
            Some(tuff),
            "below the seam"
        );
        assert_eq!(back.map(IdSpace::Block, sulfur), None, "26.2's own block");
        assert_eq!(
            forward.map(IdSpace::Block, 998),
            Some(1025),
            "775's calcite lands past the 27 blocks inserted after it"
        );
        let last = (0u32..)
            .take_while(|id| BlockKind::is_valid_id(*id))
            .last()
            .expect("the block registry is not empty") as i32;
        assert_eq!(last, 1195, "26.2 has 1196 blocks");
        assert_eq!(
            back.map(IdSpace::Block, last),
            Some(1167),
            "and 775's last is its last"
        );
    }

    #[test]
    fn the_endpoints_are_the_two_versions() {
        for (from, to) in [
            (774, 775),
            (775, 774),
            (775, 776),
            (776, 775),
            (774, 776),
            (776, 774),
        ] {
            let r = pair(from, to);
            assert_eq!((r.from().protocol, r.to().protocol), (from, to));
        }
        assert_eq!(
            Remap::to_native(774).map(|r| r.to().protocol),
            Some(NATIVE.protocol)
        );
        assert_eq!(
            Remap::from_native(775).map(|r| r.from().protocol),
            Some(NATIVE.protocol)
        );
        assert!(Remap::between(773, 776).is_none());
        assert!(Remap::between(776, 776).is_none());
    }

    #[test]
    fn known_boundaries() {
        let r = pair(774, 775);
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
    fn known_775_boundaries() {
        let r = pair(775, 776);
        assert_eq!(r.map(IdSpace::BlockState, 24686), Some(24686));
        assert_eq!(r.map(IdSpace::BlockState, 24687), Some(27160));
        assert_eq!(r.map(IdSpace::BlockState, 29872), Some(32365));
        assert_eq!(r.map(IdSpace::Item, 25), Some(25));
        assert_eq!(r.map(IdSpace::Item, 26), Some(53));
        assert_eq!(r.map(IdSpace::Attribute, 0), Some(1));
        assert_eq!(r.map(IdSpace::Attribute, 5), Some(7));
        assert_eq!(r.map(IdSpace::Attribute, 34), Some(39));
        assert_eq!(r.map(IdSpace::DataComponentType, 77), Some(77));
        assert_eq!(r.map(IdSpace::DataComponentType, 78), Some(79));
        assert_eq!(r.map(IdSpace::EntityType, 129), Some(129));
        assert_eq!(r.map(IdSpace::EntityType, 130), Some(131));
        assert_eq!(r.map(IdSpace::ParticleType, 3), Some(3));
        assert_eq!(r.map(IdSpace::ParticleType, 4), Some(11));
        assert_eq!(r.map(IdSpace::SoundEvent, 227), Some(227));
        assert_eq!(r.map(IdSpace::SoundEvent, 228), Some(229));
        assert_eq!(r.map(IdSpace::SoundEvent, 1605), Some(1615));
        assert_eq!(r.map(IdSpace::BlockEntityType, 24), Some(24));
        assert_eq!(r.map(IdSpace::BlockEntityType, 25), None);
        assert_eq!(r.map(IdSpace::BlockEntityType, 26), Some(25));
    }

    #[test]
    fn an_id_out_of_range_does_not_map() {
        let r = pair(774, 775);
        assert_eq!(r.map(IdSpace::BlockState, 29670), Some(29872));
        assert_eq!(r.map(IdSpace::BlockState, 29671), None);
        assert_eq!(r.map(IdSpace::BlockState, -1), None);
        assert_eq!(r.map(IdSpace::Item, 1505), None);
    }

    fn is_the_bed(from: i32, to: i32, space: IdSpace, id: i32) -> bool {
        to == 776 && matches!(from, 774 | 775) && space == IdSpace::BlockEntityType && id == 25
    }

    #[test]
    fn the_bed_block_entity_has_nowhere_to_go() {
        use azalea_registry::Registry as _;
        use azalea_registry::builtin::BlockEntityKind;
        for from in [774, 775] {
            let r = pair(from, 776);
            assert_eq!(r.map(IdSpace::BlockEntityType, 25), None);
            assert_eq!(
                r.map(IdSpace::BlockEntityType, 26),
                Some(BlockEntityKind::Conduit.to_u32() as i32),
                "the conduit is the first type the removal moved"
            );
        }
    }

    #[test]
    fn every_id_round_trips() {
        for (from, to) in [(774, 775), (775, 776), (774, 776)] {
            let forward = pair(from, to);
            let back = pair(to, from);
            for space in IdSpace::ALL {
                for id in 0..count(forward, space) {
                    let Some(there) = forward.map(space, id) else {
                        assert!(
                            is_the_bed(from, to, space, id),
                            "{from} -> {to}: {space:?} id {id} does not map"
                        );
                        continue;
                    };
                    assert_eq!(
                        back.map(space, there),
                        Some(id),
                        "{from} -> {to}: {space:?} id {id} -> {there} did not come back"
                    );
                }
            }
        }
    }

    #[test]
    fn no_two_ids_share_a_target() {
        for (from, to) in [(774, 775), (775, 776), (774, 776)] {
            let r = pair(from, to);
            for space in IdSpace::ALL {
                let mut taken = std::collections::HashMap::new();
                for id in 0..count(r, space) {
                    let Some(there) = r.map(space, id) else {
                        continue;
                    };
                    if let Some(first) = taken.insert(there, id) {
                        panic!(
                            "{from} -> {to} {space:?}: ids {first} and {id} both land on {there}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn sound_events_were_reordered_not_just_inserted() {
        let r = pair(774, 775);
        assert_eq!(r.map(IdSpace::SoundEvent, 280), Some(291));
        assert_eq!(
            r.map(IdSpace::SoundEvent, 281),
            Some(288),
            "281 lands below 280, which is what makes the map non-monotonic"
        );
        let n = count(r, IdSpace::SoundEvent);
        let descents = (1..n)
            .filter(|id| r.map(IdSpace::SoundEvent, *id) <= r.map(IdSpace::SoundEvent, id - 1))
            .count();
        assert_eq!(descents, 8);
    }

    #[test]
    fn an_id_775_added_has_nowhere_to_go_back_to() {
        let back = pair(775, 774);
        assert_eq!(count(back, IdSpace::SoundEvent), 1902);
        assert_eq!(count(back, IdSpace::BlockState), 29_873);
        assert_eq!(back.map(IdSpace::BlockState, 29_872), Some(29_670));
        assert_eq!(back.map(IdSpace::BlockState, 29_873), None);
        assert_eq!(back.map(IdSpace::Item, 230), None, "the item 775 inserted");
        assert_eq!(back.map(IdSpace::Item, 231), Some(230));
    }

    fn missing(back: &Remap, space: IdSpace) -> usize {
        (0..count(back, space))
            .filter(|id| back.map(space, *id).is_none())
            .count()
    }

    #[test]
    fn what_775_added_that_774_cannot_receive() {
        let back = pair(775, 774);
        assert_eq!(missing(back, IdSpace::SoundEvent), 1902 - 1838);
        assert_eq!(missing(back, IdSpace::Item), 1);
        assert_eq!(missing(back, IdSpace::DataComponentType), 110 - 104);
        assert_eq!(missing(back, IdSpace::EntityDataSerializer), 4);
        assert_eq!(missing(back, IdSpace::BlockState), 29_873 - 29_671);
        assert_eq!(
            missing(back, IdSpace::EntityType),
            0,
            "identity has no holes"
        );
    }

    #[test]
    fn what_776_added_that_775_cannot_receive() {
        let back = pair(776, 775);
        assert_eq!(count(back, IdSpace::BlockState), 32_366);
        assert_eq!(count(back, IdSpace::Item), 1537);
        assert_eq!(missing(back, IdSpace::Block), 1196 - 1168);
        assert_eq!(missing(back, IdSpace::BlockState), 32_366 - 29_873);
        assert_eq!(missing(back, IdSpace::Item), 1537 - 1506);
        assert_eq!(missing(back, IdSpace::Attribute), 40 - 35);
        assert_eq!(missing(back, IdSpace::SoundEvent), 1968 - 1902);
        assert_eq!(missing(back, IdSpace::ParticleType), 125 - 117);
        assert_eq!(missing(back, IdSpace::DataComponentType), 1);
        assert_eq!(missing(back, IdSpace::EntityType), 1);
        assert_eq!(missing(back, IdSpace::GameEvent), 1);
        assert_eq!(missing(back, IdSpace::BlockEntityType), 1);
        assert_eq!(missing(back, IdSpace::EntityDataSerializer), 0);
    }

    #[test]
    fn composition_equals_two_lookups() {
        for (from, middle, to) in [(774, 775, 776), (776, 775, 774)] {
            let first = pair(from, middle);
            let second = pair(middle, to);
            let composed = pair(from, to);
            for space in IdSpace::ALL {
                for id in -1..=count(first, space) {
                    let expected = first.map(space, id).and_then(|m| second.map(space, m));
                    assert_eq!(
                        composed.map(space, id),
                        expected,
                        "{from} -> {to} {space:?} id {id}"
                    );
                }
            }
        }
    }

    #[test]
    fn composed_spot_values() {
        let forward = pair(774, 776);
        let back = pair(776, 774);
        assert_eq!(forward.map(IdSpace::Block, 500), Some(502));
        assert_eq!(forward.map(IdSpace::Block, 996), Some(1025));
        assert_eq!(forward.map(IdSpace::Item, 230), Some(258));
        assert_eq!(back.map(IdSpace::Item, 257), None);
        assert_eq!(back.map(IdSpace::Item, 258), Some(230));
        assert_eq!(forward.map(IdSpace::Attribute, 0), Some(1));
        assert_eq!(back.map(IdSpace::Attribute, 0), None);
        assert_eq!(back.map(IdSpace::Attribute, 1), Some(0));
        assert_eq!(forward.map(IdSpace::SoundEvent, 280), Some(293));
        assert_eq!(back.map(IdSpace::SoundEvent, 293), Some(280));
        assert_eq!(forward.map(IdSpace::ParticleType, 109), Some(118));
        assert_eq!(forward.map(IdSpace::BlockState, 29_670), Some(32_365));
        assert_eq!(forward.map(IdSpace::BlockState, 29_671), None);
    }

    #[test]
    fn composition_keeps_holes_on_either_side() {
        let first = Runs {
            runs: vec![(0, Some(0)), (2, None), (4, Some(1))],
            count: 6,
        };
        let second = Runs {
            runs: vec![(0, Some(0)), (5, None), (6, Some(-6))],
            count: 7,
        };
        let composed = first.then(&second);
        assert_eq!(composed.runs, [(0, Some(0)), (2, None), (5, Some(-5))]);
        assert_eq!(composed.count, 6);
        let got: Vec<_> = (0..6).map(|id| composed.get(id)).collect();
        assert_eq!(got, [Some(0), Some(1), None, None, None, Some(0)]);

        let first = Runs {
            runs: vec![(0, Some(3))],
            count: 3,
        };
        let second = Runs {
            runs: vec![(0, Some(0))],
            count: 4,
        };
        let composed = first.then(&second);
        assert_eq!(composed.runs, [(0, Some(3)), (1, None)]);
    }
}
