#import bevy_pbr::mesh_functions
#import bevy_pbr::mesh_view_bindings::{view, fog, globals}
#import bevy_pbr::view_transformations::position_world_to_clip
#import client::vanilla_fog::vanilla_fog

#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping::tone_mapping
#endif

struct WeatherParams {
    fade: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var rain_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var rain_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var snow_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var snow_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var lightmap_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var lightmap_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var<uniform> params: WeatherParams;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) column: vec4<f32>,
    @location(3) packed: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) world_position: vec3<f32>,
    @location(3) @interpolate(flat) snow: u32,
};

fn sample_lightmap(block: u32, sky: u32) -> vec4<f32> {
    let coord = (vec2(f32(block), f32(sky)) + 0.5) / 16.0;
    return textureSampleLevel(lightmap_texture, lightmap_sampler, coord, 0.0);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4(vertex.position, 1.0));
    let centre = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4(vertex.column.x, 0.0, vertex.column.y, 1.0),
    );
    out.clip_position = position_world_to_clip(world.xyz);
    out.world_position = world.xyz;

    let snow = (vertex.packed >> 8u) & 1u;
    out.snow = snow;
    let ticks = globals.time * 20.0;
    var uv = vertex.uv;
    if snow == 0u {
        let offset = -(ticks + vertex.column.z) / 32.0 * vertex.column.w;
        uv.y += offset % 32.0;
    } else {
        uv.x += ticks * vertex.column.z;
        uv.y += ticks * vertex.column.w - (ticks % 512.0) / 512.0;
    }
    out.uv = uv;

    let rel = centre.xz - view.world_position.xz;
    let max_alpha = select(1.0, 0.8, snow == 1u);
    let alpha = mix(max_alpha, 0.5, min(dot(rel, rel) / params.fade.y, 1.0)) * params.fade.x;
    let light = sample_lightmap(vertex.packed & 15u, (vertex.packed >> 4u) & 15u);
    out.color = vec4(light.rgb, alpha);
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var texel: vec4<f32>;
    if in.snow == 0u {
        texel = textureSampleLevel(rain_texture, rain_sampler, in.uv, 0.0);
    } else {
        texel = textureSampleLevel(snow_texture, snow_sampler, in.uv, 0.0);
    }
    var color = texel * in.color;
    if color.a < 0.1 {
        discard;
    }
    if fog.mode != 0u {
        color = vanilla_fog(color, in.world_position - view.world_position);
    }
#ifdef TONEMAP_IN_SHADER
#ifndef TONEMAP_METHOD_NONE
    color = tone_mapping(color, view.color_grading);
#endif
#endif
    return color;
}
