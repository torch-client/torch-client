#import bevy_pbr::mesh_functions
#import bevy_pbr::mesh_view_bindings::{view, globals}
#import bevy_pbr::view_transformations::position_world_to_clip

#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping::tone_mapping
#endif

struct BorderParams {
    bounds: vec4<f32>,
    tint: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var forcefield_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var forcefield_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> params: BorderParams;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) wall: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) alpha: f32,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let cam = view.world_position;
    let b = params.bounds;
    let rd = params.tint.a;
    var distances = vec4(b.w - cam.z, cam.x - b.x, cam.z - b.z, b.y - cam.x);
    if distances[vertex.wall] >= rd {
        out.clip_position = vec4(2.0, 2.0, 2.0, 1.0);
        return out;
    }
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4(vertex.position, 1.0));
    out.clip_position = position_world_to_clip(world.xyz);

    out.uv = vertex.uv + fract(globals.time / 3.0);

    let nearest = min(min(distances.x, distances.y), min(distances.z, distances.w));
    out.alpha = clamp(pow(max(1.0 - nearest / rd, 0.0), 4.0), 0.0, 1.0);
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(forcefield_texture, forcefield_sampler, in.uv);
    if texel.a == 0.0 {
        discard;
    }
    var color = texel * vec4(params.tint.rgb, in.alpha);
#ifdef TONEMAP_IN_SHADER
#ifndef TONEMAP_METHOD_NONE
    color = tone_mapping(color, view.color_grading);
#endif
#endif
    return color;
}
