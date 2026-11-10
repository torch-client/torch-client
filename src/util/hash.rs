#[cfg(any(
    feature = "asset_download",
    feature = "skins",
    feature = "update_checker",
    resource_packs,
    lua_mods
))]
use sha1::Digest;

#[cfg(any(
    feature = "asset_download",
    feature = "skins",
    feature = "update_checker",
    resource_packs,
    lua_mods
))]
pub(crate) fn sha1_hex(bytes: &[u8]) -> String {
    sha1::Sha1::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(self_update)]
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    <sha2::Sha256 as sha2::Digest>::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(feature = "click_gui")]
pub(crate) fn obfuscate_seed(seed: i64) -> i64 {
    let digest = <sha2::Sha256 as sha2::Digest>::digest(seed.to_le_bytes());
    let mut first = [0u8; 8];
    first.copy_from_slice(&digest[..8]);
    i64::from_le_bytes(first)
}

#[cfg(not(feature = "click_gui"))]
pub(crate) fn obfuscate_seed(_: i64) -> i64 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(self_update)]
    #[test]
    fn sha256_matches_the_published_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[cfg(any(
        feature = "asset_download",
        feature = "skins",
        feature = "update_checker",
        resource_packs,
        lua_mods
    ))]
    #[test]
    fn sha1_matches_the_published_vectors() {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            sha1_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
    }
}
