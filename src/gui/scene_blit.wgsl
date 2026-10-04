@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;

struct BlitOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn blit_vertex(@builtin(vertex_index) index: u32) -> BlitOutput {
    let uv = vec2(f32(index >> 1u), f32(index & 1u)) * 2.0;
    var out: BlitOutput;
    out.clip_position = vec4(uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn blit_fragment(in: BlitOutput) -> @location(0) vec4<f32> {
    return vec4(textureSampleLevel(scene_texture, scene_sampler, in.uv, 0.0).rgb, 1.0);
}
