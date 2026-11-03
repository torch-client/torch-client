use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError};

use mlua::{AnyUserData, Error, Lua, Table, UserData, UserDataMethods, Value};

use super::api::Ctx;
use super::fs::Target;
use super::game::{self, Pending, game_fn};
use super::nbt::{self, JobResult};
use crate::renderer::mod_capture::{self as gpu, Done, ImageOp, TexelFormat, Ticket};

const MAX_SIDE: u32 = 4096;
const MAX_PIXELS: u64 = 64 << 20;
const MAX_JOBS: usize = 4;
const MAX_PNG_FILE: u64 = 64 << 20;
pub(super) const CAPTURES_QUEUED_PER_MODULE: u32 = 1024;

const TILE_MAGIC: [u8; 4] = *b"TILE";
const TILE_VERSION: u16 = 1;
const TILE_HEADER: usize = 20;
const TILE_RGBA8: u16 = 0;
const TILE_R16: u16 = 1;
const R16_MAX: f64 = 65535.0;

type Pixels = Arc<Box<[u8]>>;

#[derive(Clone, Copy)]
enum FileKind {
    Png,
    Tile,
}

impl FileKind {
    fn of(path: &str) -> FileKind {
        if path.to_ascii_lowercase().ends_with(".png") {
            FileKind::Png
        } else {
            FileKind::Tile
        }
    }
}

struct Save {
    version: u64,
    ticket: Ticket,
    target: Target,
    kind: FileKind,
    slot: Slot,
}

struct Entry {
    owner: usize,
    w: u32,
    h: u32,
    format: TexelFormat,
    version: u64,
    cpu: Option<Pixels>,
    reading: Option<u64>,
    saves: Vec<Save>,
}

impl Entry {
    fn pixels(&self) -> u64 {
        u64::from(self.w) * u64::from(self.h)
    }

    fn wrote(&mut self) {
        self.version += 1;
        self.cpu = None;
    }
}

#[derive(Default)]
struct Registry {
    images: BTreeMap<u64, Entry>,
    captures: HashMap<usize, u32>,
    late: Vec<Pending>,
}

static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(Mutex::default);
static NEXT: AtomicU64 = AtomicU64::new(1);
static WORKING: AtomicUsize = AtomicUsize::new(0);

fn registry() -> MutexGuard<'static, Registry> {
    REGISTRY.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) fn clear() {
    *registry() = Registry::default();
    gpu::send(ImageOp::Clear);
}

pub(super) fn drain(out: &mut Vec<Pending>) {
    let mut reg = registry();
    out.append(&mut reg.late);
    let generation = nbt::generation();
    gpu::take_done(|done| match done {
        Done::Capture { ticket, result } => {
            if ticket.generation != generation {
                return;
            }
            if let Some(n) = reg.captures.get_mut(&ticket.owner) {
                *n = n.saturating_sub(1);
            }
            out.push(job(ticket, JobResult::Captured(result)));
        }
        Done::Readback {
            key,
            version,
            ticket,
            result,
        } => read_back(&mut reg, out, key, version, ticket, result),
    });
}

fn job(t: Ticket, result: JobResult) -> Pending {
    Pending::Job {
        owner: t.owner,
        id: t.id,
        generation: t.generation,
        result: Box::new(result),
    }
}

fn read_back(
    reg: &mut Registry,
    out: &mut Vec<Pending>,
    key: u64,
    version: u64,
    ticket: Option<Ticket>,
    result: Result<Box<[u8]>, String>,
) {
    let current = nbt::generation();
    let ticket = ticket.filter(|t| t.generation == current);
    let Some(e) = reg.images.get_mut(&key) else {
        if let Some(t) = ticket {
            out.push(job(t, JobResult::Read(Err("the image was removed".into()))));
        }
        return;
    };
    if e.reading == Some(version) {
        e.reading = None;
    }
    let waiting: Vec<Save> = e.saves.extract_if(.., |s| s.version == version).collect();
    let header = (e.w, e.h, e.format);
    match result {
        Ok(bytes) => {
            let pixels: Pixels = Arc::new(bytes);
            let fresh = e.version == version;
            if fresh {
                e.cpu = Some(pixels.clone());
            }
            for s in waiting {
                if let Err(why) =
                    spawn_save(s.ticket, s.target, s.kind, header, pixels.clone(), s.slot)
                {
                    out.push(job(s.ticket, JobResult::Saved(Err(why))));
                }
            }
            if let Some(t) = ticket {
                let answer = if fresh {
                    Ok(())
                } else {
                    Err("the image was written before the readback finished".into())
                };
                out.push(job(t, JobResult::Read(answer)));
            }
        }
        Err(why) => {
            for s in waiting {
                out.push(job(s.ticket, JobResult::Saved(Err(why.clone()))));
            }
            if let Some(t) = ticket {
                out.push(job(t, JobResult::Read(Err(why))));
            }
        }
    }
}

fn texel(format: TexelFormat, v: f64, what: &str) -> mlua::Result<[u8; 4]> {
    let whole = v.is_finite() && v.fract() == 0.0;
    match format {
        TexelFormat::Rgba8 if whole && (f64::from(i32::MIN)..=f64::from(u32::MAX)).contains(&v) => {
            let c = v as i64 as u32;
            Ok([(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8])
        }
        TexelFormat::R16 if whole && (0.0..=R16_MAX).contains(&v) => {
            let [lo, hi] = (v as u16).to_le_bytes();
            Ok([lo, hi, 0, 0])
        }
        TexelFormat::Rgba8 => Err(Error::runtime(format!(
            "{what}: a pixel is 0xAARRGGBB, not {v}"
        ))),
        TexelFormat::R16 => Err(Error::runtime(format!(
            "{what}: an r16 texel is 0 to 65535, not {v}"
        ))),
    }
}

fn texel_value(format: TexelFormat, p: &[u8]) -> u32 {
    match format {
        TexelFormat::Rgba8 => {
            u32::from(p[3]) << 24 | u32::from(p[0]) << 16 | u32::from(p[1]) << 8 | u32::from(p[2])
        }
        TexelFormat::R16 => u32::from(u16::from_le_bytes([p[0], p[1]])),
    }
}

fn whole(v: f64, what: &str) -> mlua::Result<i64> {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e9 {
        Ok(v as i64)
    } else {
        Err(Error::runtime(format!(
            "{what} takes whole numbers, not {v}"
        )))
    }
}

fn point(e: &Entry, x: f64, y: f64, what: &str) -> mlua::Result<(u32, u32)> {
    let (x, y) = (whole(x, what)?, whole(y, what)?);
    if !(0..i64::from(e.w)).contains(&x) || !(0..i64::from(e.h)).contains(&y) {
        return Err(Error::runtime(format!(
            "{what}: {x}, {y} is outside the {}x{} image (0-based)",
            e.w, e.h
        )));
    }
    Ok((x as u32, y as u32))
}

fn rect(e: &Entry, r: [f64; 4], what: &str) -> mlua::Result<[u32; 4]> {
    let mut v = [0i64; 4];
    for (out, n) in v.iter_mut().zip(r) {
        *out = whole(n, what)?;
    }
    let [x, y, w, h] = v;
    if w < 1 || h < 1 || x < 0 || y < 0 || x + w > i64::from(e.w) || y + h > i64::from(e.h) {
        return Err(Error::runtime(format!(
            "{what}: {w}x{h} at {x}, {y} does not fit in the {}x{} image",
            e.w, e.h
        )));
    }
    Ok([x as u32, y as u32, w as u32, h as u32])
}

pub(super) struct Handle(u64, Arc<Ctx>);

fn key_of(ud: &AnyUserData, what: &str) -> mlua::Result<u64> {
    Ok(ud
        .borrow::<Handle>()
        .map_err(|_| Error::runtime(format!("{what} takes an image from torch.image")))?
        .0)
}

pub(super) fn of(ud: &AnyUserData, what: &str) -> mlua::Result<Option<(u64, u32, u32)>> {
    let key = key_of(ud, what)?;
    let reg = registry();
    let Some(e) = reg.images.get(&key) else {
        return Ok(None);
    };
    if e.format != TexelFormat::Rgba8 {
        return Err(Error::runtime(format!(
            "{what} draws rgba8 images; this one is {}",
            e.format.name()
        )));
    }
    Ok(Some((key, e.w, e.h)))
}

pub(super) fn begin_capture(
    owner: usize,
    into: &AnyUserData,
    depth: Option<&AnyUserData>,
    r: [Option<f64>; 4],
    what: &str,
) -> mlua::Result<Result<(u64, Option<u64>, [u32; 4]), String>> {
    let into = key_of(into, what)?;
    let depth = depth.map(|d| key_of(d, what)).transpose()?;
    let mut reg = registry();
    let Some(e) = reg.images.get(&into) else {
        return Ok(Err("the image was removed".into()));
    };
    if e.format != TexelFormat::Rgba8 {
        return Err(Error::runtime(format!(
            "{what}: `into` is an rgba8 image, not {}",
            e.format.name()
        )));
    }
    let [x, y, w, h] = r;
    let (x, y) = (x.unwrap_or(0.0), y.unwrap_or(0.0));
    let r = rect(
        e,
        [
            x,
            y,
            w.unwrap_or(f64::from(e.w) - x),
            h.unwrap_or(f64::from(e.h) - y),
        ],
        what,
    )?;
    let size = (e.w, e.h);
    if let Some(d) = depth {
        let Some(de) = reg.images.get(&d) else {
            return Ok(Err("the depth image was removed".into()));
        };
        if de.format != TexelFormat::R16 || (de.w, de.h) != size {
            return Err(Error::runtime(format!(
                "{what}: `depth` is an r16 image of {}x{}, the size of `into`",
                size.0, size.1
            )));
        }
    }
    let queued = reg.captures.entry(owner).or_default();
    if *queued >= CAPTURES_QUEUED_PER_MODULE {
        return Ok(Err(format!(
            "{CAPTURES_QUEUED_PER_MODULE} captures are already queued"
        )));
    }
    *queued += 1;
    for key in [Some(into), depth].into_iter().flatten() {
        if let Some(e) = reg.images.get_mut(&key) {
            e.wrote();
        }
    }
    Ok(Ok((into, depth, r)))
}

fn write(key: u64, op: impl FnOnce(&Entry) -> mlua::Result<ImageOp>) -> mlua::Result<bool> {
    let mut reg = registry();
    let Some(e) = reg.images.get_mut(&key) else {
        return Ok(false);
    };
    let op = op(e)?;
    e.wrote();
    gpu::send(op);
    Ok(true)
}

impl UserData for Handle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("size", |_, h, ()| {
            Ok(registry().images.get(&h.0).map(|e| (e.w, e.h)).unzip())
        });
        methods.add_method("format", |_, h, ()| {
            Ok(registry().images.get(&h.0).map(|e| e.format.name()))
        });
        methods.add_method("set", |_, h, (x, y, c): (f64, f64, f64)| {
            let what = "img:set";
            write(h.0, |e| {
                let (x, y) = point(e, x, y, what)?;
                Ok(ImageOp::Set {
                    key: h.0,
                    x,
                    y,
                    texel: texel(e.format, c, what)?,
                })
            })
        });
        methods.add_method("get", |_, h, (x, y): (f64, f64)| {
            let what = "img:get";
            let reg = registry();
            let Some(e) = reg.images.get(&h.0) else {
                return Ok((None, Some("the image was removed".to_string())));
            };
            let (x, y) = point(e, x, y, what)?;
            let Some(px) = &e.cpu else {
                return Ok((
                    None,
                    Some("no CPU copy since the last write; call img:readback()".to_string()),
                ));
            };
            let bpp = e.format.bytes();
            let i = (y as usize * e.w as usize + x as usize) * bpp;
            Ok((Some(texel_value(e.format, &px[i..i + bpp])), None))
        });
        methods.add_method("fill", |_, h, c: f64| {
            write(h.0, |e| {
                Ok(ImageOp::Fill {
                    key: h.0,
                    texel: texel(e.format, c, "img:fill")?,
                })
            })
        });
        methods.add_method(
            "blit",
            |_, h, (buf, x, y, w, hh): (mlua::Buffer, f64, f64, f64, f64)| {
                write(h.0, |e| blit(h.0, e, &buf, [x, y, w, hh]))
            },
        );
        methods.add_method("readback", |_, h, ()| readback(&h.1, h.0));
        methods.add_method("save", |_, h, path: String| save(&h.1, h.0, &path));
        methods.add_method("remove", |_, h, ()| {
            let mut reg = registry();
            let Some(e) = reg.images.remove(&h.0) else {
                return Ok(false);
            };
            let failed: Vec<Pending> = e
                .saves
                .into_iter()
                .map(|s| {
                    job(
                        s.ticket,
                        JobResult::Saved(Err("the image was removed".into())),
                    )
                })
                .collect();
            reg.late.extend(failed);
            gpu::send(ImageOp::Remove { key: h.0 });
            Ok(true)
        });
    }
}

fn blit(key: u64, e: &Entry, buf: &mlua::Buffer, r: [f64; 4]) -> mlua::Result<ImageOp> {
    let what = "img:blit";
    let rect = rect(e, r, what)?;
    let bpp = e.format.bytes();
    let n = rect[2] as usize * rect[3] as usize * bpp;
    if buf.len() < n {
        return Err(Error::runtime(format!(
            "{what}: {}x{} needs {n} bytes; the buffer has {}",
            rect[2],
            rect[3],
            buf.len()
        )));
    }
    let mut bytes = vec![0u8; n].into_boxed_slice();
    buf.clone()
        .cursor()
        .read_exact(&mut bytes)
        .map_err(|e| Error::runtime(format!("{what}: {e}")))?;
    if e.format == TexelFormat::Rgba8 {
        for t in bytes.chunks_exact_mut(bpp) {
            t.swap(0, 2);
        }
    }
    Ok(ImageOp::Write { key, rect, bytes })
}

struct Slot;

impl Slot {
    fn take() -> Option<Slot> {
        if WORKING.fetch_add(1, Relaxed) >= MAX_JOBS {
            WORKING.fetch_sub(1, Relaxed);
            return None;
        }
        Some(Slot)
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        WORKING.fetch_sub(1, Relaxed);
    }
}

fn on_worker(job: impl FnOnce() + Send + 'static) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        crate::platform::executor::spawn(async move { job() });
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::Builder::new()
        .name("lua-png".to_string())
        .spawn(job)
        .map(drop)
        .map_err(|e| format!("could not start: {e}"))
}

fn readback(ctx: &Ctx, key: u64) -> mlua::Result<(Option<u64>, Option<String>)> {
    let (k, _) = ctx.caller("img:readback")?;
    let mut reg = registry();
    let Some(e) = reg.images.get_mut(&key) else {
        return Ok((None, Some("the image was removed".into())));
    };
    let id = ctx.jobs.fetch_add(1, Relaxed) + 1;
    let ticket = Ticket {
        owner: k,
        id,
        generation: nbt::generation(),
    };
    if e.cpu.is_some() {
        reg.late.push(job(ticket, JobResult::Read(Ok(()))));
    } else {
        e.reading = Some(e.version);
        gpu::send(ImageOp::Readback {
            key,
            version: e.version,
            ticket: Some(ticket),
        });
    }
    Ok((Some(id), None))
}

fn save(ctx: &Ctx, key: u64, path: &str) -> mlua::Result<(Option<u64>, Option<String>)> {
    let what = "img:save";
    let (k, _) = ctx.caller(what)?;
    let target = match super::fs::writable(ctx, path, what)? {
        Ok(t) => t,
        Err(why) => return Ok((None, Some(why))),
    };
    let mut reg = registry();
    let Some(e) = reg.images.get_mut(&key) else {
        return Ok((None, Some("the image was removed".into())));
    };
    let Some(slot) = Slot::take() else {
        return Ok((
            None,
            Some(format!("{MAX_JOBS} saves and loads are already running")),
        ));
    };
    let id = ctx.jobs.fetch_add(1, Relaxed) + 1;
    let ticket = Ticket {
        owner: k,
        id,
        generation: nbt::generation(),
    };
    let kind = FileKind::of(path);
    if let Some(px) = e.cpu.clone() {
        let header = (e.w, e.h, e.format);
        drop(reg);
        if let Err(why) = spawn_save(ticket, target, kind, header, px, slot) {
            return Ok((None, Some(why)));
        }
        return Ok((Some(id), None));
    }
    let version = e.version;
    e.saves.push(Save {
        version,
        ticket,
        target,
        kind,
        slot,
    });
    if e.reading != Some(version) {
        e.reading = Some(version);
        gpu::send(ImageOp::Readback {
            key,
            version,
            ticket: None,
        });
    }
    Ok((Some(id), None))
}

fn spawn_save(
    ticket: Ticket,
    target: Target,
    kind: FileKind,
    (w, h, format): (u32, u32, TexelFormat),
    px: Pixels,
    slot: Slot,
) -> Result<(), String> {
    on_worker(move || {
        let _slot = slot;
        let result = encode(kind, w, h, format, &px)
            .and_then(|file| super::fs::write_to(&target, &file).map(|()| file.len() as u64));
        if nbt::generation() == ticket.generation {
            game::push(job(ticket, JobResult::Saved(result)));
        }
    })
}

fn encode(
    kind: FileKind,
    w: u32,
    h: u32,
    format: TexelFormat,
    px: &[u8],
) -> Result<Vec<u8>, String> {
    match kind {
        FileKind::Tile => {
            let mut out = Vec::with_capacity(TILE_HEADER + px.len());
            out.extend_from_slice(&tile_header(w, h, format, crc32(px)));
            out.extend_from_slice(px);
            Ok(out)
        }
        FileKind::Png => {
            use ::image::ImageEncoder;
            let color = match format {
                TexelFormat::Rgba8 => ::image::ExtendedColorType::Rgba8,
                TexelFormat::R16 => ::image::ExtendedColorType::L16,
            };
            let mut out = Vec::new();
            ::image::codecs::png::PngEncoder::new(&mut out)
                .write_image(px, w, h, color)
                .map_err(|e| format!("could not encode the PNG: {e}"))?;
            Ok(out)
        }
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = flate2::Crc::new();
    crc.update(bytes);
    crc.sum()
}

fn tile_header(w: u32, h: u32, format: TexelFormat, crc: u32) -> [u8; TILE_HEADER] {
    let code = match format {
        TexelFormat::Rgba8 => TILE_RGBA8,
        TexelFormat::R16 => TILE_R16,
    };
    let mut out = [0u8; TILE_HEADER];
    out[0..4].copy_from_slice(&TILE_MAGIC);
    out[4..6].copy_from_slice(&TILE_VERSION.to_le_bytes());
    out[6..8].copy_from_slice(&code.to_le_bytes());
    out[8..12].copy_from_slice(&w.to_le_bytes());
    out[12..16].copy_from_slice(&h.to_le_bytes());
    out[16..20].copy_from_slice(&crc.to_le_bytes());
    out
}

fn parse_tile_header(h: &[u8; TILE_HEADER]) -> Result<(u32, u32, TexelFormat, u32), String> {
    let u16_at = |i: usize| u16::from_le_bytes([h[i], h[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([h[i], h[i + 1], h[i + 2], h[i + 3]]);
    if h[0..4] != TILE_MAGIC {
        return Err("not a .tile file".into());
    }
    if u16_at(4) != TILE_VERSION {
        return Err(format!(".tile version {} is not {TILE_VERSION}", u16_at(4)));
    }
    let format = match u16_at(6) {
        TILE_RGBA8 => TexelFormat::Rgba8,
        TILE_R16 => TexelFormat::R16,
        other => return Err(format!(".tile format {other} is not 0 (rgba8) or 1 (r16)")),
    };
    let (w, h) = (u32_at(8), u32_at(12));
    if !(1..=MAX_SIDE).contains(&w) || !(1..=MAX_SIDE).contains(&h) {
        return Err(format!(".tile size {w}x{h}: each side is 1 to {MAX_SIDE}"));
    }
    Ok((w, h, format, u32_at(16)))
}

#[cfg(target_arch = "wasm32")]
fn tile_from_bytes(mut file: Vec<u8>) -> Result<Decoded, String> {
    let head: &[u8; TILE_HEADER] = file
        .get(..TILE_HEADER)
        .and_then(|s| s.try_into().ok())
        .ok_or("not a .tile file")?;
    let (w, h, format, crc) = parse_tile_header(head)?;
    let n = w as usize * h as usize * format.bytes();
    if file.len() != TILE_HEADER + n {
        return Err(format!(
            "the .tile file is {} bytes; {w}x{h} {} needs {}",
            file.len(),
            format.name(),
            TILE_HEADER + n
        ));
    }
    file.drain(..TILE_HEADER);
    if crc32(&file) != crc {
        return Err("the .tile file's CRC does not match its texels".into());
    }
    Ok(Decoded {
        w,
        h,
        format,
        pixels: file.into_boxed_slice(),
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn read_tile(path: &std::path::Path) -> Result<Decoded, String> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut head = [0u8; TILE_HEADER];
    f.read_exact(&mut head)
        .map_err(|_| "not a .tile file".to_string())?;
    let (w, h, format, crc) = parse_tile_header(&head)?;
    let mut pixels = vec![0u8; w as usize * h as usize * format.bytes()].into_boxed_slice();
    f.read_exact(&mut pixels)
        .map_err(|_| format!("the .tile file ends before its {w}x{h} texels"))?;
    if f.read(&mut [0u8; 1]).map_err(|e| e.to_string())? != 0 {
        return Err("the .tile file runs past its texels".into());
    }
    if crc32(&pixels) != crc {
        return Err("the .tile file's CRC does not match its texels".into());
    }
    Ok(Decoded {
        w,
        h,
        format,
        pixels,
    })
}

fn decode_png(file: &[u8]) -> Result<Decoded, String> {
    let mut reader =
        ::image::ImageReader::with_format(std::io::Cursor::new(file), ::image::ImageFormat::Png);
    let mut limits = ::image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(u64::from(MAX_SIDE) * u64::from(MAX_SIDE) * 8);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|e| format!("not a PNG this can read: {e}"))?;
    let (w, h) = (decoded.width(), decoded.height());
    Ok(match decoded {
        ::image::DynamicImage::ImageLuma16(grey) => {
            let mut pixels = Vec::with_capacity(grey.len() * 2);
            for v in grey.into_raw() {
                pixels.extend_from_slice(&v.to_le_bytes());
            }
            Decoded {
                w,
                h,
                format: TexelFormat::R16,
                pixels: pixels.into_boxed_slice(),
            }
        }
        other => Decoded {
            w,
            h,
            format: TexelFormat::Rgba8,
            pixels: other.into_rgba8().into_raw().into_boxed_slice(),
        },
    })
}

pub(super) struct Decoded {
    w: u32,
    h: u32,
    format: TexelFormat,
    pixels: Box<[u8]>,
}

pub(super) struct Loaded {
    owner: usize,
    ctx: Arc<Ctx>,
    image: Decoded,
}

fn read_image(path: PathBuf, kind: FileKind) -> Result<Decoded, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        match kind {
            FileKind::Tile => read_tile(&path),
            FileKind::Png => {
                let mut raw = Vec::new();
                std::fs::File::open(&path)
                    .and_then(|f| f.take(MAX_PNG_FILE + 1).read_to_end(&mut raw))
                    .map_err(|e| e.to_string())?;
                if raw.len() as u64 > MAX_PNG_FILE {
                    return Err(format!("the PNG is over {} MiB", MAX_PNG_FILE >> 20));
                }
                decode_png(&raw)
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let raw = crate::platform::packfs::read(&path).map_err(|e| e.to_string())?;
        match kind {
            FileKind::Tile => tile_from_bytes(raw),
            FileKind::Png if raw.len() as u64 > MAX_PNG_FILE => {
                Err(format!("the PNG is over {} MiB", MAX_PNG_FILE >> 20))
            }
            FileKind::Png => decode_png(&raw),
        }
    }
}

pub(super) fn adopt(lua: &Lua, loaded: Loaded) -> mlua::Result<Result<Value, String>> {
    let Loaded { owner, ctx, image } = loaded;
    let Decoded {
        w,
        h,
        format,
        pixels,
    } = image;
    let Some(key) = register(owner, w, h, format) else {
        return Ok(Err(format!(
            "over the {} M pixels one module's images may hold",
            MAX_PIXELS >> 20
        )));
    };
    gpu::send(ImageOp::Write {
        key,
        rect: [0, 0, w, h],
        bytes: pixels,
    });
    Ok(Ok(Value::UserData(lua.create_userdata(Handle(key, ctx))?)))
}

fn register(owner: usize, w: u32, h: u32, format: TexelFormat) -> Option<u64> {
    let mut reg = registry();
    let held: u64 = reg
        .images
        .values()
        .filter(|e| e.owner == owner)
        .map(Entry::pixels)
        .sum();
    if held + u64::from(w) * u64::from(h) > MAX_PIXELS {
        return None;
    }
    let key = NEXT.fetch_add(1, Relaxed);
    let entry = Entry {
        owner,
        w,
        h,
        format,
        version: 0,
        cpu: None,
        reading: None,
        saves: Vec::new(),
    };
    reg.images.insert(key, entry);
    gpu::send(ImageOp::Create { key, w, h, format });
    Some(key)
}

fn format_of(name: Option<String>, what: &str) -> mlua::Result<TexelFormat> {
    match name.as_deref() {
        None | Some("rgba8") => Ok(TexelFormat::Rgba8),
        Some("r16") => Ok(TexelFormat::R16),
        Some(other) => Err(Error::runtime(format!(
            "{what}: format is rgba8 or r16, not `{other}`"
        ))),
    }
}

pub(super) fn table(lua: &Lua, ctx: &Arc<Ctx>) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    let own = ctx.clone();
    t.raw_set(
        "new",
        game_fn(
            lua,
            ctx,
            "torch.image.new",
            move |lua, ctx, (w, h, format): (f64, f64, Option<String>)| {
                let what = "torch.image.new";
                let (k, _) = ctx.caller(what)?;
                let (w, h) = (whole(w, what)?, whole(h, what)?);
                let side = 1..=i64::from(MAX_SIDE);
                if !side.contains(&w) || !side.contains(&h) {
                    return Err(Error::runtime(format!(
                        "{what}: each side is 1 to {MAX_SIDE}, not {w}x{h}"
                    )));
                }
                let format = format_of(format, what)?;
                let Some(key) = register(k, w as u32, h as u32, format) else {
                    return Ok((
                        Value::Nil,
                        Some(format!(
                            "over the {} M pixels one module's images may hold",
                            MAX_PIXELS >> 20
                        )),
                    ));
                };
                Ok((
                    Value::UserData(lua.create_userdata(Handle(key, own.clone()))?),
                    None,
                ))
            },
        )?,
    )?;
    let own = ctx.clone();
    t.raw_set(
        "load",
        game_fn(lua, ctx, "torch.image.load", move |_, ctx, path: String| {
            let what = "torch.image.load";
            let (k, _) = ctx.caller(what)?;
            let file = match super::fs::existing_file(ctx, &path, what)? {
                Ok(file) => file,
                Err(why) => return Ok((None, Some(why))),
            };
            let Some(slot) = Slot::take() else {
                return Ok((
                    None,
                    Some(format!("{MAX_JOBS} saves and loads are already running")),
                ));
            };
            let id = ctx.jobs.fetch_add(1, Relaxed) + 1;
            let ticket = Ticket {
                owner: k,
                id,
                generation: nbt::generation(),
            };
            let (kind, ctx) = (FileKind::of(&path), own.clone());
            let started = on_worker(move || {
                let _slot = slot;
                let result = read_image(file, kind).map(|image| Loaded {
                    owner: k,
                    ctx,
                    image,
                });
                if nbt::generation() == ticket.generation {
                    game::push(job(ticket, JobResult::Loaded(Mutex::new(Some(result)))));
                }
            });
            Ok(match started {
                Ok(()) => (Some(id), None),
                Err(why) => (None, Some(why)),
            })
        })?,
    )?;
    t.set_readonly(true);
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_header_round_trips() {
        let head = tile_header(16, 9, TexelFormat::R16, 0xDEAD_BEEF);
        assert_eq!(
            parse_tile_header(&head),
            Ok((16, 9, TexelFormat::R16, 0xDEAD_BEEF))
        );
        assert_eq!(&head[..4], b"TILE");
    }

    #[test]
    fn tile_header_refuses_what_it_cannot_bound() {
        let mut head = tile_header(16, 16, TexelFormat::Rgba8, 0);
        head[0] = b'X';
        assert!(parse_tile_header(&head).is_err());
        let too_wide = tile_header(MAX_SIDE + 1, 1, TexelFormat::Rgba8, 0);
        assert!(parse_tile_header(&too_wide).is_err());
        let empty = tile_header(0, 1, TexelFormat::Rgba8, 0);
        assert!(parse_tile_header(&empty).is_err());
        let mut format = tile_header(1, 1, TexelFormat::Rgba8, 0);
        format[6] = 9;
        assert!(parse_tile_header(&format).is_err());
    }

    #[test]
    fn crc_is_the_standard_crc32() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn a_tile_file_is_its_header_then_the_texels() {
        let px = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let file = encode(FileKind::Tile, 2, 1, TexelFormat::Rgba8, &px).expect("encodes");
        assert_eq!(file.len(), TILE_HEADER + px.len());
        assert_eq!(&file[TILE_HEADER..], &px);
        let head: &[u8; TILE_HEADER] = file[..TILE_HEADER].try_into().expect("sized");
        assert_eq!(
            parse_tile_header(head),
            Ok((2, 1, TexelFormat::Rgba8, crc32(&px)))
        );
    }
}
