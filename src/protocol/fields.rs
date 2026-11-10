use std::collections::HashMap;
use std::sync::OnceLock;

use azalea_registry::Registry as _;
use azalea_registry::builtin::EntityKind;

pub(crate) const END: u8 = 0xFF;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Run {
    pub(crate) from: u8,
    pub(crate) offset: i16,
}

pub(crate) struct Fields {
    by_kind: HashMap<u32, Box<[Run]>>,
}

impl Fields {
    pub(crate) fn for_wire(protocol: i32) -> Option<&'static Fields> {
        static FROM_774: OnceLock<Fields> = OnceLock::new();
        static FROM_775: OnceLock<Fields> = OnceLock::new();
        match protocol {
            774 => {
                Some(FROM_774.get_or_init(|| Fields::compose(&[pair_774_775(), pair_775_776()])))
            }
            775 => Some(FROM_775.get_or_init(|| Fields::compose(&[pair_775_776()]))),
            _ => None,
        }
    }

    pub(crate) fn runs(&self, native_kind: i32) -> Option<&[Run]> {
        self.by_kind
            .get(&u32::try_from(native_kind).ok()?)
            .map(|runs| &**runs)
    }

    fn compose(pairs: &[&Pair]) -> Fields {
        let names: std::collections::BTreeSet<&str> = pairs
            .iter()
            .flat_map(|p| p.keys().map(String::as_str))
            .collect();
        let mut by_kind = HashMap::new();
        for name in names {
            let kind: EntityKind = name.parse().unwrap_or_else(|()| {
                panic!("metadata table names {name}, which this client has no entity type for")
            });
            let mut runs: Vec<Run> = Vec::new();
            for index in 0..END {
                let mut moved = Some(index);
                for pair in pairs {
                    moved = moved.and_then(|i| pair.get(name).map_or(Some(i), |runs| map(runs, i)));
                }
                let Some(moved) = moved else {
                    break;
                };
                let offset = i16::from(moved) - i16::from(index);
                if runs.last().map_or(0, |r| r.offset) != offset {
                    runs.push(Run {
                        from: index,
                        offset,
                    });
                }
            }
            if !runs.is_empty() {
                by_kind.insert(kind.to_u32(), runs.into_boxed_slice());
            }
        }
        Fields { by_kind }
    }
}

pub(crate) fn map(runs: &[Run], index: u8) -> Option<u8> {
    let offset = runs
        .iter()
        .rev()
        .find(|run| run.from <= index)
        .map_or(0, |run| run.offset);
    u8::try_from(i16::from(index) + offset)
        .ok()
        .filter(|moved| *moved != END)
}

type Pair = HashMap<String, Box<[Run]>>;

fn parse(json: &str) -> Result<Pair, String> {
    let file: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let types = file["entity_types"]
        .as_object()
        .ok_or("entity_types is missing")?;
    types
        .iter()
        .map(|(name, runs)| {
            let runs = runs
                .as_array()
                .ok_or_else(|| format!("{name}: runs are not a list"))?
                .iter()
                .map(|run| {
                    let from = run[0].as_u64().and_then(|v| u8::try_from(v).ok());
                    let offset = run[1].as_i64().and_then(|v| i16::try_from(v).ok());
                    match (from, offset) {
                        (Some(from), Some(offset)) => Ok(Run { from, offset }),
                        _ => Err(format!("{name}: {run} is not [index, offset]")),
                    }
                })
                .collect::<Result<Box<[Run]>, String>>()?;
            Ok((name.clone(), runs))
        })
        .collect()
}

fn pair_774_775() -> &'static Pair {
    static PAIR: OnceLock<Pair> = OnceLock::new();
    PAIR.get_or_init(|| {
        parse(include_str!("data/metadata-774-775.json"))
            .unwrap_or_else(|e| panic!("embedded 774 -> 775 metadata table: {e}"))
    })
}

fn pair_775_776() -> &'static Pair {
    static PAIR: OnceLock<Pair> = OnceLock::new();
    PAIR.get_or_init(|| {
        parse(include_str!("data/metadata-775-776.json"))
            .unwrap_or_else(|e| panic!("embedded 775 -> 776 metadata table: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(k: EntityKind) -> i32 {
        k.to_u32() as i32
    }

    #[test]
    fn an_ageable_mob_moves_one_from_17_on_774() {
        let runs = Fields::for_wire(774)
            .unwrap()
            .runs(kind(EntityKind::Sheep))
            .expect("moved");
        assert_eq!(map(runs, 16), Some(16));
        assert_eq!(map(runs, 17), Some(18));
        assert_eq!(map(runs, 3), Some(3));
        assert!(
            Fields::for_wire(775)
                .unwrap()
                .runs(kind(EntityKind::Sheep))
                .is_none()
        );
    }

    #[test]
    fn a_cube_moves_two_from_16_on_both_wires() {
        for wire in [774, 775] {
            for k in [EntityKind::Slime, EntityKind::MagmaCube] {
                let runs = Fields::for_wire(wire)
                    .unwrap()
                    .runs(kind(k))
                    .expect("moved");
                assert_eq!(
                    runs,
                    [Run {
                        from: 16,
                        offset: 2
                    }],
                    "{wire} {k:?}"
                );
            }
        }
    }

    #[test]
    fn an_unchanged_chain_has_no_entry() {
        for wire in [774, 775] {
            let f = Fields::for_wire(wire).unwrap();
            for k in [
                EntityKind::Player,
                EntityKind::Zombie,
                EntityKind::Creeper,
                EntityKind::TextDisplay,
            ] {
                assert!(f.runs(kind(k)).is_none(), "{wire} {k:?}");
            }
        }
    }

    #[test]
    fn nothing_lands_on_the_terminator() {
        let runs = [Run {
            from: 16,
            offset: 2,
        }];
        assert_eq!(map(&runs, 252), Some(254));
        assert_eq!(map(&runs, 253), None);
    }
}
