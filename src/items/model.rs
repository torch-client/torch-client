use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use super::json::Json;
use crate::direction::Direction;

pub const DOWN: usize = Direction::Down.index();
pub const UP: usize = Direction::Up.index();
pub const NORTH: usize = Direction::North.index();
pub const SOUTH: usize = Direction::South.index();
pub const WEST: usize = Direction::West.index();
#[allow(
    dead_code,
    reason = "completes the six; the +X face is never named directly"
)]
pub const EAST: usize = Direction::East.index();

pub const DIR_NAMES: [&str; 6] = ["down", "up", "north", "south", "west", "east"];

pub const DIR_SHADE: [f32; 6] = [0.5, 1.0, 0.8, 0.8, 0.6, 0.6];

pub fn dir_by_name(name: &str) -> Option<usize> {
    DIR_NAMES.iter().position(|n| *n == name)
}

pub(crate) const FACE_CORNERS: [[[bool; 3]; 4]; 6] = [
    [
        [false, false, true],
        [false, false, false],
        [true, false, false],
        [true, false, true],
    ],
    [
        [false, true, false],
        [false, true, true],
        [true, true, true],
        [true, true, false],
    ],
    [
        [true, true, false],
        [true, false, false],
        [false, false, false],
        [false, true, false],
    ],
    [
        [false, true, true],
        [false, false, true],
        [true, false, true],
        [true, true, true],
    ],
    [
        [false, true, false],
        [false, false, false],
        [false, false, true],
        [false, true, true],
    ],
    [
        [true, true, true],
        [true, false, true],
        [true, false, false],
        [true, true, false],
    ],
];

pub fn face_positions(from: [f32; 3], to: [f32; 3], dir: usize) -> [[f32; 3]; 4] {
    let mut out = [[0.0f32; 3]; 4];
    for (v, sel) in out.iter_mut().zip(FACE_CORNERS[dir].iter()) {
        for axis in 0..3 {
            v[axis] = if sel[axis] { to[axis] } else { from[axis] };
        }
    }
    out
}

pub fn default_face_uv(from: [f32; 3], to: [f32; 3], dir: usize) -> [f32; 4] {
    match dir {
        DOWN => [from[0], 16.0 - to[2], to[0], 16.0 - from[2]],
        UP => [from[0], from[2], to[0], to[2]],
        NORTH => [16.0 - to[0], 16.0 - to[1], 16.0 - from[0], 16.0 - from[1]],
        SOUTH => [from[0], 16.0 - to[1], to[0], 16.0 - from[1]],
        WEST => [from[2], 16.0 - to[1], to[2], 16.0 - from[1]],
        _ => [16.0 - to[2], 16.0 - to[1], 16.0 - from[2], 16.0 - from[1]],
    }
}

pub fn corner_uv(uv: [f32; 4], quadrant: u8, index: usize) -> [f32; 2] {
    let i = (index + quadrant as usize) & 3;
    let u = if i == 0 || i == 1 { uv[0] } else { uv[2] };
    let v = if i == 0 || i == 3 { uv[1] } else { uv[3] };
    [u, v]
}

#[derive(Debug, Clone, Copy)]
pub struct ElemRot {
    pub origin: [f32; 3],
    pub axis: usize,
    pub angle: f32,
    pub rescale: bool,
}

#[derive(Debug, Clone)]
pub struct FaceDef {
    pub dir: usize,
    pub texture: String,
    pub uv: Option<[f32; 4]>,
    pub quadrant: u8,
    pub tint: i32,
    pub cull: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct Elem {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub rotation: Option<ElemRot>,
    pub shade: bool,
    pub faces: Vec<FaceDef>,
}

#[derive(Debug, Clone, Copy)]
pub struct Transform {
    pub rotation: [f32; 3],
    pub translation: [f32; 3],
    pub scale: [f32; 3],
}

impl Transform {
    pub const NONE: Transform = Transform {
        rotation: [0.0; 3],
        translation: [0.0; 3],
        scale: [1.0; 3],
    };

    fn parse(node: &Json) -> Transform {
        let rotation = node
            .get("rotation")
            .and_then(Json::as_vec3)
            .unwrap_or([0.0; 3]);
        let mut translation = node
            .get("translation")
            .and_then(Json::as_vec3)
            .unwrap_or([0.0; 3]);
        for t in translation.iter_mut() {
            *t = (*t * 0.0625).clamp(-5.0, 5.0);
        }
        let mut scale = node
            .get("scale")
            .and_then(Json::as_vec3)
            .unwrap_or([1.0; 3]);
        for s in scale.iter_mut() {
            *s = s.clamp(-4.0, 4.0);
        }
        Transform {
            rotation,
            translation,
            scale,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DisplayContext {
    ThirdPersonLeftHand,
    ThirdPersonRightHand,
    FirstPersonLeftHand,
    FirstPersonRightHand,
    Head,
    Gui,
    Ground,
    Fixed,
    OnShelf,
}

impl DisplayContext {
    pub const ALL: [DisplayContext; 9] = [
        DisplayContext::ThirdPersonLeftHand,
        DisplayContext::ThirdPersonRightHand,
        DisplayContext::FirstPersonLeftHand,
        DisplayContext::FirstPersonRightHand,
        DisplayContext::Head,
        DisplayContext::Gui,
        DisplayContext::Ground,
        DisplayContext::Fixed,
        DisplayContext::OnShelf,
    ];

    pub fn name(self) -> &'static str {
        match self {
            DisplayContext::ThirdPersonLeftHand => "thirdperson_lefthand",
            DisplayContext::ThirdPersonRightHand => "thirdperson_righthand",
            DisplayContext::FirstPersonLeftHand => "firstperson_lefthand",
            DisplayContext::FirstPersonRightHand => "firstperson_righthand",
            DisplayContext::Head => "head",
            DisplayContext::Gui => "gui",
            DisplayContext::Ground => "ground",
            DisplayContext::Fixed => "fixed",
            DisplayContext::OnShelf => "on_shelf",
        }
    }

    pub fn parse(name: &str) -> Option<DisplayContext> {
        DisplayContext::ALL
            .into_iter()
            .find(|ctx| ctx.name() == name)
    }

    pub fn right_hand_of(self) -> Option<DisplayContext> {
        match self {
            DisplayContext::ThirdPersonLeftHand => Some(DisplayContext::ThirdPersonRightHand),
            DisplayContext::FirstPersonLeftHand => Some(DisplayContext::FirstPersonRightHand),
            _ => None,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayTransforms([Option<Transform>; 9]);

impl DisplayTransforms {
    pub fn get(&self, ctx: DisplayContext) -> Option<Transform> {
        self.0[ctx.index()]
    }

    pub fn get_named(&self, name: &str) -> Option<Transform> {
        DisplayContext::parse(name).and_then(|ctx| self.get(ctx))
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(Option::is_none)
    }

    pub fn for_context(&self, ctx: DisplayContext) -> Transform {
        self.get(ctx)
            .or_else(|| ctx.right_hand_of().and_then(|right| self.get(right)))
            .unwrap_or(Transform::NONE)
    }

    fn set_if_absent(&mut self, ctx: DisplayContext, transform: impl FnOnce() -> Transform) {
        let slot = &mut self.0[ctx.index()];
        if slot.is_none() {
            *slot = Some(transform());
        }
    }
}

#[derive(Debug)]
pub struct Resolved {
    pub textures: HashMap<String, String>,
    pub elements: Vec<Elem>,
    pub gui: Option<Transform>,
    pub display: DisplayTransforms,
    pub generated: bool,
    pub ambient_occlusion: bool,
}

impl Default for Resolved {
    fn default() -> Self {
        Self {
            textures: HashMap::new(),
            elements: Vec::new(),
            gui: None,
            display: DisplayTransforms::default(),
            generated: false,
            ambient_occlusion: true,
        }
    }
}

impl Resolved {
    pub fn texture_path(&self, reference: &str) -> Option<String> {
        let mut name = reference.strip_prefix('#').unwrap_or(reference).to_string();
        for _ in 0..8 {
            let value = self.textures.get(&name)?;
            match value.strip_prefix('#') {
                Some(next) => name = next.to_string(),
                None => return Some(strip_namespace(value).to_string()),
            }
        }
        None
    }
}

pub fn strip_namespace(name: &str) -> &str {
    name.split_once(':').map(|(_, rest)| rest).unwrap_or(name)
}

pub struct Tex {
    pub w: u32,
    pub h: u32,
    pub px: Vec<[u8; 4]>,
}

impl Tex {
    pub fn get(&self, x: u32, y: u32) -> [u8; 4] {
        let x = x.min(self.w.saturating_sub(1));
        let y = y.min(self.h.saturating_sub(1));
        self.px[(y * self.w + x) as usize]
    }

    pub fn sample(&self, u: f32, v: f32) -> [u8; 4] {
        let x = (u * self.w as f32).floor().clamp(0.0, self.w as f32 - 1.0) as u32;
        let y = (v * self.h as f32).floor().clamp(0.0, self.h as f32 - 1.0) as u32;
        self.get(x, y)
    }
}

pub struct Assets {
    root: PathBuf,
    files: Mutex<HashMap<String, Option<Arc<Json>>>>,
    models: Mutex<HashMap<String, Arc<Resolved>>>,
    textures: Mutex<HashMap<String, Option<Arc<Tex>>>>,
}

pub fn shared() -> &'static Assets {
    static SHARED: OnceLock<Assets> = OnceLock::new();
    SHARED.get_or_init(|| Assets::new(crate::assets_root()))
}

impl Assets {
    pub fn new(root: &Path) -> Assets {
        Assets {
            root: root.to_path_buf(),
            files: Mutex::new(HashMap::new()),
            models: Mutex::new(HashMap::new()),
            textures: Mutex::new(HashMap::new()),
        }
    }

    pub fn json(&self, relative: &str) -> Option<Arc<Json>> {
        if let Some(hit) = self.files.lock().unwrap().get(relative) {
            return hit.clone();
        }
        let parsed = crate::platform::assets::read_to_string(self.root.join(relative))
            .and_then(|text| Json::parse(&text))
            .map(Arc::new);
        self.files
            .lock()
            .unwrap()
            .insert(relative.to_string(), parsed.clone());
        parsed
    }

    pub fn texture(&self, path: &str) -> Option<Arc<Tex>> {
        if let Some(hit) = self.textures.lock().unwrap().get(path) {
            return hit.clone();
        }
        let loaded = self.decode_texture(path).map(Arc::new);
        self.textures
            .lock()
            .unwrap()
            .insert(path.to_string(), loaded.clone());
        loaded
    }

    fn decode_texture(&self, path: &str) -> Option<Tex> {
        let file = self.root.join(format!("textures/{}.png", path));
        let img = crate::platform::assets::open_image(&file).ok()?.to_rgba8();
        let (w, mut h) = (img.width(), img.height());
        if w == 0 || h == 0 {
            return None;
        }
        if h > w && h % w == 0 {
            h = w;
        }
        let mut px = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            for x in 0..w {
                px.push(img.get_pixel(x, y).0);
            }
        }
        Some(Tex { w, h, px })
    }

    pub fn colormap(&self, name: &str, x: u32, y: u32) -> [f32; 3] {
        match self.texture(&format!("colormap/{}", name)) {
            Some(tex) => {
                let p = tex.get(x, y);
                [
                    p[0] as f32 / 255.0,
                    p[1] as f32 / 255.0,
                    p[2] as f32 / 255.0,
                ]
            }
            None => [1.0, 1.0, 1.0],
        }
    }

    pub fn model(&self, path: &str) -> Arc<Resolved> {
        let key = strip_namespace(path).to_string();
        if let Some(hit) = self.models.lock().unwrap().get(&key) {
            return hit.clone();
        }
        let resolved = Arc::new(self.resolve_chain(&key));
        self.models.lock().unwrap().insert(key, resolved.clone());
        resolved
    }

    fn resolve_chain(&self, path: &str) -> Resolved {
        let mut chain: Vec<Arc<Json>> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        let mut generated = false;
        let mut current = path.to_string();
        for _ in 0..16 {
            if current.starts_with("builtin/") {
                generated = current == "builtin/generated";
                break;
            }
            if seen.contains(&current) {
                break;
            }
            seen.push(current.clone());
            let Some(node) = self.json(&format!("models/{}.json", current)) else {
                break;
            };
            let parent = node
                .get("parent")
                .and_then(Json::as_str)
                .map(|p| strip_namespace(p).to_string());
            chain.push(node);
            match parent {
                Some(p) => current = p,
                None => break,
            }
        }

        let mut out = Resolved {
            generated,
            ..Resolved::default()
        };
        for node in chain.iter() {
            if let Some(Json::Bool(flag)) = node.get("ambientocclusion") {
                out.ambient_occlusion = *flag;
                break;
            }
        }

        for node in chain.iter().rev() {
            if let Some(Json::Obj(fields)) = node.get("textures") {
                for (k, v) in fields {
                    let value = v
                        .as_str()
                        .or_else(|| v.get("sprite").and_then(Json::as_str));
                    if let Some(s) = value {
                        out.textures.insert(k.clone(), s.to_string());
                    }
                }
            }
        }
        for node in chain.iter() {
            if out.elements.is_empty() {
                if let Some(elements) = node.get("elements") {
                    out.elements = parse_elements(elements);
                }
            }
            if out.gui.is_none() {
                if let Some(gui) = node.get("display").and_then(|d| d.get("gui")) {
                    out.gui = Some(Transform::parse(gui));
                }
            }
            if let Some(Json::Obj(slots)) = node.get("display") {
                for (name, value) in slots {
                    if let Some(ctx) = DisplayContext::parse(name) {
                        out.display.set_if_absent(ctx, || Transform::parse(value));
                    }
                }
            }
        }
        out
    }
}

pub(crate) fn parse_elements(node: &Json) -> Vec<Elem> {
    let mut out = Vec::new();
    for e in node.arr() {
        let Some(from) = e.get("from").and_then(Json::as_vec3) else {
            continue;
        };
        let Some(to) = e.get("to").and_then(Json::as_vec3) else {
            continue;
        };
        let rotation = e.get("rotation").and_then(|r| {
            let axis = match r.get("axis").and_then(Json::as_str)? {
                "x" => 0,
                "y" => 1,
                _ => 2,
            };
            Some(ElemRot {
                origin: r.get("origin").and_then(Json::as_vec3).unwrap_or([8.0; 3]),
                axis,
                angle: r.get("angle").and_then(Json::as_f32).unwrap_or(0.0),
                rescale: r.get("rescale").and_then(Json::as_bool).unwrap_or(false),
            })
        });
        let shade = e.get("shade").and_then(Json::as_bool).unwrap_or(true);
        let mut faces = Vec::new();
        if let Some(Json::Obj(fields)) = e.get("faces") {
            for (name, face) in fields {
                let Some(dir) = dir_by_name(name) else {
                    continue;
                };
                let Some(texture) = face.get("texture").and_then(Json::as_str) else {
                    continue;
                };
                faces.push(FaceDef {
                    dir,
                    texture: texture.to_string(),
                    uv: face.get("uv").and_then(Json::as_vec4),
                    quadrant: ((face.get("rotation").and_then(Json::as_i32).unwrap_or(0) / 90) & 3)
                        as u8,
                    tint: face.get("tintindex").and_then(Json::as_i32).unwrap_or(-1),
                    cull: face
                        .get("cullface")
                        .and_then(Json::as_str)
                        .and_then(dir_by_name),
                });
            }
        }
        out.push(Elem {
            from,
            to,
            rotation,
            shade,
            faces,
        });
    }
    out
}
