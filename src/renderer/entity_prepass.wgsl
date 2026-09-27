#import bevy_pbr::mesh_functions
#import bevy_pbr::view_transformations::position_world_to_clip

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var entity_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var entity_sampler: sampler;
struct Params {
    tint: vec4<f32>,
    light: vec4<f32>,
};
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> params: Params;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    @location(1) unclipped_depth: f32,
#endif
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world_position =
        mesh_functions::mesh_position_local_to_world(world_from_local, vec4(vertex.position, 1.0));
    out.position = position_world_to_clip(world_position.xyz);
    out.uv = vertex.uv;

#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
#endif
    return out;
}

fn casts(uv: vec2<f32>) -> bool {
    return textureSampleLevel(entity_texture, entity_sampler, uv, 0.0).a >= params.light.a;
}

#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
@fragment
fn fragment(in: VertexOutput) -> @builtin(frag_depth) f32 {
    if !casts(in.uv) {
        discard;
    }
    return in.unclipped_depth;
}
#else
@fragment
fn fragment(in: VertexOutput) {
    if !casts(in.uv) {
        discard;
    }
}
#endif
