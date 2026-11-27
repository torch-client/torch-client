use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

const WORLD_FILE: &str = "world.json";
const WORLD_TEMP: &str = "world.json.tmp";
const ICON_FILE: &str = "icon.png";
const LOCK_FILE: &str = "session.lock";
const LOCK_CONTENT: &str = "\u{2603}";
const MAX_FILE_NAME: usize = 255;
const FALLBACK_FOLDER: &str = "World";
const DELETE_ATTEMPTS: u32 = 5;
const DELETE_RETRY: std::time::Duration = std::time::Duration::from_millis(500);

const ILLEGAL_FILE_CHARACTERS: [char; 15] = [
    '/', '\n', '\r', '\t', '\0', '\u{c}', '`', '?', '*', '\\', '<', '>', '|', '"', ':',
];
const RESERVED_NAMES: [&str; 6] = ["COM", "CLOCK$", "CON", "PRN", "AUX", "NUL"];

const K_NAME: &str = "name";
const K_GAME_MODE: &str = "gameMode";
const K_DIFFICULTY: &str = "difficulty";
const K_ALLOW_COMMANDS: &str = "allowCommands";
const K_SEED: &str = "seed";
const K_WORLD_TYPE: &str = "worldType";
const K_FLAT_PRESET: &str = "flatPreset";
const K_SINGLE_BIOME: &str = "singleBiome";
const K_GENERATE_STRUCTURES: &str = "generateStructures";
const K_BONUS_CHEST: &str = "bonusChest";
const K_GAME_RULES: &str = "gameRules";
const K_EXPERIMENTS: &str = "experiments";
const K_DATA_PACKS: &str = "dataPacks";
const K_LAST_PLAYED: &str = "lastPlayed";
const K_VERSION: &str = "version";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum GameMode {
    #[default]
    Survival,
    Hardcore,
    Creative,
}

impl GameMode {
    pub const ALL: [GameMode; 3] = [GameMode::Survival, GameMode::Hardcore, GameMode::Creative];

    pub fn key(self) -> &'static str {
        match self {
            GameMode::Survival => "survival",
            GameMode::Hardcore => "hardcore",
            GameMode::Creative => "creative",
        }
    }

    fn from_key(key: &str) -> Option<GameMode> {
        Self::ALL.into_iter().find(|m| m.key() == key)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum Difficulty {
    Peaceful,
    Easy,
    #[default]
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 4] = [
        Difficulty::Peaceful,
        Difficulty::Easy,
        Difficulty::Normal,
        Difficulty::Hard,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Difficulty::Peaceful => "peaceful",
            Difficulty::Easy => "easy",
            Difficulty::Normal => "normal",
            Difficulty::Hard => "hard",
        }
    }

    fn from_key(key: &str) -> Option<Difficulty> {
        Self::ALL.into_iter().find(|d| d.key() == key)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct WorldSettings {
    pub name: String,
    pub game_mode: GameMode,
    pub difficulty: Difficulty,
    pub allow_commands: Option<bool>,
    pub seed: String,
    pub world_type: String,
    pub flat_preset: String,
    pub single_biome: String,
    pub generate_structures: bool,
    pub bonus_chest: bool,
    pub game_rules: BTreeMap<String, String>,
    pub experiments: Vec<String>,
    pub data_packs: Vec<String>,
}

impl Default for WorldSettings {
    fn default() -> Self {
        WorldSettings {
            name: String::new(),
            game_mode: GameMode::default(),
            difficulty: Difficulty::default(),
            allow_commands: None,
            seed: String::new(),
            world_type: "minecraft:normal".to_string(),
            flat_preset: String::new(),
            single_biome: String::new(),
            generate_structures: true,
            bonus_chest: false,
            game_rules: BTreeMap::new(),
            experiments: Vec::new(),
            data_packs: Vec::new(),
        }
    }
}

impl WorldSettings {
    pub fn is_hardcore(&self) -> bool {
        self.game_mode == GameMode::Hardcore
    }

    pub fn effective_difficulty(&self) -> Difficulty {
        if self.is_hardcore() {
            Difficulty::Hard
        } else {
            self.difficulty
        }
    }

    pub fn effective_allow_commands(&self) -> bool {
        if self.is_hardcore() {
            return false;
        }
        self.allow_commands
            .unwrap_or(self.game_mode == GameMode::Creative)
    }

    pub fn effective_bonus_chest(&self) -> bool {
        !self.is_hardcore() && self.bonus_chest
    }
}

#[derive(Clone, Debug)]
pub(crate) struct WorldSummary {
    pub folder: String,
    pub settings: WorldSettings,
    pub last_played: u64,
    pub version: String,
    pub locked: bool,
    pub icon: Option<PathBuf>,
}

pub(crate) fn saves_dir() -> PathBuf {
    crate::platform::storage::dir().join("saves")
}

pub(crate) fn backups_dir() -> PathBuf {
    crate::platform::storage::dir().join("backups")
}

pub(crate) fn world_dir(folder: &str) -> PathBuf {
    saves_dir().join(folder)
}

pub(crate) fn scan() -> Vec<WorldSummary> {
    scan_in(&saves_dir())
}

fn scan_in(root: &Path) -> Vec<WorldSummary> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            crate::log_warn!("worlds", "could not list {}: {e}", root.display());
            return Vec::new();
        }
    };
    let mut worlds = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let Some(folder) = entry.file_name().to_str().map(str::to_string) else {
            crate::log_warn!(
                "worlds",
                "skipping {}: the folder name is not UTF-8",
                entry.path().display()
            );
            continue;
        };
        if !entry.path().join(WORLD_FILE).is_file() {
            continue;
        }
        match read_in(root, &folder) {
            Ok(summary) => worlds.push(summary),
            Err(e) => crate::log_warn!("worlds", "skipping {folder}: {e}"),
        }
    }
    worlds.sort_by(compare);
    worlds
}

fn compare(a: &WorldSummary, b: &WorldSummary) -> Ordering {
    b.last_played
        .cmp(&a.last_played)
        .then_with(|| a.folder.encode_utf16().cmp(b.folder.encode_utf16()))
}

fn read_in(root: &Path, folder: &str) -> Result<WorldSummary, String> {
    let dir = root.join(folder);
    let text = std::fs::read_to_string(dir.join(WORLD_FILE))
        .map_err(|e| format!("could not read {WORLD_FILE}: {e}"))?;
    let json: Value =
        serde_json::from_str(&text).map_err(|e| format!("{WORLD_FILE} is not valid JSON: {e}"))?;
    let mut summary = from_json(folder, &json)?;
    summary.locked = is_locked(&dir);
    let icon = dir.join(ICON_FILE);
    summary.icon = icon.is_file().then_some(icon);
    Ok(summary)
}

fn from_json(folder: &str, json: &Value) -> Result<WorldSummary, String> {
    let map = json
        .as_object()
        .ok_or_else(|| format!("{WORLD_FILE} is not a JSON object"))?;
    let defaults = WorldSettings::default();
    let text = |key: &str| map.get(key).and_then(Value::as_str);
    let flag = |key: &str| map.get(key).and_then(Value::as_bool);
    let list = |key: &str| -> Vec<String> {
        map.get(key)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    let name = match text(K_NAME) {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => folder.to_string(),
    };
    let game_rules = map
        .get(K_GAME_RULES)
        .and_then(Value::as_object)
        .map(|rules| {
            rules
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        v.as_str().map_or_else(|| v.to_string(), str::to_string),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let settings = WorldSettings {
        name,
        game_mode: text(K_GAME_MODE)
            .and_then(GameMode::from_key)
            .unwrap_or_default(),
        difficulty: text(K_DIFFICULTY)
            .and_then(Difficulty::from_key)
            .unwrap_or_default(),
        allow_commands: flag(K_ALLOW_COMMANDS),
        seed: text(K_SEED).unwrap_or_default().to_string(),
        world_type: text(K_WORLD_TYPE).map_or(defaults.world_type, str::to_string),
        flat_preset: text(K_FLAT_PRESET).unwrap_or_default().to_string(),
        single_biome: text(K_SINGLE_BIOME).unwrap_or_default().to_string(),
        generate_structures: flag(K_GENERATE_STRUCTURES).unwrap_or(defaults.generate_structures),
        bonus_chest: flag(K_BONUS_CHEST).unwrap_or(defaults.bonus_chest),
        game_rules,
        experiments: list(K_EXPERIMENTS),
        data_packs: list(K_DATA_PACKS),
    };
    Ok(WorldSummary {
        folder: folder.to_string(),
        settings,
        last_played: map.get(K_LAST_PLAYED).and_then(Value::as_u64).unwrap_or(0),
        version: text(K_VERSION).unwrap_or_default().to_string(),
        locked: false,
        icon: None,
    })
}

fn write_fields(map: &mut Map<String, Value>, summary: &WorldSummary) {
    let s = &summary.settings;
    let strings = |v: &[String]| Value::Array(v.iter().cloned().map(Value::String).collect());
    map.insert(K_NAME.into(), Value::String(s.name.clone()));
    map.insert(K_GAME_MODE.into(), Value::String(s.game_mode.key().into()));
    map.insert(
        K_DIFFICULTY.into(),
        Value::String(s.difficulty.key().into()),
    );
    match s.allow_commands {
        Some(on) => map.insert(K_ALLOW_COMMANDS.into(), Value::Bool(on)),
        None => map.remove(K_ALLOW_COMMANDS),
    };
    map.insert(K_SEED.into(), Value::String(s.seed.clone()));
    map.insert(K_WORLD_TYPE.into(), Value::String(s.world_type.clone()));
    map.insert(K_FLAT_PRESET.into(), Value::String(s.flat_preset.clone()));
    map.insert(K_SINGLE_BIOME.into(), Value::String(s.single_biome.clone()));
    map.insert(
        K_GENERATE_STRUCTURES.into(),
        Value::Bool(s.generate_structures),
    );
    map.insert(K_BONUS_CHEST.into(), Value::Bool(s.bonus_chest));
    let rules = s
        .game_rules
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
        .collect();
    map.insert(K_GAME_RULES.into(), Value::Object(rules));
    map.insert(K_EXPERIMENTS.into(), strings(&s.experiments));
    map.insert(K_DATA_PACKS.into(), strings(&s.data_packs));
    map.insert(K_LAST_PLAYED.into(), Value::from(summary.last_played));
    map.insert(K_VERSION.into(), Value::String(summary.version.clone()));
}

fn edit_world_json(
    dir: &Path,
    must_exist: bool,
    edit: impl FnOnce(&mut Map<String, Value>),
) -> Result<(), String> {
    let path = dir.join(WORLD_FILE);
    let existing = std::fs::read_to_string(&path)
        .map_err(|e| format!("could not read {WORLD_FILE}: {e}"))
        .and_then(|text| match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(map)) => Ok(map),
            Ok(_) => Err(format!("{WORLD_FILE} is not a JSON object")),
            Err(e) => Err(format!("{WORLD_FILE} is not valid JSON: {e}")),
        });
    let mut map = match existing {
        Ok(map) => map,
        Err(e) if must_exist => return Err(e),
        Err(_) => Map::new(),
    };
    edit(&mut map);
    let text = serde_json::to_string_pretty(&Value::Object(map)).map_err(|e| e.to_string())?;
    write_atomic(dir, &text).map_err(|e| format!("could not write {WORLD_FILE}: {e}"))
}

fn write_atomic(dir: &Path, text: &str) -> std::io::Result<()> {
    let temp = dir.join(WORLD_TEMP);
    std::fs::write(&temp, text)?;
    std::fs::rename(&temp, dir.join(WORLD_FILE)).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

pub(crate) fn available_folder(name: &str) -> String {
    let root = saves_dir();
    if let Err(e) = std::fs::create_dir_all(&root) {
        crate::log_error!("worlds", "could not create {}: {e}", root.display());
    }
    available_folder_or_fallback(&root, name)
}

fn available_folder_or_fallback(root: &Path, name: &str) -> String {
    let trimmed = java_trim(name);
    let base = if trimmed.is_empty() {
        FALLBACK_FOLDER
    } else {
        trimmed
    };
    match find_available_name(root, base) {
        Ok(folder) => folder,
        Err(e) => {
            crate::log_warn!("worlds", "no folder for {base:?}: {e}");
            find_available_name(root, FALLBACK_FOLDER).unwrap_or_else(|e| {
                crate::log_error!("worlds", "no folder for {FALLBACK_FOLDER:?}: {e}");
                FALLBACK_FOLDER.to_string()
            })
        }
    }
}

fn find_available_name(root: &Path, name: &str) -> std::io::Result<String> {
    let mut base = sanitize_name(name);
    if !is_path_part_portable(&base) {
        base = format!("_{base}_");
    }
    let base = truncate_utf16(&base, MAX_FILE_NAME).to_string();
    let mut count = 0u32;
    loop {
        let candidate = if count == 0 {
            base.clone()
        } else {
            let suffix = format!(" ({count})");
            format!(
                "{}{suffix}",
                truncate_utf16(&base, MAX_FILE_NAME - suffix.len())
            )
        };
        let path = root.join(&candidate);
        match std::fs::create_dir(&path) {
            Ok(()) => {
                let _ = std::fs::remove_dir(&path);
                return Ok(candidate);
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => count += 1,
            Err(e) => return Err(e),
        }
    }
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if ILLEGAL_FILE_CHARACTERS.contains(&c) || matches!(c, '.' | '/' | '"') {
                '_'
            } else {
                c
            }
        })
        .collect()
}

fn is_path_part_portable(name: &str) -> bool {
    if name.ends_with('.') {
        return false;
    }
    let stem = name.split('.').next().unwrap_or_default();
    !is_reserved_stem(stem)
}

fn is_reserved_stem(stem: &str) -> bool {
    if RESERVED_NAMES.iter().any(|r| stem.eq_ignore_ascii_case(r)) {
        return true;
    }
    let bytes = stem.as_bytes();
    bytes.len() == 4
        && (bytes[..3].eq_ignore_ascii_case(b"COM") || bytes[..3].eq_ignore_ascii_case(b"LPT"))
        && (b'1'..=b'9').contains(&bytes[3])
}

fn truncate_utf16(s: &str, max: usize) -> &str {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        units += c.len_utf16();
        if units > max {
            return &s[..i];
        }
    }
    s
}

pub(crate) fn java_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

pub(crate) fn parse_seed(seed: &str) -> Option<i64> {
    let seed = java_trim(seed);
    if seed.is_empty() {
        return None;
    }
    Some(
        seed.parse::<i64>()
            .unwrap_or_else(|_| i64::from(java_hash(seed))),
    )
}

fn java_hash(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |h, unit| {
        h.wrapping_mul(31).wrapping_add(i32::from(unit))
    })
}

pub(crate) fn create(settings: &WorldSettings, folder_base: &str) -> Result<WorldSummary, String> {
    create_named(&saves_dir(), settings, folder_base)
}

#[cfg(test)]
fn create_in(root: &Path, settings: &WorldSettings) -> Result<WorldSummary, String> {
    create_named(root, settings, &settings.name)
}

fn create_named(
    root: &Path,
    settings: &WorldSettings,
    folder_base: &str,
) -> Result<WorldSummary, String> {
    std::fs::create_dir_all(root).map_err(|e| format!("Could not create the saves folder: {e}"))?;
    let folder = available_folder_or_fallback(root, folder_base);
    let dir = root.join(&folder);
    std::fs::create_dir(&dir).map_err(|e| format!("Could not create the world folder: {e}"))?;
    let summary = WorldSummary {
        folder,
        settings: settings.clone(),
        last_played: now_millis(),
        version: crate::ASSET_VERSION.to_string(),
        locked: false,
        icon: None,
    };
    if let Err(e) = save_in(root, &summary) {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(e);
    }
    Ok(summary)
}

#[allow(
    dead_code,
    reason = "called by the singleplayer backend, which does not exist yet"
)]
pub(crate) fn save(summary: &WorldSummary) -> Result<(), String> {
    save_in(&saves_dir(), summary)
}

fn save_in(root: &Path, summary: &WorldSummary) -> Result<(), String> {
    check_folder(&summary.folder)?;
    let dir = root.join(&summary.folder);
    if !dir.is_dir() {
        return Err("The world folder no longer exists.".to_string());
    }
    edit_world_json(&dir, false, |map| write_fields(map, summary))
}

pub(crate) fn rename(folder: &str, new_name: &str) -> Result<(), String> {
    rename_in(&saves_dir(), folder, new_name)
}

fn rename_in(root: &Path, folder: &str, new_name: &str) -> Result<(), String> {
    let _lock = SessionLock::take_in(root, folder)?;
    edit_world_json(&root.join(folder), true, |map| {
        map.insert(K_NAME.into(), Value::String(new_name.to_string()));
    })
}

pub(crate) fn delete(folder: &str) -> Result<(), String> {
    delete_in(&saves_dir(), folder)
}

fn delete_in(root: &Path, folder: &str) -> Result<(), String> {
    let lock = SessionLock::take_in(root, folder)?;
    let dir = root.join(folder);
    let mut attempt = 1;
    loop {
        match clear_except_lock(&dir) {
            Ok(()) => break,
            Err(e) if attempt < DELETE_ATTEMPTS => {
                crate::log_warn!(
                    "worlds",
                    "failed to delete {folder} (attempt {attempt}): {e}"
                );
                std::thread::sleep(DELETE_RETRY);
                attempt += 1;
            }
            Err(e) => return Err(format!("Could not delete the world: {e}")),
        }
    }
    drop(lock);
    match std::fs::remove_file(dir.join(LOCK_FILE)) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(format!("Could not delete the world: {e}")),
    }
    std::fs::remove_dir(&dir).map_err(|e| format!("Could not delete the world: {e}"))
}

fn clear_except_lock(dir: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_name() == LOCK_FILE {
            continue;
        }
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(entry.path())?;
        } else {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

pub(crate) fn reset_icon(folder: &str) -> Result<(), String> {
    reset_icon_in(&saves_dir(), folder)
}

fn reset_icon_in(root: &Path, folder: &str) -> Result<(), String> {
    let _lock = SessionLock::take_in(root, folder)?;
    match std::fs::remove_file(root.join(folder).join(ICON_FILE)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Could not delete the icon: {e}")),
    }
}

pub(crate) fn backup(folder: &str) -> Result<u64, String> {
    check_folder(folder)?;
    Err("World backups are not supported in this build yet.".to_string())
}

fn is_locked(dir: &Path) -> bool {
    let file = match OpenOptions::new().write(true).open(dir.join(LOCK_FILE)) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => return true,
        Err(e) if e.kind() == ErrorKind::NotFound => return false,
        Err(e) => {
            crate::log_warn!(
                "worlds",
                "could not open {}: {e}",
                dir.join(LOCK_FILE).display()
            );
            return false;
        }
    };
    match file.try_lock() {
        Ok(()) => false,
        Err(TryLockError::WouldBlock) => true,
        Err(TryLockError::Error(e)) => {
            crate::log_warn!(
                "worlds",
                "could not test {}: {e}",
                dir.join(LOCK_FILE).display()
            );
            false
        }
    }
}

pub(crate) struct SessionLock {
    file: File,
}

impl SessionLock {
    #[allow(
        dead_code,
        reason = "called by the singleplayer backend, which does not exist yet"
    )]
    pub fn take(folder: &str) -> Result<SessionLock, String> {
        Self::take_in(&saves_dir(), folder)
    }

    fn take_in(root: &Path, folder: &str) -> Result<SessionLock, String> {
        check_folder(folder)?;
        let dir = root.join(folder);
        if !dir.is_dir() {
            return Err("The world folder no longer exists.".to_string());
        }
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(dir.join(LOCK_FILE))
            .map_err(|e| format!("Could not open {LOCK_FILE}: {e}"))?;
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err("This world is already open.".to_string()),
            Err(TryLockError::Error(e)) => return Err(format!("Could not lock the world: {e}")),
        }
        file.write_all(LOCK_CONTENT.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|e| format!("Could not write {LOCK_FILE}: {e}"))?;
        Ok(SessionLock { file })
    }
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

fn check_folder(folder: &str) -> Result<(), String> {
    let bad = folder.is_empty()
        || folder == "."
        || folder == ".."
        || folder.contains(['/', '\\', '\0'])
        || Path::new(folder).is_absolute();
    if bad {
        Err(format!("Invalid world folder name: {folder:?}"))
    } else {
        Ok(())
    }
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new(tag: &str) -> TempRoot {
            let path =
                std::env::temp_dir().join(format!("mc-worlds-test-{}-{tag}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            TempRoot(path)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn parse_seed_matches_world_options() {
        assert_eq!(parse_seed(""), None);
        assert_eq!(parse_seed(" \t\n"), None);
        assert_eq!(parse_seed(" 42 "), Some(42));
        assert_eq!(parse_seed("-9223372036854775808"), Some(i64::MIN));
        assert_eq!(parse_seed("test1"), Some(110_251_487));
        assert_eq!(parse_seed("\u{1F600}"), Some(1_772_899));
        assert_eq!(parse_seed("polygenelubricants"), Some(-2_147_483_648));
        assert_eq!(
            parse_seed("9223372036854775808"),
            Some(i64::from(java_hash("9223372036854775808")))
        );
    }

    #[test]
    fn available_folder_sanitises_like_file_util() {
        let root = TempRoot::new("names");
        let name = |n: &str| available_folder_or_fallback(&root.0, n);
        assert_eq!(name("a/b:c.d\"e"), "a_b_c_d_e");
        assert_eq!(name("  My World  "), "My World");
        assert_eq!(name("CON"), "_CON_");
        assert_eq!(name("lpt3"), "_lpt3_");
        assert_eq!(name("COM10"), "COM10");
        assert_eq!(name("   "), FALLBACK_FOLDER);
        assert_eq!(name(&"x".repeat(300)).len(), MAX_FILE_NAME);

        std::fs::create_dir(root.0.join("World")).unwrap();
        assert_eq!(name("World"), "World (1)");
        std::fs::create_dir(root.0.join("World (1)")).unwrap();
        assert_eq!(name("World"), "World (2)");
        assert!(!root.0.join("World (2)").exists());
    }

    #[test]
    fn effective_rules_follow_world_creation_ui_state() {
        let mut s = WorldSettings {
            difficulty: Difficulty::Easy,
            bonus_chest: true,
            ..WorldSettings::default()
        };
        assert_eq!(s.effective_difficulty(), Difficulty::Easy);
        assert!(!s.effective_allow_commands());
        assert!(s.effective_bonus_chest());

        s.game_mode = GameMode::Creative;
        assert!(s.effective_allow_commands());
        s.allow_commands = Some(false);
        assert!(!s.effective_allow_commands());

        s.game_mode = GameMode::Hardcore;
        s.allow_commands = Some(true);
        assert!(s.is_hardcore());
        assert_eq!(s.effective_difficulty(), Difficulty::Hard);
        assert!(!s.effective_allow_commands());
        assert!(!s.effective_bonus_chest());
    }

    fn sample() -> WorldSummary {
        let mut settings = WorldSettings {
            name: "Test".into(),
            game_mode: GameMode::Creative,
            difficulty: Difficulty::Peaceful,
            allow_commands: Some(false),
            seed: "test1".into(),
            bonus_chest: true,
            experiments: vec!["minecraft:trade_rebalance".into()],
            data_packs: vec!["vanilla".into()],
            ..WorldSettings::default()
        };
        settings
            .game_rules
            .insert("doDaylightCycle".into(), "false".into());
        WorldSummary {
            folder: "Test".into(),
            settings,
            last_played: 1_700_000_000_000,
            version: "26.2".into(),
            locked: false,
            icon: None,
        }
    }

    #[test]
    fn json_round_trip_keeps_unknown_fields() {
        let summary = sample();
        let mut map = match serde_json::from_str::<Value>(r#"{"name":"Old","futureField":{"a":1}}"#)
            .unwrap()
        {
            Value::Object(map) => map,
            _ => unreachable!(),
        };
        write_fields(&mut map, &summary);
        assert_eq!(map["futureField"], serde_json::json!({"a": 1}));
        let back = from_json("Test", &Value::Object(map)).unwrap();
        assert_eq!(back.settings, summary.settings);
        assert_eq!(back.last_played, summary.last_played);
        assert_eq!(back.version, summary.version);
    }

    #[test]
    fn save_and_rename_keep_unknown_fields_on_disk() {
        let root = TempRoot::new("disk");
        let summary = sample();
        let dir = root.0.join(&summary.folder);
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join(WORLD_FILE), r#"{"futureField":[1,2]}"#).unwrap();

        save_in(&root.0, &summary).unwrap();
        rename_in(&root.0, &summary.folder, "Renamed").unwrap();

        let json: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join(WORLD_FILE)).unwrap()).unwrap();
        assert_eq!(json["futureField"], serde_json::json!([1, 2]));
        let scanned = scan_in(&root.0);
        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].settings.name, "Renamed");
        assert_eq!(scanned[0].settings.game_mode, GameMode::Creative);
        assert!(!scanned[0].locked);
    }

    #[test]
    fn session_lock_is_exclusive_and_blocks_delete() {
        let root = TempRoot::new("lock");
        let created = create_in(
            &root.0,
            &WorldSettings {
                name: "Locked".into(),
                ..WorldSettings::default()
            },
        )
        .unwrap();
        let lock = SessionLock::take_in(&root.0, &created.folder).unwrap();
        assert!(is_locked(&root.0.join(&created.folder)));
        assert!(SessionLock::take_in(&root.0, &created.folder).is_err());
        assert!(delete_in(&root.0, &created.folder).is_err());
        drop(lock);
        assert!(!is_locked(&root.0.join(&created.folder)));
        delete_in(&root.0, &created.folder).unwrap();
        assert!(!root.0.join(&created.folder).exists());
    }

    #[test]
    fn list_order_follows_level_summary() {
        let mut a = sample();
        let mut b = sample();
        let mut c = sample();
        a.folder = "b".into();
        a.last_played = 5;
        b.folder = "a".into();
        b.last_played = 5;
        c.folder = "z".into();
        c.last_played = 9;
        let mut list = [a, b, c];
        list.sort_by(compare);
        let order: Vec<&str> = list.iter().map(|s| s.folder.as_str()).collect();
        assert_eq!(order, ["z", "a", "b"]);
    }

    #[test]
    fn folder_names_cannot_leave_the_saves_folder() {
        for bad in ["", ".", "..", "a/b", "a\\b", "/abs"] {
            assert!(check_folder(bad).is_err(), "{bad:?}");
        }
        assert!(check_folder("New World (1)").is_ok());
    }
}
