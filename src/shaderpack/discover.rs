#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Pack {
    pub(crate) name: String,
    pub(crate) kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Directory,
    Zip,
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn dir() -> std::path::PathBuf {
    crate::platform::storage::dir().join("shaders")
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn list() -> Vec<Pack> {
    scan(&dir())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn list() -> Vec<Pack> {
    Vec::new()
}

#[cfg(not(target_arch = "wasm32"))]
fn scan(dir: &std::path::Path) -> Vec<Pack> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut packs: Vec<Pack> = entries
        .flatten()
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            let name = entry.file_name().into_string().ok()?;

            let kind = if file_type.is_file() {
                if is_zip(&name) {
                    Kind::Zip
                } else {
                    return None;
                }
            } else {
                Kind::Directory
            };

            if name.starts_with('.') {
                return None;
            }

            Some(Pack { name, kind })
        })
        .collect();

    packs.sort_by_cached_key(|pack| pack.name.to_lowercase());
    packs
}

#[cfg(not(target_arch = "wasm32"))]
fn is_zip(name: &str) -> bool {
    name.len() > 4 && name[name.len() - 4..].eq_ignore_ascii_case(".zip")
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("torch-client-tests").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(dir: &std::path::Path, name: &str) {
        std::fs::write(dir.join(name), b"").unwrap();
    }

    #[test]
    fn a_missing_directory_is_an_empty_list() {
        let dir = fixture("missing").join("not-created");
        assert_eq!(scan(&dir), Vec::new());
    }

    #[test]
    fn zips_and_directories_are_both_packs() {
        let dir = fixture("both");
        touch(&dir, "Solas Shader V3.7b.zip");
        std::fs::create_dir(dir.join("BSL")).unwrap();

        let packs = scan(&dir);
        assert_eq!(
            packs,
            vec![
                Pack {
                    name: "BSL".into(),
                    kind: Kind::Directory
                },
                Pack {
                    name: "Solas Shader V3.7b.zip".into(),
                    kind: Kind::Zip
                },
            ]
        );
    }

    #[test]
    fn a_file_that_is_not_a_zip_is_not_a_pack() {
        let dir = fixture("stray-files");
        touch(&dir, "Solas Shader V3.7b.zip.txt");
        touch(&dir, "notes.md");
        touch(&dir, "zip");
        assert_eq!(scan(&dir), Vec::new());
    }

    #[test]
    fn the_extension_is_matched_whatever_its_case() {
        let dir = fixture("case");
        touch(&dir, "Loud.ZIP");
        assert_eq!(scan(&dir).len(), 1);
    }

    #[test]
    fn an_options_document_is_not_a_pack() {
        let dir = fixture("options-doc");
        touch(&dir, "Solas Shader V3.7b.zip");
        touch(&dir, "Solas Shader V3.7b.zip.txt");
        assert_eq!(scan(&dir).len(), 1);
    }

    #[test]
    fn dotfiles_are_skipped() {
        let dir = fixture("dotfiles");
        touch(&dir, ".DS_Store");
        std::fs::create_dir(dir.join(".git")).unwrap();
        assert_eq!(scan(&dir), Vec::new());
    }

    #[test]
    fn the_list_sorts_without_regard_to_case() {
        let dir = fixture("sorting");
        for name in ["zebra.zip", "Apple.zip", "banana.zip"] {
            touch(&dir, name);
        }
        let names: Vec<String> = scan(&dir).into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["Apple.zip", "banana.zip", "zebra.zip"]);
    }
}
