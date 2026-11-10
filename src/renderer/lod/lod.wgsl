#import bevy_pbr::mesh_functions
#import bevy_pbr::mesh_view_bindings::view
#ifdef DISTANCE_FOG
#import bevy_pbr::mesh_view_bindings::fog
#endif
#import bevy_pbr::view_transformations::position_world_to_clip
#import client::vanilla_fog::{srgb_encode, srgb_decode, vanilla_fog_ramp}

#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping::tone_mapping
#endif

struct LodParams {
    clip: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: LodParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var lightmap_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var lightmap_sampler: sampler;

const MICRO_OFFSET: f32 = 0.01;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) packed: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) @interpolate(flat) inward: vec3<f32>,
};

fn face_normal(face: u32) -> vec3<f32> {
    if face == 0u { return vec3(0.0, -1.0, 0.0); }
    if face == 1u { return vec3(0.0, 1.0, 0.0); }
    if face == 2u { return vec3(0.0, 0.0, -1.0); }
    if face == 3u { return vec3(0.0, 0.0, 1.0); }
    if face == 4u { return vec3(-1.0, 0.0, 0.0); }
    return vec3(1.0, 0.0, 0.0);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return srgb_decode(c);
}

fn sample_lightmap(uv: vec2<f32>) -> vec4<f32> {
    let coord = clamp(uv / 256.0 + 0.5 / 16.0, vec2(0.5 / 16.0), vec2(15.5 / 16.0));
    return textureSampleLevel(lightmap_texture, lightmap_sampler, coord, 0.0);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    var local_position = vertex.position;

    let micro = (vertex.packed >> 11u) & 63u;
    var mx = select(0.0, MICRO_OFFSET, (micro & 1u) != 0u);
    mx = select(mx, -mx, (micro & 2u) != 0u);
    var mz = select(0.0, MICRO_OFFSET, (micro & 16u) != 0u);
    mz = select(mz, -mz, (micro & 32u) != 0u);
    local_position.x += mx;
    local_position.z += mz;

    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4(local_position, 1.0));
    out.clip_position = position_world_to_clip(world_position.xyz);
    out.world_position = world_position.xyz;
    out.inward = -0.5 * face_normal((vertex.packed >> 8u) & 7u);

    let sky_light = f32(vertex.packed & 15u);
    let block_light = f32((vertex.packed >> 4u) & 15u);
    let light = sample_lightmap(vec2(block_light, sky_light) * 16.0).rgb;
    out.color = vec4(srgb_to_linear(vertex.color.rgb) * light, vertex.color.a);
    return out;
}

#ifdef DISTANCE_FOG
fn far_fog(distance: f32) -> f32 {
    let start = fog.be.x;
    let band = max(fog.be.y - fog.be.x, 1.0);
    let x = max((distance - start) / band, 0.0) * 2.5;
    return 1.0 - 1.0 / exp(x * x);
}
#endif

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let section_min = floor((in.world_position + in.inward) / 16.0) * 16.0;
    let eye = view.world_position;
    let d = clamp(eye, section_min, section_min + vec3(16.0)) - eye;
    if max(length(d.xz), abs(d.y)) <= params.clip.x {
        discard;
    }

    var color = in.color;
#ifdef DISTANCE_FOG
    let rel = in.world_position - view.world_position;
    if fog.mode != 0u {
        let environmental = fog.directional_light_color.xy;
        let value = max(
            vanilla_fog_ramp(length(rel), environmental.x, environmental.y),
            far_fog(length(rel.xz)),
        );
        let mixed = mix(srgb_encode(color.rgb), srgb_encode(fog.base_color.rgb), value * fog.base_color.a);
        color = vec4(srgb_decode(mixed), color.a);
    }
#endif
#ifdef TONEMAP_IN_SHADER
#ifndef TONEMAP_METHOD_NONE
    color = tone_mapping(color, view.color_grading);
#endif
#endif
    return color;
}
