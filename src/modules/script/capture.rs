use std::sync::Arc;

use mlua::{AnyUserData, Error, Lua, Table, Value};

use super::api::Ctx;
use super::game::game_fn;
use super::render::{known, point};
use crate::renderer::mod_capture::{
    self as gpu, Capture, ImageOp, Light, Projection, Ticket, View,
};

const FIELDS: [&str; 11] = [
    "into", "x", "y", "w", "h", "camera", "layers", "light", "fog", "depth", "wait",
];
const ORTHO_FIELDS: [&str; 7] = ["kind", "center", "extent", "dir", "up", "near", "far"];
const PERSPECTIVE_FIELDS: [&str; 7] = ["kind", "pos", "yaw", "pitch", "fov", "near", "far"];
const LAYER_FIELDS: [&str; 4] = ["terrain", "fluids", "entities", "sky"];

const MAX_SPAN: f64 = 4096.0;
const ORTHO_DEPTH: f64 = 512.0;
const FOV_DEFAULT: f64 = 70.0;
const FOV_MIN: f64 = 10.0;
const FOV_MAX: f64 = 170.0;
const PERSPECTIVE_NEAR: f64 = 0.05;
const PERSPECTIVE_FAR: f64 = 512.0;

fn axis(name: &str, what: &str, key: &str) -> mlua::Result<[f64; 3]> {
    Ok(match name {
        "down" => [0.0, -1.0, 0.0],
        "up" => [0.0, 1.0, 0.0],
        "north" => [0.0, 0.0, -1.0],
        "south" => [0.0, 0.0, 1.0],
        "east" => [1.0, 0.0, 0.0],
        "west" => [-1.0, 0.0, 0.0],
        other => {
            return Err(Error::runtime(format!(
                "{what}: camera `{key}` is down, up, north, south, east or west, not `{other}`"
            )));
        }
    })
}

fn number(t: &Table, key: &str, what: &str) -> mlua::Result<Option<f64>> {
    match t.raw_get::<Option<f64>>(key)? {
        Some(n) if !n.is_finite() => Err(Error::runtime(format!(
            "{what}: `{key}` is a finite number"
        ))),
        n => Ok(n),
    }
}

fn flag(t: &Table, key: &str, default: bool) -> mlua::Result<bool> {
    Ok(t.raw_get::<Option<bool>>(key)?.unwrap_or(default))
}

fn extent(v: Value, what: &str) -> mlua::Result<(f64, f64)> {
    let bad = || {
        Error::runtime(format!(
            "{what}: camera `extent` is {{ w, h }}, each above 0 and at most {MAX_SPAN}"
        ))
    };
    let Value::Table(t) = v else {
        return Err(bad());
    };
    let side = |name: &str, i: usize| -> mlua::Result<f64> {
        let n = super::render::number_at(&t, name, i)?.ok_or_else(bad)?;
        if n.is_finite() && n > 0.0 && n <= MAX_SPAN {
            Ok(n)
        } else {
            Err(bad())
        }
    };
    Ok((side("w", 1)?, side("h", 2)?))
}

fn depth_range(near: f64, far: f64, what: &str) -> mlua::Result<(f64, f64)> {
    if far <= near || far - near > MAX_SPAN || near.abs() > MAX_SPAN {
        return Err(Error::runtime(format!(
            "{what}: the camera keeps {near} to {far}; far must be past near, by at most {MAX_SPAN}"
        )));
    }
    Ok((near, far))
}

fn ortho(cam: &Table, what: &str) -> mlua::Result<View> {
    known(cam, &ORTHO_FIELDS, what)?;
    let center = point(cam.raw_get("center")?, "center", what)?;
    let (width, height) = extent(cam.raw_get("extent")?, what)?;
    let dir_name = cam
        .raw_get::<Option<String>>("dir")?
        .unwrap_or_else(|| "down".into());
    let forward = axis(&dir_name, what, "dir")?;
    let vertical = forward[1] != 0.0;
    let up_name = cam
        .raw_get::<Option<String>>("up")?
        .unwrap_or_else(|| (if vertical { "north" } else { "up" }).into());
    let up = axis(&up_name, what, "up")?;
    if forward.iter().zip(&up).any(|(f, u)| f * u != 0.0) {
        return Err(Error::runtime(format!(
            "{what}: camera `up` ({up_name}) has to be at right angles to `dir` ({dir_name})"
        )));
    }
    let (near, far) = (number(cam, "near", what)?, number(cam, "far", what)?);
    let (eye, near, far) = if dir_name == "down" {
        let top = near.unwrap_or(center[1]);
        let bottom = far.unwrap_or(top - ORTHO_DEPTH);
        let (n, f) = depth_range(0.0, top - bottom, what)?;
        ([center[0], top, center[2]], n, f)
    } else {
        let (n, f) = depth_range(near.unwrap_or(0.0), far.unwrap_or(ORTHO_DEPTH), what)?;
        (center, n, f)
    };
    Ok(View {
        eye,
        forward,
        up,
        near,
        far,
        projection: Projection::Ortho { width, height },
    })
}

fn perspective(cam: &Table, what: &str) -> mlua::Result<View> {
    known(cam, &PERSPECTIVE_FIELDS, what)?;
    let pos = point(cam.raw_get("pos")?, "pos", what)?;
    let yaw = number(cam, "yaw", what)?.unwrap_or(0.0).to_radians();
    let pitch = number(cam, "pitch", what)?
        .unwrap_or(0.0)
        .clamp(-90.0, 90.0)
        .to_radians();
    let fov = number(cam, "fov", what)?.unwrap_or(FOV_DEFAULT);
    if !(FOV_MIN..=FOV_MAX).contains(&fov) {
        return Err(Error::runtime(format!(
            "{what}: camera `fov` is {FOV_MIN} to {FOV_MAX} degrees, not {fov}"
        )));
    }
    let near = number(cam, "near", what)?.unwrap_or(PERSPECTIVE_NEAR);
    let far = number(cam, "far", what)?.unwrap_or(PERSPECTIVE_FAR);
    if near <= 0.0 {
        return Err(Error::runtime(format!(
            "{what}: camera `near` is above 0, not {near}"
        )));
    }
    let (near, far) = depth_range(near, far, what)?;
    let forward = [
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
        yaw.cos() * pitch.cos(),
    ];
    let right = [-yaw.cos(), 0.0, -yaw.sin()];
    let up = [
        right[1] * forward[2] - right[2] * forward[1],
        right[2] * forward[0] - right[0] * forward[2],
        right[0] * forward[1] - right[1] * forward[0],
    ];
    Ok(View {
        eye: pos,
        forward,
        up,
        near,
        far,
        projection: Projection::Perspective {
            fov_y: fov.to_radians(),
        },
    })
}

fn light(t: &Table, what: &str) -> mlua::Result<Light> {
    match t.raw_get::<Option<String>>("light")?.as_deref() {
        None | Some("day") => Ok(Light::Day),
        Some("world") => Ok(Light::World),
        Some("full") => Ok(Light::Full),
        Some(other) => Err(Error::runtime(format!(
            "{what}: light is world, day or full, not `{other}`"
        ))),
    }
}

fn layers(v: Option<Table>, what: &str) -> mlua::Result<(bool, bool)> {
    let Some(t) = v else { return Ok((true, true)) };
    known(&t, &LAYER_FIELDS, what)?;
    for missing in ["entities", "sky"] {
        if flag(&t, missing, false)? {
            return Err(Error::runtime(format!(
                "{what}: layers.{missing} is not supported yet"
            )));
        }
    }
    Ok((flag(&t, "terrain", true)?, flag(&t, "fluids", true)?))
}

fn capture(ctx: &Ctx, spec: &Table) -> mlua::Result<(Option<u64>, Option<String>)> {
    let what = "torch.render.capture";
    let (k, _) = ctx.caller(what)?;
    known(spec, &FIELDS, what)?;
    let into: AnyUserData = spec
        .raw_get::<Option<AnyUserData>>("into")?
        .ok_or_else(|| {
            Error::runtime(format!(
                "{what}: `into` is required, an image from torch.image"
            ))
        })?;
    let depth: Option<AnyUserData> = spec.raw_get("depth")?;
    let Some(cam) = spec.raw_get::<Option<Table>>("camera")? else {
        return Err(Error::runtime(format!("{what}: `camera` is required")));
    };
    let view = match cam.raw_get::<Option<String>>("kind")?.as_deref() {
        None | Some("ortho") => ortho(&cam, what)?,
        Some("perspective") => perspective(&cam, what)?,
        Some(other) => {
            return Err(Error::runtime(format!(
                "{what}: camera kind is ortho or perspective, not `{other}`"
            )));
        }
    };
    let (terrain, fluids) = layers(spec.raw_get("layers")?, what)?;
    let light = light(spec, what)?;
    let (fog, wait) = (flag(spec, "fog", false)?, flag(spec, "wait", true)?);
    let r = [
        number(spec, "x", what)?,
        number(spec, "y", what)?,
        number(spec, "w", what)?,
        number(spec, "h", what)?,
    ];
    let (into, depth, rect) = match super::image::begin_capture(k, &into, depth.as_ref(), r, what)?
    {
        Ok(found) => found,
        Err(why) => return Ok((None, Some(why))),
    };
    let id = ctx.jobs.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    let ticket = Ticket {
        owner: k,
        id,
        generation: super::nbt::generation(),
    };
    gpu::send(ImageOp::Capture(Capture {
        into,
        depth,
        rect,
        view,
        terrain,
        fluids,
        light,
        fog,
        wait,
        ticket,
        partial: false,
    }));
    Ok((Some(id), None))
}

pub(super) fn function(lua: &Lua, ctx: &Arc<Ctx>) -> mlua::Result<mlua::Function> {
    game_fn(lua, ctx, "torch.render.capture", |_, ctx, spec: Table| {
        capture(ctx, &spec)
    })
}
