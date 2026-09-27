use std::collections::HashMap;
use std::sync::OnceLock;

use crate::protocol::version::{NATIVE, ProtocolVersion};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Phase {
    Handshake,
    Status,
    Login,
    Configuration,
    Game,
}

impl Phase {
    pub(crate) const ALL: [Phase; 5] = [
        Phase::Handshake,
        Phase::Status,
        Phase::Login,
        Phase::Configuration,
        Phase::Game,
    ];

    fn key(self) -> &'static str {
        match self {
            Phase::Handshake => "handshake",
            Phase::Status => "status",
            Phase::Login => "login",
            Phase::Configuration => "configuration",
            Phase::Game => "game",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Direction {
    Serverbound,
    Clientbound,
}

impl Direction {
    pub(crate) const ALL: [Direction; 2] = [Direction::Serverbound, Direction::Clientbound];
}

pub(crate) struct PacketTable {
    version: ProtocolVersion,
    phases: [PhaseTable; 5],
}

struct PhaseTable {
    serverbound: DirectionTable,
    clientbound: DirectionTable,
}

struct DirectionTable {
    names: Vec<String>,
    ids: HashMap<String, u32>,
}

impl PacketTable {
    pub(crate) fn native() -> &'static PacketTable {
        static TABLE: OnceLock<PacketTable> = OnceLock::new();
        TABLE.get_or_init(|| {
            Self::parse(include_str!("data/packets-26.1.1.json"), NATIVE)
                .unwrap_or_else(|e| panic!("embedded {} packet table: {e}", NATIVE.name))
        })
    }

    pub(crate) fn v1_21_11() -> &'static PacketTable {
        static TABLE: OnceLock<PacketTable> = OnceLock::new();
        TABLE.get_or_init(|| {
            let version =
                ProtocolVersion::from_name("1.21.11").expect("1.21.11 is a known version");
            Self::parse(include_str!("data/packets-1.21.11.json"), version)
                .unwrap_or_else(|e| panic!("embedded 1.21.11 packet table: {e}"))
        })
    }

    pub(crate) fn for_protocol(protocol: i32) -> Option<&'static PacketTable> {
        match protocol {
            p if p == NATIVE.protocol => Some(Self::native()),
            774 => Some(Self::v1_21_11()),
            _ => None,
        }
    }

    fn parse(json: &str, expected: ProtocolVersion) -> Result<Self, String> {
        let file: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let version = file["version"].as_str().unwrap_or_default();
        let protocol = file["protocol"].as_i64().unwrap_or(-1);
        if version != expected.name || protocol != i64::from(expected.protocol) {
            return Err(format!(
                "table is {version}/{protocol}, expected {}/{}",
                expected.name, expected.protocol
            ));
        }

        let names = |phase: &str, dir: &str| -> Result<Vec<String>, String> {
            let list = file[phase][dir]
                .as_array()
                .ok_or_else(|| format!("{phase}/{dir} is not an array"))?;
            list.iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("{phase}/{dir} holds a non-string"))
                })
                .collect()
        };

        let mut phases = Vec::with_capacity(Phase::ALL.len());
        for phase in Phase::ALL {
            let key = phase.key();
            phases.push(PhaseTable {
                serverbound: DirectionTable::build(names(key, "serverbound")?),
                clientbound: DirectionTable::build(names(key, "clientbound")?),
            });
            let table = phases.last().expect("just pushed");
            for dir in [&table.serverbound, &table.clientbound] {
                if dir.ids.len() != dir.names.len() {
                    return Err(format!("{key} registers a packet twice"));
                }
            }
        }
        let phases: [PhaseTable; 5] = phases
            .try_into()
            .map_err(|_| "wrong number of phases".to_owned())?;

        let first = phases[Phase::Game as usize].clientbound.names.first();
        if first.map(String::as_str) != Some("bundle_delimiter") {
            return Err(format!(
                "game clientbound id 0 is {first:?}, expected bundle_delimiter"
            ));
        }

        Ok(Self {
            version: expected,
            phases,
        })
    }

    pub(crate) fn version(&self) -> ProtocolVersion {
        self.version
    }

    pub(crate) fn id(&self, phase: Phase, dir: Direction, name: &str) -> Option<u32> {
        self.direction(phase, dir).ids.get(name).copied()
    }

    pub(crate) fn name_of(&self, phase: Phase, dir: Direction, id: u32) -> Option<&str> {
        self.direction(phase, dir)
            .names
            .get(id as usize)
            .map(String::as_str)
    }

    pub(crate) fn count(&self, phase: Phase, dir: Direction) -> usize {
        self.direction(phase, dir).names.len()
    }

    fn direction(&self, phase: Phase, dir: Direction) -> &DirectionTable {
        let phase = &self.phases[phase as usize];
        match dir {
            Direction::Serverbound => &phase.serverbound,
            Direction::Clientbound => &phase.clientbound,
        }
    }
}

impl DirectionTable {
    fn build(names: Vec<String>) -> Self {
        let ids = names
            .iter()
            .enumerate()
            .map(|(id, name)| (name.clone(), id as u32))
            .collect();
        Self { names, ids }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_native_table_is_azaleas_table() {
        let table = PacketTable::native();
        for (phase, dir_name) in [
            (Phase::Handshake, "handshake"),
            (Phase::Status, "status"),
            (Phase::Login, "login"),
            (Phase::Configuration, "config"),
            (Phase::Game, "game"),
        ] {
            for direction in Direction::ALL {
                let theirs = azalea_names(dir_name, direction);
                let ours: Vec<String> = (0..table.count(phase, direction))
                    .map(|id| {
                        table
                            .name_of(phase, direction, id as u32)
                            .unwrap()
                            .replace('/', "_")
                    })
                    .collect();
                assert_eq!(
                    ours, theirs,
                    "{dir_name}/{direction:?} does not match azalea's own list"
                );
            }
        }
    }

    fn azalea_names(phase_dir: &str, direction: Direction) -> Vec<String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("vendor/azalea/azalea-protocol/src/packets")
            .join(phase_dir)
            .join("mod.rs");
        let source =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let head = match direction {
            Direction::Serverbound => "Serverbound => [",
            Direction::Clientbound => "Clientbound => [",
        };
        let Some(start) = source.find(head) else {
            return Vec::new();
        };
        let body = &source[start + head.len()..];
        let end = body.find("\n    ]").expect("unterminated packet list");
        body[..end]
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("//"))
            .map(|line| line.trim_end_matches(',').to_owned())
            .collect()
    }

    #[test]
    fn per_phase_counts() {
        let t = PacketTable::native();
        use Direction::{Clientbound, Serverbound};
        assert_eq!(t.version(), NATIVE);
        assert_eq!(t.count(Phase::Handshake, Serverbound), 1);
        assert_eq!(t.count(Phase::Handshake, Clientbound), 0);
        assert_eq!(t.count(Phase::Status, Serverbound), 2);
        assert_eq!(t.count(Phase::Status, Clientbound), 2);
        assert_eq!(t.count(Phase::Login, Serverbound), 5);
        assert_eq!(t.count(Phase::Login, Clientbound), 6);
        assert_eq!(t.count(Phase::Configuration, Serverbound), 10);
        assert_eq!(t.count(Phase::Configuration, Clientbound), 20);
        assert_eq!(t.count(Phase::Game, Serverbound), 69);
        assert_eq!(t.count(Phase::Game, Clientbound), 141);
    }

    #[test]
    fn anchors() {
        let t = PacketTable::native();
        use Direction::{Clientbound, Serverbound};
        assert_eq!(
            t.name_of(Phase::Game, Clientbound, 0),
            Some("bundle_delimiter")
        );
        assert_eq!(t.id(Phase::Game, Clientbound, "level_particles"), Some(47));
        assert_eq!(t.id(Phase::Game, Serverbound, "attack"), Some(1));
        assert_eq!(t.id(Phase::Handshake, Serverbound, "intention"), Some(0));
        assert_eq!(t.id(Phase::Login, Clientbound, "login_finished"), Some(2));
        assert_eq!(t.id(Phase::Game, Serverbound, "no_such_packet"), None);
        assert_eq!(t.name_of(Phase::Game, Clientbound, 141), None);
    }

    #[test]
    fn the_debug_packets_keep_vanillas_slash() {
        let t = PacketTable::native();
        assert_eq!(
            t.name_of(Phase::Game, Direction::Clientbound, 26),
            Some("debug/block_value")
        );
    }

    #[test]
    fn the_embedded_protocols_have_tables() {
        assert!(PacketTable::for_protocol(NATIVE.protocol).is_some());
        assert!(PacketTable::for_protocol(774).is_some());
        assert!(PacketTable::for_protocol(773).is_none());
    }

    #[test]
    fn what_moved_between_774_and_775() {
        let old = PacketTable::v1_21_11();
        let new = PacketTable::native();
        let moved = |phase, dir| {
            (0..old.count(phase, dir))
                .filter(|id| {
                    old.name_of(phase, dir, *id as u32) != new.name_of(phase, dir, *id as u32)
                })
                .count()
        };
        use Direction::{Clientbound, Serverbound};
        for phase in [
            Phase::Handshake,
            Phase::Status,
            Phase::Login,
            Phase::Configuration,
        ] {
            for dir in Direction::ALL {
                assert_eq!(moved(phase, dir), 0, "{phase:?}/{dir:?} moved");
                assert_eq!(old.count(phase, dir), new.count(phase, dir));
            }
        }
        assert_eq!(moved(Phase::Game, Serverbound), 65);
        assert_eq!(moved(Phase::Game, Clientbound), 100);
        assert_eq!(old.count(Phase::Game, Serverbound), 66);
        assert_eq!(old.count(Phase::Game, Clientbound), 139);
    }

    #[test]
    fn every_774_packet_still_exists_in_775() {
        let old = PacketTable::v1_21_11();
        let new = PacketTable::native();
        for phase in Phase::ALL {
            for dir in Direction::ALL {
                for id in 0..old.count(phase, dir) {
                    let name = old.name_of(phase, dir, id as u32).unwrap();
                    assert!(
                        new.id(phase, dir, name).is_some(),
                        "{name} exists in 774 and not in 775"
                    );
                }
            }
        }
    }
}
