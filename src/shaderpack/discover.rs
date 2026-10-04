use crate::util::pack::Pack;
#[cfg(all(test, not(target_arch = "wasm32")))]
use crate::util::pack::Source;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn dir() -> std::path::PathBuf {
    crate::platform::storage::dir().join("shaders")
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn dir() -> std::path::PathBuf {
    std::path::PathBuf::from("shaders")
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn list() -> Vec<Pack> {
    crate::util::pack::scan(&dir())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn list() -> Vec<Pack> {
    Vec::new()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) fn installed() -> impl Iterator<Item = (String, Source)> {
    list().into_iter().filter_map(|pack| {
        Source::open(&dir().join(&pack.name), super::ROOT)
            .ok()
            .map(|s| (pack.name, s))
    })
}
