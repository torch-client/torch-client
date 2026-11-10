#import bevy_pbr::mesh_functions
#import bevy_pbr::mesh_view_bindings::view
#ifdef DISTANCE_FOG
#import bevy_pbr::mesh_view_bindings::fog
#endif
#import bevy_pbr::view_transformations::position_world_to_clip

#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping::tone_mapping
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var sky_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var sky_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var portal_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var portal_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<uniform> params: vec4<f32>;

var<private> COLORS: array<vec3<f32>, 16> = array<vec3<f32>, 16>(
    vec3(0.022087, 0.098399, 0.110818),
    vec3(0.011892, 0.095924, 0.089485),
    vec3(0.027636, 0.101689, 0.100326),
    vec3(0.046564, 0.109883, 0.114838),
    vec3(0.064901, 0.117696, 0.097189),
    vec3(0.063761, 0.086895, 0.123646),
    vec3(0.084817, 0.111994, 0.166380),
    vec3(0.097489, 0.154120, 0.091064),
    vec3(0.106152, 0.131144, 0.195191),
    vec3(0.097721, 0.110188, 0.187229),
    vec3(0.133516, 0.138278, 0.148582),
    vec3(0.070006, 0.243332, 0.235792),
    vec3(0.196766, 0.142899, 0.214696),
    vec3(0.047281, 0.315338, 0.321970),
    vec3(0.204675, 0.390010, 0.302066),
    vec3(0.080955, 0.314821, 0.661491),
);

const SCALE_TRANSLATE: mat4x4<f32> = mat4x4<f32>(
    0.5, 0.0, 0.0, 0.25,
    0.0, 0.5, 0.0, 0.25,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0,
);

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) tex_proj: vec4<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4(vertex.position, 1.0));
    out.world_position = world.xyz;
    out.clip_position = position_world_to_clip(world.xyz);
    let clip = out.clip_position;
    out.tex_proj = vec4((clip.x + clip.w) * 0.5, (clip.y + clip.w) * 0.5, clip.z, clip.w);
    return out;
}

fn rotate_z(radians: f32) -> mat2x2<f32> {
    return mat2x2<f32>(cos(radians), -sin(radians), sin(radians), cos(radians));
}

fn end_portal_layer(layer: f32) -> mat4x4<f32> {
    let translate = mat4x4<f32>(
        1.0, 0.0, 0.0, 17.0 / layer,
        0.0, 1.0, 0.0, (2.0 + layer / 1.5) * (params.x * 1.5),
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    );
    let rotate = rotate_z(radians((layer * layer * 4321.0 + layer * 9.0) * 2.0));
    let scale = (4.5 - layer / 4.0) * 2.0;
    let m = mat2x2<f32>(scale, 0.0, 0.0, scale) * rotate;
    let embedded = mat4x4<f32>(
        vec4(m[0], 0.0, 0.0),
        vec4(m[1], 0.0, 0.0),
        vec4(0.0, 0.0, 1.0, 0.0),
        vec4(0.0, 0.0, 0.0, 1.0),
    );
    return embedded * translate * SCALE_TRANSLATE;
}

fn sample_proj(t: texture_2d<f32>, s: sampler, coord: vec4<f32>) -> vec3<f32> {
    return textureSampleLevel(t, s, coord.xy / coord.w, 0.0).rgb;
}

fn srgb_encode(c: vec3<f32>) -> vec3<f32> {
    let low = c * 12.92;
    let high = 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055;
    return select(high, low, c <= vec3(0.0031308));
}

fn srgb_decode(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3(2.4));
    return select(high, low, c <= vec3(0.04045));
}

fn vanilla_fog_ramp(distance: f32, start: f32, end: f32) -> f32 {
    if distance <= start {
        return 0.0;
    } else if distance >= end {
        return 1.0;
    }
    return (distance - start) / (end - start);
}

#ifdef DISTANCE_FOG
fn vanilla_fog(color: vec3<f32>, rel: vec3<f32>) -> vec3<f32> {
    let spherical = length(rel);
    let cylindrical = max(length(rel.xz), abs(rel.y));
    let environmental = fog.directional_light_color.xy;
    let value = max(
        vanilla_fog_ramp(spherical, environmental.x, environmental.y),
        vanilla_fog_ramp(cylindrical, fog.be.x, fog.be.y),
    );
    let mixed = mix(srgb_encode(color), srgb_encode(fog.base_color.rgb), value * fog.base_color.a);
    return srgb_decode(mixed);
}
#endif

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = sample_proj(sky_texture, sky_sampler, in.tex_proj) * COLORS[0];
    let layers = i32(params.y);
    for (var i = 0; i < layers; i++) {
        let coord = in.tex_proj * end_portal_layer(f32(i + 1));
        color += sample_proj(portal_texture, portal_sampler, coord) * COLORS[i];
    }
    var rgb = srgb_decode(color);
#ifdef DISTANCE_FOG
    if fog.mode != 0u {
        rgb = vanilla_fog(rgb, in.world_position - view.world_position);
    }
#endif
    var out = vec4(rgb, 1.0);
#ifdef TONEMAP_IN_SHADER
#ifndef TONEMAP_METHOD_NONE
    out = tone_mapping(out, view.color_grading);
#endif
#endif
    return out;
}
