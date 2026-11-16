use std::collections::HashMap;

use naga::Handle;
use naga::common::wgsl::TypeContext as _;

use super::caps::{Class, Format};
use super::compose::Composed;
use super::manifest::Api;
use super::options::{self, Kind};
use super::problem::Problem;

pub(crate) const ENGINE_PREFIX: &str = "torch_";

trait Wgsl {
    const TYPE: &'static str;
    const ALIGN: usize;
}

impl Wgsl for f32 {
    const TYPE: &'static str = "f32";
    const ALIGN: usize = 4;
}

impl Wgsl for u32 {
    const TYPE: &'static str = "u32";
    const ALIGN: usize = 4;
}

impl Wgsl for [f32; 2] {
    const TYPE: &'static str = "vec2<f32>";
    const ALIGN: usize = 8;
}

impl Wgsl for [f32; 3] {
    const TYPE: &'static str = "vec3<f32>";
    const ALIGN: usize = 16;
}

impl Wgsl for [[f32; 4]; 4] {
    const TYPE: &'static str = "mat4x4<f32>";
    const ALIGN: usize = 16;
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Member {
    pub(crate) name: &'static str,
    pub(crate) ty: &'static str,
    pub(crate) offset: usize,
    pub(crate) doc: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameStruct {
    pub(crate) name: &'static str,
    pub(crate) doc: &'static str,
    pub(crate) members: &'static [Member],
}

impl FrameStruct {
    fn wgsl(&self) -> String {
        let mut out = String::new();
        comment(&mut out, "", self.doc);
        out.push_str(&format!("struct {} {{\n", self.name));
        for member in self.members {
            comment(&mut out, "    ", member.doc);
            out.push_str(&format!(
                "    {}: {}, // byte {}\n",
                member.name, member.ty, member.offset
            ));
        }
        out.push_str("}\n");
        out
    }
}

macro_rules! frame_struct {
    (
        $(#[doc = $sdoc:literal])*
        struct $name:ident {
            $( $(#[doc = $doc:literal])* $field:ident : $ty:ty, )+
        }
    ) => {
        $(#[doc = $sdoc])*
        #[repr(C)]
        #[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
        #[allow(dead_code, reason = "phase 2 of docs/SHADER_FORMAT.md fills the uniform; phase 1 reads only the layout")]
        pub(crate) struct $name {
            $( $(#[doc = $doc])* pub(crate) $field: $ty, )+
        }

        impl Wgsl for $name {
            const TYPE: &'static str = stringify!($name);
            const ALIGN: usize = 16;
        }

        impl $name {
            const LAYOUT: FrameStruct = FrameStruct {
                name: stringify!($name),
                doc: concat!("" $(, $sdoc, "\n")*),
                members: &[$(
                    Member {
                        name: stringify!($field),
                        ty: <$ty as Wgsl>::TYPE,
                        offset: ::core::mem::offset_of!($name, $field),
                        doc: concat!("" $(, $doc, "\n")*),
                    },
                )+],
            };
        }

        const _: () = {
            assert!(::core::mem::size_of::<$name>() == 0 $( + ::core::mem::size_of::<$ty>() )+);
            $( assert!(::core::mem::offset_of!($name, $field) % <$ty as Wgsl>::ALIGN == 0); )+
            assert!(::core::mem::size_of::<$name>() % 16 == 0);
        };
    };
}

frame_struct! {
    struct FrameCamera {
        view: [[f32; 4]; 4],
        view_inverse: [[f32; 4]; 4],
        projection: [[f32; 4]; 4],
        projection_inverse: [[f32; 4]; 4],
        previous_view: [[f32; 4]; 4],
        previous_projection: [[f32; 4]; 4],
        position: [f32; 3],
        near: f32,
        far: f32,
        _pad0: u32,
        _pad1: u32,
        _pad2: u32,
    }
}

frame_struct! {
    struct FrameTime {
        frame: u32,
        seconds: f32,
        delta: f32,
        day_ticks: f32,
        day_fraction: f32,
        _pad0: u32,
        _pad1: u32,
        _pad2: u32,
    }
}

frame_struct! {
    struct FrameSky {
        sun: [f32; 3],
        moon_phase: u32,
        moon: [f32; 3],
        fog_start: f32,
        sky_color: [f32; 3],
        fog_end: f32,
        fog_color: [f32; 3],
        _pad0: u32,
    }
}

frame_struct! {
    struct FrameWeather {
        rain: f32,
        thunder: f32,
        wetness: f32,
        temperature: f32,
        rainfall: f32,
        _pad0: u32,
        _pad1: u32,
        _pad2: u32,
    }
}

frame_struct! {
    struct FramePlayer {
        eye_medium: u32,
        blindness: f32,
        eye_light: [f32; 2],
        night_vision: f32,
        darkness: f32,
        held_light: [f32; 2],
    }
}

frame_struct! {
    struct FrameShadow {
        view: [[f32; 4]; 4],
        projection: [[f32; 4]; 4],
        resolution: f32,
        distance: f32,
        _pad0: u32,
        _pad1: u32,
    }
}

frame_struct! {
    struct FrameView {
        size: [f32; 2],
        texel: [f32; 2],
    }
}

frame_struct! {
    struct Frame {
        camera: FrameCamera,
        time: FrameTime,
        sky: FrameSky,
        weather: FrameWeather,
        player: FramePlayer,
        shadow: FrameShadow,
        view: FrameView,
    }
}

pub(crate) const FRAME_STRUCTS: [FrameStruct; 8] = [
    FrameCamera::LAYOUT,
    FrameTime::LAYOUT,
    FrameSky::LAYOUT,
    FrameWeather::LAYOUT,
    FramePlayer::LAYOUT,
    FrameShadow::LAYOUT,
    FrameView::LAYOUT,
    Frame::LAYOUT,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ty {
    F32,
    U32,
    Bool,
    Vec2F,
    Vec3F,
    Vec4F,
    Vec2U,
    Vec3U,
    Surface,
}

impl Ty {
    pub(crate) fn wgsl(self) -> &'static str {
        match self {
            Ty::F32 => "f32",
            Ty::U32 => "u32",
            Ty::Bool => "bool",
            Ty::Vec2F => "vec2<f32>",
            Ty::Vec3F => "vec3<f32>",
            Ty::Vec4F => "vec4<f32>",
            Ty::Vec2U => "vec2<u32>",
            Ty::Vec3U => "vec3<u32>",
            Ty::Surface => SURFACE.name,
        }
    }

    fn flat(self) -> bool {
        matches!(self, Ty::U32 | Ty::Vec2U | Ty::Vec3U)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MemberDef {
    pub(crate) name: &'static str,
    pub(crate) ty: Ty,
    pub(crate) doc: &'static str,
}

const fn m(name: &'static str, ty: Ty, doc: &'static str) -> MemberDef {
    MemberDef { name, ty, doc }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct StructDef {
    pub(crate) name: &'static str,
    pub(crate) since: u32,
    pub(crate) doc: &'static str,
    pub(crate) members: &'static [MemberDef],
}

impl StructDef {
    fn wgsl(&self) -> String {
        let mut out = String::new();
        comment(&mut out, "", self.doc);
        out.push_str(&format!("struct {} {{\n", self.name));
        for member in self.members {
            comment(&mut out, "    ", member.doc);
            out.push_str(&format!("    {}: {},\n", member.name, member.ty.wgsl()));
        }
        out.push_str("}\n");
        out
    }
}

fn comment(out: &mut String, indent: &str, doc: &str) {
    for line in doc.lines().map(str::trim).filter(|l| !l.is_empty()) {
        out.push_str(&format!("{indent}// {line}\n"));
    }
}

const BASE_DOC: &str =
    "What the engine's default for this stage returns: change what differs and return it.";

pub(crate) const SURFACE: StructDef = StructDef {
    name: "Surface",
    since: 0,
    doc: "What a geometry stage returns, and what `encode` writes to the group's targets.",
    members: &[
        m("color", Ty::Vec4F, "Linear rgb, and alpha."),
        m(
            "normal",
            Ty::Vec3F,
            "World space, unit length. The geometric normal until the client has normal atlases.",
        ),
        m("light", Ty::Vec2F, "Block light and sky light, 0 to 1."),
        m(
            "emission",
            Ty::F32,
            "0 to 1; 0 until the client has specular atlases.",
        ),
        m(
            "roughness",
            Ty::F32,
            "1 when unknown, which it is until the client has specular atlases.",
        ),
        m(
            "metalness",
            Ty::F32,
            "0 when unknown, which it is until the client has specular atlases.",
        ),
        m(
            "material",
            Ty::U32,
            "A `MATERIAL_*` constant, or 0 for none.",
        ),
        m(
            "custom",
            Ty::Vec4F,
            "The pack's own data, passed through to `encode` untouched.",
        ),
    ],
};

pub(crate) const FRAGMENT_INFO: StructDef = StructDef {
    name: "FragmentInfo",
    since: 0,
    doc: "Where a geometry fragment is, for `encode`.",
    members: &[
        m(
            "pixel",
            Ty::Vec2F,
            "The fragment's position in pixels from the top left, at the pixel's centre.",
        ),
        m(
            "depth",
            Ty::F32,
            "The fragment's depth, reversed: 1 at the near plane, 0 at the far.",
        ),
        m(
            "front_facing",
            Ty::Bool,
            "Whether the fragment's primitive faces the camera.",
        ),
        m(
            "position",
            Ty::Vec3F,
            "Its position relative to the camera, in world axes and blocks.",
        ),
    ],
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Input {
    Sky,
    Terrain,
    Entity,
    Cloud,
    Weather,
    Vertex,
    Screen,
    Compute,
}

static INPUTS: [StructDef; 8] = [
    StructDef {
        name: "SkyInput",
        since: 0,
        doc: "The sky stage's input.",
        members: &[
            m("base", Ty::Surface, BASE_DOC),
            m(
                "position",
                Ty::Vec3F,
                "A point of the sky relative to the camera; `normalize` it for the view direction.",
            ),
        ],
    },
    StructDef {
        name: "TerrainInput",
        since: 0,
        doc: "The terrain and water stages' input: one fragment of a block face.",
        members: &[
            m("base", Ty::Surface, BASE_DOC),
            m(
                "position",
                Ty::Vec3F,
                "Relative to the camera, in world axes and blocks.",
            ),
            m("uv", Ty::Vec2F, "The block atlas coordinate."),
            m(
                "mid_uv",
                Ty::Vec2F,
                "The centre of the face's sprite in the block atlas.",
            ),
            m(
                "color",
                Ty::Vec4F,
                "Ambient occlusion times the biome tint in rgb, the face's shade in alpha.",
            ),
            m("light", Ty::Vec2F, "Block light and sky light, 0 to 1."),
            m(
                "normal",
                Ty::Vec3F,
                "The face's normal, world space, unit length.",
            ),
            m(
                "tangent",
                Ty::Vec4F,
                "Along the texture's u, world space, with the handedness of v in w.",
            ),
            m(
                "mid_block",
                Ty::Vec3F,
                "From this point to the centre of its block, in blocks.",
            ),
        ],
    },
    StructDef {
        name: "EntityInput",
        since: 0,
        doc: "The input of the stages drawn as entities: mobs, block entities, the hand, particles.",
        members: &[
            m("base", Ty::Surface, BASE_DOC),
            m(
                "position",
                Ty::Vec3F,
                "Relative to the camera, in world axes and blocks.",
            ),
            m("uv", Ty::Vec2F, "The coordinate in the entity's texture."),
            m(
                "color",
                Ty::Vec4F,
                "The vertex colour times the draw's tint.",
            ),
            m(
                "light",
                Ty::Vec2F,
                "Block light and sky light at the entity, 0 to 1.",
            ),
            m("normal", Ty::Vec3F, "World space, unit length."),
            m(
                "overlay",
                Ty::Vec4F,
                "The hurt or TNT flash: its colour in rgb, its strength in alpha.",
            ),
        ],
    },
    StructDef {
        name: "CloudInput",
        since: 0,
        doc: "The clouds stage's input.",
        members: &[
            m("base", Ty::Surface, BASE_DOC),
            m(
                "position",
                Ty::Vec3F,
                "Relative to the camera, in world axes and blocks.",
            ),
            m("normal", Ty::Vec3F, "The cloud face's normal, world space."),
            m("color", Ty::Vec4F, "The cloud's colour and alpha."),
        ],
    },
    StructDef {
        name: "WeatherInput",
        since: 0,
        doc: "The weather stage's input: rain and snow.",
        members: &[
            m("base", Ty::Surface, BASE_DOC),
            m(
                "position",
                Ty::Vec3F,
                "Relative to the camera, in world axes and blocks.",
            ),
            m(
                "uv",
                Ty::Vec2F,
                "The coordinate in the rain or snow texture.",
            ),
            m(
                "color",
                Ty::Vec4F,
                "The light at the column times its fade.",
            ),
            m(
                "light",
                Ty::Vec2F,
                "Block light and sky light at the column, 0 to 1.",
            ),
            m("snow", Ty::U32, "1 for snow, 0 for rain."),
        ],
    },
    StructDef {
        name: "VertexInput",
        since: 0,
        doc: "The input of `displace`: one terrain vertex, before it moves.",
        members: &[
            m(
                "position",
                Ty::Vec3F,
                "Relative to the camera, in world axes and blocks.",
            ),
            m("uv", Ty::Vec2F, "The block atlas coordinate."),
            m(
                "mid_uv",
                Ty::Vec2F,
                "The centre of the face's sprite in the block atlas.",
            ),
            m(
                "normal",
                Ty::Vec3F,
                "The face's normal, world space, unit length.",
            ),
            m("light", Ty::Vec2F, "Block light and sky light, 0 to 1."),
            m(
                "material",
                Ty::U32,
                "The block's `MATERIAL_*` constant, or 0.",
            ),
            m(
                "mid_block",
                Ty::Vec3F,
                "From this vertex to the centre of its block, in blocks.",
            ),
        ],
    },
    StructDef {
        name: "ScreenInput",
        since: 0,
        doc: "A screen pass's input: one pixel of its output.",
        members: &[
            m(
                "uv",
                Ty::Vec2F,
                "0 to 1 across the output, 0 at the top left.",
            ),
            m("pixel", Ty::Vec2U, "The pixel being written."),
            m("size", Ty::Vec2F, "The output's size in pixels."),
        ],
    },
    StructDef {
        name: "ComputeInput",
        since: 0,
        doc: "A compute pass's input: one invocation.",
        members: &[
            m(
                "global_id",
                Ty::Vec3U,
                "The invocation's id across the whole dispatch.",
            ),
            m(
                "local_id",
                Ty::Vec3U,
                "The invocation's id within its workgroup.",
            ),
        ],
    },
];

impl Input {
    pub(crate) const ALL: [Input; 8] = [
        Input::Sky,
        Input::Terrain,
        Input::Entity,
        Input::Cloud,
        Input::Weather,
        Input::Vertex,
        Input::Screen,
        Input::Compute,
    ];

    pub(crate) fn def(self) -> &'static StructDef {
        &INPUTS[self as usize]
    }

    pub(crate) fn name(self) -> &'static str {
        self.def().name
    }

    fn stem(self) -> &'static str {
        match self {
            Input::Sky => "sky",
            Input::Terrain => "terrain",
            Input::Entity => "entity",
            Input::Cloud => "cloud",
            Input::Weather => "weather",
            Input::Vertex => "vertex",
            Input::Screen => "screen",
            Input::Compute => "compute",
        }
    }
}

fn is_engine_struct(name: &str) -> bool {
    name == FRAGMENT_INFO.name
        || Input::ALL.iter().any(|input| input.name() == name)
        || FRAME_STRUCTS.iter().any(|s| s.name == name)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Group {
    Opaque,
    Translucent,
}

impl Group {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Group::Opaque => "opaque",
            Group::Translucent => "translucent",
        }
    }

    pub(crate) fn stages(self) -> impl Iterator<Item = Stage> {
        Stage::ALL
            .into_iter()
            .filter(move |stage| stage.info().group == self)
    }

    fn output_struct(self) -> &'static str {
        match self {
            Group::Opaque => "torch_opaque_out",
            Group::Translucent => "torch_translucent_out",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Stage {
    Sky,
    Terrain,
    Entity,
    BlockEntity,
    Hand,
    Water,
    Particle,
    Clouds,
    Weather,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct StageInfo {
    pub(crate) function: &'static str,
    pub(crate) input: Input,
    pub(crate) group: Group,
}

static STAGES: [StageInfo; 9] = [
    StageInfo {
        function: "sky",
        input: Input::Sky,
        group: Group::Opaque,
    },
    StageInfo {
        function: "terrain",
        input: Input::Terrain,
        group: Group::Opaque,
    },
    StageInfo {
        function: "entity",
        input: Input::Entity,
        group: Group::Opaque,
    },
    StageInfo {
        function: "block_entity",
        input: Input::Entity,
        group: Group::Opaque,
    },
    StageInfo {
        function: "hand",
        input: Input::Entity,
        group: Group::Opaque,
    },
    StageInfo {
        function: "water",
        input: Input::Terrain,
        group: Group::Translucent,
    },
    StageInfo {
        function: "particle",
        input: Input::Entity,
        group: Group::Translucent,
    },
    StageInfo {
        function: "clouds",
        input: Input::Cloud,
        group: Group::Translucent,
    },
    StageInfo {
        function: "weather",
        input: Input::Weather,
        group: Group::Translucent,
    },
];

impl Stage {
    pub(crate) const ALL: [Stage; 9] = [
        Stage::Sky,
        Stage::Terrain,
        Stage::Entity,
        Stage::BlockEntity,
        Stage::Hand,
        Stage::Water,
        Stage::Particle,
        Stage::Clouds,
        Stage::Weather,
    ];

    pub(crate) fn info(self) -> &'static StageInfo {
        &STAGES[self as usize]
    }
}

pub(crate) const ENCODE: &str = "encode";

pub(crate) const DISPLACE: &str = "displace";

pub(crate) const SHADOW: &str = "shadow";

const SHADOW_TARGET: &str = "shadow_color";

fn texel_wgsl(class: Class) -> &'static str {
    match class {
        Class::Float | Class::Depth => "f32",
        Class::Uint => "u32",
    }
}

fn texel_scalar(class: Class) -> naga::Scalar {
    match class {
        Class::Float | Class::Depth => naga::Scalar::F32,
        Class::Uint => naga::Scalar::U32,
    }
}

fn sample_kind(class: Class) -> SampleKind {
    match class {
        Class::Float => SampleKind::Float,
        Class::Uint => SampleKind::Uint,
        Class::Depth => SampleKind::Depth,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PreludeTarget {
    pub(crate) id: String,
    format: Format,
    pub(crate) history: bool,
}

impl PreludeTarget {
    pub(crate) fn new(id: String, format: Format, history: bool) -> Option<PreludeTarget> {
        (format.info().class != Class::Depth).then_some(PreludeTarget {
            id,
            format,
            history,
        })
    }

    pub(crate) fn class(&self) -> Class {
        self.format.info().class
    }
}

fn option_scalar(kind: &Kind) -> naga::Scalar {
    match kind {
        Kind::Bool => naga::Scalar::BOOL,
        Kind::Int { .. } => naga::Scalar::I32,
        Kind::Float { .. } => naga::Scalar::F32,
        Kind::Choice { .. } => naga::Scalar::U32,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PreludeOption {
    pub(crate) id: String,
    pub(crate) kind: Kind,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PackInterface {
    pub(crate) api: Api,
    pub(crate) targets: Vec<PreludeTarget>,
    pub(crate) textures: Vec<String>,
    pub(crate) options: Vec<PreludeOption>,
    pub(crate) materials: Vec<String>,
    pub(crate) shadow_color: bool,
    pub(crate) post_only: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ScreenOutput {
    Screen,
    Targets(Vec<usize>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum OutputSize {
    Scale(f32),
    Fixed([u32; 2]),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ModuleKind {
    Geometry {
        group: Group,
        writes: Vec<usize>,
        stages: Vec<Stage>,
        vertex: bool,
    },
    Shadow,
    Screen {
        function: String,
        output: ScreenOutput,
        size: OutputSize,
    },
    Compute {
        function: String,
        writes: Vec<usize>,
        workgroup: [u32; 3],
    },
}

impl ModuleKind {
    pub(crate) fn function(&self) -> Option<&str> {
        match self {
            ModuleKind::Screen { function, .. } | ModuleKind::Compute { function, .. } => {
                Some(function)
            }
            ModuleKind::Geometry { .. } | ModuleKind::Shadow => None,
        }
    }

    fn storage_writes(&self) -> &[usize] {
        match self {
            ModuleKind::Compute { writes, .. } => writes,
            ModuleKind::Screen { .. } | ModuleKind::Geometry { .. } | ModuleKind::Shadow => &[],
        }
    }

    pub(crate) fn called(&self) -> Vec<&str> {
        match self {
            ModuleKind::Geometry { stages, vertex, .. } => {
                let mut out: Vec<&str> = stages.iter().map(|stage| stage.info().function).collect();
                out.push(ENCODE);
                if *vertex {
                    out.push(DISPLACE);
                }
                out
            }
            ModuleKind::Shadow => vec![SHADOW],
            ModuleKind::Screen { function, .. } | ModuleKind::Compute { function, .. } => {
                vec![function.as_str()]
            }
        }
    }
}

pub(crate) fn is_identifier(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && name != "_"
        && !name.starts_with("__")
        && !name.starts_with(ENGINE_PREFIX)
        && !naga::keywords::wgsl::RESERVED.contains(&name)
}

pub(crate) const GLOBALS_GROUP: u32 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Resource {
    Frame,
    LinearClamp,
    LinearRepeat,
    NearestClamp,
    ShadowCompare,
    SceneDepth,
    ShadowDepth,
    ShadowColor,
    BlockAtlas,
    Lightmap,
    Noise,
    SceneColor,
    Target(usize),
    TargetPrevious(usize),
    Texture(usize),
    TargetOut(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BindingKind {
    Uniform,
    Sampler,
    ComparisonSampler,
    Texture(SampleKind),
    StorageTexture,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SampleKind {
    Float,
    Uint,
    Depth,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Binding {
    pub(crate) name: String,
    pub(crate) binding: u32,
    pub(crate) resource: Resource,
    pub(crate) kind: BindingKind,
    ty: String,
}

struct Fixed {
    name: &'static str,
    resource: Resource,
    since: u32,
}

const FIXED: [Fixed; 12] = [
    Fixed {
        name: "frame",
        resource: Resource::Frame,
        since: 0,
    },
    Fixed {
        name: "linear_clamp",
        resource: Resource::LinearClamp,
        since: 0,
    },
    Fixed {
        name: "linear_repeat",
        resource: Resource::LinearRepeat,
        since: 0,
    },
    Fixed {
        name: "nearest_clamp",
        resource: Resource::NearestClamp,
        since: 0,
    },
    Fixed {
        name: "shadow_compare",
        resource: Resource::ShadowCompare,
        since: 0,
    },
    Fixed {
        name: "scene_depth",
        resource: Resource::SceneDepth,
        since: 0,
    },
    Fixed {
        name: "shadow_depth",
        resource: Resource::ShadowDepth,
        since: 0,
    },
    Fixed {
        name: "shadow_color",
        resource: Resource::ShadowColor,
        since: 0,
    },
    Fixed {
        name: "block_atlas",
        resource: Resource::BlockAtlas,
        since: 0,
    },
    Fixed {
        name: "lightmap",
        resource: Resource::Lightmap,
        since: 0,
    },
    Fixed {
        name: "noise",
        resource: Resource::Noise,
        since: 0,
    },
    Fixed {
        name: "scene_color",
        resource: Resource::SceneColor,
        since: 0,
    },
];

pub(crate) fn bindings(pack: &PackInterface, kind: &ModuleKind) -> Vec<Binding> {
    bindings_writing(pack, kind.storage_writes())
}

fn bindings_writing(pack: &PackInterface, outs: &[usize]) -> Vec<Binding> {
    let mut list = Vec::new();
    for (number, fixed) in FIXED.iter().enumerate() {
        let present = fixed.since <= pack.api.minor
            && match fixed.resource {
                Resource::ShadowColor => pack.shadow_color,
                Resource::SceneColor => pack.post_only,
                _ => true,
            };
        if present {
            list.push(binding(
                pack,
                fixed.name.to_owned(),
                number as u32,
                fixed.resource,
            ));
        }
    }
    let mut next = FIXED.len() as u32;
    let mut add = |name: String, resource: Resource| {
        list.push(binding(pack, name, next, resource));
        next += 1;
    };
    for (index, target) in pack.targets.iter().enumerate() {
        add(target.id.clone(), Resource::Target(index));
    }
    for (index, target) in pack.targets.iter().enumerate().filter(|(_, t)| t.history) {
        add(
            format!("{}_prev", target.id),
            Resource::TargetPrevious(index),
        );
    }
    for (index, texture) in pack.textures.iter().enumerate() {
        add(texture.clone(), Resource::Texture(index));
    }
    for &index in outs {
        add(
            format!("{}_out", pack.targets[index].id),
            Resource::TargetOut(index),
        );
    }
    list
}

fn binding(pack: &PackInterface, name: String, number: u32, resource: Resource) -> Binding {
    let float = || {
        (
            BindingKind::Texture(SampleKind::Float),
            "texture_2d<f32>".to_owned(),
        )
    };
    let (kind, ty) = match resource {
        Resource::Frame => (BindingKind::Uniform, Frame::LAYOUT.name.to_owned()),
        Resource::LinearClamp | Resource::LinearRepeat | Resource::NearestClamp => {
            (BindingKind::Sampler, "sampler".to_owned())
        }
        Resource::ShadowCompare => (
            BindingKind::ComparisonSampler,
            "sampler_comparison".to_owned(),
        ),
        Resource::SceneDepth | Resource::ShadowDepth => (
            BindingKind::Texture(SampleKind::Depth),
            "texture_depth_2d".to_owned(),
        ),
        Resource::ShadowColor
        | Resource::BlockAtlas
        | Resource::Lightmap
        | Resource::Noise
        | Resource::SceneColor
        | Resource::Texture(_) => float(),
        Resource::Target(index) | Resource::TargetPrevious(index) => {
            let class = pack.targets[index].class();
            (
                BindingKind::Texture(sample_kind(class)),
                format!("texture_2d<{}>", texel_wgsl(class)),
            )
        }
        Resource::TargetOut(index) => (
            BindingKind::StorageTexture,
            format!(
                "texture_storage_2d<{}, write>",
                pack.targets[index].format.name()
            ),
        ),
    };
    Binding {
        name,
        binding: number,
        resource,
        kind,
        ty,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Owner {
    Target(usize),
    Texture(usize),
    Option(usize),
    Material(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum What {
    Engine,
    Target(usize),
    TargetPrevious(usize),
    TargetOut(usize),
    Texture(usize),
    Option(usize),
    Material(usize),
    Choice(usize),
}

impl What {
    fn of(resource: Resource) -> What {
        match resource {
            Resource::Target(index) => What::Target(index),
            Resource::TargetPrevious(index) => What::TargetPrevious(index),
            Resource::TargetOut(index) => What::TargetOut(index),
            Resource::Texture(index) => What::Texture(index),
            _ => What::Engine,
        }
    }

    fn describe(self) -> &'static str {
        match self {
            What::Engine => "a name the engine declares",
            What::Target(_) => "a target",
            What::TargetPrevious(_) => "a target's history (`<id>_prev`)",
            What::TargetOut(_) => "a compute output (`<id>_out`)",
            What::Texture(_) => "a texture",
            What::Option(_) => "an option",
            What::Material(_) => "a material constant (`MATERIAL_<ID>`)",
            What::Choice(_) => "a choice constant (`<ID>_<VALUE>`)",
        }
    }

    fn owner(self) -> Option<Owner> {
        match self {
            What::Engine => None,
            What::Target(index) | What::TargetPrevious(index) | What::TargetOut(index) => {
                Some(Owner::Target(index))
            }
            What::Texture(index) => Some(Owner::Texture(index)),
            What::Option(index) | What::Choice(index) => Some(Owner::Option(index)),
            What::Material(index) => Some(Owner::Material(index)),
        }
    }
}

impl Owner {
    fn id(self, pack: &PackInterface) -> &str {
        match self {
            Owner::Target(index) => &pack.targets[index].id,
            Owner::Texture(index) => &pack.textures[index],
            Owner::Option(index) => &pack.options[index].id,
            Owner::Material(index) => &pack.materials[index],
        }
    }
}

struct Decl {
    name: String,
    since: u32,
    what: What,
    text: String,
}

fn declarations(pack: &PackInterface, bindings: &[Binding]) -> Vec<Decl> {
    let mut decls = Vec::new();
    let engine_structs = [&SURFACE, &FRAGMENT_INFO].into_iter().chain(INPUTS.iter());
    for def in engine_structs {
        decls.push(Decl {
            name: def.name.to_owned(),
            since: def.since,
            what: What::Engine,
            text: def.wgsl(),
        });
    }
    for s in &FRAME_STRUCTS {
        decls.push(Decl {
            name: s.name.to_owned(),
            since: 0,
            what: What::Engine,
            text: s.wgsl(),
        });
    }
    for b in bindings {
        let space = if b.kind == BindingKind::Uniform {
            "<uniform>"
        } else {
            ""
        };
        decls.push(Decl {
            name: b.name.clone(),
            since: 0,
            what: What::of(b.resource),
            text: format!(
                "@group({GLOBALS_GROUP}) @binding({}) var{space} {}: {};\n",
                b.binding, b.name, b.ty
            ),
        });
    }
    for (index, id) in pack.materials.iter().enumerate() {
        let name = format!("MATERIAL_{}", id.to_ascii_uppercase());
        let text = format!("const {name}: u32 = {}u;\n", index + 1);
        decls.push(Decl {
            name,
            since: 0,
            what: What::Material(index),
            text,
        });
    }
    for (option, decl) in pack.options.iter().enumerate() {
        let Kind::Choice { values } = &decl.kind else {
            continue;
        };
        for (index, value) in values.iter().enumerate() {
            let name = options::constant_name(&decl.id, value);
            let text = format!("const {name}: u32 = {index}u;\n");
            decls.push(Decl {
                name,
                since: 0,
                what: What::Choice(option),
                text,
            });
        }
    }
    decls
}

fn emit(decls: &[Decl], api: Api) -> String {
    let mut out = String::new();
    for decl in decls.iter().filter(|d| d.since <= api.minor) {
        out.push_str(&decl.text);
        out.push('\n');
    }
    out
}

pub(crate) fn prelude(pack: &PackInterface, bindings: &[Binding]) -> String {
    emit(&declarations(pack, bindings), pack.api)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NameProblem {
    pub(crate) owner: Option<Owner>,
    pub(crate) message: String,
}

pub(crate) fn check_interface(pack: &PackInterface) -> Vec<NameProblem> {
    let mut names: Vec<(String, What)> = FIXED
        .iter()
        .map(|fixed| (fixed.name.to_owned(), What::Engine))
        .collect();
    names.push((SCREEN_TARGET.to_owned(), What::Engine));
    let every_target: Vec<usize> = (0..pack.targets.len()).collect();
    let bindings = bindings_writing(pack, &every_target);
    names.extend(
        declarations(pack, &bindings)
            .into_iter()
            .filter(|d| d.since <= pack.api.minor)
            .map(|d| (d.name, d.what)),
    );
    names.extend(
        pack.options
            .iter()
            .enumerate()
            .map(|(index, o)| (o.id.clone(), What::Option(index))),
    );

    let mut problems = Vec::new();
    let mut seen: HashMap<&str, What> = HashMap::new();
    for (name, what) in &names {
        if matches!(what, What::Target(_) | What::Texture(_) | What::Option(_))
            && !is_identifier(name)
        {
            problems.push(NameProblem {
                owner: what.owner(),
                message: format!(
                    "`{name}`, {}, cannot be a WGSL name: it is a reserved word or starts with `{ENGINE_PREFIX}`",
                    what.describe()
                ),
            });
        }
        let Some(first) = seen.get(name.as_str()).copied() else {
            seen.insert(name.as_str(), *what);
            continue;
        };
        let repeated_id = match (first.owner(), what.owner()) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                std::mem::discriminant(&first) == std::mem::discriminant(what)
                    && a.id(pack) == b.id(pack)
            }
            _ => false,
        };
        if !repeated_id {
            problems.push(NameProblem {
                owner: what.owner().or(first.owner()),
                message: format!(
                    "`{name}` is both {} and {}",
                    first.describe(),
                    what.describe()
                ),
            });
        }
    }
    problems
}

pub(crate) const SCREEN_VERTEX: &str = "torch_screen_vertex";
pub(crate) const SCREEN_FRAGMENT: &str = "torch_screen_fragment";
pub(crate) const COMPUTE_ENTRY: &str = "torch_compute";

const SCREEN_WGSL: &str = include_str!("wgsl/screen.wgsl");

const SCREEN_OUT: &str = "torch_screen_out";

const SCREEN_TARGET: &str = "screen";

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Entries {
    pub(crate) text: String,
    pub(crate) names: Vec<String>,
}

pub(crate) fn fragment_entry(stage: Stage) -> String {
    format!("{ENGINE_PREFIX}{}_fragment", stage.info().function)
}

pub(crate) fn vertex_entry(input: Input) -> String {
    format!("{ENGINE_PREFIX}{}_vertex", input.stem())
}

pub(crate) fn entries(pack: &PackInterface, kind: &ModuleKind) -> Entries {
    entries_with(pack, kind, geometry_seam)
}

fn geometry_seam(input: Input) -> Option<&'static str> {
    match input {
        Input::Sky | Input::Terrain | Input::Entity | Input::Cloud | Input::Weather => None,
        Input::Vertex | Input::Screen | Input::Compute => None,
    }
}

fn entries_with(
    pack: &PackInterface,
    kind: &ModuleKind,
    seam: fn(Input) -> Option<&'static str>,
) -> Entries {
    match kind {
        ModuleKind::Geometry {
            group,
            writes,
            stages,
            ..
        } => geometry_entries(&outputs(pack, writes), *group, stages, seam),
        ModuleKind::Shadow => shadow_entries(seam),
        ModuleKind::Screen {
            function,
            output,
            size,
        } => screen_entries(&screen_outputs(pack, output), function, *size),
        ModuleKind::Compute {
            function,
            workgroup,
            ..
        } => compute_entries(function, *workgroup),
    }
}

fn outputs<'p>(pack: &'p PackInterface, writes: &[usize]) -> Vec<(&'p str, Class)> {
    writes
        .iter()
        .map(|&index| (pack.targets[index].id.as_str(), pack.targets[index].class()))
        .collect()
}

fn shadow_outputs() -> Vec<(&'static str, Class)> {
    vec![(SHADOW_TARGET, Class::Float)]
}

fn screen_outputs<'p>(pack: &'p PackInterface, output: &ScreenOutput) -> Vec<(&'p str, Class)> {
    match output {
        ScreenOutput::Screen => vec![(SCREEN_TARGET, Class::Float)],
        ScreenOutput::Targets(writes) => outputs(pack, writes),
    }
}

fn output_struct(name: &str, outputs: &[(&str, Class)]) -> String {
    let mut text = format!("struct {name} {{\n");
    for (location, (member, class)) in outputs.iter().enumerate() {
        text.push_str(&format!(
            "    @location({location}) {member}: vec4<{}>,\n",
            texel_wgsl(*class)
        ));
    }
    text.push_str("}\n\n");
    text
}

fn output_value(name: &str, value: &str, outputs: &[(&str, Class)]) -> String {
    if let [_] = outputs {
        return format!("{name}({value})");
    }
    let members: Vec<String> = outputs
        .iter()
        .map(|(member, _)| format!("{value}.{member}"))
        .collect();
    format!("{name}({})", members.join(", "))
}

fn screen_entries(outputs: &[(&str, Class)], function: &str, size: OutputSize) -> Entries {
    if outputs.is_empty() {
        return Entries::default();
    }
    let size = match size {
        OutputSize::Scale(scale) if scale == 1.0 => "frame.view.size".to_owned(),
        OutputSize::Scale(scale) => {
            format!("max(floor(frame.view.size * {scale:?}), vec2<f32>(1.0))")
        }
        OutputSize::Fixed([width, height]) => format!("vec2<f32>({width}.0, {height}.0)"),
    };
    let mut text = String::from(SCREEN_WGSL);
    text.push('\n');
    text.push_str(&output_struct(SCREEN_OUT, outputs));
    text.push_str(&format!(
        "@fragment\nfn {SCREEN_FRAGMENT}(torch_v: torch_screen_varyings) -> {SCREEN_OUT} {{\n    \
         let torch_size = {size};\n    \
         let torch_r = {function}(ScreenInput(torch_v.uv, vec2<u32>(torch_v.clip.xy), torch_size));\n    \
         return {};\n}}\n",
        output_value(SCREEN_OUT, "torch_r", outputs),
    ));
    Entries {
        text,
        names: vec![SCREEN_VERTEX.to_owned(), SCREEN_FRAGMENT.to_owned()],
    }
}

fn compute_entries(function: &str, [x, y, z]: [u32; 3]) -> Entries {
    let text = format!(
        "@compute @workgroup_size({x}, {y}, {z})\n\
         fn {COMPUTE_ENTRY}(@builtin(global_invocation_id) torch_global: vec3<u32>, \
         @builtin(local_invocation_id) torch_local: vec3<u32>) {{\n    \
         {function}(ComputeInput(torch_global, torch_local));\n}}\n"
    );
    Entries {
        text,
        names: vec![COMPUTE_ENTRY.to_owned()],
    }
}

fn varyings_name(input: Input) -> String {
    format!("{ENGINE_PREFIX}{}_varyings", input.stem())
}

fn varyings_struct(input: Input) -> String {
    let mut text = format!(
        "struct {} {{\n    @builtin(position) clip: vec4<f32>,\n",
        varyings_name(input)
    );
    let carried = input
        .def()
        .members
        .iter()
        .filter(|member| member.ty != Ty::Surface);
    for (location, member) in carried.enumerate() {
        let flat = if member.ty.flat() {
            " @interpolate(flat)"
        } else {
            ""
        };
        text.push_str(&format!(
            "    @location({location}){flat} {}: {},\n",
            member.name,
            member.ty.wgsl()
        ));
    }
    text.push_str("}\n\n");
    text
}

fn input_value(input: Input) -> String {
    let stem = input.stem();
    let members: Vec<String> = input
        .def()
        .members
        .iter()
        .map(|member| {
            if member.ty == Ty::Surface {
                format!("{ENGINE_PREFIX}{stem}_base(torch_v)")
            } else {
                format!("torch_v.{}", member.name)
            }
        })
        .collect();
    format!("{}({})", input.name(), members.join(", "))
}

fn geometry_entries(
    outputs: &[(&str, Class)],
    group: Group,
    defined: &[Stage],
    seam: fn(Input) -> Option<&'static str>,
) -> Entries {
    let stages: Vec<Stage> = group.stages().collect();
    let mut inputs: Vec<Input> = Vec::new();
    for stage in &stages {
        if !inputs.contains(&stage.info().input) {
            inputs.push(stage.info().input);
        }
    }
    let Some(seams) = inputs
        .iter()
        .map(|&input| seam(input))
        .collect::<Option<Vec<&str>>>()
    else {
        return Entries::default();
    };
    if outputs.is_empty() {
        return Entries::default();
    }
    let out = group.output_struct();
    let mut text = output_struct(out, outputs);
    let mut names = Vec::new();
    for (&input, seam_text) in inputs.iter().zip(seams) {
        text.push_str(&varyings_struct(input));
        text.push_str(seam_text);
        text.push('\n');
        names.push(vertex_entry(input));
    }
    for stage in stages {
        let info = stage.info();
        let surface = if defined.contains(&stage) {
            format!("{}({})", info.function, input_value(info.input))
        } else {
            format!("{ENGINE_PREFIX}{}_base(torch_v)", info.input.stem())
        };
        let name = fragment_entry(stage);
        text.push_str(&format!(
            "@fragment\nfn {name}(torch_v: {varyings}, @builtin(front_facing) torch_facing: bool) -> {out} {{\n    \
             let torch_s = {surface};\n    \
             let torch_e = {ENCODE}(torch_s, FragmentInfo(torch_v.clip.xy, torch_v.clip.z, torch_facing, torch_v.position));\n    \
             return {};\n}}\n\n",
            output_value(out, "torch_e", outputs),
            varyings = varyings_name(info.input),
        ));
        names.push(name);
    }
    Entries { text, names }
}

pub(crate) const SHADOW_FRAGMENT: &str = "torch_shadow_fragment";

fn shadow_entries(seam: fn(Input) -> Option<&'static str>) -> Entries {
    let input = Input::Terrain;
    let Some(seam_text) = seam(input) else {
        return Entries::default();
    };
    let out = "torch_shadow_out";
    let outputs = shadow_outputs();
    let mut text = output_struct(out, &outputs);
    text.push_str(&varyings_struct(input));
    text.push_str(seam_text);
    text.push('\n');
    text.push_str(&format!(
        "@fragment\nfn {SHADOW_FRAGMENT}(torch_v: {}) -> {out} {{\n    \
         let torch_c = {SHADOW}({});\n    \
         return {};\n}}\n",
        varyings_name(input),
        input_value(input),
        output_value(out, "torch_c", &outputs),
    ));
    Entries {
        text,
        names: vec![vertex_entry(input), SHADOW_FRAGMENT.to_owned()],
    }
}

pub(crate) fn check_rules(module: &naga::Module, composed: &Composed) -> Vec<Problem> {
    let mut problems = Vec::new();
    for entry in &module.entry_points {
        if composed
            .entry_points()
            .iter()
            .any(|name| *name == entry.name)
        {
            continue;
        }
        problems.push(
            composed
                .span_problem(
                    first_span(&entry.function),
                    format!("`{}` is an entry point, and a pack declares none", entry.name),
                )
                .hint("remove its @vertex, @fragment or @compute: the engine's entry points call the pack's functions"),
        );
    }
    for (handle, global) in module.global_variables.iter() {
        let span = module.global_variables.get_span(handle);
        if global.binding.is_some() && composed.in_pack(span) {
            problems.push(
                composed
                    .span_problem(
                        span,
                        format!(
                            "`{}` has a @group or @binding, and the engine binds every resource",
                            name_of(&global.name)
                        ),
                    )
                    .hint("read the global the prelude declares instead"),
            );
        }
    }
    reserved_names(module, composed, &mut problems);
    constructions(module, composed, &mut problems);
    problems
}

fn name_of(name: &Option<String>) -> &str {
    name.as_deref().unwrap_or("(unnamed)")
}

fn first_span(function: &naga::Function) -> naga::Span {
    function
        .body
        .span_iter()
        .map(|(_, span)| *span)
        .chain(
            function
                .expressions
                .iter()
                .map(|(handle, _)| function.expressions.get_span(handle)),
        )
        .find(naga::Span::is_defined)
        .unwrap_or_default()
}

fn reserved_names(module: &naga::Module, composed: &Composed, problems: &mut Vec<Problem>) {
    let mut check = |name: &Option<String>, span: naga::Span| {
        if let Some(name) = name
            && name.starts_with(ENGINE_PREFIX)
            && composed.in_pack(span)
        {
            problems.push(composed.span_problem(
                span,
                format!("`{name}` starts with `{ENGINE_PREFIX}`, which only the engine's own declarations use"),
            ));
        }
    };
    for (handle, ty) in module.types.iter() {
        check(&ty.name, module.types.get_span(handle));
    }
    for (handle, constant) in module.constants.iter() {
        check(&constant.name, module.constants.get_span(handle));
    }
    for (handle, value) in module.overrides.iter() {
        check(&value.name, module.overrides.get_span(handle));
    }
    for (handle, global) in module.global_variables.iter() {
        check(&global.name, module.global_variables.get_span(handle));
    }
    let functions = module
        .functions
        .iter()
        .map(|(handle, f)| (Some(module.functions.get_span(handle)), f));
    let entry_points = module.entry_points.iter().map(|e| (None, &e.function));
    for (span, function) in functions.chain(entry_points) {
        if let Some(span) = span {
            check(&function.name, span);
        }
        for (handle, local) in function.local_variables.iter() {
            check(&local.name, function.local_variables.get_span(handle));
        }
    }
}

fn constructions(module: &naga::Module, composed: &Composed, problems: &mut Vec<Problem>) {
    let engine: Vec<Handle<naga::Type>> = module
        .types
        .iter()
        .filter(|(_, ty)| matches!(ty.inner, naga::TypeInner::Struct { .. }))
        .filter(|(_, ty)| ty.name.as_deref().is_some_and(is_engine_struct))
        .map(|(handle, _)| handle)
        .collect();
    let mut check = |expressions: &naga::Arena<naga::Expression>| {
        for (handle, expression) in expressions.iter() {
            let ty = match *expression {
                naga::Expression::Compose { ty, .. } | naga::Expression::ZeroValue(ty) => ty,
                _ => continue,
            };
            let span = expressions.get_span(handle);
            if engine.contains(&ty) && composed.in_pack(span) {
                problems.push(
                    composed
                        .span_problem(span, format!("pack code constructs `{}`, which only the engine builds", name_of(&module.types[ty].name)))
                        .hint("copy one the engine handed in and change it; constructing it would break when an API version adds members"),
                );
            }
        }
    };
    check(&module.global_expressions);
    for (_, function) in module.functions.iter() {
        check(&function.expressions);
    }
    for entry in &module.entry_points {
        check(&entry.function.expressions);
    }
}

enum Returns<'p> {
    Surface,
    Vec3F,
    Nothing,
    Outputs(Vec<(&'p str, Class)>),
}

struct Expected<'p> {
    function: &'p str,
    why: String,
    args: Vec<&'static str>,
    returns: Returns<'p>,
}

pub(crate) fn check_signatures(
    module: &naga::Module,
    composed: &Composed,
    pack: &PackInterface,
    kind: &ModuleKind,
) -> Vec<Problem> {
    let mut expected = Vec::new();
    match kind {
        ModuleKind::Geometry {
            group,
            writes,
            stages,
            vertex,
        } => {
            for stage in stages {
                let info = stage.info();
                expected.push(Expected {
                    function: info.function,
                    why: format!("the {} group's `stages` lists it", group.name()),
                    args: vec![info.input.name()],
                    returns: Returns::Surface,
                });
            }
            expected.push(Expected {
                function: ENCODE,
                why: format!("the {} group writes its targets through it", group.name()),
                args: vec![SURFACE.name, FRAGMENT_INFO.name],
                returns: Returns::Outputs(outputs(pack, writes)),
            });
            if *vertex {
                expected.push(Expected {
                    function: DISPLACE,
                    why: format!("the {} group sets `vertex = true`", group.name()),
                    args: vec![Input::Vertex.name()],
                    returns: Returns::Vec3F,
                });
            }
        }
        ModuleKind::Shadow => expected.push(Expected {
            function: SHADOW,
            why: "[shadow] sets `color = true`".to_owned(),
            args: vec![Input::Terrain.name()],
            returns: Returns::Outputs(shadow_outputs()),
        }),
        ModuleKind::Screen {
            function, output, ..
        } => expected.push(Expected {
            function: function.as_str(),
            why: "the pass of that id calls it".to_owned(),
            args: vec![Input::Screen.name()],
            returns: Returns::Outputs(screen_outputs(pack, output)),
        }),
        ModuleKind::Compute { function, .. } => expected.push(Expected {
            function: function.as_str(),
            why: "the pass of that id calls it".to_owned(),
            args: vec![Input::Compute.name()],
            returns: Returns::Nothing,
        }),
    }
    let mut problems = Vec::new();
    for expected in &expected {
        check_signature(module, composed, expected, &mut problems);
    }
    problems
}

fn check_signature(
    module: &naga::Module,
    composed: &Composed,
    expected: &Expected<'_>,
    problems: &mut Vec<Problem>,
) {
    let name = expected.function;
    let found = module
        .functions
        .iter()
        .find(|(_, f)| f.name.as_deref() == Some(name));
    let Some((handle, function)) = found else {
        problems.push(Problem::file(
            composed.file(),
            format!("no `fn {name}` is defined, and {}", expected.why),
        ));
        return;
    };
    let span = module.functions.get_span(handle);
    let ctx = module.to_ctx();

    let args_match = function.arguments.len() == expected.args.len()
        && function
            .arguments
            .iter()
            .zip(&expected.args)
            .all(|(argument, want)| module.types[argument.ty].name.as_deref() == Some(*want));
    if !args_match {
        let takes: Vec<String> = function
            .arguments
            .iter()
            .map(|a| ctx.type_to_string(a.ty))
            .collect();
        problems.push(composed.span_problem(
            span,
            format!(
                "`fn {name}` must take ({}); it takes ({})",
                expected.args.join(", "),
                takes.join(", ")
            ),
        ));
    }

    let result = function.result.as_ref().map(|r| r.ty);
    let wrong = |want: &str| {
        format!(
            "`fn {name}` must return {want}; it returns {}",
            describe(module, result)
        )
    };
    match &expected.returns {
        Returns::Surface => {
            if result.map(|ty| module.types[ty].name.as_deref()) != Some(Some(SURFACE.name)) {
                problems.push(composed.span_problem(span, wrong(&format!("`{}`", SURFACE.name))));
            }
        }
        Returns::Vec3F => {
            if !result
                .is_some_and(|ty| is_vector(module, ty, naga::VectorSize::Tri, naga::Scalar::F32))
            {
                problems.push(composed.span_problem(span, wrong("`vec3<f32>`")));
            }
        }
        Returns::Nothing => {
            if result.is_some() {
                problems.push(composed.span_problem(span, wrong("nothing")));
            }
        }
        Returns::Outputs(outputs) => {
            check_outputs(module, composed, span, name, result, outputs, problems)
        }
    }
}

fn describe(module: &naga::Module, ty: Option<Handle<naga::Type>>) -> String {
    ty.map_or_else(
        || "nothing".to_owned(),
        |ty| format!("`{}`", module.to_ctx().type_to_string(ty)),
    )
}

fn is_vector(
    module: &naga::Module,
    ty: Handle<naga::Type>,
    size: naga::VectorSize,
    scalar: naga::Scalar,
) -> bool {
    module.types[ty].inner == naga::TypeInner::Vector { size, scalar }
}

fn check_outputs(
    module: &naga::Module,
    composed: &Composed,
    span: naga::Span,
    function: &str,
    result: Option<Handle<naga::Type>>,
    outputs: &[(&str, Class)],
    problems: &mut Vec<Problem>,
) {
    let vec4 = |class: Class| format!("vec4<{}>", texel_wgsl(class));
    if let [(target, class)] = outputs {
        if !result
            .is_some_and(|ty| is_vector(module, ty, naga::VectorSize::Quad, texel_scalar(*class)))
        {
            problems.push(composed.span_problem(
                span,
                format!(
                    "`fn {function}` writes one target, `{target}`, and must return `{}`; it returns {}",
                    vec4(*class),
                    describe(module, result)
                ),
            ));
        }
        return;
    }
    let members = match result.map(|ty| (ty, &module.types[ty].inner)) {
        Some((ty, naga::TypeInner::Struct { members, .. })) => (ty, members),
        _ => {
            let names: Vec<&str> = outputs.iter().map(|(target, _)| *target).collect();
            problems.push(composed.span_problem(
                span,
                format!(
                    "`fn {function}` writes {} targets and must return a struct with a member for each ({}); it returns {}",
                    outputs.len(),
                    names.join(", "),
                    describe(module, result)
                ),
            ));
            return;
        }
    };
    let (ty, members) = members;
    let at = module.types.get_span(ty);
    let at = if composed.in_pack(at) { at } else { span };
    let struct_name = name_of(&module.types[ty].name);
    for (target, class) in outputs {
        match members.iter().find(|member| member.name.as_deref() == Some(*target)) {
            None => problems.push(composed.span_problem(
                at,
                format!("`{struct_name}` has no member `{target}`, and `fn {function}` writes that target"),
            )),
            Some(member) if !is_vector(module, member.ty, naga::VectorSize::Quad, texel_scalar(*class)) => {
                problems.push(composed.span_problem(
                    at,
                    format!(
                        "`{struct_name}.{target}` is `{}`, and the target's format takes `{}`",
                        module.to_ctx().type_to_string(member.ty),
                        vec4(*class)
                    ),
                ))
            }
            Some(_) => {}
        }
    }
    for member in members {
        if let Some(name) = &member.name
            && !outputs.iter().any(|(target, _)| *target == name.as_str())
        {
            problems.push(composed.span_problem(
                at,
                format!("`{struct_name}.{name}` names no target `fn {function}` writes"),
            ));
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Overrides {
    pub(crate) problems: Vec<Problem>,
    pub(crate) options: Vec<usize>,
}

pub(crate) fn check_overrides(
    module: &naga::Module,
    composed: &Composed,
    pack: &PackInterface,
) -> Overrides {
    let mut found = Overrides::default();
    for (handle, value) in module.overrides.iter() {
        let span = module.overrides.get_span(handle);
        let Some(name) = value.name.as_deref() else {
            continue;
        };
        if !composed.in_pack(span) {
            continue;
        }
        match pack.options.iter().position(|option| option.id == name) {
            Some(index) => {
                let kind = &pack.options[index].kind;
                if module.types[value.ty].inner != naga::TypeInner::Scalar(option_scalar(kind)) {
                    found.problems.push(composed.span_problem(
                        span,
                        format!(
                            "the option `{name}` is a {}, so its override is `{}`; this one is `{}`",
                            kind.type_name(),
                            kind.wgsl_type(),
                            module.to_ctx().type_to_string(value.ty)
                        ),
                    ));
                }
                if !found.options.contains(&index) {
                    found.options.push(index);
                }
            }
            None if value.init.is_none() => found.problems.push(
                composed
                    .span_problem(span, format!("`override {name}` names no option and has no initialiser, so nothing gives it a value"))
                    .hint("add an [[option]] with this id, or an initialiser"),
            ),
            None => {}
        }
    }
    found
}

pub(crate) fn used_globals<'c>(
    module: &naga::Module,
    info: &naga::valid::ModuleInfo,
    composed: &'c Composed,
    entries: &[&str],
) -> Vec<&'c Binding> {
    used_where(module, composed, |global| {
        module
            .entry_points
            .iter()
            .enumerate()
            .any(|(index, entry)| {
                entries.contains(&entry.name.as_str())
                    && !info.get_entry_point(index)[global].is_empty()
            })
    })
}

pub(crate) fn used_by_functions<'c>(
    module: &naga::Module,
    info: &naga::valid::ModuleInfo,
    composed: &'c Composed,
    functions: &[&str],
) -> Vec<&'c Binding> {
    used_where(module, composed, |global| {
        module.functions.iter().any(|(handle, function)| {
            function
                .name
                .as_deref()
                .is_some_and(|name| functions.contains(&name))
                && !info[handle][global].is_empty()
        })
    })
}

fn used_where<'c>(
    module: &naga::Module,
    composed: &'c Composed,
    uses: impl Fn(Handle<naga::GlobalVariable>) -> bool,
) -> Vec<&'c Binding> {
    let bindings = composed.bindings();
    let mut used = vec![false; bindings.len()];
    for (handle, global) in module.global_variables.iter() {
        let Some(resource) = &global.binding else {
            continue;
        };
        if resource.group != GLOBALS_GROUP || !uses(handle) {
            continue;
        }
        if let Some(slot) = bindings.iter().position(|b| b.binding == resource.binding) {
            used[slot] = true;
        }
    }
    bindings
        .iter()
        .zip(used)
        .filter_map(|(binding, used)| used.then_some(binding))
        .collect()
}

#[cfg(test)]
pub(crate) fn test_pack() -> PackInterface {
    let target =
        |id: &str, format, history| PreludeTarget::new(id.to_owned(), format, history).unwrap();
    PackInterface {
        api: super::manifest::API,
        targets: vec![
            target("lit", Format::Rgba16Float, true),
            target("albedo", Format::Rgba8Unorm, false),
            target("ids", Format::R32Uint, false),
        ],
        textures: vec!["clouds_tex".to_owned()],
        options: vec![
            PreludeOption {
                id: "exposure".to_owned(),
                kind: Kind::Float {
                    min: 0.0,
                    max: 4.0,
                    step: 0.1,
                },
            },
            PreludeOption {
                id: "quality".to_owned(),
                kind: Kind::Choice {
                    values: vec!["low".to_owned(), "high".to_owned()],
                },
            },
        ],
        materials: vec!["water".to_owned(), "foliage".to_owned()],
        shadow_color: true,
        post_only: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaderpack::native::compose::compose;
    use crate::shaderpack::native::import::memory_reader;

    fn validator() -> naga::valid::Validator {
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::default(),
        )
    }

    fn parse(text: &str) -> naga::Module {
        naga::front::wgsl::parse_str(text)
            .unwrap_or_else(|e| panic!("{}\n{text}", e.emit_to_string(text)))
    }

    fn validate(text: &str) -> (naga::Module, naga::valid::ModuleInfo) {
        let module = parse(text);
        let info = validator()
            .validate(&module)
            .unwrap_or_else(|e| panic!("{}\n{text}", e.emit_to_string(text)));
        (module, info)
    }

    fn screen_kind(writes: Vec<usize>) -> ModuleKind {
        ModuleKind::Screen {
            function: "lighting".to_owned(),
            output: ScreenOutput::Targets(writes),
            size: OutputSize::Scale(1.0),
        }
    }

    fn build(text: &str, kind: &ModuleKind) -> Composed {
        let files = [("p.wgsl", text)];
        let read = memory_reader(&files);
        compose(&test_pack(), "p.wgsl", kind, &read).unwrap()
    }

    fn screen_module(text: &str) -> (Composed, naga::Module) {
        let composed = build(text, &screen_kind(vec![0]));
        let module = naga::front::wgsl::parse_str(composed.text())
            .unwrap_or_else(|e| panic!("{}", composed.parse_problem(&e)));
        (composed, module)
    }

    const LIGHTING: &str =
        "fn lighting(in: ScreenInput) -> vec4<f32> {\n    return vec4<f32>(in.uv, 0.0, 1.0);\n}\n";

    fn none(problems: &[Problem]) {
        assert!(problems.is_empty(), "{problems:#?}");
    }

    fn lines(problems: &[Problem]) -> Vec<u32> {
        problems
            .iter()
            .map(|p| p.at.map_or(0, |at| at.line))
            .collect()
    }

    #[test]
    fn frame_layout_matches_naga() {
        let mut text = String::new();
        for s in &FRAME_STRUCTS {
            text.push_str(&s.wgsl());
        }
        text.push_str(
            "@group(0) @binding(0) var<uniform> frame: Frame;\n\
             @fragment fn f() -> @location(0) vec4<f32> {\n    \
             return vec4<f32>(frame.view.size, frame.time.seconds, frame.player.held_light.y);\n}\n",
        );
        let (module, _) = validate(&text);
        let sizes = [
            ("FrameCamera", size_of::<FrameCamera>()),
            ("FrameTime", size_of::<FrameTime>()),
            ("FrameSky", size_of::<FrameSky>()),
            ("FrameWeather", size_of::<FrameWeather>()),
            ("FramePlayer", size_of::<FramePlayer>()),
            ("FrameShadow", size_of::<FrameShadow>()),
            ("FrameView", size_of::<FrameView>()),
            ("Frame", size_of::<Frame>()),
        ];
        assert_eq!(sizes.map(|(name, _)| name), FRAME_STRUCTS.map(|s| s.name));
        for (s, (_, rust_size)) in FRAME_STRUCTS.iter().zip(sizes) {
            let (_, ty) = module
                .types
                .iter()
                .find(|(_, t)| t.name.as_deref() == Some(s.name))
                .unwrap();
            let naga::TypeInner::Struct { members, span } = &ty.inner else {
                panic!("{} is not a struct", s.name)
            };
            let theirs: Vec<(&str, u32)> = members
                .iter()
                .map(|m| (m.name.as_deref().unwrap(), m.offset))
                .collect();
            let ours: Vec<(&str, u32)> = s
                .members
                .iter()
                .map(|m| (m.name, m.offset as u32))
                .collect();
            assert_eq!(theirs, ours, "{}", s.name);
            assert_eq!(*span as usize, rust_size, "{}", s.name);
        }
        assert_eq!(size_of::<Frame>(), 736);
    }

    #[test]
    fn prelude_names_follow_the_pack_api() {
        let pack = test_pack();
        let bindings = bindings(&pack, &screen_kind(vec![0]));
        let mut decls = declarations(&pack, &bindings);
        decls.push(Decl {
            name: "later_name".to_owned(),
            since: 1,
            what: What::Engine,
            text: "const later_name: u32 = 1u;\n".to_owned(),
        });
        let older = emit(&decls, Api { major: 1, minor: 0 });
        let newer = emit(&decls, Api { major: 1, minor: 1 });
        assert!(!older.contains("later_name"));
        assert!(newer.contains("later_name"));
        let own = "const later_name: f32 = 2.0;\n";
        parse(&format!("{older}{own}"));
        assert!(naga::front::wgsl::parse_str(&format!("{newer}{own}")).is_err());
        assert_eq!(
            prelude(&pack, &bindings),
            emit(&declarations(&pack, &bindings), pack.api)
        );
    }

    #[test]
    fn bindings_are_stable_and_complete() {
        let pack = test_pack();
        let kind = ModuleKind::Compute {
            function: "voxelize".to_owned(),
            writes: vec![1],
            workgroup: [8, 8, 1],
        };
        let list = bindings(&pack, &kind);
        let named: Vec<(&str, u32)> = list.iter().map(|b| (b.name.as_str(), b.binding)).collect();
        assert_eq!(
            named,
            [
                ("frame", 0),
                ("linear_clamp", 1),
                ("linear_repeat", 2),
                ("nearest_clamp", 3),
                ("shadow_compare", 4),
                ("scene_depth", 5),
                ("shadow_depth", 6),
                ("shadow_color", 7),
                ("block_atlas", 8),
                ("lightmap", 9),
                ("noise", 10),
                ("lit", 12),
                ("albedo", 13),
                ("ids", 14),
                ("lit_prev", 15),
                ("clouds_tex", 16),
                ("albedo_out", 17),
            ]
        );
        assert_eq!(list[13].kind, BindingKind::Texture(SampleKind::Uint));
        assert_eq!(list[16].resource, Resource::TargetOut(1));
        assert_eq!(
            bindings(&pack, &kind),
            list,
            "the same inputs give the same numbers"
        );
    }

    #[test]
    fn a_screen_pass_module_validates() {
        let text = "override exposure: f32 = 1.0;\n\
                    override quality: u32;\n\
                    fn lighting(in: ScreenInput) -> vec4<f32> {\n    \
                        var c = textureSampleLevel(lit_prev, linear_clamp, in.uv, 0.0);\n    \
                        if quality == QUALITY_HIGH { c *= exposure; }\n    \
                        return c + vec4<f32>(f32(MATERIAL_WATER) * 0.0);\n\
                    }\n// block_atlas is named only in this comment.\n";
        let kind = ModuleKind::Screen {
            function: "lighting".to_owned(),
            output: ScreenOutput::Targets(vec![1]),
            size: OutputSize::Scale(0.5),
        };
        let composed = build(text, &kind);
        let (module, info) = validate(composed.text());
        let pack = test_pack();
        none(&check_rules(&module, &composed));
        none(&check_signatures(&module, &composed, &pack, &kind));
        let overrides = check_overrides(&module, &composed, &pack);
        none(&overrides.problems);
        assert_eq!(overrides.options, [0, 1]);
        let used: Vec<&str> =
            used_globals(&module, &info, &composed, &[SCREEN_VERTEX, SCREEN_FRAGMENT])
                .iter()
                .map(|b| b.name.as_str())
                .collect();
        assert_eq!(used, ["frame", "linear_clamp", "lit_prev"]);
        assert_eq!(composed.entry_points(), [SCREEN_VERTEX, SCREEN_FRAGMENT]);
    }

    #[test]
    fn a_screen_pass_writes_several_targets_or_the_view() {
        let several = "struct Out { lit: vec4<f32>, albedo: vec4<f32> }\n\
                       fn lighting(in: ScreenInput) -> Out { return Out(vec4<f32>(in.size, 0.0, 1.0), vec4<f32>(1.0)); }\n";
        let kind = screen_kind(vec![0, 1]);
        let composed = build(several, &kind);
        let (module, _) = validate(composed.text());
        none(&check_signatures(&module, &composed, &test_pack(), &kind));

        let kind = ModuleKind::Screen {
            function: "lighting".to_owned(),
            output: ScreenOutput::Screen,
            size: OutputSize::Fixed([640, 480]),
        };
        let composed = build(LIGHTING, &kind);
        validate(composed.text());
    }

    #[test]
    fn a_compute_pass_module_validates() {
        let text = "fn voxelize(in: ComputeInput) {\n    textureStore(lit_out, vec2<i32>(in.global_id.xy), vec4<f32>(1.0));\n}\n";
        let kind = ModuleKind::Compute {
            function: "voxelize".to_owned(),
            writes: vec![0],
            workgroup: [8, 8, 1],
        };
        let composed = build(text, &kind);
        let (module, info) = validate(composed.text());
        none(&check_rules(&module, &composed));
        none(&check_signatures(&module, &composed, &test_pack(), &kind));
        let used: Vec<&str> = used_globals(&module, &info, &composed, &[COMPUTE_ENTRY])
            .iter()
            .map(|b| b.name.as_str())
            .collect();
        assert_eq!(used, ["lit_out"]);
    }

    fn test_seam(input: Input) -> Option<&'static str> {
        match input {
            Input::Sky => Some(
                "fn torch_sky_base(v: torch_sky_varyings) -> Surface {\n    \
                 return Surface(vec4<f32>(normalize(v.position), 1.0), vec3<f32>(0.0, 1.0, 0.0), vec2<f32>(1.0), 0.0, 1.0, 0.0, 0u, vec4<f32>(0.0));\n}\n",
            ),
            Input::Terrain => Some(
                "fn torch_terrain_base(v: torch_terrain_varyings) -> Surface {\n    \
                 return Surface(textureSample(block_atlas, nearest_clamp, v.uv) * v.color, v.normal, v.light, 0.0, 1.0, 0.0, 0u, vec4<f32>(0.0));\n}\n",
            ),
            Input::Entity => Some(
                "fn torch_entity_base(v: torch_entity_varyings) -> Surface {\n    \
                 return Surface(v.color, v.normal, v.light, 0.0, 1.0, 0.0, 0u, v.overlay);\n}\n",
            ),
            _ => None,
        }
    }

    const OPAQUE: &str = "struct Opaque { albedo: vec4<f32>, ids: vec4<u32> }\n\
                          fn encode(s: Surface, at: FragmentInfo) -> Opaque {\n    \
                              return Opaque(s.color * at.depth, vec4<u32>(s.material, 0u, 0u, 0u));\n\
                          }\n\
                          fn terrain(in: TerrainInput) -> Surface {\n    \
                              var s = in.base;\n    \
                              s.color = vec4<f32>(in.mid_block, 1.0);\n    \
                              return s;\n\
                          }\n";

    fn opaque_kind() -> ModuleKind {
        ModuleKind::Geometry {
            group: Group::Opaque,
            writes: vec![1, 2],
            stages: vec![Stage::Terrain],
            vertex: false,
        }
    }

    #[test]
    fn geometry_fragment_entries_validate() {
        let pack = test_pack();
        let kind = opaque_kind();
        let bindings = bindings(&pack, &kind);
        let entries = entries_with(&pack, &kind, test_seam);
        let text = format!("{}{OPAQUE}\n{}", prelude(&pack, &bindings), entries.text);
        let (module, _) = validate(&text);
        let fragments: Vec<String> = Group::Opaque.stages().map(fragment_entry).collect();
        for name in &fragments {
            assert!(
                module.entry_points.iter().any(|e| e.name == *name),
                "{name}"
            );
        }
        assert!(entries.text.contains("let torch_s = terrain(TerrainInput("));
        assert!(
            entries
                .text
                .contains("let torch_s = torch_sky_base(torch_v);")
        );

        let composed = build(OPAQUE, &kind);
        assert!(composed.entry_points().is_empty());
        let (module, _) = validate(composed.text());
        none(&check_signatures(&module, &composed, &pack, &kind));
        none(&check_rules(&module, &composed));
    }

    #[test]
    fn geometry_signatures_are_checked_by_type() {
        let pack = test_pack();
        let kind = ModuleKind::Geometry {
            group: Group::Opaque,
            writes: vec![1, 2],
            stages: vec![Stage::Terrain, Stage::Hand],
            vertex: true,
        };
        let text = "struct Opaque { albedo: vec4<f32>, ids: vec4<f32>, extra: f32 }\n\
                    fn encode(s: Surface, at: FragmentInfo) -> Opaque { return Opaque(s.color, s.color, 0.0); }\n\
                    fn terrain(in: EntityInput) -> Surface { return in.base; }\n\
                    fn displace(in: VertexInput) -> vec4<f32> { return vec4<f32>(0.0); }\n";
        let composed = build(text, &kind);
        let (module, _) = validate(composed.text());
        let problems = check_signatures(&module, &composed, &pack, &kind);
        let messages: Vec<&str> = problems.iter().map(|p| p.message.as_str()).collect();
        assert_eq!(problems.len(), 5, "{messages:#?}");
        assert!(
            messages
                .iter()
                .any(|m| m.contains("`fn terrain` must take (TerrainInput)")),
            "{messages:#?}"
        );
        assert!(
            messages.iter().any(|m| m.starts_with("no `fn hand`")),
            "{messages:#?}"
        );
        assert!(
            messages
                .iter()
                .any(|m| m.contains("`Opaque.ids` is `vec4<f32>`")),
            "{messages:#?}"
        );
        assert!(
            messages
                .iter()
                .any(|m| m.contains("`Opaque.extra` names no target")),
            "{messages:#?}"
        );
        assert!(
            messages
                .iter()
                .any(|m| m.contains("`fn displace` must return `vec3<f32>`")),
            "{messages:#?}"
        );
        assert_eq!(
            lines(&problems).iter().filter(|&&line| line == 1).count(),
            2,
            "{problems:#?}"
        );
    }

    #[test]
    fn a_pass_function_returns_what_the_pass_writes() {
        let pack = test_pack();
        let kind = screen_kind(vec![0]);
        let composed = build(
            "fn lighting(in: ScreenInput) -> vec3<f32> {\n    return vec3<f32>(in.uv, 1.0);\n}\n",
            &kind,
        );
        assert!(
            naga::front::wgsl::parse_str(composed.text()).is_err()
                || validator().validate(&parse(composed.text())).is_err()
        );
        let module = parse(composed.without_entries());
        let problems = check_signatures(&module, &composed, &pack, &kind);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(problems[0].file, "p.wgsl");
        assert_eq!(problems[0].at.map(|at| at.line), Some(1));
        assert!(
            problems[0].message.contains("must return `vec4<f32>`"),
            "{}",
            problems[0].message
        );

        let kind = screen_kind(vec![2]);
        let composed = build(LIGHTING, &kind);
        let module = parse(composed.without_entries());
        assert_eq!(check_signatures(&module, &composed, &pack, &kind).len(), 1);

        let kind = screen_kind(vec![0, 1]);
        let composed = build(LIGHTING, &kind);
        let module = parse(composed.without_entries());
        assert!(
            check_signatures(&module, &composed, &pack, &kind)[0]
                .message
                .contains("must return a struct")
        );

        let kind = ModuleKind::Compute {
            function: "lighting".to_owned(),
            writes: vec![],
            workgroup: [1, 1, 1],
        };
        let composed = build(LIGHTING, &kind);
        let module = parse(composed.without_entries());
        let problems = check_signatures(&module, &composed, &pack, &kind);
        let messages: Vec<&str> = problems.iter().map(|p| p.message.as_str()).collect();
        assert_eq!(messages.len(), 2, "{messages:?}");
    }

    #[test]
    fn a_pack_declares_no_entry_point() {
        let (composed, module) = screen_module(LIGHTING);
        none(&check_rules(&module, &composed));
        let text = format!(
            "{LIGHTING}@fragment fn mine() -> @location(0) vec4<f32> {{\n    return vec4<f32>(1.0);\n}}\n"
        );
        let (composed, module) = screen_module(&text);
        let problems = check_rules(&module, &composed);
        assert_eq!(lines(&problems), [5], "{problems:?}");
        assert!(problems[0].message.contains("`mine` is an entry point"));
    }

    #[test]
    fn a_pack_binds_nothing() {
        let text = format!("{LIGHTING}@group(0) @binding(40) var<uniform> mine: vec4<f32>;\n");
        let (composed, module) = screen_module(&text);
        let problems = check_rules(&module, &composed);
        assert_eq!(lines(&problems), [4], "{problems:?}");
        assert!(problems[0].message.contains("@binding"));
    }

    #[test]
    fn a_pack_names_nothing_torch() {
        let text = format!(
            "{LIGHTING}fn torch_mine() -> f32 {{\n    var torch_local = 1.0;\n    return torch_local;\n}}\nconst torch_c: f32 = 1.0;\n"
        );
        let (composed, module) = screen_module(&text);
        let mut found = lines(&check_rules(&module, &composed));
        found.sort_unstable();
        assert_eq!(found, [4, 5, 8]);
    }

    #[test]
    fn a_pack_constructs_no_engine_struct() {
        let text = format!(
            "{LIGHTING}fn fake() -> ScreenInput {{\n    return ScreenInput(vec2<f32>(0.0), vec2<u32>(0u), vec2<f32>(1.0));\n}}\n\
             fn zero() -> FragmentInfo {{\n    return FragmentInfo();\n}}\n\
             fn copy(in: ScreenInput) -> ScreenInput {{\n    var c = in;\n    c.uv = vec2<f32>(0.5);\n    return c;\n}}\n\
             fn own() -> Surface {{\n    return Surface(vec4<f32>(1.0), vec3<f32>(0.0), vec2<f32>(0.0), 0.0, 1.0, 0.0, 0u, vec4<f32>(0.0));\n}}\n"
        );
        let (composed, module) = screen_module(&text);
        let problems = check_rules(&module, &composed);
        assert_eq!(lines(&problems), [5, 8], "{problems:?}");
    }

    #[test]
    fn overrides_match_options() {
        let pack = test_pack();
        let text = format!(
            "{LIGHTING}override exposure: i32;\noverride strength: f32;\noverride spare: f32 = 2.0;\n"
        );
        let (composed, module) = screen_module(&text);
        let found = check_overrides(&module, &composed, &pack);
        assert_eq!(lines(&found.problems), [4, 5], "{:?}", found.problems);
        assert_eq!(found.options, [0]);
    }

    #[test]
    fn interface_names_do_not_collide() {
        let pack = test_pack();
        let problems = check_interface(&pack);
        assert!(problems.is_empty(), "{problems:#?}");

        let mut noisy = test_pack();
        noisy.targets[1].id = "noise".to_owned();
        let problems = check_interface(&noisy);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(problems[0].owner, Some(Owner::Target(1)));

        let mut overlapping = test_pack();
        overlapping.options.push(PreludeOption {
            id: "clouds_tex".to_owned(),
            kind: Kind::Bool,
        });
        overlapping.options.push(PreludeOption {
            id: "material".to_owned(),
            kind: Kind::Choice {
                values: vec!["water".to_owned(), "lava".to_owned()],
            },
        });
        overlapping.targets[0].id = "loop".to_owned();
        let problems = check_interface(&overlapping);
        assert_eq!(problems.len(), 3, "{problems:#?}");

        let mut repeated = test_pack();
        repeated.targets[1].id = "lit".to_owned();
        repeated.targets[1].history = true;
        let problems = check_interface(&repeated);
        assert!(problems.is_empty(), "{problems:#?}");
    }

    #[test]
    fn a_shadow_colour_module_validates() {
        let pack = test_pack();
        let text = "fn shadow(in: TerrainInput) -> vec4<f32> {\n    return in.base.color;\n}\n";
        let bindings = bindings(&pack, &ModuleKind::Shadow);
        let entries = entries_with(&pack, &ModuleKind::Shadow, test_seam);
        validate(&format!(
            "{}{text}\n{}",
            prelude(&pack, &bindings),
            entries.text
        ));
        assert_eq!(
            entries.names,
            [vertex_entry(Input::Terrain), SHADOW_FRAGMENT.to_owned()]
        );

        let composed = build(text, &ModuleKind::Shadow);
        let (module, _) = validate(composed.text());
        none(&check_signatures(
            &module,
            &composed,
            &pack,
            &ModuleKind::Shadow,
        ));
        let wrong = build(
            "fn shadow(in: TerrainInput) -> Surface {\n    return in.base;\n}\n",
            &ModuleKind::Shadow,
        );
        let (module, _) = validate(wrong.text());
        let problems = check_signatures(&module, &wrong, &pack, &ModuleKind::Shadow);
        assert_eq!(lines(&problems), [1], "{problems:#?}");
    }

    #[test]
    fn entries_build_inputs_in_member_order() {
        let names = |def: &StructDef| def.members.iter().map(|m| m.name).collect::<Vec<_>>();
        assert_eq!(names(Input::Screen.def()), ["uv", "pixel", "size"]);
        assert_eq!(names(Input::Compute.def()), ["global_id", "local_id"]);
        assert_eq!(
            names(&FRAGMENT_INFO),
            ["pixel", "depth", "front_facing", "position"]
        );
        for stage in Stage::ALL {
            let def = stage.info().input.def();
            assert_eq!(def.members[0].ty, Ty::Surface, "{}", def.name);
            assert!(
                def.members
                    .iter()
                    .any(|m| m.name == "position" && m.ty == Ty::Vec3F),
                "{}",
                def.name
            );
        }
        for input in Input::ALL {
            assert_eq!(
                input
                    .def()
                    .name
                    .trim_end_matches("Input")
                    .to_ascii_lowercase(),
                input.stem()
            );
        }
        assert_eq!(
            Group::Translucent.stages().collect::<Vec<_>>(),
            [Stage::Water, Stage::Particle, Stage::Clouds, Stage::Weather]
        );
    }
}
