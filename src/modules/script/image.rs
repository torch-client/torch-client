use std::collections::BTreeMap;
use std::io::Read;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Mutex, PoisonError, RwLock, RwLockReadGuard};

use mlua::{AnyUserData, Error, Lua, Table, UserData, UserDataMethods, Value};

use super::api::Ctx;
use super::game::{self, Pending, game_fn};

const MAX_SIDE: u32 = 4096;
const MAX_PIXELS: u64 = 64 << 20;
const MAX_SAVES: usize = 4;
const BPP: usize = 4;

pub struct Pixels {
    pub w: u32,
    pub h: u32,
    generation: AtomicU64,
    rgba: RwLock<Vec<u8>>,
}

impl Pixels {
    pub fn generation(&self) -> u64 {
        self.generation.load(Relaxed)
    }

    pub fn read(&self) -> RwLockReadGuard<'_, Vec<u8>> {
        self.rgba.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self, f: impl FnOnce(&mut Vec<u8>)) {
        f(&mut self.rgba.write().unwrap_or_else(PoisonError::into_inner));
        self.generation.fetch_add(1, Relaxed);
    }
}

struct Entry {
    owner: usize,
    pixels: Arc<Pixels>,
}

static IMAGES: Mutex<BTreeMap<u64, Entry>> = Mutex::new(BTreeMap::new());
static NEXT: AtomicU64 = AtomicU64::new(1);
static SAVING: AtomicUsize = AtomicUsize::new(0);

pub fn shared(key: u64) -> Option<Arc<Pixels>> {
    IMAGES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
        .map(|e| e.pixels.clone())
}

pub(super) fn clear() {
    IMAGES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
}

fn argb(v: f64, what: &str) -> mlua::Result<u32> {
    if v.is_finite() && v.fract() == 0.0 && (f64::from(i32::MIN)..=f64::from(u32::MAX)).contains(&v)
    {
        return Ok(v as i64 as u32);
    }
    Err(Error::runtime(format!(
        "{what}: a pixel is 0xAARRGGBB, not {v}"
    )))
}

fn to_rgba(c: u32) -> [u8; 4] {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]
}

fn to_argb(p: &[u8]) -> u32 {
    u32::from(p[3]) << 24 | u32::from(p[0]) << 16 | u32::from(p[1]) << 8 | u32::from(p[2])
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

pub(super) struct Handle(u64, Arc<Ctx>);

pub(super) fn of(ud: &AnyUserData, what: &str) -> mlua::Result<Option<(u64, u32, u32)>> {
    let key = ud
        .borrow::<Handle>()
        .map_err(|_| Error::runtime(format!("{what} takes an image from torch.image.new")))?
        .0;
    Ok(shared(key).map(|p| (key, p.w, p.h)))
}

impl UserData for Handle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("size", |_, h, ()| {
            Ok(shared(h.0).map(|p| (p.w, p.h)).unzip())
        });
        methods.add_method("set", |_, h, (x, y, c): (f64, f64, f64)| {
            let what = "img:set";
            let Some(p) = shared(h.0) else {
                return Ok(false);
            };
            let i = index(&p, x, y, what)?;
            let px = to_rgba(argb(c, what)?);
            p.write(|v| v[i..i + BPP].copy_from_slice(&px));
            Ok(true)
        });
        methods.add_method("get", |_, h, (x, y): (f64, f64)| {
            let Some(p) = shared(h.0) else {
                return Ok(None);
            };
            let i = index(&p, x, y, "img:get")?;
            Ok(Some(to_argb(&p.read()[i..i + BPP])))
        });
        methods.add_method("fill", |_, h, c: f64| {
            let Some(p) = shared(h.0) else {
                return Ok(false);
            };
            let px = to_rgba(argb(c, "img:fill")?);
            p.write(|v| v.chunks_exact_mut(BPP).for_each(|d| d.copy_from_slice(&px)));
            Ok(true)
        });
        methods.add_method(
            "blit",
            |_, h, (buf, x, y, w, hh): (mlua::Buffer, f64, f64, f64, f64)| {
                blit(h.0, &buf, [x, y, w, hh])
            },
        );
        methods.add_method("save", |_, h, path: String| save(&h.1, h.0, &path));
        methods.add_method("remove", |_, h, ()| {
            Ok(IMAGES
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&h.0)
                .is_some())
        });
    }
}

fn index(p: &Pixels, x: f64, y: f64, what: &str) -> mlua::Result<usize> {
    let (x, y) = (whole(x, what)?, whole(y, what)?);
    if !(0..i64::from(p.w)).contains(&x) || !(0..i64::from(p.h)).contains(&y) {
        return Err(Error::runtime(format!(
            "{what}: {x}, {y} is outside the {}x{} image (0-based)",
            p.w, p.h
        )));
    }
    Ok((y as usize * p.w as usize + x as usize) * BPP)
}

fn blit(key: u64, buf: &mlua::Buffer, rect: [f64; 4]) -> mlua::Result<bool> {
    let what = "img:blit";
    let Some(p) = shared(key) else {
        return Ok(false);
    };
    let [x, y, w, h] = rect;
    let (x, y, w, h) = (
        whole(x, what)?,
        whole(y, what)?,
        whole(w, what)?,
        whole(h, what)?,
    );
    if w < 1 || h < 1 || x < 0 || y < 0 || x + w > i64::from(p.w) || y + h > i64::from(p.h) {
        return Err(Error::runtime(format!(
            "{what}: {w}x{h} at {x}, {y} does not fit in the {}x{} image",
            p.w, p.h
        )));
    }
    let (x, y, w, h) = (x as usize, y as usize, w as usize, h as usize);
    let n = w * h * BPP;
    if buf.len() < n {
        return Err(Error::runtime(format!(
            "{what}: {w}x{h} needs {n} bytes; the buffer has {}",
            buf.len()
        )));
    }
    let mut src = vec![0u8; n];
    buf.clone()
        .cursor()
        .read_exact(&mut src)
        .map_err(|e| Error::runtime(format!("{what}: {e}")))?;
    let stride = p.w as usize * BPP;
    p.write(|v| {
        for row in 0..h {
            let from = &src[row * w * BPP..(row + 1) * w * BPP];
            let at = (y + row) * stride + x * BPP;
            for (d, s) in v[at..at + w * BPP]
                .chunks_exact_mut(BPP)
                .zip(from.chunks_exact(BPP))
            {
                d.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
            }
        }
    });
    Ok(true)
}

struct Slot;

impl Drop for Slot {
    fn drop(&mut self) {
        SAVING.fetch_sub(1, Relaxed);
    }
}

fn save(ctx: &Ctx, key: u64, path: &str) -> mlua::Result<(Option<u64>, Option<String>)> {
    let what = "img:save";
    let (k, _) = ctx.caller(what)?;
    let target = match super::fs::writable(ctx, path, what)? {
        Ok(t) => t,
        Err(why) => return Ok((None, Some(why))),
    };
    let Some(p) = shared(key) else {
        return Ok((None, Some("the image was removed".into())));
    };
    if SAVING.fetch_add(1, Relaxed) >= MAX_SAVES {
        SAVING.fetch_sub(1, Relaxed);
        return Ok((None, Some(format!("{MAX_SAVES} saves are already running"))));
    }
    let (w, h, rgba) = (p.w, p.h, p.read().clone());
    let id = ctx.jobs.fetch_add(1, Relaxed) + 1;
    let generation = super::nbt::generation();
    let slot = Slot;
    let job = move || {
        let _slot = slot;
        let result = encode(w, h, &rgba)
            .and_then(|png| super::fs::write_to(&target, &png).map(|()| png.len() as u64));
        if super::nbt::generation() == generation {
            game::push(Pending::Job {
                owner: k,
                id,
                generation,
                result: super::nbt::JobResult::Saved(result),
            });
        }
    };
    #[cfg(target_arch = "wasm32")]
    crate::platform::executor::spawn(async move { job() });
    #[cfg(not(target_arch = "wasm32"))]
    if let Err(e) = std::thread::Builder::new()
        .name("lua-png".to_string())
        .spawn(job)
    {
        return Ok((None, Some(format!("could not start the save: {e}"))));
    }
    Ok((Some(id), None))
}

fn encode(w: u32, h: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    use ::image::ImageEncoder;
    let mut out = Vec::new();
    ::image::codecs::png::PngEncoder::new(&mut out)
        .write_image(rgba, w, h, ::image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("could not encode the PNG: {e}"))?;
    Ok(out)
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
            move |lua, ctx, (w, h): (f64, f64)| {
                let what = "torch.image.new";
                let (k, _) = ctx.caller(what)?;
                let (w, h) = (whole(w, what)?, whole(h, what)?);
                let side = 1..=i64::from(MAX_SIDE);
                if !side.contains(&w) || !side.contains(&h) {
                    return Err(Error::runtime(format!(
                        "{what}: each side is 1 to {MAX_SIDE}, not {w}x{h}"
                    )));
                }
                let (w, h) = (w as u32, h as u32);
                let mut all = IMAGES.lock().unwrap_or_else(PoisonError::into_inner);
                let held: u64 = all
                    .values()
                    .filter(|e| e.owner == k)
                    .map(|e| u64::from(e.pixels.w) * u64::from(e.pixels.h))
                    .sum();
                if held + u64::from(w) * u64::from(h) > MAX_PIXELS {
                    return Ok((
                        Value::Nil,
                        Some(format!(
                            "over the {} M pixels one module's images may hold",
                            MAX_PIXELS >> 20
                        )),
                    ));
                }
                let key = NEXT.fetch_add(1, Relaxed);
                let rgba = vec![0u8; w as usize * h as usize * BPP];
                let pixels = Arc::new(Pixels {
                    w,
                    h,
                    generation: AtomicU64::new(1),
                    rgba: RwLock::new(rgba),
                });
                all.insert(key, Entry { owner: k, pixels });
                Ok((
                    Value::UserData(lua.create_userdata(Handle(key, own.clone()))?),
                    None,
                ))
            },
        )?,
    )?;
    t.set_readonly(true);
    Ok(t)
}
