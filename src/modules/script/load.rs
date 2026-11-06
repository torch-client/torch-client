use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use mlua::{ChunkMode, Error, Lua, Table, Value};
use serde_json::Value as Json;

use super::super::commands::ScriptCommand;
use super::super::registry::{COUNT, Extra, ModuleIx, SETTING_COUNT};
use super::api::{Api, Ctx, PACKET, intern};
use super::{ModInfo, ModuleInfo, Shape, perm, run, state};
use crate::gui::hud_layout;
use crate::platform::packfs::{self, Kind};
use crate::util::pack::Source;

const API: u64 = 1;

const MANIFEST_FIELDS: [&str; 9] = [
    "id",
    "name",
    "version",
    "api",
    "license",
    "description",
    "author",
    "source",
    "permissions",
];

pub(super) struct Loaded {
    pub errors: Vec<String>,
    pub info: Vec<ModInfo>,
    pub mods: Vec<state::Mod>,
    pub runtime: Option<run::Runtime>,
    pub extras: Vec<Extra>,
    pub commands: Vec<ScriptCommand>,
}

struct Found {
    id: String,
    file: String,
    path: PathBuf,
    shape: Shape,
}

struct Files {
    id: &'static str,
    source: Option<Source>,
    cache: Mutex<HashMap<String, Option<Result<Value, String>>>>,
    env_mt: OnceLock<Table>,
    ctx: Arc<Ctx>,
}

pub(super) fn all(dir: &Path, disabled: &std::collections::BTreeSet<String>) -> Loaded {
    let ctx = Arc::new(Ctx::new());
    let (found, mut errors, mut info) = discover(dir);
    let mut loaded = Loaded {
        errors: Vec::new(),
        info: Vec::new(),
        mods: Vec::new(),
        runtime: None,
        extras: Vec::new(),
        commands: Vec::new(),
    };

    let mut apis: Vec<Api> = Vec::new();
    for f in found {
        if disabled.contains(&f.id) {
            let mut row = ModInfo::of(f.id.clone(), f.file.clone(), f.shape);
            row.disabled = true;
            if let Ok(i) = inspect(&f.path) {
                row.name = i.name;
                row.version = i.version;
                row.license = i.license;
                row.description = i.description;
                row.author = i.author;
                row.source = i.source;
            }
            crate::log_info!("mods", "{} is disabled; not loaded", f.file);
            info.push(row);
            continue;
        }
        let id = intern(f.id.clone());
        let mut row = ModInfo::of(f.id.clone(), f.file.clone(), f.shape);
        let start = {
            let mut load = ctx.load.lock().unwrap_or_else(PoisonError::into_inner);
            load.mod_id = id;
            load.defs.len()
        };
        let result = Api::new(ctx.clone())
            .map_err(|e| format!("its Lua state could not be created: {e}"))
            .and_then(|api| load_one(&api, &f, id, &mut row).map(|what| (api, what)));
        let mut load = ctx.load.lock().unwrap_or_else(PoisonError::into_inner);
        let defined = load.defs.len() - start;
        let result = result.and_then(|ok| {
            if defined == 0 {
                return Err("defines no modules: call torch.module at least once".to_string());
            }
            for d in &load.defs[start..] {
                match (!d.packets.is_empty(), d.handlers[PACKET].is_some()) {
                    (false, true) => {
                        return Err(format!(
                            "module `{}` has onpacket but lists no packets: add packets = {{ ... }} to torch.module",
                            d.name
                        ));
                    }
                    (true, false) => {
                        return Err(format!("module `{}` lists packets but has no onpacket", d.name));
                    }
                    _ => {}
                }
            }
            Ok(ok)
        });
        match result {
            Ok((api, what)) => {
                crate::log_info!("mods", "loaded {what}: {defined} module(s)");
                for d in &mut load.defs[start..] {
                    d.api = apis.len();
                }
                row.modules = load.defs[start..]
                    .iter()
                    .map(|d| ModuleInfo {
                        name: d.name,
                        mode: d.mode,
                    })
                    .collect();
                apis.push(api);
            }
            Err(e) => {
                load.defs.truncate(start);
                errors.push(format!("[mods] {} did not load: {e}", f.file));
                row.error = Some(e);
            }
        }
        info.push(row);
    }
    info.sort_by(|a, b| (&a.id, &a.file).cmp(&(&b.id, &b.file)));
    loaded.errors = errors;
    loaded.info = info;

    if let Some(e) = apis.iter().find_map(|api| api.finish_loading().err()) {
        loaded
            .errors
            .push(format!("[mods] no mod could be started: {e}"));
        hud_layout::set_mod_panels(&[]);
        return loaded;
    }
    let defs = std::mem::take(&mut ctx.load.lock().unwrap_or_else(PoisonError::into_inner).defs);
    if defs.is_empty() {
        hud_layout::set_mod_panels(&[]);
        return loaded;
    }

    let mut panels: Vec<hud_layout::ModPanel> = Vec::new();
    let mut panel_slots: Vec<(usize, &'static str, u8)> = Vec::new();
    for (k, d) in defs.iter().enumerate() {
        for p in &d.panels {
            let slot = hud_layout::LUA_FIRST + panels.len();
            if slot == hud_layout::MOD_PANELS {
                loaded.errors.push(format!(
                    "[mods] {}/{}: panel `{}` not shown; every mod together has {} HUD panels",
                    d.mod_id,
                    d.name,
                    p.name,
                    hud_layout::MOD_PANELS - hud_layout::LUA_FIRST
                ));
                panel_slots.push((k, p.name, super::game::NO_SLOT));
                continue;
            }
            panel_slots.push((k, p.name, slot as u8));
            panels.push(hud_layout::ModPanel {
                key: intern(format!("mod:{}/{}", d.mod_id, p.name)),
                label: p.label,
                w: p.w,
                h: p.h,
                anchor: p.anchor,
                offset: bevy::math::Vec2::new(p.x, p.y),
            });
        }
    }

    let mut slot = SETTING_COUNT;
    let mut labels = Vec::with_capacity(defs.len());
    let mut mod_ids = Vec::with_capacity(defs.len());
    let mut lives = Vec::with_capacity(defs.len());
    let mut mods: Vec<state::Mod> = Vec::new();
    for (k, d) in defs.into_iter().enumerate() {
        let ix = ModuleIx((COUNT + k) as u16);
        let first = slot;
        slot += d.settings.len();
        labels.push(format!("{}/{}", d.mod_id, d.name));
        mod_ids.push(d.mod_id);
        if mods.last().is_none_or(|m| m.id != d.mod_id) {
            mods.push(state::Mod {
                id: d.mod_id,
                doc: state::doc(d.mod_id),
                modules: Vec::new(),
            });
        }
        if let Some(m) = mods.last_mut() {
            m.modules.push(state::Module {
                ix,
                name: d.name,
                settings: (d.settings.iter().enumerate())
                    .map(|(i, (key, def))| (first + i, *key, def.kind))
                    .collect(),
            });
        }
        loaded.extras.push(Extra {
            name: d.name,
            desc: d.desc,
            category: d.category,
            mode: d.mode,
            settings: d.settings.iter().map(|(_, def)| *def).collect(),
        });
        loaded
            .commands
            .extend(d.commands.iter().map(|(decl, _)| ScriptCommand {
                mod_id: d.mod_id,
                module: ix,
                decl: decl.clone(),
            }));
        lives.push(run::Live::new(ix, first, d));
    }
    ctx.labels.set(labels).ok();
    ctx.mod_ids.set(mod_ids).ok();
    match run::Runtime::new(ctx.clone(), apis, lives) {
        Ok(rt) => {
            loaded.runtime = Some(rt);
            loaded.mods = mods;
            ctx.game
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .panels = panel_slots;
            hud_layout::set_mod_panels(&panels);
        }
        Err(e) => {
            loaded
                .errors
                .push(format!("[mods] no mod could be started: {e}"));
            loaded.extras.clear();
            loaded.commands.clear();
            hud_layout::set_mod_panels(&[]);
        }
    }
    loaded
}

fn discover(dir: &Path) -> (Vec<Found>, Vec<String>, Vec<ModInfo>) {
    let (mut errors, mut info) = (Vec::new(), Vec::new());
    let Ok(entries) = packfs::entries(dir) else {
        return (Vec::new(), errors, info);
    };
    let mut by_id: BTreeMap<String, Vec<Found>> = BTreeMap::new();
    for (name, kind) in entries {
        if name.starts_with('.') {
            continue;
        }
        let path = dir.join(&name);
        let kind = match kind {
            Kind::Other if packfs::is_dir(&path) => Kind::Dir,
            Kind::Other if packfs::is_file(&path) => Kind::File,
            kind => kind,
        };
        let (id, shape) = match kind {
            Kind::Dir => (name.clone(), Shape::Folder),
            Kind::File => match (name.strip_suffix(".lua"), name.strip_suffix(".zip")) {
                (Some(stem), _) => (stem.to_string(), Shape::File),
                (_, Some(stem)) => (stem.to_string(), Shape::Zip),
                _ => continue,
            },
            Kind::Other => continue,
        };
        by_id.entry(id.clone()).or_default().push(Found {
            id,
            file: name,
            path,
            shape,
        });
    }

    let mut found = Vec::new();
    for (id, mut same) in by_id {
        let error = if same.len() > 1 {
            let files: Vec<&str> = same.iter().map(|f| f.file.as_str()).collect();
            errors.push(format!(
                "[mods] {} all have the id `{id}`, so none of them loaded",
                files.join(", ")
            ));
            Some(format!(
                "{} share this id; none of them loaded",
                files.join(", ")
            ))
        } else if !valid_id(&id) {
            let e = format!("`{id}` is not a mod id ({ID_RULE})");
            errors.push(format!("[mods] {} did not load: {e}", same[0].file));
            Some(e)
        } else {
            None
        };
        match error {
            Some(e) => info.extend(same.into_iter().map(|f| ModInfo {
                error: Some(e.clone()),
                ..ModInfo::of(f.id, f.file, f.shape)
            })),
            None => found.push(same.remove(0)),
        }
    }
    (found, errors, info)
}

pub(super) struct Inspected {
    pub id: String,
    pub shape: Shape,
    pub name: Option<String>,
    pub version: Option<String>,
    pub license: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub source: Option<String>,
    pub perms: Vec<&'static str>,
}

pub(super) fn inspect(path: &Path) -> Result<Inspected, String> {
    let file = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (id, shape) = if packfs::is_dir(path) {
        (file.clone(), Shape::Folder)
    } else if let Some(stem) = file.strip_suffix(".lua") {
        (stem.to_string(), Shape::File)
    } else if let Some(stem) = file.strip_suffix(".zip") {
        (stem.to_string(), Shape::Zip)
    } else {
        return Err("not a mod: mods are a .lua file, a .zip, or a folder".to_string());
    };
    if !valid_id(&id) {
        return Err(format!("`{id}` is not a mod id: rename it to {ID_RULE}"));
    }
    let out = Inspected::bare(id, shape);
    if shape == Shape::File {
        return Ok(out);
    }
    let source = Source::open(path, "")?;
    with_manifest(out, &source)
}

pub(super) fn inspect_zip(id: &str, bytes: std::sync::Arc<Vec<u8>>) -> Result<Inspected, String> {
    let source = Source::from_shared_zip(bytes, "")?;
    with_manifest(Inspected::bare(id.to_string(), Shape::Zip), &source)
}

impl Inspected {
    fn bare(id: String, shape: Shape) -> Inspected {
        Inspected {
            id,
            shape,
            name: None,
            version: None,
            license: None,
            description: None,
            author: None,
            source: None,
            perms: Vec::new(),
        }
    }
}

fn with_manifest(mut out: Inspected, source: &Source) -> Result<Inspected, String> {
    let Some(text) = source.read_text("/mod.json") else {
        return Err(missing_manifest(source, out.shape));
    };
    let m = manifest(&text, &out.id)?;
    if source.read_text("/init.lua").is_none() {
        return Err("has no init.lua at its top level".to_string());
    }
    out.name = Some(m.name);
    out.version = Some(m.version);
    out.license = m.license;
    out.description = m.description;
    out.author = m.author;
    out.source = m.source;
    out.perms = m.perms.iter().map(|&ix| perm::PERMS[ix].name).collect();
    Ok(out)
}

pub(super) const ID_RULE: &str =
    "2 to 64 characters: a lowercase letter, then lowercase letters, digits, `-` and `_`";

pub(super) fn valid_id(id: &str) -> bool {
    let b = id.as_bytes();
    (2..=64).contains(&b.len())
        && b[0].is_ascii_lowercase()
        && b[1..]
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-' || *c == b'_')
}

fn load_one(api: &Api, f: &Found, id: &'static str, row: &mut ModInfo) -> Result<String, String> {
    if f.shape == Shape::File {
        let bytes = packfs::read(&f.path).map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(crate::platform::assets::skip_bom(&bytes)).into_owned();
        let files = Files::new(api, id, None).map_err(|e| e.to_string())?;
        run_file(&api.lua, &files, &format!("@{id}.lua"), text).map_err(|e| e.to_string())?;
        return Ok(id.to_string());
    }

    let source = Source::open(&f.path, "")?;
    let Some(text) = source.read_text("/mod.json") else {
        return Err(missing_manifest(&source, f.shape));
    };
    let m = manifest(&text, id)?;
    let what = format!("{} {} ({id})", m.name, m.version);
    row.name = Some(m.name);
    row.version = Some(m.version);
    row.license = m.license;
    row.description = m.description;
    row.author = m.author;
    row.source = m.source;
    let mut declared = api.ctx.perms.lock().unwrap_or_else(PoisonError::into_inner);
    for &ix in &m.perms {
        declared.insert((id, ix));
        row.perms.push(super::Grant::of(ix, id));
    }
    drop(declared);
    let init = source
        .read_text("/init.lua")
        .ok_or("has no init.lua at its top level")?;
    let files = Files::new(api, id, Some(source)).map_err(|e| e.to_string())?;
    run_file(&api.lua, &files, &format!("@{id}/init.lua"), init).map_err(|e| e.to_string())?;
    Ok(what)
}

fn missing_manifest(source: &Source, shape: Shape) -> String {
    let wrapped = source.files().into_iter().find_map(|p| {
        p.strip_suffix("/mod.json")
            .map(str::to_string)
            .filter(|dir| dir.matches('/').count() == 1)
    });
    match (shape, wrapped) {
        (Shape::Zip, Some(dir)) => format!(
            "mod.json is inside the folder `{}` in the zip; mod.json and init.lua go at the zip's top level \
             (zip the files, not the folder holding them)",
            dir.trim_start_matches('/')
        ),
        _ => "has no mod.json at its top level".to_string(),
    }
}

#[derive(Debug)]
struct Manifest {
    name: String,
    version: String,
    license: Option<String>,
    description: Option<String>,
    author: Option<String>,
    source: Option<String>,
    perms: Vec<usize>,
}

const DESCRIPTION_MAX: usize = 300;
const AUTHOR_MAX: usize = 100;
const SOURCE_MAX: usize = 300;

fn manifest(text: &str, id: &str) -> Result<Manifest, String> {
    let json: Json =
        serde_json::from_str(text).map_err(|e| format!("mod.json is not valid JSON: {e}"))?;
    let Json::Object(m) = json else {
        return Err("mod.json is not a JSON object".to_string());
    };
    for key in m.keys() {
        if !MANIFEST_FIELDS.contains(&key.as_str()) {
            return Err(format!(
                "mod.json: unknown field `{key}`{}",
                super::api::suggest(key, &MANIFEST_FIELDS)
            ));
        }
    }
    fn string<'a>(
        m: &'a serde_json::Map<String, Json>,
        key: &str,
    ) -> Result<Option<&'a str>, String> {
        match m.get(key) {
            None => Ok(None),
            Some(Json::String(s)) => Ok(Some(s)),
            Some(_) => Err(format!("mod.json: `{key}` is a string")),
        }
    }
    let required =
        |key: &str| string(&m, key)?.ok_or_else(|| format!("mod.json: `{key}` is required"));

    let manifest_id = required("id")?;
    if !valid_id(manifest_id) {
        return Err(format!("mod.json: `{manifest_id}` is not a mod id"));
    }
    if manifest_id != id {
        return Err(format!(
            "mod.json says its id is `{manifest_id}`, but the folder or zip is named `{id}`; the two must match"
        ));
    }
    let name = required("name")?;
    let version = required("version")?;
    let semver = version.split('.').collect::<Vec<_>>();
    if semver.len() != 3
        || semver
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|c| c.is_ascii_digit()))
    {
        return Err(format!(
            "mod.json: version `{version}` is not MAJOR.MINOR.PATCH"
        ));
    }
    match m.get("api") {
        Some(Json::Number(n)) if n.as_u64() == Some(API) => {}
        Some(Json::Number(n)) => {
            return Err(format!(
                "needs API {n}, and this client implements API {API}"
            ));
        }
        Some(_) => return Err("mod.json: `api` is a whole number".to_string()),
        None => return Err("mod.json: `api` is required".to_string()),
    }
    let license = string(&m, "license")?.map(str::to_string);
    let text = |key: &str, max: usize| -> Result<Option<String>, String> {
        let Some(t) = string(&m, key)?.map(str::trim).filter(|t| !t.is_empty()) else {
            return Ok(None);
        };
        let n = t.chars().count();
        if n > max {
            return Err(format!(
                "mod.json: `{key}` is {n} characters; at most {max}"
            ));
        }
        Ok(Some(t.to_string()))
    };
    let description = text("description", DESCRIPTION_MAX)?;
    let author = text("author", AUTHOR_MAX)?;
    let source = text("source", SOURCE_MAX)?;
    let mut perms = Vec::new();
    match m.get("permissions") {
        None => {}
        Some(Json::Array(list)) => {
            for p in list {
                let Json::String(p) = p else {
                    return Err("mod.json: `permissions` is a list of strings".to_string());
                };
                let Some(ix) = perm::index(p) else {
                    let names = perm::names();
                    return Err(format!(
                        "mod.json: `{p}` is not a permission{}; there are {}",
                        super::api::suggest(p, &names),
                        names.join(", ")
                    ));
                };
                if !perms.contains(&ix) {
                    perms.push(ix);
                }
            }
        }
        Some(_) => return Err("mod.json: `permissions` is a list of strings".to_string()),
    }
    Ok(Manifest {
        name: name.to_string(),
        version: version.to_string(),
        license,
        description,
        author,
        source,
        perms,
    })
}

impl Files {
    fn new(api: &Api, id: &'static str, source: Option<Source>) -> mlua::Result<Arc<Files>> {
        let files = Arc::new(Files {
            id,
            source,
            cache: Mutex::new(HashMap::new()),
            env_mt: OnceLock::new(),
            ctx: api.ctx.clone(),
        });
        let base = api.lua.create_table()?;
        base.raw_set("require", require_fn(api, &files)?)?;
        let base_mt = api.lua.create_table()?;
        base_mt.raw_set("__index", &api.globals)?;
        base.set_metatable(Some(base_mt))?;
        let env_mt = api.lua.create_table()?;
        env_mt.raw_set("__index", &base)?;
        files.env_mt.set(env_mt).ok();
        Ok(files)
    }
}

fn run_file(lua: &Lua, files: &Files, chunk: &str, text: String) -> mlua::Result<Value> {
    let env = lua.create_table()?;
    env.set_metatable(files.env_mt.get().cloned())?;
    let value: Value = files.ctx.top_level(|| {
        lua.load(text)
            .set_name(chunk)
            .set_mode(ChunkMode::Text)
            .set_environment(env)
            .call(())
    })?;
    Ok(if value.is_nil() {
        Value::Boolean(true)
    } else {
        value
    })
}

fn require_fn(api: &Api, files: &Arc<Files>) -> mlua::Result<mlua::Function> {
    let torch = api.torch.clone();
    let files = files.clone();
    api.lua.create_function(move |lua, name: String| {
        if name == "torch" {
            return Ok(Value::Table(torch.clone()));
        }
        let Some(source) = &files.source else {
            return Err(Error::runtime(format!(
                "require(\"{name}\"): a single-file mod has no other files to require"
            )));
        };
        let parts: Vec<&str> = name.split('.').collect();
        if name == "init" || parts.iter().any(|p| p.is_empty() || p.contains(['/', '\\', ':'])) {
            return Err(Error::runtime(format!(
                "require(\"{name}\"): not a module name in this mod (`utils.util` reads utils/util.lua; \
                 `init` and paths are not allowed)"
            )));
        }
        let path = format!("/{}.lua", parts.join("/"));

        match files.cache.lock().unwrap_or_else(PoisonError::into_inner).get(&name) {
            Some(Some(Ok(value))) => return Ok(value.clone()),
            Some(Some(Err(e))) => return Err(Error::runtime(e.clone())),
            Some(None) => {
                return Err(Error::runtime(format!("require(\"{name}\"): required again while it is still loading")));
            }
            None => {}
        }
        let Some(text) = source.read_text(&path) else {
            return Err(Error::runtime(format!("require(\"{name}\"): {}{path} does not exist", files.id)));
        };
        files.cache.lock().unwrap_or_else(PoisonError::into_inner).insert(name.clone(), None);
        let result = run_file(lua, &files, &format!("@{}{path}", files.id), text);
        let mut cache = files.cache.lock().unwrap_or_else(PoisonError::into_inner);
        match result {
            Ok(value) => {
                cache.insert(name, Some(Ok(value.clone())));
                Ok(value)
            }
            Err(e) => {
                cache.insert(name.clone(), Some(Err(format!("require(\"{name}\") failed earlier: {e}"))));
                Err(e)
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_ids_follow_the_rule() {
        assert!(valid_id("autogg"));
        assert!(valid_id("a1"));
        assert!(valid_id("low-health_2"));
        assert!(!valid_id("a"));
        assert!(!valid_id("1ab"));
        assert!(!valid_id("AutoGG"));
        assert!(!valid_id("auto.gg"));
        assert!(!valid_id(&"a".repeat(65)));
    }

    #[test]
    fn a_manifest_is_checked_field_by_field() {
        let ok = r#"{ "id": "autogg", "name": "Auto GG", "version": "1.0.0", "api": 1, "license": "AGPL-3.0" }"#;
        assert_eq!(
            manifest(ok, "autogg").map(|m| m.license),
            Ok(Some("AGPL-3.0".to_string()))
        );
        assert!(manifest(ok, "other").is_err(), "the folder name must match");
        let described = r#"{ "id": "autogg", "name": "x", "version": "1.0.0", "api": 1, "description": " Says gg " }"#;
        assert_eq!(
            manifest(described, "autogg").map(|m| m.description),
            Ok(Some("Says gg".to_string()))
        );
        let long = format!(
            r#"{{ "id": "autogg", "name": "x", "version": "1.0.0", "api": 1, "description": "{}" }}"#,
            "a".repeat(DESCRIPTION_MAX + 1)
        );
        assert!(manifest(&long, "autogg").is_err());
        let credited = r#"{ "id": "autogg", "name": "x", "version": "1.0.0", "api": 1,
            "author": " Someone ", "source": "https://example.com/autogg" }"#;
        assert_eq!(
            manifest(credited, "autogg").map(|m| (m.author, m.source)),
            Ok((
                Some("Someone".to_string()),
                Some("https://example.com/autogg".to_string())
            ))
        );
        let blank = r#"{ "id": "autogg", "name": "x", "version": "1.0.0", "api": 1, "author": "  ", "source": "" }"#;
        assert_eq!(
            manifest(blank, "autogg").map(|m| (m.author, m.source)),
            Ok((None, None))
        );
        let numeric =
            r#"{ "id": "autogg", "name": "x", "version": "1.0.0", "api": 1, "author": 5 }"#;
        assert!(manifest(numeric, "autogg").is_err());
        let extra = r#"{ "id": "autogg", "name": "x", "version": "1.0.0", "api": 1, "nmae": "x" }"#;
        assert!(
            manifest(extra, "autogg")
                .unwrap_err()
                .contains("did you mean `name`")
        );
        let api = r#"{ "id": "autogg", "name": "x", "version": "1.0.0", "api": 2 }"#;
        assert!(manifest(api, "autogg").is_err());
        let semver = r#"{ "id": "autogg", "name": "x", "version": "1.0", "api": 1 }"#;
        assert!(manifest(semver, "autogg").is_err());
    }
}
