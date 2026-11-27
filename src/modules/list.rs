use std::borrow::Cow;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};

use crate::lang::Key;
use crate::text::contains_ci;

pub const MAX_GROUPS: usize = 5;

pub const NO_ROW: u16 = u16::MAX;

pub struct Entry {
    pub group: u8,
    pub label: Cow<'static, str>,
    pub shown: Key,
    pub on: bool,
    pub color: u32,
}

pub type Built = (Vec<Entry>, Vec<(u32, u32)>);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Preset {
    Defaults,
    Clear,
}

impl Preset {
    pub fn label(self) -> &'static str {
        match self {
            Preset::Defaults => crate::lang::tr!("modmsg.list.preset.defaults"),
            Preset::Clear => crate::lang::tr!("modmsg.list.preset.none"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Match {
    Exact,
    Prefix,
    Inside,
}

impl Match {
    pub const ALL: [Match; 3] = [Match::Exact, Match::Prefix, Match::Inside];
}

fn match_of(text: &str, query: &str) -> Option<Match> {
    if !contains_ci(text, query) {
        return None;
    }
    let mut rest = text.chars().flat_map(char::to_lowercase);
    let starts = query
        .chars()
        .flat_map(char::to_lowercase)
        .all(|q| rest.next() == Some(q));
    Some(match (starts, rest.next()) {
        (true, None) => Match::Exact,
        (true, Some(_)) => Match::Prefix,
        (false, _) => Match::Inside,
    })
}

const fn pack(on: bool, color: u32) -> u32 {
    (color & 0xFF_FF_FF) << 8 | on as u32
}

struct Table {
    labels: Box<[Cow<'static, str>]>,
    shown: Box<[Key]>,
    rows: Box<[AtomicU32]>,
    defaults: Box<[u32]>,
    starts: [u32; MAX_GROUPS + 1],
    index: Box<[u16]>,
}

pub struct BitList {
    pub key: &'static str,
    title_key: Key,
    mod_title: &'static str,
    pub width: f32,
    groups: &'static [Key],
    pub colored: bool,
    pub presets: &'static [Preset],
    pub inline: bool,
    build: fn() -> Built,
    table: OnceLock<Table>,
    generation: AtomicU32,
    selected: AtomicU64,
    group_on: [AtomicU64; MAX_GROUPS],
}

impl BitList {
    pub const fn new(
        key: &'static str,
        title: Key,
        width: f32,
        groups: &'static [Key],
        colored: bool,
        presets: &'static [Preset],
        build: fn() -> Built,
    ) -> BitList {
        BitList {
            key,
            title_key: title,
            mod_title: "",
            width,
            groups,
            colored,
            presets,
            inline: false,
            build,
            table: OnceLock::new(),
            generation: AtomicU32::new(0),
            selected: AtomicU64::new(u64::MAX),
            group_on: [const { AtomicU64::new(u64::MAX) }; MAX_GROUPS],
        }
    }

    #[cfg(lua_mods)]
    pub fn dynamic(
        key: &'static str,
        title: &'static str,
        width: f32,
        groups: &'static [Key],
        presets: &'static [Preset],
        built: Built,
    ) -> &'static BitList {
        let mut list = BitList::new(key, Key::NONE, width, groups, false, presets, unbuilt);
        list.mod_title = title;
        list.inline = true;
        let table = list.make_table(built);
        let _ = list.table.set(table);
        Box::leak(Box::new(list))
    }

    fn table(&self) -> &Table {
        self.table.get_or_init(|| self.make_table((self.build)()))
    }

    fn make_table(&self, (entries, keys): Built) -> Table {
        debug_assert!(entries.len() < NO_ROW as usize);
        assert!(
            self.groups.len() <= MAX_GROUPS,
            "{} has {} groups, MAX_GROUPS is {MAX_GROUPS}",
            self.key,
            self.groups.len()
        );
        assert!(
            entries
                .iter()
                .all(|e| (e.group as usize) < self.groups.len()),
            "{} has a row outside its groups",
            self.key
        );

        let mut tagged: Vec<(u32, Entry)> = entries
            .into_iter()
            .enumerate()
            .map(|(i, e)| (i as u32, e))
            .collect();
        tagged.sort_by(|a, b| {
            a.1.group
                .cmp(&b.1.group)
                .then_with(|| a.1.label.cmp(&b.1.label))
        });
        let mut rank = vec![NO_ROW; tagged.len()];
        for (new, (old, _)) in tagged.iter().enumerate() {
            rank[*old as usize] = new as u16;
        }

        let mut starts = [0u32; MAX_GROUPS + 1];
        for (i, (_, e)) in tagged.iter().enumerate() {
            for s in starts.iter_mut().skip(e.group as usize + 1) {
                *s = (i + 1) as u32;
            }
        }
        starts[MAX_GROUPS] = tagged.len() as u32;

        let defaults: Box<[u32]> = tagged.iter().map(|(_, e)| pack(e.on, e.color)).collect();
        let rows = defaults.iter().map(|d| AtomicU32::new(*d)).collect();
        let shown: Box<[Key]> = if tagged.iter().any(|(_, e)| e.shown != Key::NONE) {
            tagged.iter().map(|(_, e)| e.shown).collect()
        } else {
            Box::default()
        };
        let labels = tagged.into_iter().map(|(_, e)| e.label).collect();

        let index: Box<[u16]> = if keys.is_empty() {
            Box::default()
        } else {
            let max = keys.iter().map(|(k, _)| *k).max().unwrap_or(0) as usize;
            let mut ix = vec![NO_ROW; max + 1];
            for (k, e) in keys {
                ix[k as usize] = rank[e as usize];
            }
            ix.into_boxed_slice()
        };

        Table {
            labels,
            shown,
            rows,
            defaults,
            starts,
            index,
        }
    }

    pub fn count(&self) -> usize {
        self.table().labels.len()
    }

    pub fn find(&self, label: &str) -> Option<usize> {
        (0..self.count()).find(|&i| self.label(i) == label)
    }

    pub fn label(&self, i: usize) -> &str {
        &self.table().labels[i]
    }

    pub fn shown(&self, i: usize) -> &str {
        let t = self.table();
        match t.shown.get(i) {
            Some(&k) if k != Key::NONE => k.get(),
            _ => &*t.labels[i],
        }
    }

    pub fn row_match(&self, i: usize, query: &str) -> Option<Match> {
        let t = self.table();
        let label = match_of(&t.labels[i], query);
        match t.shown.get(i) {
            Some(&k) if k != Key::NONE => match_of(k.get(), query).into_iter().chain(label).min(),
            _ => label,
        }
    }

    pub fn title(&self) -> &'static str {
        self.title_key.or(self.mod_title)
    }

    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    pub fn group(&self, g: usize) -> &'static str {
        self.groups[g].get()
    }

    pub fn group_matches(&self, g: usize, query: &str) -> bool {
        let k = self.groups[g];
        contains_ci(k.get(), query) || contains_ci(k.english(), query)
    }

    pub fn group_range(&self, g: usize) -> std::ops::Range<usize> {
        let t = self.table();
        t.starts[g] as usize..t.starts[g + 1] as usize
    }

    pub fn enabled(&self, i: usize) -> bool {
        self.table().rows[i].load(Relaxed) & 1 != 0
    }

    pub fn color(&self, i: usize) -> u32 {
        self.table().rows[i].load(Relaxed) >> 8
    }

    fn write(&self, i: usize, on: bool) {
        let r = &self.table().rows[i];
        let v = r.load(Relaxed);
        r.store(v & !1 | on as u32, Relaxed);
    }

    pub fn set_enabled(&self, i: usize, on: bool) {
        self.write(i, on);
        self.bump();
    }

    pub fn set_group(&self, g: usize, on: bool) {
        for i in self.group_range(g) {
            self.write(i, on);
        }
        self.bump();
    }

    pub fn set_color(&self, i: usize, color: u32) {
        let r = &self.table().rows[i];
        let v = r.load(Relaxed);
        r.store((color & 0xFF_FF_FF) << 8 | (v & 1), Relaxed);
        self.bump();
    }

    pub fn is_default(&self, i: usize) -> bool {
        let t = self.table();
        t.rows[i].load(Relaxed) == t.defaults[i]
    }

    pub fn default_on(&self, i: usize) -> bool {
        self.table().defaults[i] & 1 != 0
    }

    pub fn apply_preset(&self, p: Preset) {
        let t = self.table();
        for i in 0..t.rows.len() {
            match p {
                Preset::Defaults => t.rows[i].store(t.defaults[i], Relaxed),
                Preset::Clear => self.write(i, false),
            }
        }
        self.bump();
    }

    pub fn group_on(&self, g: usize) -> (usize, usize) {
        let range = self.group_range(g);
        let total = range.len();
        let want = self.generation();
        let cached = self.group_on[g].load(Relaxed);
        if (cached >> 32) as u32 == want {
            return (cached as u32 as usize, total);
        }
        let n = range.filter(|&i| self.enabled(i)).count();
        self.group_on[g].store((want as u64) << 32 | n as u64, Relaxed);
        (n, total)
    }

    pub fn selected(&self) -> usize {
        let want = self.generation();
        let cached = self.selected.load(Relaxed);
        if (cached >> 32) as u32 == want {
            return cached as u32 as usize;
        }
        let n = (0..self.count()).filter(|&i| self.enabled(i)).count();
        self.selected.store((want as u64) << 32 | n as u64, Relaxed);
        n
    }

    pub fn generation(&self) -> u32 {
        self.generation.load(Relaxed)
    }

    fn bump(&self) {
        self.generation.fetch_add(1, Relaxed);
        #[cfg(lua_mods)]
        if self.inline {
            super::script::note_setting();
        }
    }

    #[cfg(lua_mods)]
    pub fn set_only(&self, on: impl Fn(&str) -> bool) {
        let t = self.table();
        for i in 0..t.rows.len() {
            self.write(i, on(&t.labels[i]));
        }
        self.bump();
    }

    pub fn touched(&self) -> bool {
        self.table.get().is_some()
    }

    pub fn index(&self) -> &[u16] {
        &self.table().index
    }

    pub fn key_enabled(&self, key: u32) -> bool {
        match self.table().index.get(key as usize) {
            Some(&i) if i != NO_ROW => self.enabled(i as usize),
            _ => false,
        }
    }

    pub fn key_color(&self, key: u32) -> Option<u32> {
        match self.table().index.get(key as usize) {
            Some(&i) if i != NO_ROW => {
                let v = self.table().rows[i as usize].load(Relaxed);
                (v & 1 != 0).then_some(v >> 8)
            }
            _ => None,
        }
    }
}

#[cfg(lua_mods)]
fn unbuilt() -> Built {
    unreachable!("a dynamic BitList's table is set when it is made")
}

pub fn widest() -> f32 {
    ALL.iter().map(|l| l.width).fold(0.0, f32::max)
}

pub static ALL: [&BitList; 13] = [
    &super::esp::blocks::XRAY,
    &super::esp::blocks::BLOCK_ESP,
    &super::esp::blocks::ORES,
    &super::seed_xray::ORES,
    &super::ore_miner::ORES,
    &super::seed_cracker::FINDERS,
    &super::esp::blocks::STORAGE,
    &super::esp::blocks::SCAFFOLD,
    &super::entities::LIST,
    &super::entities::ESP,
    &super::items::LIST,
    &super::sound_esp::SOUNDS,
    &super::pathfinder::THROWAWAY,
];
