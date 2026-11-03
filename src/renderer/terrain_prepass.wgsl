#import bevy_pbr::view_transformations::position_world_to_clip
#import bevy_render::globals::Globals

@group(0) @binding(1) var<uniform> globals: Globals;

#import client::terrain_common::{
    atlas_texture, atlas_sampler, lightmap_texture, lightmap_sampler, params, section_origin, MATERIAL_LEAF,
    MATERIAL_PLANT, WIND_SPEED, wind_offset, wind_frequency, wind_phase, wind_weight
}

const SHADOW_ALPHA_CUTOFF: f32 = 0.5;

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
            wind_phase(material),
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
    return textureSampleLevel(atlas_texture, atlas_sampler, uv, params.misc.z).a
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
