#import bevy_pbr::view_transformations::position_world_to_clip
#import bevy_render::globals::Globals

@group(0) @binding(1) var<uniform> globals: Globals;

@group(2) @binding(0) var atlas_texture: texture_2d<f32>;
@group(2) @binding(1) var atlas_sampler: sampler;
@group(2) @binding(2) var lightmap_texture: texture_2d<f32>;
@group(2) @binding(3) var lightmap_sampler: sampler;
@group(2) @binding(4) var<uniform> params: vec4<f32>;

struct SlotMeta { min: vec3<f32>, first_index: u32, max: vec3<f32>, index_count: u32, origin: vec3<i32>, base_vertex: u32, solid_count: u32, flags: u32, pool: u32, pad: u32 }
#ifdef META_TEXTURE
const ORIGIN_ROW: i32 = 256;
@group(2) @binding(5) var origins: texture_2d<i32>;
#else
@group(2) @binding(5) var<storage, read> metas: array<SlotMeta>;
#endif
fn section_origin(instance_index: u32, slot_lane: i32) -> vec3<f32> {
#ifdef META_TEXTURE
    let slot = slot_lane & 0xFFFF;
    let texel = textureLoad(origins, vec2<i32>(slot % ORIGIN_ROW, slot / ORIGIN_ROW), 0);
    return vec3<f32>(texel.xyz);
#else
    return vec3<f32>(metas[instance_index].origin);
#endif
}

const MATERIAL_LEAF: f32 = 1.0;
const MATERIAL_PLANT: f32 = 2.0;

const SHADOW_ALPHA_CUTOFF: f32 = 0.5;
const WIND_LEAF: f32 = 0.045;
const WIND_PLANT: f32 = 0.115;
const WIND_SPEED: f32 = 1.0;
const WIND_FREQ_PLANT: f32 = 0.42;
const WIND_FREQ_LEAF: f32 = 0.05;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec4<i32>,
    @location(1) uv: vec2<f32>,
    @location(2) light: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) material: f32,
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    @location(6) unclipped_depth: f32,
#endif
};

fn wind_offset(world_position: vec3<f32>, t: f32, freq: f32) -> vec3<f32> {
    let p = world_position * freq;
    let sway = sin(p.x + p.z + t * 1.7) + 0.5 * sin(p.x * 1.7 - p.z * 1.1 + t * 2.6);
    let lift = cos(p.z * 0.9 - p.x * 0.4 + t * 1.3);
    return vec3(sway, lift * 0.25, lift * 0.8);
}

fn wind_frequency(material: f32) -> f32 {
    return select(WIND_FREQ_PLANT, WIND_FREQ_LEAF, material == MATERIAL_LEAF);
}

fn wind_weight(material: f32, anchor: f32) -> f32 {
    if material == MATERIAL_PLANT {
        return WIND_PLANT * anchor;
    }
    if material == MATERIAL_LEAF {
        return WIND_LEAF;
    }
    return 0.0;
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    var world_position = vec4(
        section_origin(vertex.instance_index, vertex.position.w)
            + vec3<f32>(vertex.position.xyz) / 256.0, 1.0);

    let material = round(vertex.light.z * 255.0);
    let weight = wind_weight(material, vertex.light.w);
    if weight > 0.0 {
        let gust = wind_offset(
            world_position.xyz,
            globals.time * WIND_SPEED,
            wind_frequency(material),
        );
        world_position = vec4(world_position.xyz + gust * weight, world_position.w);
    }

    out.position = position_world_to_clip(world_position.xyz);
    out.uv = vertex.uv;
    out.material = material;

    if material == MATERIAL_PLANT {
        out.position = vec4(2.0, 2.0, 2.0, 1.0);
    }

#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
#endif
    return out;
}

fn casts(uv: vec2<f32>, material: f32) -> bool {
    if material == MATERIAL_PLANT {
        return false;
    }
    return textureSampleLevel(atlas_texture, atlas_sampler, uv, params.z).a
        >= SHADOW_ALPHA_CUTOFF;
}

#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
@fragment
fn fragment(in: VertexOutput) -> @builtin(frag_depth) f32 {
    if !casts(in.uv, in.material) {
        discard;
    }
    return in.unclipped_depth;
}
#else
@fragment
fn fragment(in: VertexOutput) {
    if !casts(in.uv, in.material) {
        discard;
    }
}
#endif
