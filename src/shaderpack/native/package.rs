use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use super::manifest::{self, FileRef, Lang, Manifest, Strings};
use super::problem::Problem;
use crate::util::pack::Source;

pub(crate) const MAX_WGSL: usize = 256 * 1024;
pub(crate) const MAX_FILE: usize = 4 * 1024 * 1024;
pub(crate) const MAX_PACK: usize = 32 * 1024 * 1024;
pub(crate) const MAX_ENTRIES: usize = 512;
pub(crate) const MAX_TEXTURE_SIDE: u32 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Manifest,
    Wgsl,
    Png,
    Icon,
    Lang,
    Document,
}

fn role(path: &str) -> Option<Role> {
    let (dir, name) = path.rsplit_once('/').map_or(("", path), |(d, n)| (d, n));
    match (dir, name) {
        ("", "pack.toml") => Some(Role::Manifest),
        ("", "icon.png") => Some(Role::Icon),
        ("", "README.md") => Some(Role::Document),
        ("", n) if n.starts_with("LICENSE") => Some(Role::Document),
        ("lang", n) if n.ends_with(".toml") => Some(Role::Lang),
        (_, n) if n.ends_with(".wgsl") => Some(Role::Wgsl),
        (_, n) if n.ends_with(".png") => Some(Role::Png),
        _ => None,
    }
}

pub(crate) fn is_plain_path(path: &str) -> bool {
    !path.is_empty()
        && path.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with('.')
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        })
}

pub(crate) fn is_native(path: &Path) -> bool {
    Source::open(path, "").is_ok_and(|source| source.contains(&format!("/{}", manifest::FILE)))
}

#[derive(Debug)]
pub(crate) struct Package {
    pub(crate) manifest: Manifest,
    #[allow(
        dead_code,
        reason = "phase 4 of docs/SHADER_FORMAT.md, the options screen, reads the labels"
    )]
    pub(crate) lang: Lang,
    wgsl: BTreeMap<String, Arc<str>>,
    #[allow(
        dead_code,
        reason = "phase 6 of docs/SHADER_FORMAT.md uploads custom textures"
    )]
    images: HashMap<String, Arc<[u8]>>,
}

#[derive(Debug)]
pub(crate) struct Opened {
    pub(crate) package: Option<Package>,
    pub(crate) problems: Vec<Problem>,
}

pub(crate) fn open(path: &Path) -> Opened {
    let mut problems = Vec::new();
    let source = match Source::open(path, "") {
        Ok(source) => source,
        Err(e) => {
            problems.push(Problem::package(format!("the pack cannot be opened: {e}")));
            return Opened {
                package: None,
                problems,
            };
        }
    };
    let package = read(&source, &mut problems);
    Opened { package, problems }
}

fn read(source: &Source, problems: &mut Vec<Problem>) -> Option<Package> {
    let mut paths: Vec<String> = source
        .files()
        .into_iter()
        .map(|p| p.trim_start_matches('/').to_owned())
        .collect();
    paths.sort();
    if paths.len() > MAX_ENTRIES {
        problems.push(Problem::package(format!(
            "the pack has {} files; the most is {MAX_ENTRIES}",
            paths.len()
        )));
        return None;
    }
    if !paths.iter().any(|p| p == manifest::FILE) {
        problems.push(Problem::package(format!(
            "the pack has no {} at its root",
            manifest::FILE
        )));
        return None;
    }

    let mut total = 0usize;
    let mut manifest_text = None;
    let mut wgsl = BTreeMap::new();
    let mut pngs: HashMap<String, Arc<[u8]>> = HashMap::new();
    let mut langs = Vec::new();
    for path in &paths {
        if !is_plain_path(path) {
            problems.push(
                Problem::file(path.clone(), "not a plain path")
                    .hint("parts of [A-Za-z0-9._-], none starting with a dot"),
            );
            continue;
        }
        let Some(role) = role(path) else {
            problems.push(
                Problem::file(path.clone(), "this file does not belong in a pack")
                    .hint("a pack holds pack.toml, .wgsl, .png named by [[texture]], lang/*.toml, README.md, LICENSE* and icon.png"),
            );
            continue;
        };
        let Some(bytes) = source.read(&format!("/{path}")) else {
            problems.push(Problem::file(path.clone(), "cannot be read"));
            continue;
        };
        total += bytes.len();
        if total > MAX_PACK {
            problems.push(Problem::package(format!(
                "the pack inflates past {} MiB; reading stopped at {path}",
                MAX_PACK / (1024 * 1024)
            )));
            return None;
        }
        let cap = if role == Role::Wgsl {
            MAX_WGSL
        } else {
            MAX_FILE
        };
        if bytes.len() > cap {
            problems.push(Problem::file(
                path.clone(),
                format!("{} bytes; the most for this file is {cap}", bytes.len()),
            ));
            continue;
        }
        match role {
            Role::Manifest => manifest_text = Some(text(&bytes)),
            Role::Wgsl => {
                wgsl.insert(path.clone(), Arc::from(text(&bytes)));
            }
            Role::Png | Role::Icon => {
                pngs.insert(path.clone(), Arc::from(bytes));
            }
            Role::Lang => langs.push((path.clone(), text(&bytes))),
            Role::Document => {}
        }
    }

    let manifest_text = manifest_text?;
    let parsed = Manifest::parse(&manifest_text);
    problems.extend(parsed.problems);
    let manifest = parsed.manifest?;

    let mut locales = HashMap::new();
    for (path, text) in langs {
        let locale = path.trim_start_matches("lang/").trim_end_matches(".toml");
        if !super::options::is_value_name(locale) {
            problems.push(
                Problem::file(path.clone(), format!("`{locale}` is not a locale name"))
                    .hint("as en_us"),
            );
            continue;
        }
        let (strings, found) = Strings::parse(&path, &text, &manifest);
        problems.extend(found);
        locales.insert(locale.to_owned(), strings);
    }

    check_file_refs(&manifest, &wgsl, &pngs, problems);

    let named: HashSet<&str> = manifest
        .textures
        .iter()
        .map(|t| t.file.path.as_str())
        .collect();
    pngs.retain(|path, bytes| {
        if path != "icon.png" && !named.contains(path.as_str()) {
            problems.push(Problem::file(
                path.clone(),
                "no [[texture]] names this file",
            ));
            return false;
        }
        match png_size(bytes) {
            None => {
                problems.push(Problem::file(path.clone(), "not a PNG"));
                false
            }
            Some((w, h))
                if path != "icon.png" && (w > MAX_TEXTURE_SIDE || h > MAX_TEXTURE_SIDE) =>
            {
                problems.push(Problem::file(
                    path.clone(),
                    format!("{w}x{h}; a texture is at most {MAX_TEXTURE_SIDE} pixels on a side"),
                ));
                false
            }
            Some(_) => true,
        }
    });

    Some(Package {
        manifest,
        lang: Lang::new(locales),
        wgsl,
        images: pngs,
    })
}

fn check_file_refs(
    manifest: &Manifest,
    wgsl: &BTreeMap<String, Arc<str>>,
    pngs: &HashMap<String, Arc<[u8]>>,
    problems: &mut Vec<Problem>,
) {
    let mut refs: Vec<&FileRef> = Vec::new();
    for layout in manifest.layouts() {
        refs.extend(layout.geometry.groups().map(|(_, group)| &group.file));
        refs.extend(layout.shadow.and_then(|s| s.file.as_ref()));
        refs.extend(layout.passes.iter().map(|p| &p.file));
    }
    let mut seen = BTreeSet::new();
    for file in refs {
        if seen.insert((file.path.as_str(), file.at)) && !wgsl.contains_key(&file.path) {
            problems.push(missing(file));
        }
    }
    for texture in &manifest.textures {
        if !pngs.contains_key(&texture.file.path) {
            problems.push(missing(&texture.file));
        }
    }
}

fn missing(file: &FileRef) -> Problem {
    Problem {
        file: manifest::FILE.to_owned(),
        at: Some(file.at),
        message: format!("`{}` is not in the pack", file.path),
        hint: None,
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(crate::platform::assets::skip_bom(bytes)).into_owned()
}

fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let at = |i: usize| u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    Some((at(16), at(20)))
}

impl Package {
    pub(crate) fn wgsl(&self, path: &str) -> Option<Arc<str>> {
        self.wgsl.get(path).cloned()
    }

    pub(crate) fn wgsl_files(&self) -> impl Iterator<Item = &str> {
        self.wgsl.keys().map(String::as_str)
    }

    #[allow(
        dead_code,
        reason = "phase 6 of docs/SHADER_FORMAT.md uploads custom textures"
    )]
    pub(crate) fn image(&self, path: &str) -> Option<Arc<[u8]>> {
        self.images.get(path).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::super::manifest::tests::EXAMPLE;
    use super::*;
    use crate::util::pack::fixture;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        out.extend_from_slice(&w.to_be_bytes());
        out.extend_from_slice(&h.to_be_bytes());
        out
    }

    const SOURCES: [&str; 4] = [
        "geometry.wgsl",
        "translucent.wgsl",
        "lighting.wgsl",
        "tonemap.wgsl",
    ];

    fn write_pack(name: &str, extra: &[(&str, &[u8])]) -> std::path::PathBuf {
        let dir = fixture(name);
        std::fs::write(dir.join("pack.toml"), EXAMPLE).unwrap();
        for file in SOURCES {
            std::fs::write(dir.join(file), "fn x() {}").unwrap();
        }
        for (path, bytes) in extra {
            let at = dir.join(path);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, bytes).unwrap();
        }
        dir
    }

    #[test]
    fn plain_paths() {
        assert!(is_plain_path("lib/noise.wgsl"));
        assert!(is_plain_path("LICENSE-MIT"));
        for bad in [
            "",
            "/a.wgsl",
            "a//b.wgsl",
            "../a.wgsl",
            ".hidden.wgsl",
            "lib/.x.wgsl",
            "a\\b.wgsl",
            "c:x.wgsl",
            "a b.wgsl",
        ] {
            assert!(!is_plain_path(bad), "{bad}");
        }
    }

    #[test]
    fn the_example_pack_opens_from_a_folder() {
        let dir = write_pack(
            "native-ok",
            &[
                ("lang/en_us.toml", b"[option.shadows]\nname = \"Shadows\"\n"),
                ("README.md", b"hi"),
                ("icon.png", &png(64, 64)),
            ],
        );
        assert!(is_native(&dir));
        let opened = open(&dir);
        assert!(opened.problems.is_empty(), "{:#?}", opened.problems);
        let package = opened.package.unwrap();
        assert_eq!(
            package.wgsl_files().collect::<Vec<_>>(),
            [
                "geometry.wgsl",
                "lighting.wgsl",
                "tonemap.wgsl",
                "translucent.wgsl"
            ]
        );
        assert_eq!(package.wgsl("lighting.wgsl").as_deref(), Some("fn x() {}"));
        assert!(package.image("icon.png").is_some());
        let shadows = package.manifest.options.get("shadows").unwrap();
        assert_eq!(package.lang.option_name("en_us", shadows), "Shadows");
    }

    #[test]
    fn the_example_pack_opens_from_a_zip() {
        let mut files: Vec<(&str, &[u8])> = vec![("pack.toml", EXAMPLE.as_bytes())];
        files.extend(SOURCES.iter().map(|f| (*f, &b"fn x() {}"[..])));
        let dir = fixture("native-zip");
        let zip = dir.join("pack.zip");
        std::fs::write(&zip, crate::util::zip::write(&files)).unwrap();
        assert!(is_native(&zip));
        let opened = open(&zip);
        assert!(opened.problems.is_empty(), "{:#?}", opened.problems);
    }

    #[test]
    fn stray_and_oversized_files_are_problems() {
        let big = vec![b' '; MAX_WGSL + 1];
        let dir = write_pack(
            "native-stray",
            &[
                ("notes.txt", b"x"),
                ("big.wgsl", &big),
                ("stray.png", &png(1, 1)),
                ("lang/x/y.toml", b""),
            ],
        );
        let opened = open(&dir);
        let files: Vec<&str> = opened.problems.iter().map(|p| p.file.as_str()).collect();
        assert_eq!(
            files,
            ["big.wgsl", "lang/x/y.toml", "notes.txt", "stray.png"],
            "{:#?}",
            opened.problems
        );
        assert!(opened.package.is_some());
    }

    #[test]
    fn a_missing_source_is_reported_in_the_manifest() {
        let dir = write_pack("native-missing", &[]);
        std::fs::remove_file(dir.join("tonemap.wgsl")).unwrap();
        let opened = open(&dir);
        assert_eq!(opened.problems.len(), 1, "{:#?}", opened.problems);
        let p = &opened.problems[0];
        assert_eq!(
            (p.file.as_str(), p.at.map(|a| a.line)),
            ("pack.toml", Some(48))
        );
    }

    #[test]
    fn a_texture_too_large_is_refused() {
        let dir = write_pack("native-texture", &[("noise.png", &png(8192, 16))]);
        let text = format!("{EXAMPLE}\n[[texture]]\nid = \"blue_noise\"\nfile = \"noise.png\"\n");
        std::fs::write(dir.join("pack.toml"), text).unwrap();
        let opened = open(&dir);
        assert_eq!(opened.problems.len(), 1, "{:#?}", opened.problems);
        assert!(opened.problems[0].message.contains("8192x16"));
    }

    #[test]
    fn a_folder_without_a_manifest_is_not_native() {
        let dir = fixture("native-none");
        std::fs::create_dir_all(dir.join("shaders")).unwrap();
        assert!(!is_native(&dir));
        assert!(open(&dir).package.is_none());
    }
}
