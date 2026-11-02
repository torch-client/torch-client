use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use mlua::{Error, Lua, Table, Value};

use super::api::Ctx;
use super::game::{FileAsk, Imported, UI_TEXT, UiForm, game_fn, show_ui};
use super::perm;
use crate::platform::packfs::{self, Kind};
use crate::platform::time::UNIX_EPOCH;

pub(super) const QUOTA: u64 = 64 << 20;
pub(super) const LARGE_QUOTA: u64 = 1 << 30;
const LARGE_ENTRIES: u32 = 65536;
const BLOCK: u64 = 4096;
const MAX_ENTRIES: u32 = 4096;
const MAX_DEPTH: usize = 8;
const MAX_PART: usize = 64;
const MAX_PATH: usize = 255;
const MAX_IO: usize = 4 << 20;
const MAX_READ: u64 = 64 << 20;

fn limits(id: &str) -> (u64, u32) {
    if perm::large_files(id) {
        (LARGE_QUOTA, LARGE_ENTRIES)
    } else {
        (QUOTA, MAX_ENTRIES)
    }
}
const TMP: &str = ".tmp-";

#[derive(Clone, Copy, Default)]
struct Usage {
    bytes: u64,
    entries: u32,
}

static USAGE: Mutex<Option<HashMap<&'static str, Usage>>> = Mutex::new(None);

pub(super) fn forget() {
    *USAGE.lock().unwrap_or_else(PoisonError::into_inner) = None;
}

fn charge(len: u64) -> u64 {
    len.div_ceil(BLOCK).max(1) * BLOCK
}

fn root(id: &str) -> PathBuf {
    packfs::root().join(super::MOD_DATA).join(id)
}

fn usage(id: &'static str) -> Usage {
    let mut all = USAGE.lock().unwrap_or_else(PoisonError::into_inner);
    *all.get_or_insert_with(HashMap::new)
        .entry(id)
        .or_insert_with(|| {
            let r = root(id);
            let mut u = Usage::default();
            if is_dir(&r) {
                u.bytes += BLOCK;
                u.entries += 1;
                walk(&r, &mut u, MAX_DEPTH * 4);
            }
            u
        })
}

fn walk(dir: &Path, u: &mut Usage, budget: usize) {
    if budget == 0 {
        return;
    }
    let Ok(entries) = packfs::entries(dir) else {
        return;
    };
    for (name, kind) in entries {
        let path = dir.join(&name);
        if name.starts_with(TMP) && kind == Kind::File {
            let _ = packfs::remove_file(&path);
            continue;
        }
        u.entries += 1;
        match kind {
            Kind::Dir => {
                u.bytes += BLOCK;
                walk(&path, u, budget - 1);
            }
            Kind::File => u.bytes += charge(packfs::len(&path).unwrap_or(0)),
            Kind::Other => u.bytes += charge(0),
        }
    }
}

fn adjust(id: &'static str, bytes: i64, entries: i32) {
    let mut all = USAGE.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(u) = all.as_mut().and_then(|m| m.get_mut(id)) {
        u.bytes = u.bytes.saturating_add_signed(bytes);
        u.entries = u.entries.saturating_add_signed(entries);
    }
}

fn is_dir(p: &Path) -> bool {
    packfs::kind(p) == Some(Kind::Dir)
}

const RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

fn parts<'a>(path: &'a str, what: &str) -> mlua::Result<Vec<&'a str>> {
    let bad = |why: &str| Error::runtime(format!("{what}: `{path}` {why}"));
    if path.len() > MAX_PATH {
        return Err(bad(&format!("is longer than {MAX_PATH} bytes")));
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() > MAX_DEPTH {
        return Err(bad(&format!("is more than {MAX_DEPTH} levels deep")));
    }
    for p in &parts {
        if p.is_empty() || *p == "." || *p == ".." {
            return Err(bad("is not a relative path of names separated by /"));
        }
        if p.len() > MAX_PART {
            return Err(bad(&format!("has a name longer than {MAX_PART} bytes")));
        }
        if !p
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b" ._-".contains(&c))
        {
            return Err(bad("has a character outside A-Z a-z 0-9 space . _ -"));
        }
        if p.starts_with(['.', ' ']) || p.ends_with(['.', ' ']) {
            return Err(bad("has a name that starts or ends with . or a space"));
        }
        let stem = p.split('.').next().unwrap_or(p).to_ascii_lowercase();
        if RESERVED.contains(&stem.as_str()) {
            return Err(bad("uses a name Windows reserves"));
        }
    }
    Ok(parts)
}

fn on_disk(root: &Path, parts: &[&str]) -> Option<String> {
    if packfs::kind(root).is_some_and(|k| k != Kind::Dir) {
        return Some("the mod's folder is not a folder".to_string());
    }
    let mut at = root.to_path_buf();
    for (i, p) in parts.iter().enumerate() {
        if let Ok(entries) = packfs::entries(&at) {
            for (n, _) in entries {
                if n != **p && n.eq_ignore_ascii_case(p) {
                    return Some(format!("`{n}` already exists with other case"));
                }
            }
        }
        at.push(p);
        match packfs::kind(&at) {
            Some(Kind::Other) if packfs::is_link(&at) => {
                return Some("goes through a link".to_string());
            }
            Some(k) if i + 1 < parts.len() && k != Kind::Dir => {
                return Some("goes through a file".to_string());
            }
            Some(Kind::Other) => return Some("is not a file or folder".to_string()),
            _ => {}
        }
    }
    None
}

type Out = (Value, Option<String>);

fn fail(why: impl Into<String>) -> mlua::Result<Out> {
    Ok((Value::Nil, Some(why.into())))
}

fn ok(v: impl mlua::IntoLua, lua: &Lua) -> mlua::Result<Out> {
    Ok((v.into_lua(lua)?, None))
}

fn gate(ctx: &Ctx, what: &str) -> mlua::Result<Result<&'static str, String>> {
    let (_, id) = ctx.caller(what)?;
    if !perm::declared(ctx, perm::FILES, id) {
        return Err(perm::undeclared(what, perm::FILES));
    }
    Ok(if perm::granted(perm::FILES, id) {
        Ok(id)
    } else {
        Err(format!(
            "the player has not allowed files for {id} (Mods tab)"
        ))
    })
}

fn open<'a>(
    ctx: &Ctx,
    path: &'a str,
    what: &str,
) -> mlua::Result<Result<(&'static str, PathBuf, Vec<&'a str>), String>> {
    let parts = parts(path, what)?;
    let id = match gate(ctx, what)? {
        Ok(id) => id,
        Err(why) => return Ok(Err(why)),
    };
    let root = root(id);
    if let Some(why) = on_disk(&root, &parts) {
        return Ok(Err(format!("`{path}` {why}")));
    }
    Ok(Ok((id, root, parts)))
}

pub(super) fn existing_file(
    ctx: &Ctx,
    path: &str,
    what: &str,
) -> mlua::Result<Result<PathBuf, String>> {
    let (_, root, parts) = match open(ctx, path, what)? {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    let at = join(&root, &parts);
    Ok(match packfs::kind(&at) {
        Some(Kind::File) => Ok(at),
        _ => Err(format!("`{path}` is not a file")),
    })
}

pub(super) struct Target {
    id: &'static str,
    root: PathBuf,
    parts: Vec<String>,
    path: String,
}

pub(super) fn writable(ctx: &Ctx, path: &str, what: &str) -> mlua::Result<Result<Target, String>> {
    let (id, root, parts) = match open(ctx, path, what)? {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    if packfs::kind(&join(&root, &parts)).is_some_and(|k| k != Kind::File) {
        return Ok(Err(format!("`{path}` is a folder")));
    }
    let parts = parts.into_iter().map(str::to_string).collect();
    Ok(Ok(Target {
        id,
        root,
        parts,
        path: path.to_string(),
    }))
}

pub(super) fn write_to(t: &Target, bytes: &[u8]) -> Result<(), String> {
    let parts: Vec<&str> = t.parts.iter().map(String::as_str).collect();
    write_at(t.id, &t.root, &parts, &t.path, bytes)
}

fn write_at(
    id: &'static str,
    root: &Path,
    parts: &[&str],
    path: &str,
    bytes: &[u8],
) -> Result<(), String> {
    let at = join(root, parts);
    let old = match packfs::kind(&at) {
        Some(Kind::File) => Some(charge(packfs::len(&at).unwrap_or(0))),
        Some(_) => return Err(format!("`{path}` is a folder")),
        None => None,
    };
    let new = charge(bytes.len() as u64);
    let entries = if old.is_none() { 1 } else { 0 };
    let delta = new as i64 - old.unwrap_or(0) as i64;
    make_room(id, root, parts, delta, entries)?;
    atomic_write(&at, bytes).map_err(|e| e.to_string())?;
    adjust(id, delta, entries);
    Ok(())
}

fn join(root: &Path, parts: &[&str]) -> PathBuf {
    parts.iter().fold(root.to_path_buf(), |p, s| p.join(s))
}

fn make_room(
    id: &'static str,
    root: &Path,
    parts: &[&str],
    bytes: i64,
    entries: i32,
) -> Result<(), String> {
    if parts.len() > 1 {
        let parent = &parts[..parts.len() - 1];
        if !is_dir(&join(root, parent)) {
            return Err(format!("`{}` does not exist", parent.join("/")));
        }
        return room(id, bytes, entries);
    }
    if is_dir(root) {
        return room(id, bytes, entries);
    }
    room(id, bytes + BLOCK as i64, entries + 1)?;
    packfs::create_dir_all(root).map_err(|e| e.to_string())?;
    adjust(id, BLOCK as i64, 1);
    Ok(())
}

fn levels(p: &Path, budget: usize) -> usize {
    if budget == 0 {
        return 0;
    }
    if !is_dir(p) {
        return 1;
    }
    let below = packfs::entries(p)
        .map(|entries| {
            entries
                .iter()
                .map(|(name, _)| levels(&p.join(name), budget - 1))
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    1 + below
}

fn room(id: &'static str, add_bytes: i64, add_entries: i32) -> Result<(), String> {
    let (quota, max_entries) = limits(id);
    let u = usage(id);
    if add_entries > 0 && u.entries as i64 + add_entries as i64 > max_entries as i64 {
        return Err(format!("more than {max_entries} files and folders"));
    }
    if add_bytes > 0 && u.bytes as i64 + add_bytes > quota as i64 {
        return Err(format!("over the {} MiB quota", quota >> 20));
    }
    Ok(())
}

fn stat(lua: &Lua, path: &Path) -> mlua::Result<Option<Table>> {
    let dir = match packfs::kind(path) {
        Some(Kind::Dir) => true,
        Some(Kind::File) => false,
        _ => return Ok(None),
    };
    let t = lua.create_table()?;
    t.raw_set("kind", if dir { "dir" } else { "file" })?;
    t.raw_set(
        "size",
        if dir {
            0
        } else {
            packfs::len(path).unwrap_or(0)
        },
    )?;
    let mtime = packfs::modified(path)
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    t.raw_set("mtime", mtime)?;
    Ok(Some(t))
}

#[cfg(not(target_arch = "wasm32"))]
fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::fs::{self, File};
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let dir = path.parent().unwrap_or(Path::new("."));
    let tmp = dir.join(format!(
        "{TMP}{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Relaxed)
    ));
    let result = (|| {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, path)?;
        if let Ok(d) = File::open(dir) {
            let _ = d.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(target_arch = "wasm32")]
fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    packfs::write(path, bytes.to_vec())
}

macro_rules! tri {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(why) => return fail(why),
        }
    };
}

pub(super) fn table(lua: &Lua, ctx: &Arc<Ctx>) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.raw_set(
        "list",
        game_fn(
            lua,
            ctx,
            "torch.fs.list",
            |lua, ctx, dir: Option<String>| {
                let what = "torch.fs.list";
                let (root, parts) = match dir.as_deref() {
                    None | Some("") => {
                        let id = tri!(gate(ctx, what)?);
                        (root(id), Vec::new())
                    }
                    Some(dir) => {
                        let (_, root, parts) = tri!(open(ctx, dir, what)?);
                        (root, parts)
                    }
                };
                let at = join(&root, &parts);
                if !is_dir(&at) {
                    return match parts.is_empty() {
                        true => ok(lua.create_table()?, lua),
                        false => fail(format!("`{}` is not a folder", parts.join("/"))),
                    };
                }
                let mut names: Vec<String> = tri!(packfs::entries(&at).map_err(|e| e.to_string()))
                    .into_iter()
                    .map(|(n, _)| n)
                    .filter(|n| !n.starts_with('.'))
                    .collect();
                names.sort();
                let out = lua.create_table()?;
                for n in names.into_iter().take(LARGE_ENTRIES as usize) {
                    if let Some(s) = stat(lua, &at.join(&n))? {
                        s.raw_set("name", n)?;
                        out.raw_set(out.raw_len() + 1, s)?;
                    }
                }
                ok(out, lua)
            },
        )?,
    )?;
    t.raw_set(
        "stat",
        game_fn(lua, ctx, "torch.fs.stat", |lua, ctx, path: String| {
            let (_, root, parts) = tri!(open(ctx, &path, "torch.fs.stat")?);
            ok(stat(lua, &join(&root, &parts))?, lua)
        })?,
    )?;
    t.raw_set(
        "read",
        game_fn(lua, ctx, "torch.fs.read", |lua, ctx, path: String| {
            let (_, root, parts) = tri!(open(ctx, &path, "torch.fs.read")?);
            let at = join(&root, &parts);
            match packfs::kind(&at) {
                Some(Kind::File) if packfs::len(&at).unwrap_or(0) > MAX_READ => {
                    return fail(format!("`{path}` is over {} MiB", MAX_READ >> 20));
                }
                Some(Kind::File) => {}
                _ => return fail(format!("`{path}` is not a file")),
            }
            let bytes = tri!(packfs::read(&at).map_err(|e| e.to_string()));
            ok(lua.create_string(&bytes)?, lua)
        })?,
    )?;
    t.raw_set(
        "write",
        game_fn(
            lua,
            ctx,
            "torch.fs.write",
            |lua, ctx, (path, data): (String, mlua::String)| {
                let what = "torch.fs.write";
                let bytes = data.as_bytes();
                if bytes.len() > MAX_IO {
                    return Err(Error::runtime(format!(
                        "{what}: more than {} MiB in one write",
                        MAX_IO >> 20
                    )));
                }
                let (id, root, parts) = tri!(open(ctx, &path, what)?);
                tri!(write_at(id, &root, &parts, &path, &bytes));
                ok(true, lua)
            },
        )?,
    )?;
    t.raw_set(
        "append",
        game_fn(
            lua,
            ctx,
            "torch.fs.append",
            |lua, ctx, (path, data): (String, mlua::String)| {
                let what = "torch.fs.append";
                let bytes = data.as_bytes();
                if bytes.len() > MAX_IO {
                    return Err(Error::runtime(format!(
                        "{what}: more than {} MiB in one append",
                        MAX_IO >> 20
                    )));
                }
                let (id, root, parts) = tri!(open(ctx, &path, what)?);
                let at = join(&root, &parts);
                let len = match packfs::kind(&at) {
                    Some(Kind::File) => Some(packfs::len(&at).unwrap_or(0)),
                    Some(_) => return fail(format!("`{path}` is a folder")),
                    None => None,
                };
                let before = len.map_or(0, charge);
                let delta = charge(len.unwrap_or(0) + bytes.len() as u64) as i64 - before as i64;
                let entries = if len.is_none() { 1 } else { 0 };
                tri!(make_room(id, &root, &parts, delta, entries));
                tri!(packfs::append(&at, &bytes).map_err(|e| e.to_string()));
                adjust(id, delta, entries);
                ok(true, lua)
            },
        )?,
    )?;
    t.raw_set(
        "mkdir",
        game_fn(lua, ctx, "torch.fs.mkdir", |lua, ctx, path: String| {
            let (id, root, parts) = tri!(open(ctx, &path, "torch.fs.mkdir")?);
            let at = join(&root, &parts);
            match packfs::kind(&at) {
                Some(Kind::Dir) => return ok(true, lua),
                Some(_) => return fail(format!("`{path}` is a file")),
                None => {}
            }
            tri!(make_room(id, &root, &parts, BLOCK as i64, 1));
            tri!(packfs::create_dir(&at).map_err(|e| e.to_string()));
            adjust(id, BLOCK as i64, 1);
            ok(true, lua)
        })?,
    )?;
    t.raw_set(
        "remove",
        game_fn(lua, ctx, "torch.fs.remove", |lua, ctx, path: String| {
            let (id, root, parts) = tri!(open(ctx, &path, "torch.fs.remove")?);
            let at = join(&root, &parts);
            let freed = match packfs::kind(&at) {
                Some(Kind::Dir) => {
                    tri!(
                        packfs::remove_dir(&at)
                            .map_err(|_| format!("`{path}` is not an empty folder"))
                    );
                    BLOCK
                }
                Some(kind) => {
                    let len = if kind == Kind::File {
                        packfs::len(&at).unwrap_or(0)
                    } else {
                        0
                    };
                    tri!(packfs::remove_file(&at).map_err(|e| e.to_string()));
                    charge(len)
                }
                None => return fail(format!("`{path}` does not exist")),
            };
            adjust(id, -(freed as i64), -1);
            ok(true, lua)
        })?,
    )?;
    t.raw_set(
        "rename",
        game_fn(
            lua,
            ctx,
            "torch.fs.rename",
            |lua, ctx, (from, to): (String, String)| {
                let what = "torch.fs.rename";
                let to_parts = parts(&to, what)?;
                let (_, root, from_parts) = tri!(open(ctx, &from, what)?);
                if let Some(why) = on_disk(&root, &to_parts) {
                    return fail(format!("`{to}` {why}"));
                }
                let (src, dst) = (join(&root, &from_parts), join(&root, &to_parts));
                if packfs::kind(&src).is_none() {
                    return fail(format!("`{from}` does not exist"));
                }
                if packfs::kind(&dst).is_some() {
                    return fail(format!("`{to}` already exists"));
                }
                if to_parts.len() > 1 && !is_dir(&join(&root, &to_parts[..to_parts.len() - 1])) {
                    return fail(format!(
                        "`{}` does not exist",
                        to_parts[..to_parts.len() - 1].join("/")
                    ));
                }
                if to_parts.starts_with(&from_parts) {
                    return fail(format!("`{to}` is inside `{from}`"));
                }
                if to_parts.len() - 1 + levels(&src, MAX_DEPTH + 1) > MAX_DEPTH {
                    return fail(format!(
                        "`{to}` would put entries more than {MAX_DEPTH} levels deep"
                    ));
                }
                tri!(packfs::rename(&src, &dst).map_err(|e| e.to_string()));
                ok(true, lua)
            },
        )?,
    )?;
    t.raw_set(
        "usage",
        game_fn(lua, ctx, "torch.fs.usage", |lua, ctx, ()| {
            let id = tri!(gate(ctx, "torch.fs.usage")?);
            let (quota, max_entries) = limits(id);
            let u = usage(id);
            let out = lua.create_table()?;
            out.raw_set("bytes", u.bytes)?;
            out.raw_set("quota", quota)?;
            out.raw_set("entries", u.entries)?;
            out.raw_set("maxentries", max_entries)?;
            ok(out, lua)
        })?,
    )?;
    t.raw_set(
        "request",
        game_fn(
            lua,
            ctx,
            "torch.fs.request",
            |lua, ctx, spec: Option<Table>| request(lua, ctx, spec),
        )?,
    )?;
    t.set_readonly(true);
    Ok(t)
}

const ACCEPT_MAX: usize = 8;
const EXT_MAX: usize = 16;

fn request(lua: &Lua, ctx: &Ctx, spec: Option<Table>) -> mlua::Result<Out> {
    let what = "torch.fs.request";
    const KEYS: [&str; 3] = ["title", "accept", "into"];
    let (k, _) = ctx.caller(what)?;
    let spec = match spec {
        Some(t) => t,
        None => lua.create_table()?,
    };
    for pair in spec.pairs::<String, Value>() {
        let (key, _) = pair?;
        if !KEYS.contains(&key.as_str()) {
            return Err(Error::runtime(format!(
                "{what}: takes {{ title = ..., accept = {{ \".ext\", ... }}, into = ... }}; `{key}` is not one of them{}",
                super::api::suggest(&key, &KEYS)
            )));
        }
    }
    let into: String = spec.raw_get::<Option<String>>("into")?.unwrap_or_default();
    if !into.is_empty() {
        let n = parts(&into, what)?.len();
        if n + 1 > MAX_DEPTH || into.len() + 1 + MAX_PART > MAX_PATH {
            return Err(Error::runtime(format!(
                "{what}: `into` leaves no room under the path limits for a file name"
            )));
        }
    }
    let mut accept = Vec::new();
    if let Some(list) = spec.raw_get::<Option<Table>>("accept")? {
        for ext in list.sequence_values::<String>() {
            let ext = ext?.to_ascii_lowercase();
            let body = ext.strip_prefix('.').unwrap_or("");
            if body.is_empty()
                || ext.len() > EXT_MAX
                || !body.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'.')
            {
                return Err(Error::runtime(format!(
                    "{what}: `{ext}` is not an extension; one is a dot and up to {} letters or digits",
                    EXT_MAX - 1
                )));
            }
            if accept.len() == ACCEPT_MAX {
                return Err(Error::runtime(format!(
                    "{what}: at most {ACCEPT_MAX} extensions"
                )));
            }
            accept.push(ext);
        }
    }
    let id = tri!(gate(ctx, what)?);
    let title: String = spec
        .raw_get::<Option<String>>("title")?
        .unwrap_or_default()
        .chars()
        .take(UI_TEXT)
        .collect();
    let title = if title.is_empty() {
        "Choose a file".to_string()
    } else {
        title
    };
    show_ui(
        k,
        title,
        String::new(),
        Vec::new(),
        UiForm::File(FileAsk {
            mod_id: id,
            into,
            accept,
        }),
    );
    ok(true, lua)
}

fn clean_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let mapped: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || " ._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut s = mapped.trim_matches(['.', ' ']).to_string();
    if s.len() > MAX_PART {
        let ext = s
            .rfind('.')
            .map(|i| s[i..].to_string())
            .filter(|e| e.len() <= EXT_MAX)
            .unwrap_or_default();
        s = format!(
            "{}{ext}",
            s[..MAX_PART - ext.len()].trim_end_matches(['.', ' '])
        );
    }
    let stem = s.split('.').next().unwrap_or(&s).to_ascii_lowercase();
    if s.is_empty() || RESERVED.contains(&stem.as_str()) {
        s = format!("_{s}");
        s.truncate(MAX_PART);
    }
    s
}

pub(super) fn import(ask: &FileAsk, name: &str, bytes: &[u8]) -> Result<Imported, String> {
    let id = ask.mod_id;
    if !perm::granted(perm::FILES, id) {
        return Err(format!("files are blocked for {id} (Mods tab)"));
    }
    let name = clean_name(name);
    let lower = name.to_ascii_lowercase();
    if !ask.accept.is_empty() && !ask.accept.iter().any(|e| lower.ends_with(e.as_str())) {
        return Err(format!("not a {} file", ask.accept.join(" or ")));
    }
    let path = if ask.into.is_empty() {
        name.clone()
    } else {
        format!("{}/{name}", ask.into)
    };
    let parts = parts(&path, "torch.fs.request").map_err(|e| e.to_string())?;
    let root = root(id);
    if let Some(why) = on_disk(&root, &parts) {
        return Err(format!("`{path}` {why}"));
    }
    let mut missing = Vec::new();
    let mut at = root.clone();
    if !is_dir(&at) {
        missing.push(at.clone());
    }
    for p in &parts[..parts.len() - 1] {
        at.push(p);
        if !is_dir(&at) {
            missing.push(at.clone());
        }
    }
    let at = join(&root, &parts);
    let old = match packfs::kind(&at) {
        Some(Kind::File) => Some(charge(packfs::len(&at).unwrap_or(0))),
        Some(_) => return Err(format!("`{path}` is a folder")),
        None => None,
    };
    let dirs = missing.len() as i64;
    let entries = dirs as i32 + i32::from(old.is_none());
    let delta = dirs * BLOCK as i64 + charge(bytes.len() as u64) as i64 - old.unwrap_or(0) as i64;
    room(id, delta, entries)?;
    if let Some(deepest) = missing.last() {
        packfs::create_dir_all(deepest).map_err(|e| e.to_string())?;
    }
    atomic_write(&at, bytes).map_err(|e| e.to_string())?;
    adjust(id, delta, entries);
    crate::log_info!(
        "mods",
        "{id}: the player gave it {path} ({} bytes)",
        bytes.len()
    );
    Ok(Imported {
        path,
        name,
        size: bytes.len() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::clean_name;

    #[test]
    fn a_chosen_name_becomes_one_path_part() {
        assert_eq!(
            clean_name("C:\\Users\\me\\My Castle (v2).schem"),
            "My Castle _v2_.schem"
        );
        assert_eq!(clean_name("/home/me/house.litematic"), "house.litematic");
        assert_eq!(clean_name(".hidden."), "hidden");
        assert_eq!(clean_name("con.schem"), "_con.schem");
        assert_eq!(clean_name("..."), "_");
        assert_eq!(clean_name("schloß.nbt"), "schlo_.nbt");
        let long = format!("{}.litematic", "a".repeat(80));
        let cut = clean_name(&long);
        assert_eq!(cut.len(), 64);
        assert!(cut.ends_with(".litematic"));
    }
}
