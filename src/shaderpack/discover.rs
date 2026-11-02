use crate::util::pack::Pack;
#[cfg(all(test, not(target_arch = "wasm32")))]
use crate::util::pack::Source;

pub(crate) fn dir() -> std::path::PathBuf {
    crate::platform::packfs::root().join("shaders")
}

pub(crate) fn is_pack_dir(path: &std::path::Path) -> bool {
    crate::platform::packfs::is_dir(&path.join(super::ROOT))
}

pub(crate) fn list() -> Vec<Pack> {
    crate::util::pack::scan(&dir())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) fn installed() -> impl Iterator<Item = (String, Source)> {
    list().into_iter().filter_map(|pack| {
        Source::open(&dir().join(&pack.name), super::ROOT)
            .ok()
            .map(|s| (pack.name, s))
    })
}
