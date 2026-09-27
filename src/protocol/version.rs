#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct ProtocolVersion {
    pub name: &'static str,
    pub protocol: i32,
}

const fn v(name: &'static str, protocol: i32) -> ProtocolVersion {
    ProtocolVersion { name, protocol }
}

pub(crate) const VERSIONS: &[ProtocolVersion] = &[
    v("26.1.2", 775),
    v("26.1.1", 775),
    v("26.1", 775),
    v("1.21.11", 774),
    v("1.21.10", 773),
    v("1.21.9", 773),
];

pub(crate) const NATIVE: ProtocolVersion = v("26.1.1", 775);

const TRANSLATED: &[i32] = &[774];

pub(crate) fn joinable(protocol: i32) -> bool {
    protocol == NATIVE.protocol || TRANSLATED.contains(&protocol)
}

pub(crate) fn selectable() -> impl Iterator<Item = ProtocolVersion> {
    std::iter::once(NATIVE).chain(
        TRANSLATED
            .iter()
            .filter_map(|protocol| ProtocolVersion::from_protocol(*protocol)),
    )
}

impl ProtocolVersion {
    pub(crate) fn from_protocol(protocol: i32) -> Option<Self> {
        VERSIONS.iter().copied().find(|v| v.protocol == protocol)
    }

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        VERSIONS.iter().copied().find(|v| v.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_is_azaleas_own_version() {
        assert_eq!(NATIVE.protocol, azalea_protocol::packets::PROTOCOL_VERSION);
        assert_eq!(NATIVE.name, crate::ASSET_VERSION);
        assert!(VERSIONS.contains(&NATIVE));
    }

    #[test]
    fn selectable_is_the_joinable_set() {
        let picked: Vec<_> = selectable().collect();
        assert_eq!(picked[0], NATIVE);
        assert_eq!(picked.len(), 1 + TRANSLATED.len());
        assert!(picked.iter().all(|v| joinable(v.protocol)));
        assert_eq!(picked[1].name, "1.21.11");
    }

    #[test]
    fn lookups() {
        assert_eq!(ProtocolVersion::from_protocol(775).unwrap().name, "26.1.2");
        assert_eq!(ProtocolVersion::from_protocol(773).unwrap().name, "1.21.10");
        assert_eq!(ProtocolVersion::from_name("1.21.11").unwrap().protocol, 774);
        assert!(ProtocolVersion::from_protocol(47).is_none());
        assert!(ProtocolVersion::from_name("1.8.9").is_none());
    }

    #[test]
    fn a_translated_version_is_joinable_and_an_unknown_one_is_not() {
        assert!(joinable(NATIVE.protocol));
        assert!(joinable(774));
        assert!(!joinable(773), "1.21.10 has no tables and no hop");
        assert!(!joinable(47));
    }
}
