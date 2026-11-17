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

const KAWASE_OFFSET: f32 = 1.0;

fn kawase_texel() -> vec2<f32> {
    return KAWASE_OFFSET / vec2<f32>(textureDimensions(scene_texture, 0));
}

fn tap(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(scene_texture, scene_sampler, uv, 0.0).rgb;
}

@fragment
fn blur_down_fragment(in: BlitOutput) -> @location(0) vec4<f32> {
    let d = kawase_texel();
    var sum = tap(in.uv) * 4.0;
    sum += tap(in.uv - d);
    sum += tap(in.uv + d);
    sum += tap(in.uv + vec2(d.x, -d.y));
    sum += tap(in.uv - vec2(d.x, -d.y));
    return vec4(sum / 8.0, 1.0);
}

@fragment
fn blur_up_fragment(in: BlitOutput) -> @location(0) vec4<f32> {
    let d = kawase_texel();
    var sum = tap(in.uv + vec2(-2.0 * d.x, 0.0));
    sum += tap(in.uv + vec2(2.0 * d.x, 0.0));
    sum += tap(in.uv + vec2(0.0, -2.0 * d.y));
    sum += tap(in.uv + vec2(0.0, 2.0 * d.y));
    sum += tap(in.uv + vec2(-d.x, d.y)) * 2.0;
    sum += tap(in.uv + vec2(d.x, d.y)) * 2.0;
    sum += tap(in.uv + vec2(d.x, -d.y)) * 2.0;
    sum += tap(in.uv + vec2(-d.x, -d.y)) * 2.0;
    return vec4(sum / 12.0, 1.0);
}
