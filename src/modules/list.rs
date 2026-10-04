use std::borrow::Cow;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};

pub const MAX_GROUPS: usize = 5;

pub const NO_ROW: u16 = u16::MAX;

pub struct Entry {
    pub group: u8,
    pub label: Cow<'static, str>,
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
            Preset::Defaults => "Defaults",
            Preset::Clear => "None",
        }
    }
}

const fn pack(on: bool, color: u32) -> u32 {
    (color & 0xFF_FF_FF) << 8 | on as u32
}

struct Table {
    labels: Box<[Cow<'static, str>]>,
    rows: Box<[AtomicU32]>,
    defaults: Box<[u32]>,
    starts: [u32; MAX_GROUPS + 1],
    index: Box<[u16]>,
}

pub struct BitList {
    pub key: &'static str,
    pub title: &'static str,
    pub width: f32,
    pub groups: &'static [&'static str],
    pub colored: bool,
    pub presets: &'static [Preset],
    build: fn() -> Built,
    table: OnceLock<Table>,
    generation: AtomicU32,
    selected: AtomicU64,
    group_on: [AtomicU64; MAX_GROUPS],
}

impl BitList {
    pub const fn new(
        key: &'static str,
        title: &'static str,
        width: f32,
        groups: &'static [&'static str],
        colored: bool,
        presets: &'static [Preset],
        build: fn() -> Built,
    ) -> BitList {
        BitList {
            key,
            title,
            width,
            groups,
            colored,
            presets,
            build,
            table: OnceLock::new(),
            generation: AtomicU32::new(0),
            selected: AtomicU64::new(u64::MAX),
            group_on: [const { AtomicU64::new(u64::MAX) }; MAX_GROUPS],
        }
    }

    fn table(&self) -> &Table {
        self.table.get_or_init(|| {
            let (entries, keys) = (self.build)();
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
                rows,
                defaults,
                starts,
                index,
            }
        })
    }

    pub fn count(&self) -> usize {
        self.table().labels.len()
    }

    pub fn label(&self, i: usize) -> &str {
        &self.table().labels[i]
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
}

pub fn widest() -> f32 {
    ALL.iter().map(|l| l.width).fold(0.0, f32::max)
}

pub static ALL: [&BitList; 5] = [
    &super::esp::blocks::XRAY,
    &super::esp::blocks::ORES,
    &super::esp::blocks::STORAGE,
    &super::entities::LIST,
    &super::items::LIST,
];
