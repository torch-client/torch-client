use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Mutex, PoisonError};

use mlua::{Error, Lua, MetaMethod, Table, UserData, UserDataMethods, Value};
use simdnbt::owned::{NbtCompound, NbtList, NbtTag};

use super::api::Ctx;
use super::game::{self, Pending, game_fn};

const MAX_FILE: u64 = 64 << 20;
const MAX_INFLATED: u64 = 512 << 20;
const MAX_READS: usize = 4;
const MAX_UNPACK: usize = 1 << 24;

static READING: AtomicUsize = AtomicUsize::new(0);
static GENERATION: AtomicU64 = AtomicU64::new(0);

pub(super) fn forget() {
    GENERATION.fetch_add(1, Relaxed);
}

pub(super) fn generation() -> u64 {
    GENERATION.load(Relaxed)
}

pub(super) enum JobResult {
    Tag(NbtTag),
    File(Mutex<Option<Result<NbtCompound, String>>>),
    Saved(Result<u64, String>),
    Read(Result<(), String>),
    Captured(Result<bool, String>),
    Loaded(Mutex<Option<Result<super::image::Loaded, String>>>),
}

impl JobResult {
    pub fn fill(&self, lua: &Lua, e: &Table) -> mlua::Result<()> {
        let failed = |why: &str| -> mlua::Result<()> {
            e.raw_set("ok", false)?;
            e.raw_set("error", why)
        };
        match self {
            JobResult::Tag(tag) => {
                e.raw_set("result", to_lua(lua, tag)?)?;
                e.raw_set("ok", true)
            }
            JobResult::File(slot) => {
                match slot.lock().unwrap_or_else(PoisonError::into_inner).take() {
                    Some(Ok(root)) => match owned_compound(lua, root) {
                        Ok(t) => {
                            e.raw_set("result", t)?;
                            e.raw_set("ok", true)
                        }
                        Err(err) => failed(&format!("could not convert the document: {err}")),
                    },
                    Some(Err(why)) => failed(&why),
                    None => failed("already delivered"),
                }
            }
            JobResult::Saved(Ok(size)) => {
                e.raw_set("size", *size)?;
                e.raw_set("ok", true)
            }
            JobResult::Saved(Err(why))
            | JobResult::Read(Err(why))
            | JobResult::Captured(Err(why)) => failed(why),
            JobResult::Read(Ok(())) => e.raw_set("ok", true),
            JobResult::Captured(Ok(partial)) => {
                e.raw_set("partial", *partial)?;
                e.raw_set("ok", true)
            }
            JobResult::Loaded(slot) => {
                match slot.lock().unwrap_or_else(PoisonError::into_inner).take() {
                    Some(Ok(loaded)) => match super::image::adopt(lua, loaded)? {
                        Ok(image) => {
                            e.raw_set("image", image)?;
                            e.raw_set("ok", true)
                        }
                        Err(why) => failed(&why),
                    },
                    Some(Err(why)) => failed(&why),
                    None => failed("already delivered"),
                }
            }
        }
    }
}

pub(super) fn compound_to_lua(lua: &Lua, c: &NbtCompound) -> mlua::Result<Table> {
    let t = lua.create_table_with_capacity(0, c.len())?;
    for (k, v) in c.iter() {
        t.raw_set(k.to_str().as_ref(), to_lua(lua, v)?)?;
    }
    Ok(t)
}

fn to_lua(lua: &Lua, tag: &NbtTag) -> mlua::Result<Value> {
    Ok(match tag {
        NbtTag::Byte(v) => Value::Number(*v as f64),
        NbtTag::Short(v) => Value::Number(*v as f64),
        NbtTag::Int(v) => Value::Number(*v as f64),
        NbtTag::Long(v) => Value::Number(*v as f64),
        NbtTag::Float(v) => Value::Number(*v as f64),
        NbtTag::Double(v) => Value::Number(*v),
        NbtTag::String(s) => Value::String(lua.create_string(s.to_str().as_ref())?),
        NbtTag::ByteArray(v) => numbers(lua, v.iter().map(|&b| b as i8 as f64))?,
        NbtTag::IntArray(v) => numbers(lua, v.iter().map(|&n| n as f64))?,
        NbtTag::LongArray(v) => numbers(lua, v.iter().map(|&n| n as f64))?,
        NbtTag::List(l) => list(lua, l)?,
        NbtTag::Compound(c) => Value::Table(compound_to_lua(lua, c)?),
    })
}

fn owned_compound(lua: &Lua, c: NbtCompound) -> mlua::Result<Table> {
    let t = lua.create_table_with_capacity(0, c.len())?;
    for (k, v) in c {
        t.raw_set(k.to_str().as_ref(), owned(lua, v)?)?;
    }
    Ok(t)
}

fn owned(lua: &Lua, tag: NbtTag) -> mlua::Result<Value> {
    match tag {
        NbtTag::ByteArray(v) => array(lua, Data::Byte(v.into_iter().map(|b| b as i8).collect())),
        NbtTag::IntArray(v) => array(lua, Data::Int(v)),
        NbtTag::LongArray(v) => array(lua, Data::Long(v)),
        NbtTag::Compound(c) => Ok(Value::Table(owned_compound(lua, c)?)),
        NbtTag::List(l) => owned_list(lua, l),
        other => to_lua(lua, &other),
    }
}

fn owned_list(lua: &Lua, l: NbtList) -> mlua::Result<Value> {
    fn each<T>(lua: &Lua, v: Vec<T>, f: impl Fn(T) -> mlua::Result<Value>) -> mlua::Result<Value> {
        let t = lua.create_table_with_capacity(v.len(), 0)?;
        for (i, x) in v.into_iter().enumerate() {
            t.raw_set(i + 1, f(x)?)?;
        }
        Ok(Value::Table(t))
    }
    match l {
        NbtList::ByteArray(v) => each(lua, v, |a| owned(lua, NbtTag::ByteArray(a))),
        NbtList::IntArray(v) => each(lua, v, |a| owned(lua, NbtTag::IntArray(a))),
        NbtList::LongArray(v) => each(lua, v, |a| owned(lua, NbtTag::LongArray(a))),
        NbtList::Compound(v) => each(lua, v, |c| Ok(Value::Table(owned_compound(lua, c)?))),
        NbtList::List(v) => each(lua, v, |l| owned_list(lua, l)),
        other => list(lua, &other),
    }
}

fn numbers(lua: &Lua, it: impl ExactSizeIterator<Item = f64>) -> mlua::Result<Value> {
    let t = lua.create_table_with_capacity(it.len(), 0)?;
    for (i, n) in it.enumerate() {
        t.raw_set(i + 1, n)?;
    }
    Ok(Value::Table(t))
}

fn list(lua: &Lua, l: &NbtList) -> mlua::Result<Value> {
    fn each<T>(lua: &Lua, v: &[T], f: impl Fn(&T) -> mlua::Result<Value>) -> mlua::Result<Value> {
        let t = lua.create_table_with_capacity(v.len(), 0)?;
        for (i, x) in v.iter().enumerate() {
            t.raw_set(i + 1, f(x)?)?;
        }
        Ok(Value::Table(t))
    }
    match l {
        NbtList::Empty => Ok(Value::Table(lua.create_table()?)),
        NbtList::Byte(v) => numbers(lua, v.iter().map(|&n| n as f64)),
        NbtList::Short(v) => numbers(lua, v.iter().map(|&n| n as f64)),
        NbtList::Int(v) => numbers(lua, v.iter().map(|&n| n as f64)),
        NbtList::Long(v) => numbers(lua, v.iter().map(|&n| n as f64)),
        NbtList::Float(v) => numbers(lua, v.iter().map(|&n| n as f64)),
        NbtList::Double(v) => numbers(lua, v.iter().copied()),
        NbtList::ByteArray(v) => each(lua, v, |a| numbers(lua, a.iter().map(|&b| b as i8 as f64))),
        NbtList::String(v) => each(lua, v, |s| {
            Ok(Value::String(lua.create_string(s.to_str().as_ref())?))
        }),
        NbtList::List(v) => each(lua, v, |l| list(lua, l)),
        NbtList::Compound(v) => each(lua, v, |c| Ok(Value::Table(compound_to_lua(lua, c)?))),
        NbtList::IntArray(v) => each(lua, v, |a| numbers(lua, a.iter().map(|&n| n as f64))),
        NbtList::LongArray(v) => each(lua, v, |a| numbers(lua, a.iter().map(|&n| n as f64))),
    }
}

const MAX_WRITE_DEPTH: usize = 64;
const EXACT: f64 = 9_007_199_254_740_992.0;

pub(super) fn lua_to_compound(t: &Table, path: &str) -> Result<NbtCompound, String> {
    compound_of(t, path, 0)
}

fn compound_of(t: &Table, path: &str, depth: usize) -> Result<NbtCompound, String> {
    if depth > MAX_WRITE_DEPTH {
        return Err(format!("{path} is nested more than {MAX_WRITE_DEPTH} deep"));
    }
    let mut values = Vec::new();
    for pair in t.pairs::<Value, Value>() {
        let (k, v) = pair.map_err(|e| e.to_string())?;
        let Value::String(k) = k else {
            return Err(format!(
                "{path} mixes string keys with others; a compound's keys are strings"
            ));
        };
        let k = k
            .to_str()
            .map_err(|_| format!("{path} has a key that is not UTF-8"))?
            .to_string();
        let tag = tag_of(&v, &format!("{path}.{k}"), depth + 1)?;
        values.push((k, tag));
    }
    values.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(NbtCompound::from_values(
        values.into_iter().map(|(k, v)| (k.into(), v)).collect(),
    ))
}

fn whole_tag(n: f64) -> Option<NbtTag> {
    if n.fract() != 0.0 || n.abs() >= EXACT {
        return None;
    }
    Some(if n >= f64::from(i32::MIN) && n <= f64::from(i32::MAX) {
        NbtTag::Int(n as i32)
    } else {
        NbtTag::Long(n as i64)
    })
}

fn tag_of(v: &Value, path: &str, depth: usize) -> Result<NbtTag, String> {
    Ok(match v {
        Value::Boolean(b) => NbtTag::Byte(*b as i8),
        Value::Integer(i) => whole_tag(*i as f64).unwrap_or(NbtTag::Long(*i as i64)),
        Value::Number(n) if !n.is_finite() => return Err(format!("{path} is NaN or infinite")),
        Value::Number(n) => whole_tag(*n).unwrap_or(NbtTag::Double(*n)),
        Value::String(s) => NbtTag::String(
            s.to_str()
                .map_err(|_| format!("{path} is not UTF-8"))?
                .to_string()
                .into(),
        ),
        Value::UserData(ud) => {
            let a = ud
                .borrow::<Array>()
                .map_err(|_| format!("{path} is a userdata that is not an array handle"))?;
            match a.0.as_ref() {
                Data::Byte(v) => NbtTag::ByteArray(v.iter().map(|&b| b as u8).collect()),
                Data::Int(v) => NbtTag::IntArray(v.clone()),
                Data::Long(v) => NbtTag::LongArray(v.clone()),
                Data::Unpacked(v) => NbtTag::IntArray(v.iter().map(|&n| n as i32).collect()),
            }
        }
        Value::Table(t) => {
            if depth > MAX_WRITE_DEPTH {
                return Err(format!("{path} is nested more than {MAX_WRITE_DEPTH} deep"));
            }
            let n = t.raw_len();
            if n == 0 {
                return Ok(NbtTag::Compound(compound_of(t, path, depth)?));
            }
            let mut items = Vec::with_capacity(n);
            for i in 1..=n {
                items.push(tag_of(
                    &t.raw_get::<Value>(i).map_err(|e| e.to_string())?,
                    &format!("{path}[{i}]"),
                    depth + 1,
                )?);
            }
            if t.pairs::<Value, Value>().count() != n {
                return Err(format!("{path} mixes 1..n keys with others"));
            }
            NbtTag::List(list_of(items, path)?)
        }
        other => {
            return Err(format!(
                "{path} is a {}, which NBT cannot hold",
                other.type_name()
            ));
        }
    })
}

fn list_of(items: Vec<NbtTag>, path: &str) -> Result<NbtList, String> {
    let numeric = items
        .iter()
        .all(|t| matches!(t, NbtTag::Int(_) | NbtTag::Long(_) | NbtTag::Double(_)));
    if numeric {
        let f = |t: &NbtTag| match t {
            NbtTag::Int(n) => *n as f64,
            NbtTag::Long(n) => *n as f64,
            NbtTag::Double(n) => *n,
            _ => 0.0,
        };
        if items.iter().any(|t| matches!(t, NbtTag::Double(_))) {
            return Ok(NbtList::Double(items.iter().map(f).collect()));
        }
        if items.iter().any(|t| matches!(t, NbtTag::Long(_))) {
            return Ok(NbtList::Long(items.iter().map(|t| f(t) as i64).collect()));
        }
        return Ok(NbtList::Int(items.iter().map(|t| f(t) as i32).collect()));
    }
    let first = std::mem::discriminant(&items[0]);
    if items.iter().any(|t| std::mem::discriminant(t) != first) {
        return Err(format!("{path} mixes tag types; an NBT list holds one"));
    }
    let mut it = items.into_iter();
    let Some(head) = it.next() else {
        return Ok(NbtList::Empty);
    };
    macro_rules! all {
        ($v:ident, $head:ident) => {
            NbtList::$v(
                std::iter::once($head)
                    .chain(it.filter_map(|t| if let NbtTag::$v(x) = t { Some(x) } else { None }))
                    .collect(),
            )
        };
    }
    Ok(match head {
        NbtTag::Byte(x) => all!(Byte, x),
        NbtTag::String(x) => all!(String, x),
        NbtTag::Compound(x) => all!(Compound, x),
        NbtTag::List(x) => all!(List, x),
        NbtTag::ByteArray(x) => all!(ByteArray, x),
        NbtTag::IntArray(x) => all!(IntArray, x),
        NbtTag::LongArray(x) => all!(LongArray, x),
        _ => return Err(format!("{path} holds a tag type no list is written for")),
    })
}

enum Data {
    Byte(Vec<i8>),
    Int(Vec<i32>),
    Long(Vec<i64>),
    Unpacked(Vec<u32>),
}

impl Data {
    fn len(&self) -> usize {
        match self {
            Data::Byte(v) => v.len(),
            Data::Int(v) => v.len(),
            Data::Long(v) => v.len(),
            Data::Unpacked(v) => v.len(),
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Data::Byte(_) => "byte",
            Data::Int(_) => "int",
            Data::Long(_) => "long",
            Data::Unpacked(_) => "unpacked",
        }
    }

    fn get(&self, i: usize) -> Option<f64> {
        match self {
            Data::Byte(v) => v.get(i).map(|&n| n as f64),
            Data::Int(v) => v.get(i).map(|&n| n as f64),
            Data::Long(v) => v.get(i).map(|&n| n as f64),
            Data::Unpacked(v) => v.get(i).map(|&n| n as f64),
        }
    }
}

struct Array(Arc<Data>);

pub(super) struct Indices {
    data: Arc<Data>,
    bits: u32,
    spanning: bool,
    count: usize,
}

impl Indices {
    pub(super) fn new(
        ud: &mlua::AnyUserData,
        bits: Option<u32>,
        spanning: bool,
        count: usize,
    ) -> Result<Indices, String> {
        let data = ud
            .borrow::<Array>()
            .map_err(|_| "blocks is an array handle from torch.nbt".to_string())?
            .0
            .clone();
        match (data.as_ref(), bits) {
            (Data::Long(longs), Some(b)) => each_entry(longs, b, count, spanning, 0..0, |_| {})?,
            (Data::Long(_), None) => return Err("a long array needs bits".into()),
            (d, Some(_)) => {
                return Err(format!(
                    "bits is for a long array, not a {} array",
                    d.kind()
                ));
            }
            (d, None) if d.len() < count => {
                return Err(format!(
                    "{count} blocks need {count} entries; the array has {}",
                    d.len()
                ));
            }
            _ => {}
        }
        Ok(Indices {
            data,
            bits: bits.unwrap_or(0),
            spanning,
            count,
        })
    }

    pub(super) fn each(&self, range: std::ops::Range<usize>, mut f: impl FnMut(u32)) {
        match self.data.as_ref() {
            Data::Long(longs) => {
                let _ = each_entry(longs, self.bits, self.count, self.spanning, range, f);
            }
            Data::Unpacked(v) => v[range].iter().for_each(|&n| f(n)),
            Data::Int(v) => v[range].iter().for_each(|&n| f(n as u32)),
            Data::Byte(v) => v[range].iter().for_each(|&n| f(n as u8 as u32)),
        }
    }
}

fn array(lua: &Lua, d: Data) -> mlua::Result<Value> {
    Ok(Value::UserData(lua.create_userdata(Array(Arc::new(d)))?))
}

fn unpack(longs: &[i64], bits: u32, count: usize, spanning: bool) -> Result<Vec<u32>, String> {
    let mut out = Vec::with_capacity(count);
    each_entry(longs, bits, count, spanning, 0..count, |v| out.push(v))?;
    Ok(out)
}

fn each_entry(
    longs: &[i64],
    bits: u32,
    count: usize,
    spanning: bool,
    range: std::ops::Range<usize>,
    mut f: impl FnMut(u32),
) -> Result<(), String> {
    if range.end > count {
        return Err(format!(
            "entries {} to {} are past the {count} there are",
            range.start + 1,
            range.end
        ));
    }
    if !(1..=32).contains(&bits) {
        return Err(format!("bits is 1 to 32, not {bits}"));
    }
    let per_long = (64 / bits) as usize;
    let needed = if spanning {
        (count * bits as usize).div_ceil(64)
    } else {
        count.div_ceil(per_long)
    };
    if needed > longs.len() {
        return Err(format!(
            "{count} entries of {bits} bits need {needed} longs; the array has {}",
            longs.len()
        ));
    }
    let mask = if bits == 32 {
        u32::MAX as u64
    } else {
        (1u64 << bits) - 1
    };
    for i in range {
        let v = if spanning {
            let bit = i * bits as usize;
            let (k, shift) = (bit / 64, (bit % 64) as u32);
            let lo = (longs[k] as u64) >> shift;
            let hi = if shift + bits > 64 {
                (longs[k + 1] as u64) << (64 - shift)
            } else {
                0
            };
            (lo | hi) & mask
        } else {
            let (k, slot) = (i / per_long, (i % per_long) as u32);
            ((longs[k] as u64) >> (slot * bits)) & mask
        };
        f(v as u32);
    }
    Ok(())
}

fn packed_counts(
    longs: &[i64],
    bits: u32,
    count: usize,
    spanning: bool,
    range: std::ops::Range<usize>,
) -> Result<Vec<(u32, u64)>, String> {
    if bits <= 16 {
        let mut flat = vec![0u64; 1 << bits];
        each_entry(longs, bits, count, spanning, range, |v| {
            flat[v as usize] += 1
        })?;
        return Ok(flat
            .into_iter()
            .enumerate()
            .filter(|(_, n)| *n > 0)
            .map(|(v, n)| (v as u32, n))
            .collect());
    }
    let mut map: HashMap<u32, u64> = HashMap::new();
    each_entry(longs, bits, count, spanning, range, |v| {
        *map.entry(v).or_default() += 1
    })?;
    Ok(map.into_iter().collect())
}

impl UserData for Array {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Len, |_, a, ()| Ok(a.0.len()));
        methods.add_method("len", |_, a, ()| Ok(a.0.len()));
        methods.add_method("kind", |_, a, ()| Ok(a.0.kind()));
        methods.add_method("get", |_, a, i: f64| {
            if i.fract() != 0.0 || i < 1.0 {
                return Ok(None);
            }
            Ok(a.0.get(i as usize - 1))
        });
        methods.add_method(
            "unpack",
            |lua, a, (bits, count, spanning): (u32, usize, Option<bool>)| {
                let Data::Long(longs) = a.0.as_ref() else {
                    return Err(Error::runtime(format!(
                        "array:unpack works on a long array, not a {} array",
                        a.0.kind()
                    )));
                };
                if count > MAX_UNPACK {
                    return Err(Error::runtime(format!(
                        "array:unpack: at most {MAX_UNPACK} entries"
                    )));
                }
                let out = unpack(longs, bits, count, spanning.unwrap_or(true))
                    .map_err(|e| Error::runtime(format!("array:unpack: {e}")))?;
                array(lua, Data::Unpacked(out))
            },
        );
        type CountArgs = (u32, usize, Option<bool>, Option<usize>, Option<usize>);
        methods.add_method(
            "counts",
            |lua, a, (bits, count, spanning, first, n): CountArgs| {
                let Data::Long(longs) = a.0.as_ref() else {
                    return Err(Error::runtime(format!(
                        "array:counts works on a long array, not a {} array",
                        a.0.kind()
                    )));
                };
                let start = first.unwrap_or(1).max(1) - 1;
                let end = n.map_or(count, |n| start.saturating_add(n).min(count));
                let counts = packed_counts(
                    longs,
                    bits,
                    count,
                    spanning.unwrap_or(true),
                    start..end.max(start),
                )
                .map_err(|e| Error::runtime(format!("array:counts: {e}")))?;
                let t = lua.create_table_with_capacity(0, counts.len())?;
                for (v, n) in counts {
                    t.raw_set(v as f64, n as f64)?;
                }
                Ok(t)
            },
        );
        methods.add_method("varints", |lua, a, count: usize| {
            let Data::Byte(bytes) = a.0.as_ref() else {
                return Err(Error::runtime(format!(
                    "array:varints works on a byte array, not a {} array",
                    a.0.kind()
                )));
            };
            if count > MAX_UNPACK {
                return Err(Error::runtime(format!(
                    "array:varints: at most {MAX_UNPACK} entries"
                )));
            }
            let out =
                varints(bytes, count).map_err(|e| Error::runtime(format!("array:varints: {e}")))?;
            array(lua, Data::Unpacked(out))
        });
        methods.add_method(
            "histogram",
            |lua, a, (first, n): (Option<usize>, Option<usize>)| {
                let len = a.0.len();
                let start = first.unwrap_or(1).max(1) - 1;
                let end = n
                    .map_or(len, |n| start.saturating_add(n))
                    .min(len)
                    .max(start.min(len));
                let range = start.min(len)..end;
                let mut counts: HashMap<i64, u64> = HashMap::new();
                let mut add = |v: i64| *counts.entry(v).or_default() += 1;
                match a.0.as_ref() {
                    Data::Byte(v) => v[range].iter().for_each(|&n| add(n as i64)),
                    Data::Int(v) => v[range].iter().for_each(|&n| add(n as i64)),
                    Data::Long(v) => v[range].iter().for_each(|&n| add(n)),
                    Data::Unpacked(v) => v[range].iter().for_each(|&n| add(n as i64)),
                }
                let t = lua.create_table_with_capacity(0, counts.len())?;
                for (v, n) in counts {
                    t.raw_set(v as f64, n as f64)?;
                }
                Ok(t)
            },
        );
    }
}

fn varints(bytes: &[i8], count: usize) -> Result<Vec<u32>, String> {
    let mut out = Vec::with_capacity(count);
    let mut it = bytes.iter().map(|&b| b as u8);
    while out.len() < count {
        let (mut value, mut shift) = (0u32, 0u32);
        loop {
            let b = it
                .next()
                .ok_or_else(|| format!("ends after {} of {count} entries", out.len()))?;
            if shift > 28 {
                return Err(format!("entry {} is longer than 5 bytes", out.len() + 1));
            }
            value |= u32::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
        }
        out.push(value);
    }
    Ok(out)
}

fn read_file(path: PathBuf) -> Result<NbtCompound, String> {
    #[cfg(not(target_arch = "wasm32"))]
    let raw = {
        let mut raw = Vec::new();
        std::fs::File::open(&path)
            .and_then(|f| f.take(MAX_FILE + 1).read_to_end(&mut raw))
            .map_err(|e| e.to_string())?;
        raw
    };
    #[cfg(target_arch = "wasm32")]
    let raw = crate::platform::packfs::read(&path).map_err(|e| e.to_string())?;
    if raw.len() as u64 > MAX_FILE {
        return Err(format!("the file is over {} MiB", MAX_FILE >> 20));
    }
    let bytes = if raw.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(raw.as_slice())
            .take(MAX_INFLATED + 1)
            .read_to_end(&mut out)
            .map_err(|e| format!("not valid gzip: {e}"))?;
        if out.len() as u64 > MAX_INFLATED {
            return Err(format!("inflates past {} MiB", MAX_INFLATED >> 20));
        }
        out
    } else {
        raw
    };
    let nbt = simdnbt::owned::read(&mut std::io::Cursor::new(bytes.as_slice()))
        .map_err(|e| format!("not NBT: {e}"))?;
    Ok(if nbt.is_some() {
        nbt.unwrap().as_compound()
    } else {
        NbtCompound::default()
    })
}

struct Slot;

impl Drop for Slot {
    fn drop(&mut self) {
        READING.fetch_sub(1, Relaxed);
    }
}

pub(super) fn table(lua: &Lua, ctx: &Arc<Ctx>) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.raw_set(
        "read",
        game_fn(lua, ctx, "torch.nbt.read", |_, ctx, path: String| {
            let what = "torch.nbt.read";
            let (k, _) = ctx.caller(what)?;
            let file = match super::fs::existing_file(ctx, &path, what)? {
                Ok(file) => file,
                Err(why) => return Ok((None, Some(why))),
            };
            if READING.fetch_add(1, Relaxed) >= MAX_READS {
                READING.fetch_sub(1, Relaxed);
                return Ok((None, Some(format!("{MAX_READS} reads are already running"))));
            }
            let id = ctx.jobs.fetch_add(1, Relaxed) + 1;
            let generation = generation();
            let slot = Slot;
            let read = move || {
                let _slot = slot;
                let result = read_file(file);
                if self::generation() == generation {
                    game::push(Pending::Job {
                        owner: k,
                        id,
                        generation,
                        result: Box::new(JobResult::File(Mutex::new(Some(result)))),
                    });
                }
            };
            #[cfg(target_arch = "wasm32")]
            crate::platform::executor::spawn(async move { read() });
            #[cfg(not(target_arch = "wasm32"))]
            if let Err(e) = std::thread::Builder::new()
                .name("lua-nbt".to_string())
                .spawn(read)
            {
                return Ok((None, Some(format!("could not start the read: {e}"))));
            }
            Ok((Some(id), None))
        })?,
    )?;
    t.set_readonly(true);
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::{unpack, varints};

    #[test]
    fn varints_read_one_and_two_bytes() {
        let bytes = [0x05i8, 0x80u8 as i8, 0x01, 0x7f];
        assert_eq!(varints(&bytes, 3).unwrap(), vec![5, 128, 127]);
        assert!(varints(&bytes, 4).is_err());
    }

    #[test]
    fn spanning_entries_cross_longs() {
        let longs = [(0b111i64 << 62) | 0b10, 0b1];
        let out = unpack(&longs, 3, 22, true).unwrap();
        assert_eq!(out[0], 0b010);
        assert_eq!(out[20], 0b100);
        assert_eq!(out[21], 0b011);
    }

    #[test]
    fn padded_entries_stay_in_their_long() {
        let longs = [0b11_01i64, 0b10];
        let out = unpack(&longs, 30, 3, false).unwrap();
        assert_eq!(out, vec![0b1101, 0, 0b10]);
    }
}
