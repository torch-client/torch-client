#import bevy_pbr::view_transformations::position_world_to_clip

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var atlas_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var atlas_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var unihex_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var unihex_sampler: sampler;

const UNIHEX_UV_BIAS: f32 = 2.0;
const UNIHEX_UV_TEST: f32 = 1.5;

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = position_world_to_clip(vertex.position);
    out.uv = vertex.uv;
    out.color = vertex.color;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    if in.uv.x >= UNIHEX_UV_TEST {
        let uv = vec2(in.uv.x - UNIHEX_UV_BIAS, in.uv.y);
        let a = textureSampleLevel(unihex_texture, unihex_sampler, uv, 0.0).r;
        return vec4(in.color.rgb, in.color.a * a);
    }
    return in.color * textureSampleLevel(atlas_texture, atlas_sampler, in.uv, 0.0);
}
