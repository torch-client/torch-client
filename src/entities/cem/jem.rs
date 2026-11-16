use serde_json::Value;

use super::{Dir, resolve};
use crate::entities::geom::PartPose;

pub struct Model {
    pub texture: Option<String>,
    pub parts: Vec<Part>,
}

pub struct Part {
    pub id: String,
    pub target: Option<String>,
    pub attach: bool,
    pub texture: Option<String>,
    pub tex_size: [f32; 2],
    pub pose: PartPose,
    pub boxes: Vec<BoxDef>,
    pub children: Vec<Part>,
}

pub struct BoxDef {
    pub coords: [f32; 6],
    pub grow: [f32; 3],
    pub uv: BoxUv,
}

pub enum BoxUv {
    Offset {
        offset: [f32; 2],
        mirror_u: bool,
        mirror_v: bool,
    },
    PerFace([Option<[f32; 4]>; 6]),
}

pub fn parse(bytes: &[u8], dir: &Dir) -> Result<Model, String> {
    let json: Value = serde_json::from_slice(crate::platform::assets::skip_bom(bytes))
        .map_err(|e| format!("not JSON: {e}"))?;
    let tex_size = floats::<2>(&json, "textureSize").unwrap_or([64.0, 32.0]);
    let texture = string(&json, "texture").and_then(|t| resolve(&t, "png", dir));
    let mut parts = Vec::new();
    for entry in json
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match prepare(entry, tex_size, dir, 0) {
            Ok(part) => parts.push(part),
            Err(e) => crate::log_warn!("cem", "{}: skipped a part: {e}", dir.file),
        }
    }
    Ok(Model { texture, parts })
}

const MAX_DEPTH: usize = 64;

fn prepare(json: &Value, parent_size: [f32; 2], dir: &Dir, depth: usize) -> Result<Part, String> {
    if depth > MAX_DEPTH {
        return Err("submodels nest too deep".into());
    }
    let jpm = match string(json, "model") {
        Some(model) => match resolve(&model, "jpm", dir).and_then(|path| super::read(&path)) {
            Some(bytes) => Some(
                serde_json::from_slice::<Value>(crate::platform::assets::skip_bom(&bytes))
                    .map_err(|e| format!("{model}: not JSON: {e}"))?,
            ),
            None => {
                crate::log_warn!("cem", "{}: part model {model} not found", dir.file);
                None
            }
        },
        None => None,
    };
    let jpm = jpm.as_ref();
    let field = |key: &str| field(json, jpm, key);

    let invert = field("invertAxis").and_then(Value::as_str).unwrap_or("");
    let (ix, iy, iz) = (
        invert.contains('x'),
        invert.contains('y'),
        invert.contains('z'),
    );
    let sign = |inverted: bool, v: f32| if inverted { -v } else { v };

    let tex_size = field("textureSize")
        .and_then(float_array::<2>)
        .unwrap_or(parent_size);
    let texture = field("texture")
        .and_then(Value::as_str)
        .and_then(|t| resolve(t, "png", dir));
    let translate = field("translate")
        .and_then(float_array::<3>)
        .unwrap_or([0.0; 3]);
    let rotate = field("rotate")
        .and_then(float_array::<3>)
        .unwrap_or([0.0; 3]);
    let scale = match number(json.get("scale")) {
        Some(s) if s != 1.0 => s,
        _ => jpm.and_then(|j| number(j.get("scale"))).unwrap_or(1.0),
    };
    let mirror = field("mirrorTexture").and_then(Value::as_str).unwrap_or("");
    let (mirror_u, mirror_v) = (mirror.contains('u'), mirror.contains('v'));

    let mut boxes = Vec::new();
    for b in field("boxes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match prepare_box(b, (ix, iy, iz), mirror_u, mirror_v) {
            Ok(b) => boxes.push(b),
            Err(e) => crate::log_warn!("cem", "{}: skipped a box: {e}", dir.file),
        }
    }

    let mut children = Vec::new();
    let subs = field("submodels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    for sub in subs.chain(field("submodel")) {
        match prepare(sub, tex_size, dir, depth + 1) {
            Ok(child) => children.push(child),
            Err(e) => crate::log_warn!("cem", "{}: skipped a submodel: {e}", dir.file),
        }
    }

    let id = json.get("id").and_then(Value::as_str).unwrap_or("");
    Ok(Part {
        id: format!("EMF_{id}"),
        target: json.get("part").and_then(Value::as_str).map(str::to_owned),
        attach: json.get("attach").is_some_and(truthy),
        texture,
        tex_size,
        pose: PartPose {
            x: sign(ix, translate[0]),
            y: sign(iy, translate[1]),
            z: sign(iz, translate[2]),
            x_rot: sign(ix, rotate[0]).to_radians(),
            y_rot: sign(iy, rotate[1]).to_radians(),
            z_rot: sign(iz, rotate[2]).to_radians(),
            x_scale: scale,
            y_scale: scale,
            z_scale: scale,
        },
        boxes,
        children,
    })
}

fn prepare_box(
    json: &Value,
    invert: (bool, bool, bool),
    mirror_u: bool,
    mirror_v: bool,
) -> Result<BoxDef, String> {
    let mut coords = float_array::<6>(json.get("coordinates").ok_or("no coordinates")?)
        .ok_or("coordinates are not six numbers")?;
    for (axis, inverted) in [invert.0, invert.1, invert.2].into_iter().enumerate() {
        if inverted {
            coords[axis] = -coords[axis] - coords[axis + 3];
        }
    }

    let one = number(json.get("sizeAdd")).unwrap_or(0.0);
    let mut grow = ["sizeAddX", "sizeAddY", "sizeAddZ"].map(|k| number(json.get(k)).unwrap_or(0.0));
    if one != 0.0 && grow == [0.0; 3] {
        grow = [one; 3];
    }
    if let Some(all) = json.get("sizesAdd").and_then(float_array::<3>) {
        grow = all;
    }

    let uv = match json.get("textureOffset") {
        Some(v) if !is_blank(v) => BoxUv::Offset {
            offset: float_array::<2>(v).ok_or("textureOffset is not two numbers")?,
            mirror_u,
            mirror_v,
        },
        _ => {
            let face = |key: &str| -> Result<Option<[f32; 4]>, String> {
                match json.get(key) {
                    None => Ok(None),
                    Some(v) if is_blank(v) => Ok(None),
                    Some(v) => float_array::<4>(v)
                        .map(Some)
                        .ok_or_else(|| format!("{key} is not four numbers")),
                }
            };
            let mut rects = [
                face("uvDown")?,
                face("uvUp")?,
                face("uvNorth")?,
                face("uvSouth")?,
                face("uvWest")?,
                face("uvEast")?,
            ];
            for (legacy, slot) in [("uvFront", 2), ("uvBack", 4), ("uvLeft", 2), ("uvRight", 5)] {
                if let Some(rect) = face(legacy).ok().flatten() {
                    rects[slot] = Some(rect);
                }
            }
            BoxUv::PerFace(rects)
        }
    };
    Ok(BoxDef { coords, grow, uv })
}

fn field<'a>(json: &'a Value, jpm: Option<&'a Value>, key: &str) -> Option<&'a Value> {
    json.get(key)
        .filter(|v| !is_blank(v))
        .or_else(|| jpm.and_then(|j| j.get(key)).filter(|v| !is_blank(v)))
}

fn is_blank(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        Value::Array(a) => a.is_empty(),
        _ => false,
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::String(s) => s.trim().eq_ignore_ascii_case("true"),
        _ => false,
    }
}

fn string(json: &Value, key: &str) -> Option<String> {
    json.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn number(v: Option<&Value>) -> Option<f32> {
    match v? {
        Value::Number(n) => n.as_f64().map(|f| f as f32),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn floats<const N: usize>(json: &Value, key: &str) -> Option<[f32; N]> {
    float_array(json.get(key)?)
}

fn float_array<const N: usize>(v: &Value) -> Option<[f32; N]> {
    let items = v.as_array()?;
    if items.len() != N {
        return None;
    }
    let mut out = [0.0; N];
    for (slot, item) in out.iter_mut().zip(items) {
        *slot = number(Some(item))?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> Dir {
        Dir {
            folder: "optifine/cem/".into(),
            file: "test.jem".into(),
        }
    }

    #[test]
    fn invert_axis_flips_translate_and_box() {
        let json = br#"{"textureSize":[64,32],"models":[{"part":"head","id":"head","invertAxis":"xy",
            "translate":[0,-18,0],"boxes":[{"textureOffset":[0,0],"coordinates":[-4,18,-4,8,8,8]}]}]}"#;
        let model = parse(json, &dir()).unwrap();
        let head = &model.parts[0];
        assert_eq!(head.id, "EMF_head");
        assert_eq!(head.target.as_deref(), Some("head"));
        assert_eq!(head.pose.y, 18.0);
        assert_eq!(head.boxes[0].coords, [-4.0, -26.0, -4.0, 8.0, 8.0, 8.0]);
    }

    #[test]
    fn bad_box_is_skipped() {
        let json = br#"{"models":[{"part":"body","boxes":[{"coordinates":[1,2]},
            {"textureOffset":[0,0],"coordinates":[0,0,0,1,1,1],"sizeAdd":0.5}]}]}"#;
        let model = parse(json, &dir()).unwrap();
        assert_eq!(model.parts[0].boxes.len(), 1);
        assert_eq!(model.parts[0].boxes[0].grow, [0.5; 3]);
        assert_eq!(model.parts[0].tex_size, [64.0, 32.0]);
    }
}
