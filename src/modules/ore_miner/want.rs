use std::sync::Arc;

use azalea::block::BlockState;
use azalea_registry::builtin::BlockKind;
use azalea_registry::tags::blocks::LOGS;

use super::{kind_named, wants};
use crate::modules::auto_mine::shared::solid;
use crate::modules::esp::blocks::{id_of, ids};
use crate::modules::esp::table::{Merged, NONE_ENTRY, row_of};
use crate::modules::seed_xray::terrain::kind_of;

#[derive(Clone)]
pub enum Want {
    Ores(u16),
    Blocks { name: String, table: Arc<Merged> },
}

impl Want {
    pub(super) fn group(&self, state: BlockState) -> Option<u8> {
        match self {
            Want::Ores(mask) => kind_of(state).filter(|k| wants(*mask, *k)).map(|k| k as u8),
            Want::Blocks { table, .. } => {
                let e = table.entry(state.id() as u32);
                (e != NONE_ENTRY).then(|| row_of(e) as u8)
            }
        }
    }

    pub(super) fn same(&self, other: &Want) -> bool {
        match (self, other) {
            (Want::Ores(a), Want::Ores(b)) => a == b,
            (Want::Blocks { table: a, .. }, Want::Blocks { table: b, .. }) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

pub fn named(name: &str) -> Option<Want> {
    let lower = name.to_ascii_lowercase();
    let bare = lower.strip_prefix("minecraft:").unwrap_or(&lower);
    if let Some(k) = kind_named(bare) {
        return Some(Want::Ores(1 << k as u16));
    }
    let table = match bare {
        "log" | "logs" => Merged::of_groups(1, |st| log_tier(st).filter(|t| *t == 0)),
        "wood" => Merged::of_groups(2, log_tier),
        _ => {
            let all = ids();
            let id = all[all.binary_search(&bare).ok()?];
            Merged::of_groups(1, |st| (id_of(st) == id && solid(st)).then_some(0))
        }
    };
    if table.of_state.iter().all(|e| *e == NONE_ENTRY) {
        return None;
    }
    Some(Want::Blocks {
        name: bare.to_string(),
        table: Arc::new(table),
    })
}

fn log_tier(state: BlockState) -> Option<u16> {
    if !LOGS.contains(&BlockKind::from(state)) {
        return None;
    }
    let id = id_of(state);
    if id.ends_with("_log") || id.ends_with("_stem") {
        Some(0)
    } else if id.ends_with("_wood") || id.ends_with("_hyphae") {
        Some(1)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::seed_xray::table::Kind;

    fn group(want: &Want, kind: BlockKind) -> Option<u8> {
        want.group(BlockState::from(kind))
    }

    #[test]
    fn log_is_every_log_and_stem() {
        for name in ["log", "logs", "LOG", "minecraft:logs"] {
            let want = named(name).unwrap();
            assert!(matches!(&want, Want::Blocks { name, .. } if name.starts_with("log")));
            for k in [
                BlockKind::OakLog,
                BlockKind::StrippedPaleOakLog,
                BlockKind::CrimsonStem,
                BlockKind::StrippedWarpedStem,
            ] {
                assert_eq!(group(&want, k), Some(0), "{k:?}");
            }
            for k in [
                BlockKind::OakWood,
                BlockKind::WarpedHyphae,
                BlockKind::OakPlanks,
                BlockKind::Stone,
            ] {
                assert_eq!(group(&want, k), None, "{k:?}");
            }
        }
    }

    #[test]
    fn wood_puts_logs_before_woods() {
        let want = named("Wood").unwrap();
        assert_eq!(group(&want, BlockKind::BirchLog), Some(0));
        assert_eq!(group(&want, BlockKind::StrippedCrimsonStem), Some(0));
        assert_eq!(group(&want, BlockKind::BirchWood), Some(1));
        assert_eq!(group(&want, BlockKind::StrippedMangroveWood), Some(1));
        assert_eq!(group(&want, BlockKind::CrimsonHyphae), Some(1));
        assert_eq!(group(&want, BlockKind::OakPlanks), None);
        assert_eq!(group(&want, BlockKind::OakLeaves), None);
    }

    #[test]
    fn an_ore_name_is_its_family() {
        let want = named("Diamond").unwrap();
        assert!(matches!(want, Want::Ores(m) if m == 1 << Kind::Diamond as u16));
        assert!(
            matches!(named("minecraft:debris"), Some(Want::Ores(m)) if m == 1 << Kind::Debris as u16)
        );
    }

    #[test]
    fn a_block_id_is_that_block_alone() {
        let want = named("minecraft:STONE").unwrap();
        assert!(matches!(&want, Want::Blocks { name, .. } if name == "stone"));
        assert_eq!(group(&want, BlockKind::Stone), Some(0));
        assert_eq!(group(&want, BlockKind::Cobblestone), None);
        let leaves = named("oak_leaves").unwrap();
        let Want::Blocks { table, .. } = &leaves else {
            panic!()
        };
        let states = table.of_state.iter().filter(|e| **e != NONE_ENTRY).count();
        assert!(states > 1, "{states}");
    }

    #[test]
    fn other_names_are_refused() {
        assert!(named("nonsense").is_none());
        assert!(named("stone_block").is_none());
        assert!(named("air").is_none());
        assert!(named("water").is_none());
        assert!(named("planks").is_none());
    }
}
