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

impl Wgsl for [i32; 3] {
    const TYPE: &'static str = "vec3<i32>";
    const ALIGN: usize = 16;
}

impl Wgsl for [f32; 4] {
    const TYPE: &'static str = "vec4<f32>";
    const ALIGN: usize = 16;
}

impl Wgsl for [[f32; 4]; 4] {
    const TYPE: &'static str = "mat4x4<f32>";
    const ALIGN: usize = 16;
}

impl Wgsl for [[[f32; 4]; 4]; MAX_CASCADES] {
    const TYPE: &'static str = "array<mat4x4<f32>, 4>";
    const ALIGN: usize = 16;
}

pub(crate) const MAX_CASCADES: usize = 4;

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
        #[allow(dead_code, reason = "the renderer writes every member and the GPU reads them; nothing in Rust reads one back")]
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
        hand_projection: [[f32; 4]; 4],
        position: [f32; 3],
        near: f32,
        far: f32,
        _pad0: u32,
        _pad1: u32,
        _pad2: u32,
        block: [i32; 3],
        _pad3: u32,
        fraction: [f32; 3],
        _pad4: u32,
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
        light_floor: [f32; 3],
        _pad1: u32,
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
        count: u32,
        _pad0: u32,
        cascades: [[[f32; 4]; 4]; MAX_CASCADES],
        far: [f32; 4],
        texel: [f32; 4],
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
            "0 to 1: a block's light level over 15 for terrain, 1 for the sky, 0 elsewhere.",
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
            m(
                "part",
                Ty::U32,
                "Which part of the sky this fragment is: one of the `SKY_PART_*` constants.",
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
            m(
                "block_lit",
                Ty::Vec3F,
                "The lightmap's block-only row at this vertex's block light, less `frame.sky.light_floor`, linear rgb. Read per vertex and interpolated, as the built-in terrain reads it.",
            ),
            m(
                "sky_lit",
                Ty::Vec3F,
                "The lightmap's sky-only column at this vertex's sky light, less `frame.sky.light_floor`, linear rgb. Read per vertex and interpolated.",
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

    fn engine_varyings(self) -> &'static [MemberDef] {
        const TERRAIN: &[MemberDef] = &[
            m(
                "torch_material",
                Ty::U32,
                "The block's `MATERIAL_*` number, or 0.",
            ),
            m(
                "torch_emission",
                Ty::F32,
                "The block's light level over 15.",
            ),
        ];
        const SKY: &[MemberDef] = &[
            m("torch_uv", Ty::Vec2F, "The mesh's texture coordinate."),
            m(
                "torch_color",
                Ty::Vec4F,
                "The vertex colour times the tint.",
            ),
            m("torch_normal", Ty::Vec3F, "World space, unit length."),
            m(
                "torch_light",
                Ty::Vec2F,
                "Block light and sky light, 0 to 1.",
            ),
        ];
        const CLOUD: &[MemberDef] = &[
            m("torch_uv", Ty::Vec2F, "The mesh's texture coordinate."),
            m(
                "torch_light",
                Ty::Vec2F,
                "Block light and sky light, 0 to 1.",
            ),
        ];
        const WEATHER: &[MemberDef] = &[m("torch_normal", Ty::Vec3F, "World space, unit length.")];
        match self {
            Input::Terrain => TERRAIN,
            Input::Sky => SKY,
            Input::Cloud => CLOUD,
            Input::Weather => WEATHER,
            Input::Entity | Input::Vertex | Input::Screen | Input::Compute => &[],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SkyPart {
    Disc,
    DarkDisc,
    Sunrise,
    Sun,
    Moon,
    Stars,
    End,
    EndFlash,
}

impl SkyPart {
    pub(crate) const ALL: [SkyPart; 8] = [
        SkyPart::Disc,
        SkyPart::DarkDisc,
        SkyPart::Sunrise,
        SkyPart::Sun,
        SkyPart::Moon,
        SkyPart::Stars,
        SkyPart::End,
        SkyPart::EndFlash,
    ];

    pub(crate) const fn id(self) -> u32 {
        self as u32 + 1
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            SkyPart::Disc => "disc",
            SkyPart::DarkDisc => "dark_disc",
            SkyPart::Sunrise => "sunrise",
            SkyPart::Sun => "sun",
            SkyPart::Moon => "moon",
            SkyPart::Stars => "stars",
            SkyPart::End => "end",
            SkyPart::EndFlash => "end_flash",
        }
    }

    pub(crate) fn parse(name: &str) -> Option<SkyPart> {
        SkyPart::ALL.into_iter().find(|part| part.name() == name)
    }

    fn constant(self) -> String {
        format!("SKY_PART_{}", self.name().to_ascii_uppercase())
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

    pub(crate) fn draw(self) -> &'static StageDraw {
        &STAGE_DRAWS[self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TerrainStream {
    Solid,
    Cutout,
    Water,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "the renderer's geometry draw reads the lists")]
pub(crate) enum Draws {
    Terrain(&'static [TerrainStream]),
    Entities(&'static [&'static str]),
    Nothing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StageDraw {
    pub(crate) draws: Draws,
    pub(crate) cutout: bool,
}

static STAGE_DRAWS: [StageDraw; 9] = [
    StageDraw {
        draws: Draws::Entities(&["SkyBasic", "SkyTextured"]),
        cutout: false,
    },
    StageDraw {
        draws: Draws::Terrain(&[TerrainStream::Solid, TerrainStream::Cutout]),
        cutout: true,
    },
    StageDraw {
        draws: Draws::Entities(&["Entity", "EntityTranslucent"]),
        cutout: true,
    },
    StageDraw {
        draws: Draws::Entities(&["Block", "BlockTranslucent"]),
        cutout: true,
    },
    StageDraw {
        draws: Draws::Entities(&["Hand", "HandWater"]),
        cutout: true,
    },
    StageDraw {
        draws: Draws::Terrain(&[TerrainStream::Water]),
        cutout: false,
    },
    StageDraw {
        draws: Draws::Entities(&["Particle", "ParticleTranslucent"]),
        cutout: true,
    },
    StageDraw {
        draws: Draws::Entities(&["Clouds"]),
        cutout: false,
    },
    StageDraw {
        draws: Draws::Nothing,
        cutout: false,
    },
];

const CUTOFF: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TerrainTier {
    Direct,
    Indirect,
}

#[cfg(test)]
impl TerrainTier {
    pub(crate) const ALL: [TerrainTier; 2] = [TerrainTier::Direct, TerrainTier::Indirect];
}

#[allow(
    dead_code,
    reason = "the renderer's geometry draw asserts its own constant against it"
)]
pub(crate) const TERRAIN_GROUP: u32 = 1;

#[allow(
    dead_code,
    reason = "the renderer's geometry draw asserts its own constant against it"
)]
pub(crate) const SLOT_TABLE_BINDING: u32 = 5;

#[allow(
    dead_code,
    reason = "the renderer's geometry draw asserts its own constant against it"
)]
pub(crate) const PACK_QUADS_GROUP: u32 = 2;

#[allow(
    dead_code,
    reason = "the renderer's geometry draw asserts its own constant against it"
)]
pub(crate) const ORIGIN_ROW: u32 = 256;

#[allow(
    dead_code,
    reason = "the renderer's geometry draw asserts its own constant against it"
)]
pub(crate) const QUAD_ROW: u32 = 2048;

#[allow(
    dead_code,
    reason = "the renderer's geometry draw asserts its own constant against it"
)]
pub(crate) const SLOT_WORDS: u32 = 6;

pub(crate) const ENCODE: &str = "encode";

pub(crate) const DISPLACE: &str = "displace";

pub(crate) const SHADOW: &str = "shadow";

const SHADOW_TARGET: &str = "shadow_color";

pub(crate) const SHADOW_COLOR_FORMAT: Format = Format::Rgba8Unorm;

#[allow(
    dead_code,
    reason = "the renderer's shadow pass asserts its own constant against it"
)]
pub(crate) const SHADOW_DRAW_GROUP: u32 = 3;

pub(crate) const SHADOW_ENTITY_DRAWS: Draws = Draws::Entities(&["ShadowCaster"]);

pub(crate) const SHADOW_TERRAIN_DRAWS: Draws =
    Draws::Terrain(&[TerrainStream::Solid, TerrainStream::Cutout]);

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
    formats: Vec<Format>,
    chosen: usize,
    pub(crate) history: bool,
}

impl PreludeTarget {
    pub(crate) fn new(
        id: String,
        formats: Vec<Format>,
        chosen: usize,
        history: bool,
    ) -> Option<PreludeTarget> {
        let class = formats.first()?.info().class;
        let valid = class != Class::Depth
            && formats.iter().all(|f| f.info().class == class)
            && chosen < formats.len();
        valid.then_some(PreludeTarget {
            id,
            formats,
            chosen,
            history,
        })
    }

    pub(crate) fn class(&self) -> Class {
        self.formats[0].info().class
    }

    fn format(&self) -> Format {
        self.formats[self.chosen]
    }

    fn format_constant(&self) -> Option<String> {
        (self.formats.len() > 1).then(|| format!("FORMAT_{}", self.id.to_ascii_uppercase()))
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
        terrain: TerrainTier,
    },
    Shadow {
        color: bool,
        displace: bool,
        terrain: TerrainTier,
    },
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
            ModuleKind::Geometry { .. } | ModuleKind::Shadow { .. } => None,
        }
    }

    fn storage_writes(&self) -> &[usize] {
        match self {
            ModuleKind::Compute { writes, .. } => writes,
            ModuleKind::Screen { .. } | ModuleKind::Geometry { .. } | ModuleKind::Shadow { .. } => {
                &[]
            }
        }
    }

    fn draws_meshes(&self) -> bool {
        matches!(
            self,
            ModuleKind::Geometry { .. } | ModuleKind::Shadow { .. }
        )
    }

    pub(crate) fn terrain(&self) -> Option<TerrainTier> {
        match self {
            ModuleKind::Geometry { terrain, .. } | ModuleKind::Shadow { terrain, .. } => {
                Some(*terrain)
            }
            ModuleKind::Screen { .. } | ModuleKind::Compute { .. } => None,
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
            ModuleKind::Shadow {
                color, displace, ..
            } => {
                let mut out = Vec::new();
                if *color {
                    out.push(SHADOW);
                }
                if *displace {
                    out.push(DISPLACE);
                }
                out
            }
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
    EntityTexture,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ViewDimension {
    D2,
    D2Array,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Binding {
    pub(crate) name: String,
    pub(crate) binding: u32,
    pub(crate) resource: Resource,
    pub(crate) kind: BindingKind,
    #[allow(dead_code, reason = "the renderer's bind group layouts read it")]
    pub(crate) dimension: ViewDimension,
    ty: String,
}

struct Fixed {
    name: &'static str,
    binding: u32,
    resource: Resource,
    since: u32,
}

const FIXED: [Fixed; 13] = [
    Fixed {
        name: "frame",
        binding: 0,
        resource: Resource::Frame,
        since: 0,
    },
    Fixed {
        name: "linear_clamp",
        binding: 1,
        resource: Resource::LinearClamp,
        since: 0,
    },
    Fixed {
        name: "linear_repeat",
        binding: 2,
        resource: Resource::LinearRepeat,
        since: 0,
    },
    Fixed {
        name: "nearest_clamp",
        binding: 3,
        resource: Resource::NearestClamp,
        since: 0,
    },
    Fixed {
        name: "shadow_compare",
        binding: 4,
        resource: Resource::ShadowCompare,
        since: 0,
    },
    Fixed {
        name: "scene_depth",
        binding: 5,
        resource: Resource::SceneDepth,
        since: 0,
    },
    Fixed {
        name: "shadow_depth",
        binding: 6,
        resource: Resource::ShadowDepth,
        since: 0,
    },
    Fixed {
        name: "shadow_color",
        binding: 7,
        resource: Resource::ShadowColor,
        since: 0,
    },
    Fixed {
        name: "block_atlas",
        binding: 8,
        resource: Resource::BlockAtlas,
        since: 0,
    },
    Fixed {
        name: "lightmap",
        binding: 9,
        resource: Resource::Lightmap,
        since: 0,
    },
    Fixed {
        name: "noise",
        binding: 10,
        resource: Resource::Noise,
        since: 0,
    },
    Fixed {
        name: "scene_color",
        binding: 11,
        resource: Resource::SceneColor,
        since: 0,
    },
    Fixed {
        name: "entity_texture",
        binding: 11,
        resource: Resource::EntityTexture,
        since: 0,
    },
];

const FIRST_PACK_BINDING: u32 = 12;

const _: () = {
    let mut i = 0;
    while i < FIXED.len() {
        assert!(FIXED[i].binding < FIRST_PACK_BINDING);
        i += 1;
    }
};

pub(crate) fn bindings(pack: &PackInterface, kind: &ModuleKind) -> Vec<Binding> {
    bindings_writing(pack, kind.storage_writes(), kind.draws_meshes())
}

fn bindings_writing(pack: &PackInterface, outs: &[usize], meshes: bool) -> Vec<Binding> {
    let mut list = Vec::new();
    for fixed in &FIXED {
        let present = fixed.since <= pack.api.minor
            && match fixed.resource {
                Resource::ShadowColor => pack.shadow_color,
                Resource::SceneColor => pack.post_only && !meshes,
                Resource::EntityTexture => meshes,
                _ => true,
            };
        if present {
            list.push(binding(
                pack,
                fixed.name.to_owned(),
                fixed.binding,
                fixed.resource,
            ));
        }
    }
    let mut next = FIRST_PACK_BINDING;
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
    let dimension = match resource {
        Resource::ShadowDepth | Resource::ShadowColor => ViewDimension::D2Array,
        _ => ViewDimension::D2,
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
        Resource::SceneDepth => float(),
        Resource::ShadowDepth => (
            BindingKind::Texture(SampleKind::Depth),
            "texture_depth_2d_array".to_owned(),
        ),
        Resource::ShadowColor => (
            BindingKind::Texture(SampleKind::Float),
            "texture_2d_array<f32>".to_owned(),
        ),
        Resource::BlockAtlas
        | Resource::Lightmap
        | Resource::Noise
        | Resource::SceneColor
        | Resource::EntityTexture
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
                pack.targets[index].format().name()
            ),
        ),
    };
    Binding {
        name,
        binding: number,
        resource,
        kind,
        dimension,
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
    Format(usize),
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
            What::Format(_) => "a format constant (`FORMAT_<ID>`)",
            What::Texture(_) => "a texture",
            What::Option(_) => "an option",
            What::Material(_) => "a material constant (`MATERIAL_<ID>`)",
            What::Choice(_) => "a choice constant (`<ID>_<VALUE>`)",
        }
    }

    fn owner(self) -> Option<Owner> {
        match self {
            What::Engine => None,
            What::Target(index)
            | What::TargetPrevious(index)
            | What::TargetOut(index)
            | What::Format(index) => Some(Owner::Target(index)),
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

const SHADOW_FUNCTIONS: [&str; 2] = ["shadow_cascade", "shadow_light"];

const SHADOW_WGSL: &str = include_str!("wgsl/shadow.wgsl");

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
    decls.push(Decl {
        name: SHADOW_FUNCTIONS[0].to_owned(),
        since: 0,
        what: What::Engine,
        text: SHADOW_WGSL.to_owned(),
    });
    for part in SkyPart::ALL {
        let name = part.constant();
        let text = format!("const {name}: u32 = {}u;\n", part.id());
        decls.push(Decl {
            name,
            since: 0,
            what: What::Engine,
            text,
        });
    }
    for b in bindings {
        decls.push(Decl {
            name: b.name.clone(),
            since: 0,
            what: What::of(b.resource),
            text: global_declaration(b),
        });
    }
    for (index, target) in pack.targets.iter().enumerate() {
        let Some(name) = target.format_constant() else {
            continue;
        };
        let text = format!("const {name}: u32 = {}u;\n", target.chosen);
        decls.push(Decl {
            name,
            since: 0,
            what: What::Format(index),
            text,
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

fn global_declaration(b: &Binding) -> String {
    let space = if b.kind == BindingKind::Uniform {
        "<uniform>"
    } else {
        ""
    };
    format!(
        "@group({GLOBALS_GROUP}) @binding({}) var{space} {}: {};\n",
        b.binding, b.name, b.ty
    )
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
    names.extend(
        SHADOW_FUNCTIONS
            .iter()
            .map(|name| ((*name).to_owned(), What::Engine)),
    );
    let every_target: Vec<usize> = (0..pack.targets.len()).collect();
    let bindings = bindings_writing(pack, &every_target, true);
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

pub(crate) fn cutout_entry(stage: Stage) -> String {
    format!("{ENGINE_PREFIX}{}_cutout_fragment", stage.info().function)
}

pub(crate) fn vertex_entry(input: Input) -> String {
    format!("{ENGINE_PREFIX}{}_vertex", input.stem())
}

const HAND_VERTEX: &str = "torch_hand_vertex";

pub(crate) fn stage_vertex_entry(stage: Stage) -> String {
    match stage {
        Stage::Hand => HAND_VERTEX.to_owned(),
        _ => vertex_entry(stage.info().input),
    }
}

pub(crate) fn entries(pack: &PackInterface, kind: &ModuleKind) -> Entries {
    entries_with(pack, kind, geometry_seam)
}

const MESH_WGSL: &str = include_str!("wgsl/mesh.wgsl");
const TERRAIN_WGSL: &str = include_str!("wgsl/terrain.wgsl");
const TERRAIN_VERTEX_WGSL: &str = include_str!("wgsl/terrain_vertex.wgsl");
const SHADOW_PASS_WGSL: &str = include_str!("wgsl/shadow_pass.wgsl");
const TERRAIN_DIRECT_WGSL: &str = include_str!("wgsl/terrain_direct.wgsl");
const TERRAIN_INDIRECT_WGSL: &str = include_str!("wgsl/terrain_indirect.wgsl");
const SKY_WGSL: &str = include_str!("wgsl/sky.wgsl");
const ENTITY_WGSL: &str = include_str!("wgsl/entity.wgsl");
const HAND_WGSL: &str = include_str!("wgsl/hand.wgsl");
const CLOUD_WGSL: &str = include_str!("wgsl/cloud.wgsl");
const WEATHER_WGSL: &str = include_str!("wgsl/weather.wgsl");

#[derive(Clone, Debug)]
struct SeamPart {
    shared: Option<&'static str>,
    text: String,
    stages: &'static [(Stage, &'static str)],
}

type Seam = fn(Input, TerrainTier) -> Option<SeamPart>;

fn geometry_seam(input: Input, terrain: TerrainTier) -> Option<SeamPart> {
    let mesh = |text: &str, stages: &'static [(Stage, &'static str)]| {
        Some(SeamPart {
            shared: Some(MESH_WGSL),
            text: text.to_owned(),
            stages,
        })
    };
    match input {
        Input::Terrain => {
            let text = format!("{}\n{TERRAIN_VERTEX_WGSL}", terrain_decode(terrain));
            Some(SeamPart {
                shared: None,
                text,
                stages: &[],
            })
        }
        Input::Sky => mesh(SKY_WGSL, &[]),
        Input::Entity => mesh(ENTITY_WGSL, &[(Stage::Hand, HAND_WGSL)]),
        Input::Cloud => mesh(CLOUD_WGSL, &[]),
        Input::Weather => mesh(WEATHER_WGSL, &[]),
        Input::Vertex | Input::Screen | Input::Compute => None,
    }
}

fn terrain_decode(terrain: TerrainTier) -> String {
    let origin = match terrain {
        TerrainTier::Direct => TERRAIN_DIRECT_WGSL,
        TerrainTier::Indirect => TERRAIN_INDIRECT_WGSL,
    };
    format!("{TERRAIN_WGSL}\n{origin}")
}

fn displace_function(vertex: bool) -> String {
    let body = if vertex {
        format!("return {DISPLACE}(torch_in);")
    } else {
        "return vec3<f32>(0.0);".to_owned()
    };
    format!("fn torch_displace(torch_in: VertexInput) -> vec3<f32> {{\n    {body}\n}}\n\n")
}

fn entries_with(pack: &PackInterface, kind: &ModuleKind, seam: Seam) -> Entries {
    match kind {
        ModuleKind::Geometry {
            group,
            writes,
            stages,
            vertex,
            terrain,
        } => geometry_entries(
            &outputs(pack, writes),
            *group,
            stages,
            *vertex,
            *terrain,
            seam,
        ),
        ModuleKind::Shadow {
            color,
            displace,
            terrain,
        } => shadow_entries(*color, *displace, *terrain),
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
    let own = input
        .def()
        .members
        .iter()
        .filter(|member| member.ty != Ty::Surface);
    let carried = own.chain(input.engine_varyings());
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
    displace: bool,
    terrain: TerrainTier,
    seam: Seam,
) -> Entries {
    let stages: Vec<Stage> = group.stages().collect();
    let mut inputs: Vec<Input> = Vec::new();
    for stage in &stages {
        if !inputs.contains(&stage.info().input) {
            inputs.push(stage.info().input);
        }
    }
    let Some(parts) = inputs
        .iter()
        .map(|&input| seam(input, terrain))
        .collect::<Option<Vec<SeamPart>>>()
    else {
        return Entries::default();
    };
    if outputs.is_empty() {
        return Entries::default();
    }
    let out = group.output_struct();
    let mut text = output_struct(out, outputs);
    if inputs.contains(&Input::Terrain) {
        text.push_str(&displace_function(displace));
    }
    let mut shared: Vec<&str> = Vec::new();
    for part in &parts {
        if let Some(part_shared) = part.shared
            && !shared.contains(&part_shared)
        {
            shared.push(part_shared);
            text.push_str(part_shared);
            text.push('\n');
        }
    }
    let mut names = Vec::new();
    for (&input, part) in inputs.iter().zip(&parts) {
        text.push_str(&varyings_struct(input));
        text.push_str(&part.text);
        text.push('\n');
        names.push(vertex_entry(input));
        for &(stage, stage_text) in part.stages {
            if stages.contains(&stage) {
                text.push_str(stage_text);
                text.push('\n');
                names.push(stage_vertex_entry(stage));
            }
        }
    }
    for stage in stages {
        let info = stage.info();
        let surface = if defined.contains(&stage) {
            format!("{}({})", info.function, input_value(info.input))
        } else {
            format!("{ENGINE_PREFIX}{}_base(torch_v)", info.input.stem())
        };
        let fragment = |name: &str, test: &str| {
            format!(
                "@fragment\nfn {name}(torch_v: {varyings}, @builtin(front_facing) torch_facing: bool) -> {out} {{\n    \
                 let torch_s = {surface};\n\
                 {test}    \
                 let torch_e = {ENCODE}(torch_s, FragmentInfo(torch_v.clip.xy, torch_v.clip.z, torch_facing, torch_v.position));\n    \
                 return {};\n}}\n\n",
                output_value(out, "torch_e", outputs),
                varyings = varyings_name(info.input),
            )
        };
        let name = fragment_entry(stage);
        text.push_str(&fragment(&name, ""));
        names.push(name);
        if stage.draw().cutout {
            let name = cutout_entry(stage);
            text.push_str(&fragment(
                &name,
                &format!("    if torch_s.color.a < {CUTOFF:?} {{\n        discard;\n    }}\n"),
            ));
            names.push(name);
        }
    }
    Entries { text, names }
}

pub(crate) const OVERLAY_VERTEX: &str = "torch_overlay_vertex";
pub(crate) const OVERLAY_CRACKS: &str = "torch_overlay_cracks_fragment";
pub(crate) const OVERLAY_LINE: &str = "torch_overlay_line_fragment";

const OVERLAY_WGSL: &str = include_str!("wgsl/overlay.wgsl");

const OVERLAY_GLOBALS: [Resource; 3] = [
    Resource::Frame,
    Resource::NearestClamp,
    Resource::EntityTexture,
];

pub(crate) fn overlay_module() -> (String, Vec<Binding>) {
    let none = PackInterface {
        api: super::manifest::API,
        targets: Vec::new(),
        textures: Vec::new(),
        options: Vec::new(),
        materials: Vec::new(),
        shadow_color: false,
        post_only: false,
    };
    let bindings: Vec<Binding> = bindings_writing(&none, &[], true)
        .into_iter()
        .filter(|b| OVERLAY_GLOBALS.contains(&b.resource))
        .collect();
    let mut text = String::new();
    for s in &FRAME_STRUCTS {
        text.push_str(&s.wgsl());
        text.push('\n');
    }
    for b in &bindings {
        text.push_str(&global_declaration(b));
    }
    text.push('\n');
    text.push_str(MESH_WGSL);
    text.push('\n');
    text.push_str(OVERLAY_WGSL);
    (text, bindings)
}

pub(crate) const SHADOW_TERRAIN_VERTEX: &str = "torch_shadow_terrain_vertex";
pub(crate) const SHADOW_TERRAIN_FRAGMENT: &str = "torch_shadow_terrain_fragment";
pub(crate) const SHADOW_TERRAIN_CUTOUT: &str = "torch_shadow_terrain_cutout_fragment";
pub(crate) const SHADOW_ENTITY_VERTEX: &str = "torch_shadow_entity_vertex";
pub(crate) const SHADOW_ENTITY_FRAGMENT: &str = "torch_shadow_entity_fragment";
pub(crate) const SHADOW_ENTITY_CUTOUT: &str = "torch_shadow_entity_cutout_fragment";

fn shadow_entries(color: bool, displace: bool, terrain: TerrainTier) -> Entries {
    let input = Input::Terrain;
    let varyings = varyings_name(input);
    let out = "torch_shadow_out";
    let outputs = shadow_outputs();
    let mut text = String::new();
    if color {
        text.push_str(&output_struct(out, &outputs));
    }
    text.push_str(&displace_function(displace));
    text.push_str(MESH_WGSL);
    text.push('\n');
    text.push_str(&varyings_struct(input));
    text.push_str(&terrain_decode(terrain));
    text.push('\n');
    let lit = if color {
        "return torch_terrain_lit(torch_v, torch_light);"
    } else {
        "return torch_v;"
    };
    text.push_str(&format!(
        "fn torch_shadow_terrain_lit(torch_v: {varyings}, torch_light: vec4<f32>) -> {varyings} {{\n    {lit}\n}}\n\n"
    ));
    text.push_str(SHADOW_PASS_WGSL);
    text.push('\n');

    let discard =
        |value: &str| format!("    if {value} < {CUTOFF:?} {{\n        discard;\n    }}\n");
    let mut names = vec![
        SHADOW_TERRAIN_VERTEX.to_owned(),
        SHADOW_ENTITY_VERTEX.to_owned(),
    ];
    let mesh = "torch_shadow_mesh_varyings";
    if color {
        let write = output_value(out, "torch_c", &outputs);
        let shade = format!(
            "    let torch_i = {};\n    let torch_c = {SHADOW}(torch_i);\n",
            input_value(input)
        );
        let ret = format!("    return {write};\n");
        text.push_str(&format!(
            "@fragment\nfn {SHADOW_TERRAIN_FRAGMENT}(torch_v: {varyings}) -> {out} {{\n{shade}{ret}}}\n\n"
        ));
        text.push_str(&format!(
            "@fragment\nfn {SHADOW_TERRAIN_CUTOUT}(torch_v: {varyings}) -> {out} {{\n{shade}{}{ret}}}\n\n",
            discard("torch_i.base.color.a"),
        ));
        let texel = "    let torch_c = torch_mesh_texel(torch_v.uv) * torch_v.color;\n";
        text.push_str(&format!(
            "@fragment\nfn {SHADOW_ENTITY_FRAGMENT}(torch_v: {mesh}) -> {out} {{\n{texel}    return {write};\n}}\n\n"
        ));
        text.push_str(&format!(
            "@fragment\nfn {SHADOW_ENTITY_CUTOUT}(torch_v: {mesh}) -> {out} {{\n{texel}{}    return {write};\n}}\n",
            discard("torch_c.a"),
        ));
        names.extend(
            [
                SHADOW_TERRAIN_FRAGMENT,
                SHADOW_TERRAIN_CUTOUT,
                SHADOW_ENTITY_FRAGMENT,
                SHADOW_ENTITY_CUTOUT,
            ]
            .map(str::to_owned),
        );
    } else {
        text.push_str(&format!(
            "@fragment\nfn {SHADOW_TERRAIN_CUTOUT}(torch_v: {varyings}) {{\n{}}}\n\n",
            discard("torch_terrain_base(torch_v).color.a"),
        ));
        text.push_str(&format!(
            "@fragment\nfn {SHADOW_ENTITY_CUTOUT}(torch_v: {mesh}) {{\n{}}}\n",
            discard("(torch_mesh_texel(torch_v.uv) * torch_v.color).a"),
        ));
        names.extend([SHADOW_TERRAIN_CUTOUT, SHADOW_ENTITY_CUTOUT].map(str::to_owned));
    }
    Entries { text, names }
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
            ..
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
        ModuleKind::Shadow {
            color, displace, ..
        } => {
            if *color {
                expected.push(Expected {
                    function: SHADOW,
                    why: "[shadow] sets `color = true`".to_owned(),
                    args: vec![Input::Terrain.name()],
                    returns: Returns::Outputs(shadow_outputs()),
                });
            }
            if *displace {
                expected.push(Expected {
                    function: DISPLACE,
                    why: format!("the {} group sets `vertex = true`", Group::Opaque.name()),
                    args: vec![Input::Vertex.name()],
                    returns: Returns::Vec3F,
                });
            }
        }
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
    let target = |id: &str, formats: &[Format], history| {
        PreludeTarget::new(id.to_owned(), formats.to_vec(), 0, history).unwrap()
    };
    PackInterface {
        api: super::manifest::API,
        targets: vec![
            target("lit", &[Format::Rgba16Float, Format::Rgba8Unorm], true),
            target("albedo", &[Format::Rgba8Unorm], false),
            target("ids", &[Format::R32Uint], false),
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
             let c = (frame.shadow.cascades[frame.shadow.count] * vec4<f32>(frame.sky.light_floor, 1.0)).x;\n    \
             return vec4<f32>(frame.view.size, frame.time.seconds, frame.player.held_light.y + c);\n}\n",
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
        assert_eq!(size_of::<FrameCamera>(), 512);
        assert_eq!(size_of::<FrameSky>(), 80);
        assert_eq!(size_of::<FrameShadow>(), 432);
        assert_eq!(size_of::<Frame>(), 1136);
    }

    #[test]
    fn overlay_module_validates() {
        let (text, bindings) = overlay_module();
        let (module, _) = validate(&text);
        for entry in [OVERLAY_VERTEX, OVERLAY_CRACKS, OVERLAY_LINE] {
            assert!(
                module.entry_points.iter().any(|e| e.name == entry),
                "{entry}"
            );
        }
        let named: Vec<(&str, u32)> = bindings
            .iter()
            .map(|b| (b.name.as_str(), b.binding))
            .collect();
        assert_eq!(
            named,
            [("frame", 0), ("nearest_clamp", 3), ("entity_texture", 11)]
        );
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
        let arrays: Vec<&str> = list
            .iter()
            .filter(|b| b.dimension == ViewDimension::D2Array)
            .map(|b| b.name.as_str())
            .collect();
        assert_eq!(arrays, ["shadow_depth", "shadow_color"]);
        assert_eq!(list[6].kind, BindingKind::Texture(SampleKind::Depth));
        assert_eq!(list[7].kind, BindingKind::Texture(SampleKind::Float));
        assert_eq!(
            bindings(&pack, &kind),
            list,
            "the same inputs give the same numbers"
        );

        let geometry = bindings(&pack, &opaque_kind(TerrainTier::Direct));
        let found = geometry
            .iter()
            .find(|b| b.resource == Resource::EntityTexture)
            .expect("declared");
        assert_eq!((found.name.as_str(), found.binding), ("entity_texture", 11));
        assert_eq!(
            geometry.iter().find(|b| b.name == "lit").map(|b| b.binding),
            Some(12)
        );
        assert!(!list.iter().any(|b| b.resource == Resource::EntityTexture));
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

    fn test_seam(input: Input, _: TerrainTier) -> Option<SeamPart> {
        let text = match input {
            Input::Sky => {
                "fn torch_sky_base(v: torch_sky_varyings) -> Surface {\n    \
                 return Surface(vec4<f32>(normalize(v.position), 1.0), vec3<f32>(0.0, 1.0, 0.0), vec2<f32>(1.0), 0.0, 1.0, 0.0, 0u, vec4<f32>(0.0));\n}\n"
            }
            Input::Terrain => {
                "fn torch_terrain_base(v: torch_terrain_varyings) -> Surface {\n    \
                 return Surface(textureSample(block_atlas, nearest_clamp, v.uv) * v.color, v.normal, v.light, 0.0, 1.0, 0.0, 0u, vec4<f32>(0.0));\n}\n"
            }
            Input::Entity => {
                "fn torch_entity_base(v: torch_entity_varyings) -> Surface {\n    \
                 return Surface(v.color, v.normal, v.light, 0.0, 1.0, 0.0, 0u, v.overlay);\n}\n"
            }
            _ => return None,
        };
        Some(SeamPart {
            shared: None,
            text: text.to_owned(),
            stages: &[],
        })
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

    fn opaque_kind(terrain: TerrainTier) -> ModuleKind {
        ModuleKind::Geometry {
            group: Group::Opaque,
            writes: vec![1, 2],
            stages: vec![Stage::Terrain],
            vertex: false,
            terrain,
        }
    }

    #[test]
    fn geometry_fragment_entries_validate() {
        let pack = test_pack();
        let kind = opaque_kind(TerrainTier::Indirect);
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
        let (module, _) = validate(composed.text());
        for name in &fragments {
            assert!(composed.entry_points().contains(name), "{name}");
        }
        none(&check_signatures(&module, &composed, &pack, &kind));
        none(&check_rules(&module, &composed));
    }

    #[test]
    fn geometry_entry_points_are_listed_in_order() {
        let composed = build(OPAQUE, &opaque_kind(TerrainTier::Direct));
        assert_eq!(
            composed.entry_points(),
            [
                "torch_sky_vertex",
                "torch_terrain_vertex",
                "torch_entity_vertex",
                "torch_hand_vertex",
                "torch_sky_fragment",
                "torch_terrain_fragment",
                "torch_terrain_cutout_fragment",
                "torch_entity_fragment",
                "torch_entity_cutout_fragment",
                "torch_block_entity_fragment",
                "torch_block_entity_cutout_fragment",
                "torch_hand_fragment",
                "torch_hand_cutout_fragment",
            ]
        );
        for stage in Stage::ALL {
            let vertex = stage_vertex_entry(stage);
            assert_eq!(vertex == HAND_VERTEX, stage == Stage::Hand, "{vertex}");
        }
    }

    #[test]
    fn every_seam_validates_in_both_tiers() {
        let pack = test_pack();
        let encode =
            "fn encode(s: Surface, at: FragmentInfo) -> vec4<f32> {\n    return s.color;\n}\n";
        for terrain in TerrainTier::ALL {
            for group in [Group::Opaque, Group::Translucent] {
                let kind = ModuleKind::Geometry {
                    group,
                    writes: vec![1],
                    stages: Vec::new(),
                    vertex: false,
                    terrain,
                };
                let composed = build(encode, &kind);
                let (module, info) = validate(composed.text());
                none(&check_rules(&module, &composed));
                for stage in group.stages() {
                    let vertex = stage_vertex_entry(stage);
                    assert!(
                        module.entry_points.iter().any(|e| e.name == vertex),
                        "{vertex}"
                    );
                    assert!(composed.entry_points().contains(&fragment_entry(stage)));
                }
                let names: Vec<&str> = composed.entry_points().iter().map(String::as_str).collect();
                let used: Vec<&str> = used_globals(&module, &info, &composed, &names)
                    .iter()
                    .map(|b| b.name.as_str())
                    .collect();
                let expected = [
                    "frame",
                    "linear_clamp",
                    "nearest_clamp",
                    "block_atlas",
                    "lightmap",
                    "entity_texture",
                ];
                assert_eq!(used, expected, "{group:?}");
                assert!(
                    !used.contains(&"shadow_depth") && !used.contains(&"shadow_compare"),
                    "{group:?}"
                );
                let storage = composed.text().contains("var<storage");
                assert_eq!(storage, terrain == TerrainTier::Indirect, "{terrain:?}");
                if terrain == TerrainTier::Direct {
                    naga::valid::Validator::new(
                        naga::valid::ValidationFlags::all(),
                        naga::valid::Capabilities::empty(),
                    )
                    .validate(&module)
                    .unwrap_or_else(|e| panic!("{}", e.emit_to_string(composed.text())));
                }
            }
        }
    }

    #[test]
    fn the_seam_files_agree_with_the_constants() {
        let slot_table = format!("@group({TERRAIN_GROUP}) @binding({SLOT_TABLE_BINDING})");
        assert!(
            TERRAIN_DIRECT_WGSL
                .contains(&format!("{slot_table} var torch_origins: texture_2d<i32>;"))
        );
        assert!(
            TERRAIN_DIRECT_WGSL.contains(&format!("const torch_origin_row: i32 = {ORIGIN_ROW};"))
        );
        assert!(TERRAIN_INDIRECT_WGSL.contains(&format!(
            "{slot_table} var<storage, read> torch_slot_table:"
        )));
        assert!(
            TERRAIN_INDIRECT_WGSL
                .contains(&format!("const torch_slot_words: u32 = {SLOT_WORDS}u;"))
        );
        assert!(TERRAIN_WGSL.contains(&format!(
            "@group({PACK_QUADS_GROUP}) @binding(0) var torch_pack_quads: texture_2d<u32>;"
        )));
        assert!(TERRAIN_WGSL.contains(&format!("const torch_quad_row: u32 = {QUAD_ROW}u;")));
    }

    fn statements(block: &naga::Block) -> Vec<&naga::Statement> {
        let mut out = Vec::new();
        for statement in block.iter() {
            out.push(statement);
            match statement {
                naga::Statement::Block(inner) => out.extend(statements(inner)),
                naga::Statement::If { accept, reject, .. } => {
                    out.extend(statements(accept));
                    out.extend(statements(reject));
                }
                _ => {}
            }
        }
        out
    }

    #[test]
    fn a_cutout_entry_discards() {
        for group in [Group::Opaque, Group::Translucent] {
            let kind = ModuleKind::Geometry {
                group,
                writes: vec![1],
                stages: Vec::new(),
                vertex: false,
                terrain: TerrainTier::Indirect,
            };
            let composed = build(
                "fn encode(s: Surface, at: FragmentInfo) -> vec4<f32> {\n    return s.color;\n}\n",
                &kind,
            );
            let (module, _) = validate(composed.text());
            let kills = |name: &str| {
                let entry = module
                    .entry_points
                    .iter()
                    .find(|e| e.name == name)
                    .unwrap_or_else(|| panic!("no {name}"));
                statements(&entry.function.body)
                    .iter()
                    .any(|s| matches!(s, naga::Statement::Kill))
            };
            for stage in group.stages() {
                assert!(!kills(&fragment_entry(stage)), "{stage:?}");
                let cutout = cutout_entry(stage);
                assert_eq!(
                    composed.entry_points().contains(&cutout),
                    stage.draw().cutout,
                    "{stage:?}"
                );
                if stage.draw().cutout {
                    assert!(kills(&cutout), "{stage:?}");
                }
            }
        }
        assert!(Stage::Terrain.draw().cutout && !Stage::Water.draw().cutout);
    }

    #[test]
    fn a_group_with_displace_moves_the_terrain() {
        let text = "fn encode(s: Surface, at: FragmentInfo) -> vec4<f32> {\n    return s.color;\n}\n\
                    fn displace(in: VertexInput) -> vec3<f32> {\n    \
                        let sway = sin(frame.time.seconds + in.position.x) * 0.05;\n    \
                        return select(vec3<f32>(0.0), vec3<f32>(sway, 0.0, 0.0), in.material == MATERIAL_FOLIAGE);\n\
                    }\n";
        for terrain in TerrainTier::ALL {
            let kind = ModuleKind::Geometry {
                group: Group::Translucent,
                writes: vec![1],
                stages: Vec::new(),
                vertex: true,
                terrain,
            };
            let composed = build(text, &kind);
            let (module, _) = validate(composed.text());
            none(&check_signatures(&module, &composed, &test_pack(), &kind));
            assert!(composed.text().contains("return displace(torch_in);"));
        }
        let still = ModuleKind::Geometry {
            group: Group::Translucent,
            writes: vec![1],
            stages: Vec::new(),
            vertex: false,
            terrain: TerrainTier::Indirect,
        };
        let composed = build(text, &still);
        validate(composed.text());
        assert!(!composed.text().contains("return displace(torch_in);"));
    }

    #[test]
    fn geometry_signatures_are_checked_by_type() {
        let pack = test_pack();
        let kind = ModuleKind::Geometry {
            group: Group::Opaque,
            writes: vec![1, 2],
            stages: vec![Stage::Terrain, Stage::Hand],
            vertex: true,
            terrain: TerrainTier::Indirect,
        };
        let text = "struct Opaque { albedo: vec4<f32>, ids: vec4<f32>, extra: f32 }\n\
                    fn encode(s: Surface, at: FragmentInfo) -> Opaque { return Opaque(s.color, s.color, 0.0); }\n\
                    fn terrain(in: EntityInput) -> Surface { return in.base; }\n\
                    fn displace(in: VertexInput) -> vec4<f32> { return vec4<f32>(0.0); }\n";
        let composed = build(text, &kind);
        let module = parse(composed.without_entries());
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

        let mut format = test_pack();
        format.options.push(PreludeOption {
            id: "format".to_owned(),
            kind: Kind::Choice {
                values: vec!["lit".to_owned(), "dark".to_owned()],
            },
        });
        let problems = check_interface(&format);
        assert_eq!(problems.len(), 1, "{problems:#?}");
        assert!(
            problems[0]
                .message
                .contains("`FORMAT_LIT` is both a format constant"),
            "{problems:#?}"
        );
        assert_eq!(problems[0].owner, Some(Owner::Option(2)));
    }

    #[test]
    fn the_sky_parts_are_prelude_constants() {
        let text = prelude(&test_pack(), &bindings(&test_pack(), &screen_kind(vec![1])));
        assert!(text.contains("const SKY_PART_DISC: u32 = 1u;"), "{text}");
        assert!(text.contains("const SKY_PART_SUN: u32 = 4u;"), "{text}");
        assert!(
            text.contains("const SKY_PART_END_FLASH: u32 = 8u;"),
            "{text}"
        );
        assert_eq!(SkyPart::ALL.map(SkyPart::id), [1, 2, 3, 4, 5, 6, 7, 8]);
        let mut pack = test_pack();
        pack.options.push(PreludeOption {
            id: "sky_part".to_owned(),
            kind: Kind::Choice {
                values: vec!["sun".to_owned(), "moon".to_owned()],
            },
        });
        assert_eq!(check_interface(&pack).len(), 2);
    }

    #[test]
    fn a_format_list_declares_its_choice() {
        let compute = ModuleKind::Compute {
            function: "voxelize".to_owned(),
            writes: vec![0],
            workgroup: [8, 8, 1],
        };
        for kind in [
            screen_kind(vec![1]),
            compute.clone(),
            shadow_kind(true, false, TerrainTier::Indirect),
        ] {
            let text = prelude(&test_pack(), &bindings(&test_pack(), &kind));
            assert!(text.contains("const FORMAT_LIT: u32 = 0u;"), "{text}");
            assert!(
                !text.contains("FORMAT_ALBEDO") && !text.contains("FORMAT_IDS"),
                "{text}"
            );
        }
        let mut pack = test_pack();
        pack.targets[0].chosen = 1;
        let text = prelude(&pack, &bindings(&pack, &compute));
        assert!(text.contains("const FORMAT_LIT: u32 = 1u;"), "{text}");
        assert!(
            text.contains("var lit_out: texture_storage_2d<rgba8unorm, write>;"),
            "{text}"
        );
        let text = prelude(&test_pack(), &bindings(&test_pack(), &compute));
        assert!(
            text.contains("var lit_out: texture_storage_2d<rgba16float, write>;"),
            "{text}"
        );

        let lists = [Format::Rgba16Float, Format::R32Uint];
        assert!(
            PreludeTarget::new("x".to_owned(), lists.to_vec(), 0, false).is_none(),
            "mixed classes"
        );
        assert!(
            PreludeTarget::new("x".to_owned(), vec![Format::Rgba8Unorm], 1, false).is_none(),
            "past the end"
        );
        assert!(
            PreludeTarget::new("x".to_owned(), Vec::new(), 0, false).is_none(),
            "empty"
        );
    }

    fn shadow_kind(color: bool, displace: bool, terrain: TerrainTier) -> ModuleKind {
        ModuleKind::Shadow {
            color,
            displace,
            terrain,
        }
    }

    fn validate_webgl2(module: &naga::Module, text: &str) {
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(text)));
    }

    const SHADOW_COLOUR: &str = "fn shadow(in: TerrainInput) -> vec4<f32> {\n    return vec4<f32>(in.base.color.rgb * (in.block_lit + in.sky_lit), 1.0);\n}\n";

    #[test]
    fn a_depth_only_shadow_module_validates() {
        let read = memory_reader(&[]);
        for terrain in TerrainTier::ALL {
            let kind = shadow_kind(false, false, terrain);
            let composed =
                super::super::compose::compose_files(&test_pack(), &[], &kind, &read).unwrap();
            assert_eq!(composed.file(), super::super::manifest::FILE);
            assert_eq!(composed.files().count(), 0);
            let (module, info) = validate(composed.text());
            validate_webgl2(&module, composed.text());
            none(&check_rules(&module, &composed));
            none(&check_signatures(&module, &composed, &test_pack(), &kind));
            assert_eq!(
                composed.entry_points(),
                [
                    SHADOW_TERRAIN_VERTEX,
                    SHADOW_ENTITY_VERTEX,
                    SHADOW_TERRAIN_CUTOUT,
                    SHADOW_ENTITY_CUTOUT
                ]
            );
            for cutout in [SHADOW_TERRAIN_CUTOUT, SHADOW_ENTITY_CUTOUT] {
                let entry = module
                    .entry_points
                    .iter()
                    .find(|e| e.name == cutout)
                    .expect("declared");
                assert!(entry.function.result.is_none(), "{cutout} writes nothing");
                assert!(
                    statements(&entry.function.body)
                        .iter()
                        .any(|s| matches!(s, naga::Statement::Kill)),
                    "{cutout}"
                );
            }
            let names: Vec<&str> = composed.entry_points().iter().map(String::as_str).collect();
            let used: Vec<&str> = used_globals(&module, &info, &composed, &names)
                .iter()
                .map(|b| b.name.as_str())
                .collect();
            assert_eq!(
                used,
                ["frame", "nearest_clamp", "block_atlas", "entity_texture"],
                "{terrain:?}"
            );
            assert_eq!(
                composed.text().contains("var<storage"),
                terrain == TerrainTier::Indirect
            );
            assert!(composed.text().contains(&format!(
                "@group({SHADOW_DRAW_GROUP}) @binding(0) var<uniform> torch_shadow_draw: vec4<u32>;"
            )));
        }
    }

    #[test]
    fn a_shadow_colour_module_validates() {
        let pack = test_pack();
        for terrain in TerrainTier::ALL {
            let kind = shadow_kind(true, false, terrain);
            let composed = build(SHADOW_COLOUR, &kind);
            assert_eq!(composed.file(), "p.wgsl");
            let (module, info) = validate(composed.text());
            if terrain == TerrainTier::Direct {
                validate_webgl2(&module, composed.text());
            }
            none(&check_rules(&module, &composed));
            none(&check_signatures(&module, &composed, &pack, &kind));
            assert_eq!(
                composed.entry_points(),
                [
                    SHADOW_TERRAIN_VERTEX,
                    SHADOW_ENTITY_VERTEX,
                    SHADOW_TERRAIN_FRAGMENT,
                    SHADOW_TERRAIN_CUTOUT,
                    SHADOW_ENTITY_FRAGMENT,
                    SHADOW_ENTITY_CUTOUT,
                ]
            );
            let names: Vec<&str> = composed.entry_points().iter().map(String::as_str).collect();
            let used: Vec<&str> = used_globals(&module, &info, &composed, &names)
                .iter()
                .map(|b| b.name.as_str())
                .collect();
            assert_eq!(
                used,
                [
                    "frame",
                    "linear_clamp",
                    "nearest_clamp",
                    "block_atlas",
                    "lightmap",
                    "entity_texture"
                ]
            );
        }
        let kind = shadow_kind(true, false, TerrainTier::Indirect);
        let wrong = build(
            "fn shadow(in: TerrainInput) -> Surface {\n    return in.base;\n}\n",
            &kind,
        );
        let module = parse(wrong.without_entries());
        let problems = check_signatures(&module, &wrong, &pack, &kind);
        assert_eq!(lines(&problems), [1], "{problems:#?}");
    }

    #[test]
    fn a_shadow_module_displaces_the_terrain() {
        let text = "fn displace(in: VertexInput) -> vec3<f32> {\n    return vec3<f32>(sin(frame.time.seconds), 0.0, 0.0);\n}\n";
        let kind = shadow_kind(false, true, TerrainTier::Direct);
        let composed = build(text, &kind);
        let (module, _) = validate(composed.text());
        validate_webgl2(&module, composed.text());
        none(&check_signatures(&module, &composed, &test_pack(), &kind));
        assert!(composed.text().contains("return displace(torch_in);"));
        let wrong = build(
            "fn displace(in: VertexInput) -> f32 {\n    return 0.0;\n}\n",
            &kind,
        );
        let module = parse(wrong.without_entries());
        assert_eq!(
            check_signatures(&module, &wrong, &test_pack(), &kind).len(),
            1
        );
    }

    #[test]
    fn the_shadow_functions_bind_only_where_called() {
        let calls = "fn lighting(in: ScreenInput) -> vec4<f32> {\n    \
                         let p = vec3<f32>(in.uv, 1.0);\n    \
                         let sun = shadow_light(p, vec3<f32>(0.0, 1.0, 0.0));\n    \
                         return vec4<f32>(sun, f32(shadow_cascade(p)), 0.0, 1.0);\n}\n";
        let (composed, module) = screen_module(calls);
        let info = validator()
            .validate(&module)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(composed.text())));
        validate_webgl2(&module, composed.text());
        let used: Vec<&str> =
            used_globals(&module, &info, &composed, &[SCREEN_VERTEX, SCREEN_FRAGMENT])
                .iter()
                .map(|b| b.name.as_str())
                .collect();
        assert_eq!(used, ["frame", "shadow_compare", "shadow_depth"]);

        let (composed, module) = screen_module(LIGHTING);
        let info = validator()
            .validate(&module)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(composed.text())));
        let used: Vec<&str> =
            used_globals(&module, &info, &composed, &[SCREEN_VERTEX, SCREEN_FRAGMENT])
                .iter()
                .map(|b| b.name.as_str())
                .collect();
        assert_eq!(used, ["frame"]);
        assert!(
            composed
                .text()
                .contains("fn shadow_light(position: vec3<f32>, normal: vec3<f32>) -> f32 {")
        );
        assert!(
            composed
                .text()
                .contains("var shadow_depth: texture_depth_2d_array;")
        );
    }

    #[test]
    fn shadow_names_are_engine_names() {
        assert!(SHADOW_PASS_WGSL.contains(&format!(
            "@group({SHADOW_DRAW_GROUP}) @binding(0) var<uniform> torch_shadow_draw:"
        )));
        let mut pack = test_pack();
        pack.options.push(PreludeOption {
            id: "shadow_light".to_owned(),
            kind: Kind::Bool,
        });
        let problems = check_interface(&pack);
        assert_eq!(problems.len(), 1, "{problems:#?}");
        assert!(
            problems[0]
                .message
                .contains("`shadow_light` is both a name the engine declares and an option"),
            "{problems:#?}"
        );
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
        assert_eq!(
            names(Input::Vertex.def()),
            [
                "position",
                "uv",
                "mid_uv",
                "normal",
                "light",
                "material",
                "mid_block"
            ]
        );
        assert_eq!(
            names(Input::Terrain.def()),
            [
                "base",
                "position",
                "uv",
                "mid_uv",
                "color",
                "light",
                "normal",
                "tangent",
                "mid_block",
                "block_lit",
                "sky_lit"
            ]
        );
        let terrain_varyings =
            Input::Terrain.def().members.len() - 1 + Input::Terrain.engine_varyings().len();
        assert_eq!(terrain_varyings, 12);
        assert_eq!(
            names(&SURFACE),
            [
                "color",
                "normal",
                "light",
                "emission",
                "roughness",
                "metalness",
                "material",
                "custom"
            ]
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
