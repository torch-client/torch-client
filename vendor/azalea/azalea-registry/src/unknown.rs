use std::{
    collections::BTreeMap,
    io::Cursor,
    sync::{
        Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU64, Ordering::Relaxed},
    },
};

use azalea_buf::{AzBufVar, BufReadError};
use tracing::warn;

use crate::Registry;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Space {
    Blocks,
    Entities,
    BlockEntities,
    Attributes,
    Items,
    Packets,
}

const SPACES: usize = 6;

static COUNTS: [AtomicU64; SPACES] = [const { AtomicU64::new(0) }; SPACES];
static FAILED_PACKETS: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Snapshot {
    pub blocks: u64,
    pub entities: u64,
    pub block_entities: u64,
    pub attributes: u64,
    pub items: u64,
    pub packets: u64,
}

impl Snapshot {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

fn count(space: Space) -> bool {
    COUNTS[space as usize].fetch_add(1, Relaxed) == 0
}

#[cold]
pub fn note(space: Space, id: u32) {
    if count(space) {
        warn!("unknown id {id} in {space:?}; further ones are only counted");
    }
}

pub fn note_packet(name: &str) -> bool {
    count(Space::Packets);
    let mut table = failed_packets_table();
    if let Some(count) = table.get_mut(name) {
        *count += 1;
        return false;
    }
    table.insert(name.to_owned(), 1);
    true
}

pub fn snapshot() -> Snapshot {
    let get = |space: Space| COUNTS[space as usize].load(Relaxed);
    Snapshot {
        blocks: get(Space::Blocks),
        entities: get(Space::Entities),
        block_entities: get(Space::BlockEntities),
        attributes: get(Space::Attributes),
        items: get(Space::Items),
        packets: get(Space::Packets),
    }
}

pub fn worst_packet() -> Option<(String, u64)> {
    failed_packets_table()
        .iter()
        .max_by_key(|(_, count)| **count)
        .map(|(name, count)| (name.clone(), *count))
}

pub fn reset() {
    for count in &COUNTS {
        count.store(0, Relaxed);
    }
    failed_packets_table().clear();
}

fn failed_packets_table() -> MutexGuard<'static, BTreeMap<String, u64>> {
    FAILED_PACKETS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

pub fn read_id<T: Registry>(
    buf: &mut Cursor<&[u8]>,
    space: Space,
) -> Result<Result<T, u32>, BufReadError> {
    let id = u32::azalea_read_var(buf)?;
    Ok(T::from_u32(id).ok_or_else(|| {
        note(space, id);
        id
    }))
}

pub fn read_list_skipping<T>(
    buf: &mut Cursor<&[u8]>,
    mut entry: impl FnMut(&mut Cursor<&[u8]>) -> Result<Option<T>, BufReadError>,
) -> Result<Vec<T>, BufReadError> {
    let length = u32::azalea_read_var(buf)? as usize;
    let mut contents = Vec::with_capacity(usize::min(length, 65536));
    for _ in 0..length {
        if let Some(value) = entry(buf)? {
            contents.push(value);
        }
    }
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use azalea_buf::AzBuf;

    use super::*;
    use crate::builtin::EntityKind;

    fn varint(id: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        id.azalea_write_var(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn counters_count_and_reset() {
        reset();
        assert!(snapshot().is_empty());

        note(Space::Blocks, 40_000);
        note(Space::Blocks, 40_001);
        note(Space::Entities, 900);
        note(Space::BlockEntities, 901);
        note(Space::Attributes, 902);
        assert!(note_packet("set_equipment"));
        assert!(!note_packet("set_equipment"));
        assert!(note_packet("container_set_slot"));

        assert_eq!(
            snapshot(),
            Snapshot {
                blocks: 2,
                entities: 1,
                block_entities: 1,
                attributes: 1,
                items: 0,
                packets: 3,
            }
        );
        assert_eq!(worst_packet(), Some(("set_equipment".to_owned(), 2)));

        let bytes = varint(u32::MAX >> 1);
        let read = read_id::<EntityKind>(&mut Cursor::new(&bytes[..]), Space::Entities);
        assert_eq!(read.unwrap(), Err(u32::MAX >> 1));
        assert_eq!(snapshot().entities, 2);

        reset();
        assert!(snapshot().is_empty());
        assert_eq!(worst_packet(), None);
    }

    #[test]
    fn derived_read_makes_an_unknown_id_the_first_variant() {
        let bytes = varint(u32::MAX >> 1);
        let kind = EntityKind::azalea_read(&mut Cursor::new(&bytes[..])).unwrap();
        assert_eq!(kind.to_u32(), 0);
    }

    #[test]
    fn list_keeps_only_what_the_entry_returns() {
        let mut bytes = varint(3);
        for value in [1, 2, 3] {
            bytes.extend(varint(value));
        }
        let mut cursor = Cursor::new(&bytes[..]);
        let list = read_list_skipping(&mut cursor, |buf| {
            let value = u32::azalea_read_var(buf)?;
            Ok((value != 2).then_some(value))
        });
        assert_eq!(list.unwrap(), vec![1, 3]);
        assert_eq!(cursor.position(), bytes.len() as u64);
    }
}
