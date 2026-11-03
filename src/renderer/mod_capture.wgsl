#import client::terrain_common::{
    atlas_texture, atlas_sampler, params, section_origin, sample_lightmap, vanilla_fog, decode_tint
}

struct Capture {
    clip_from_world: mat4x4<f32>,
    eye: vec4<f32>,
    forward: vec4<f32>,
    mode: vec4<u32>,
    day: array<vec4<f32>, 16>,
}
@group(0) @binding(0) var<uniform> capture: Capture;

const LIGHT_WORLD: u32 = 0u;
const LIGHT_DAY: u32 = 1u;
const LEVEL_STEP: f32 = 16.0;
const TOP_LEVEL: f32 = 15.0;
const DEPTH_MAX: f32 = 65535.0;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec4<i32>,
    @location(1) uv: vec2<f32>,
    @location(2) light: vec4<f32>,
    @location(3) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) rel: vec3<f32>,
};

struct Targets {
    @location(0) color: vec4<f32>,
#ifdef DEPTH_TARGET
    @location(1) depth: u32,
#endif
};

fn day_light(block: f32) -> vec3<f32> {
    let level = clamp(block / LEVEL_STEP, 0.0, TOP_LEVEL);
    let lo = u32(floor(level));
    let hi = min(lo + 1u, u32(TOP_LEVEL));
    return mix(capture.day[lo].rgb, capture.day[hi].rgb, fract(level));
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_position = section_origin(vertex.instance_index, vertex.position.w)
        + vec3<f32>(vertex.position.xyz) / 256.0;
    out.clip_position = capture.clip_from_world * vec4(world_position, 1.0);
    out.uv = vertex.uv;
    out.rel = world_position - capture.eye.xyz;
    let shaded = vec4(decode_tint(vertex.color.rgb) * vertex.color.a, 1.0);
    var light = vec4(1.0);
    if capture.mode.x == LIGHT_WORLD {
        light = sample_lightmap(vertex.light.xy * 255.0);
    } else if capture.mode.x == LIGHT_DAY {
        light = vec4(day_light(vertex.light.x * 255.0), 1.0);
    }
    out.color = shaded * light;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> Targets {
    var out: Targets;
    var color = textureSample(atlas_texture, atlas_sampler, in.uv) * in.color;
#ifdef ALPHA_CUTOUT
    if color.a < params.misc.x {
        discard;
    }
#endif
#ifndef WATER
    color.a = 1.0;
#endif
    if capture.mode.y != 0u {
        color = vanilla_fog(color, vec2(length(in.rel), max(length(in.rel.xz), abs(in.rel.y))));
    }
    out.color = color;
#ifdef DEPTH_TARGET
    let along = (dot(in.rel, capture.forward.xyz) - capture.eye.w) / (capture.forward.w - capture.eye.w);
    out.depth = u32(round((1.0 - saturate(along)) * DEPTH_MAX));
#endif
    return out;
}

@vertex
fn clear_vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4(uv * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn clear_fragment() -> Targets {
    var out: Targets;
    out.color = vec4(0.0);
#ifdef DEPTH_TARGET
    out.depth = 0u;
#endif
    return out;
}
