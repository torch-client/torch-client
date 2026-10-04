#import bevy_pbr::mesh_view_bindings::{view, fog}
#import bevy_pbr::view_transformations::position_world_to_clip

#ifdef FANCY_SHADERS
#import bevy_pbr::mesh_view_bindings::{lights, globals}
#import bevy_pbr::mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT
#import bevy_pbr::shadows
#endif

#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping::tone_mapping
#endif

fn vanilla_fog_ramp(distance: f32, start: f32, end: f32) -> f32 {
    if distance <= start {
        return 0.0;
    } else if distance >= end {
        return 1.0;
    }
    return (distance - start) / (end - start);
}

fn vanilla_fog(color: vec4<f32>, rel: vec3<f32>) -> vec4<f32> {
    let spherical = length(rel);
    let cylindrical = max(length(rel.xz), abs(rel.y));
    let environmental = fog.directional_light_color.xy;
    let value = max(
        vanilla_fog_ramp(spherical, environmental.x, environmental.y),
        vanilla_fog_ramp(cylindrical, fog.be.x, fog.be.y),
    );
    let mixed = mix(encode_srgb(color.rgb), encode_srgb(fog.base_color.rgb), value * fog.base_color.a);
    return vec4(decode_srgb(mixed), color.a);
}

fn encode_srgb(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3(0.0)), vec3(1.0 / 2.2));
}

fn decode_srgb(c: vec3<f32>) -> vec3<f32> {
    return pow(c, vec3(2.2));
}

@group(2) @binding(0) var atlas_texture: texture_2d<f32>;
@group(2) @binding(1) var atlas_sampler: sampler;
@group(2) @binding(2) var lightmap_texture: texture_2d<f32>;
@group(2) @binding(3) var lightmap_sampler: sampler;
@group(2) @binding(4) var<uniform> params: vec4<f32>;

struct SlotMeta { min: vec3<f32>, first_index: u32, max: vec3<f32>, index_count: u32, origin: vec3<i32>, base_vertex: u32, solid_count: u32, flags: u32, pool: u32, cutout_first: u32 }
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
const MATERIAL_WATER: f32 = 3.0;
const MATERIAL_LAVA: f32 = 4.0;
const MATERIAL_EMISSIVE_BASE: f32 = 5.0;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec4<i32>,
    @location(1) uv: vec2<f32>,
    @location(2) light: vec4<f32>,
    @location(3) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
#ifdef FANCY_SHADERS
    @location(3) torch: vec3<f32>,
    @location(4) sky: vec3<f32>,
    @location(5) wave_normal: vec3<f32>,
    @location(6) @interpolate(flat) material: f32,
#endif
};

fn sample_lightmap(uv: vec2<f32>) -> vec4<f32> {
    let coord = clamp(uv / 256.0 + 0.5 / 16.0, vec2(0.5 / 16.0), vec2(15.5 / 16.0));
    return textureSampleLevel(lightmap_texture, lightmap_sampler, coord, 0.0);
}

#ifdef FANCY_SHADERS

fn pow4(x: f32) -> f32 {
    let x2 = x * x;
    return x2 * x2;
}

fn pow5(x: f32) -> f32 {
    let x2 = x * x;
    return x2 * x2 * x;
}

const SUN_GAIN: f32 = 1.60;
const AMBIENT_GAIN: f32 = 0.31;

const GROUND_TINT: vec3<f32> = vec3(0.85, 0.80, 0.68);

const AMBIENT_SHADE_MIX: f32 = 0.55;

const BLOCK_GAIN: f32 = 2.20;

const EMISSIVE_GAIN: f32 = 2.60;

const SSS_GAIN: f32 = 2.20;

const ROUGH_BLOCK: f32 = 0.86;
const ROUGH_WATER: f32 = 0.05;

const F0_BLOCK: f32 = 0.04;
const F0_WATER: f32 = 0.02;

const HEIGHT_FOG_Y: f32 = 62.0;
const HEIGHT_FOG_FALLOFF: f32 = 0.045;
const HEIGHT_FOG_DENSITY: f32 = 0.00085;

const FOG_SUN_TINT: f32 = 0.55;

const WIND_LEAF: f32 = 0.045;
const WIND_PLANT: f32 = 0.115;
const WIND_SPEED: f32 = 1.0;

const WIND_FREQ_PLANT: f32 = 0.42;
const WIND_FREQ_LEAF: f32 = 0.05;

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

fn wave_height(xz: vec2<f32>, t: f32) -> f32 {
    let a = sin(xz.x * 0.55 + t * 1.30) + sin(xz.y * 0.42 - t * 0.90);
    let b = sin((xz.x + xz.y) * 0.29 + t * 0.65);
    return (a * 0.5 + b) * 0.028;
}

fn wave_surface_normal(xz: vec2<f32>, t: f32) -> vec3<f32> {
    let dx = (cos(xz.x * 0.55 + t * 1.30) * 0.55 * 0.5
        + cos((xz.x + xz.y) * 0.29 + t * 0.65) * 0.29) * 0.028;
    let dz = (cos(xz.y * 0.42 - t * 0.90) * 0.42 * 0.5
        + cos((xz.x + xz.y) * 0.29 + t * 0.65) * 0.29) * 0.028;
    return normalize(vec3(-dx, 1.0, -dz));
}
#endif

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    out.world_position = vec4(
        section_origin(vertex.instance_index, vertex.position.w)
            + vec3<f32>(vertex.position.xyz) / 256.0, 1.0);

#ifdef FANCY_SHADERS
    let material = round(vertex.light.z * 255.0);
    out.material = material;
    out.wave_normal = vec3(0.0);
    if material == MATERIAL_WATER || material == MATERIAL_LAVA {
        let calm = select(1.0, 0.33, material == MATERIAL_LAVA);
        let speed = select(1.0, 0.5, material == MATERIAL_LAVA);
        let xz = out.world_position.xz;
        let t = globals.time * speed;
        out.world_position.y += wave_height(xz, t) * calm;
        out.wave_normal = wave_surface_normal(xz, t);
    } else {
        let weight = wind_weight(material, vertex.light.w);
        if weight > 0.0 {
            let gust = wind_offset(
                out.world_position.xyz,
                globals.time * WIND_SPEED,
                wind_frequency(material),
            );
            out.world_position = vec4(
                out.world_position.xyz + gust * weight,
                out.world_position.w,
            );
        }
    }
#endif

    out.clip_position = position_world_to_clip(out.world_position.xyz);
    out.uv = vertex.uv;

#ifdef FANCY_SHADERS
    out.color = vertex.color;

    let light_level = vertex.light.xy * 255.0;
    let floor_light = sample_lightmap(vec2(0.0)).rgb;
    let block_light = max(sample_lightmap(vec2(light_level.x, 0.0)).rgb - floor_light, vec3(0.0));
    out.torch = floor_light + block_light * BLOCK_GAIN;
    out.sky = max(sample_lightmap(vec2(0.0, light_level.y)).rgb - floor_light, vec3(0.0));
#else
    let shaded = vec4(vertex.color.rgb * vertex.color.a, 1.0);
    out.color = shaded * sample_lightmap(vertex.light.xy * 255.0);
#endif

    return out;
}

#ifdef FANCY_SHADERS
fn face_normal(world_position: vec3<f32>, to_view: vec3<f32>) -> vec3<f32> {
    let n = normalize(cross(dpdx(world_position), dpdy(world_position)));
    return select(-n, n, dot(n, to_view) > 0.0);
}

fn specular_ggx(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32, f0: f32) -> f32 {
    let h = normalize(v + l);
    let n_o_h = saturate(dot(n, h));
    let n_o_v = saturate(dot(n, v)) + 1e-5;
    let n_o_l = saturate(dot(n, l));
    let v_o_h = saturate(dot(v, h));

    let a = roughness * roughness;
    let a2 = a * a;
    let d_denom = n_o_h * n_o_h * (a2 - 1.0) + 1.0;
    let d = a2 / (3.14159265 * d_denom * d_denom);

    let lambda_v = n_o_l * (n_o_v * (1.0 - a) + a);
    let lambda_l = n_o_v * (n_o_l * (1.0 - a) + a);
    let vis = 0.5 / max(lambda_v + lambda_l, 1e-4);

    let f = f0 + (1.0 - f0) * pow5(1.0 - v_o_h);
    return d * vis * f * n_o_l;
}
fn emissive_mask(albedo: vec3<f32>) -> f32 {
    let value = max(max(albedo.r, albedo.g), albedo.b);
    let chroma = value - min(min(albedo.r, albedo.g), albedo.b);
    let saturation = chroma / max(value, 1e-5);

    let saturated = saturate(saturation * 3.125 - 0.125) * pow4(value);
    let desaturated = saturate(value * 7.0 - 6.0);
    return max(saturated, desaturated);
}

fn fog_color(view_dir: vec3<f32>, sun_color: vec3<f32>, to_light: vec3<f32>, has_sun: bool) -> vec3<f32> {
    let base = fog.base_color.rgb;
    if !has_sun {
        return base;
    }
    let toward_sun = saturate(dot(view_dir, to_light));
    let near_horizon = 1.0 - abs(view_dir.y);
    let mix_amount = pow4(toward_sun) * near_horizon * FOG_SUN_TINT;
    return mix(base, sun_color * max(base, vec3(0.25)), mix_amount);
}

#endif

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSampleBias(atlas_texture, atlas_sampler, in.uv, params.y);
#ifdef FANCY_SHADERS
    var color = vec4(texel.rgb * in.color.rgb, texel.a);
#else
    var color = texel * in.color;
#endif
#ifdef ALPHA_CUTOUT
    if color.a < params.x {
        discard;
    }
#endif
#ifndef WATER
    color.a = 1.0;
#endif

    let world_position = in.world_position.xyz;
    let view_distance = length(world_position - view.world_position);

#ifdef FANCY_SHADERS
    let water = in.material == MATERIAL_WATER;
    let lava = in.material == MATERIAL_LAVA;
    let foliage = in.material == MATERIAL_LEAF || in.material == MATERIAL_PLANT;
    var emission_level = saturate((in.material - MATERIAL_EMISSIVE_BASE) / 15.0);
    if lava {
        emission_level = 1.0;
    }
    let v = normalize(view.world_position - world_position);
    let geometric = face_normal(world_position, v);
    let level = abs(geometric.y) > 0.5;
    let wave = select(-in.wave_normal, in.wave_normal, dot(in.wave_normal, v) > 0.0);
    let n = select(geometric, wave, (water || lava) && level);

    let sky_light = in.sky;

    var sun_visibility = 0.0;
    var sun_shadow = 0.0;
    var sun_color = vec3(0.0);
    var to_light = vec3(0.0, 1.0, 0.0);
    var has_sun = false;
    if lights.n_directional_lights > 0u {
        has_sun = true;
        let light = &lights.directional_lights[0];
        to_light = (*light).direction_to_light;
        sun_color = (*light).color.rgb;
        var n_o_l = dot(n, to_light);
        if foliage {
            n_o_l = saturate(n_o_l * 0.5 + 0.5);
        } else {
            n_o_l = saturate(n_o_l);
        }
        if n_o_l > 0.0 {
            var shadow = 1.0;
            if ((*light).flags & DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
                let view_z = dot(vec4(
                    view.view_from_world[0].z,
                    view.view_from_world[1].z,
                    view.view_from_world[2].z,
                    view.view_from_world[3].z,
                ), in.world_position);
                shadow = shadows::fetch_directional_shadow(
                    0u, in.world_position, geometric, view_z);
            }
            sun_shadow = shadow;
            sun_visibility = n_o_l * shadow;
        }
    }

    let hemisphere = mix(GROUND_TINT, lights.ambient_color.rgb, n.y * 0.5 + 0.5);
    let ambient_shade = mix(1.0, in.color.a, AMBIENT_SHADE_MIX);
    let ambient = sky_light * AMBIENT_GAIN * hemisphere * ambient_shade;
    let direct = sky_light * SUN_GAIN * sun_color * sun_visibility;
    let lighting = in.torch * in.color.a + ambient + direct;

    var lit = color.rgb * lighting;

    if emission_level > 0.0 {
        let mask = emissive_mask(texel.rgb);
        lit += mix(texel.rgb, vec3(1.0), 0.30) * mask * emission_level * EMISSIVE_GAIN;
    }

    if foliage && has_sun {
        let through = pow5(saturate(dot(-v, to_light)));
        lit += color.rgb * sun_color * sky_light * through * sun_shadow * SSS_GAIN;
    }

    let f0 = select(F0_BLOCK, F0_WATER, water);
#ifdef SUN_SPECULAR
    let roughness = select(ROUGH_BLOCK, ROUGH_WATER, water);
    if sun_visibility > 0.0 {
        lit += sun_color * sky_light * sun_shadow * specular_ggx(n, v, to_light, roughness, f0);
    }
#endif

    if water {
        let sky_access = saturate(max(max(sky_light.r, sky_light.g), sky_light.b));
        let fresnel = f0 + (1.0 - f0) * pow5(1.0 - saturate(dot(n, v)));
        let reflected = fresnel * 0.85 * sky_access;
        lit = mix(lit, fog.base_color.rgb * sky_light, reflected);
        color.a = mix(color.a, 1.0, reflected);
    }

    color = vec4(lit, color.a);

    let height_density = exp(-max(world_position.y - HEIGHT_FOG_Y, 0.0) * HEIGHT_FOG_FALLOFF);
    let ground_fog = 1.0 - exp(-view_distance * HEIGHT_FOG_DENSITY * height_density);
    color = vec4(mix(color.rgb, fog_color(-v, sun_color, to_light, has_sun), ground_fog), color.a);
#endif

    if fog.mode != 0u {
        color = vanilla_fog(color, world_position - view.world_position);
    }
#ifdef TONEMAP_IN_SHADER
    color = tone_mapping(color, view.color_grading);
#endif
    return color;
}
