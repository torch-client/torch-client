use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use super::{folder, load, perm};
use crate::platform::packfs;

#[derive(Clone, Debug)]
pub enum Verdict {
    New,
    Replaces(Vec<String>),
    Rejected(String),
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub path: PathBuf,
    pub file: String,
    pub title: String,
    pub version: Option<String>,
    pub verdict: Verdict,
    pub id: String,
    pub perms: Vec<&'static str>,
    bytes: Option<Arc<Vec<u8>>>,
}

impl Candidate {
    pub fn installs(&self) -> bool {
        !matches!(self.verdict, Verdict::Rejected(_))
    }
}

static NOTE: Mutex<Option<String>> = Mutex::new(None);

pub fn note() -> Option<String> {
    NOTE.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

pub fn set_note(text: String) {
    *NOTE.lock().unwrap_or_else(PoisonError::into_inner) = Some(text);
}

pub fn clear_note() {
    *NOTE.lock().unwrap_or_else(PoisonError::into_inner) = None;
}

pub fn check(paths: Vec<PathBuf>, already: &[Candidate]) -> Vec<Candidate> {
    let dir = folder();
    let canonical_dir = packfs::canonicalize(&dir).ok();
    let mut out: Vec<Candidate> = Vec::new();
    for path in paths {
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let inspected = load::inspect(&path);
        let (title, version, id, perms) = match &inspected {
            Ok(i) => (
                i.name.clone().unwrap_or_else(|| i.id.clone()),
                i.version.clone(),
                i.id.clone(),
                i.perms.clone(),
            ),
            Err(_) => (file.clone(), None, String::new(), Vec::new()),
        };
        let verdict = match inspected {
            Err(e) => Verdict::Rejected(e),
            Ok(_)
                if canonical_dir.is_some()
                    && path.parent().and_then(|p| packfs::canonicalize(p).ok())
                        == canonical_dir =>
            {
                Verdict::Rejected("is already in the mods folder; press Reload mods".to_string())
            }
            Ok(i)
                if already
                    .iter()
                    .chain(&out)
                    .any(|c| c.installs() && id_of(c) == Some(i.id.as_str())) =>
            {
                Verdict::Rejected(format!(
                    "`{}` is dropped twice; only the first is installed",
                    i.id
                ))
            }
            Ok(i) => {
                let same = same_id(&dir, &i.id);
                if same.is_empty() {
                    Verdict::New
                } else {
                    Verdict::Replaces(same)
                }
            }
        };
        out.push(Candidate {
            path,
            file,
            title,
            version,
            verdict,
            id,
            perms,
            bytes: None,
        });
    }
    out
}

pub(super) fn from_memory(
    id: &str,
    title: &str,
    version: &str,
    perms: &[&'static str],
    bytes: Arc<Vec<u8>>,
) -> Candidate {
    let file = format!("{id}.zip");
    let same = same_id(&folder(), id);
    Candidate {
        path: PathBuf::from(&file),
        file,
        title: title.to_string(),
        version: Some(version.to_string()),
        verdict: if same.is_empty() {
            Verdict::New
        } else {
            Verdict::Replaces(same)
        },
        id: id.to_string(),
        perms: perms.to_vec(),
        bytes: Some(bytes),
    }
}

fn id_of(c: &Candidate) -> Option<&str> {
    let f = c.file.as_str();
    Some(
        f.strip_suffix(".lua")
            .or_else(|| f.strip_suffix(".zip"))
            .unwrap_or(f),
    )
}

pub(super) fn same_id(dir: &Path, id: &str) -> Vec<String> {
    let Ok(entries) = packfs::entries(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .into_iter()
        .map(|(n, _)| n)
        .filter(|n| n == id || n == &format!("{id}.lua") || n == &format!("{id}.zip"))
        .collect();
    names.sort();
    names
}

pub fn install(candidates: Vec<Candidate>) {
    clear_note();
    super::on_worker(move || {
        let dir = folder();
        let mut problems = Vec::new();
        if let Err(e) = packfs::create_dir_all(&dir) {
            problems.push(format!("could not create {}: {e}", dir.display()));
        }
        for c in candidates.iter().filter(|c| c.installs()) {
            if let Err(e) = install_one(&dir, c) {
                crate::log_warn!("mods", "could not install {}: {e}", c.path.display());
                problems.push(format!("{}: {e}", c.file));
            } else {
                crate::log_info!("mods", "installed {} into {}", c.file, dir.display());
                for ix in c
                    .perms
                    .iter()
                    .filter_map(|p| perm::index(p))
                    .filter(|&ix| perm::PERMS[ix].on_install)
                {
                    perm::set_granted(ix, &c.id, true);
                }
            }
        }
        if !problems.is_empty() {
            set_note(format!(
                "Some mods did not install:\n{}",
                problems.join("\n")
            ));
        }
        super::enable_ids(candidates.iter().filter(|c| c.installs()).filter_map(id_of));
        let world = super::in_world();
        super::load(world, world);
    });
}

pub fn uninstall(id: String) {
    clear_note();
    super::on_worker(move || {
        let dir = folder();
        let mut problems = Vec::new();
        for name in same_id(&dir, &id) {
            let path = dir.join(&name);
            let gone = if packfs::is_dir(&path) {
                packfs::remove_dir_all(&path)
            } else {
                packfs::remove_file(&path)
            };
            if let Err(e) = gone {
                crate::log_warn!("mods", "could not remove {}: {e}", path.display());
                problems.push(format!("{name}: {e}"));
            }
        }
        for ix in 0..perm::PERMS.len() {
            perm::set_granted(ix, &id, false);
        }
        super::enable_ids([id.as_str()]);
        let world = super::in_world();
        super::load(world, world);
        super::state::delete(&id);
        if problems.is_empty() {
            crate::log_info!("mods", "uninstalled {id}");
        } else {
            set_note(format!(
                "Some files did not uninstall:\n{}",
                problems.join("\n")
            ));
        }
    });
}

fn install_one(dir: &Path, c: &Candidate) -> std::io::Result<()> {
    if let Verdict::Replaces(old) = &c.verdict {
        for name in old {
            let path = dir.join(name);
            if packfs::is_dir(&path) {
                packfs::remove_dir_all(&path)?;
            } else {
                packfs::remove_file(&path)?;
            }
        }
    }
    match &c.bytes {
        Some(bytes) => packfs::write(dir.join(&c.file), bytes.to_vec()),
        None => crate::util::pack::copy_tree(&c.path, &dir.join(&c.file)),
    }
}
