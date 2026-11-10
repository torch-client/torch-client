#define_import_path client::terrain_common

@group(2) @binding(0) var atlas_texture: texture_2d<f32>;
@group(2) @binding(1) var atlas_sampler: sampler;
@group(2) @binding(2) var lightmap_texture: texture_2d<f32>;
@group(2) @binding(3) var lightmap_sampler: sampler;
struct TerrainParams {
    misc: vec4<f32>,
    fog_color: vec4<f32>,
    fog_ramps: vec4<f32>,
    origin: vec4<i32>,
    wave_phase: vec4<f32>,
    wind_plant_phase: vec4<f32>,
    wind_leaf_phase: vec4<f32>,
}
@group(2) @binding(4) var<uniform> params: TerrainParams;

struct SlotMeta { min: vec3<f32>, first_index: u32, max: vec3<f32>, index_count: u32, origin: vec3<i32>, base_vertex: u32, solid_count: u32, flags: u32, pool: u32, cutout_first: u32, faces: array<vec4<u32>, 2>, }
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
    return vec3<f32>(texel.xyz - params.origin.xyz);
#else
    return vec3<f32>(metas[instance_index].origin - params.origin.xyz);
#endif
}

fn decode_tint(rgb: vec3<f32>) -> vec3<f32> {
    let k = rgb * (255.0 / 128.0);
    return k * k;
}

fn sample_lightmap(uv: vec2<f32>) -> vec4<f32> {
    let coord = clamp(uv / 256.0 + 0.5 / 16.0, vec2(0.5 / 16.0), vec2(15.5 / 16.0));
    return textureSampleLevel(lightmap_texture, lightmap_sampler, coord, 0.0);
}

fn vanilla_fog_ramp(distance: f32, ramp: vec2<f32>) -> f32 {
    return saturate((distance - ramp.x) * ramp.y);
}

fn vanilla_fog(color: vec4<f32>, distance: vec2<f32>) -> vec4<f32> {
    let value = max(
        vanilla_fog_ramp(distance.x, params.fog_ramps.xy),
        vanilla_fog_ramp(distance.y, params.fog_ramps.zw),
    );
    let t = value * params.fog_color.a;
    let linear = max(color.rgb, vec3(0.0));
    let encoded = fog_encode(linear);
    let mixed = fog_decode(mix(encoded, params.fog_color.rgb, t)) + (1.0 - t) * (linear - fog_decode(encoded));
    return vec4(mixed, color.a);
}

fn fog_encode(c: vec3<f32>) -> vec3<f32> {
    let r = sqrt(c);
    return r * (1.2563794 + c * 0.2620541) - c * 0.5098001;
}

fn fog_decode(y: vec3<f32>) -> vec3<f32> {
    return y * y * (0.6272203 + y * (0.5902529 - y * 0.2205727));
}

fn encode_srgb(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3(0.0)), vec3(1.0 / 2.2));
}

fn decode_srgb(c: vec3<f32>) -> vec3<f32> {
    return pow(c, vec3(2.2));
}

const MATERIAL_LEAF: f32 = 1.0;
const MATERIAL_PLANT: f32 = 2.0;

const WIND_LEAF: f32 = 0.045;
const WIND_PLANT: f32 = 0.115;
const WIND_SPEED: f32 = 1.0;

const WIND_FREQ_PLANT: f32 = 0.42;
const WIND_FREQ_LEAF: f32 = 0.05;

fn wind_offset(world_position: vec3<f32>, t: f32, freq: f32, phase: vec3<f32>) -> vec3<f32> {
    let p = world_position * freq;
    let sway = sin(p.x + p.z + phase.x + t * 1.7) + 0.5 * sin(p.x * 1.7 - p.z * 1.1 + phase.y + t * 2.6);
    let lift = cos(p.z * 0.9 - p.x * 0.4 + phase.z + t * 1.3);
    return vec3(sway, lift * 0.25, lift * 0.8);
}

fn wind_frequency(material: f32) -> f32 {
    return select(WIND_FREQ_PLANT, WIND_FREQ_LEAF, material == MATERIAL_LEAF);
}

fn wind_phase(material: f32) -> vec3<f32> {
    return select(params.wind_plant_phase.xyz, params.wind_leaf_phase.xyz, material == MATERIAL_LEAF);
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
