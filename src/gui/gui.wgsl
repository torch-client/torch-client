@group(0) @binding(0) var atlas_texture: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;
@group(0) @binding(2) var unihex_texture: texture_2d<f32>;
@group(0) @binding(3) var unihex_sampler: sampler;

const UNIHEX_UV_BIAS: f32 = 2.0;
const UNIHEX_UV_TEST: f32 = 1.5;

const GLINT_UV_BIAS: f32 = 2.0;
const GLINT_UV_TEST: f32 = 1.5;

struct Glint {
    region: vec4<f32>,
    params: vec4<f32>,
};
@group(0) @binding(4) var<uniform> glint: Glint;

const GLINT_GAMMA: f32 = 2.2;

fn add_glint(rgb: vec3<f32>, frag: vec2<f32>) -> vec3<f32> {
    let rot = bitcast<f32>(#{GLINT_ROTATION_BITS}u);
    let c = cos(rot);
    let s = sin(rot);
    let p = frag * glint.params.z;
    let g = glint.params.xy + vec2(c * p.x - s * p.y, s * p.x + c * p.y);
    let uv = glint.region.xy + fract(g) * glint.region.zw;
    let texel = textureSampleLevel(atlas_texture, atlas_sampler, uv, 0.0).rgb;
    let add = pow(texel, vec3(1.0 / GLINT_GAMMA)) * glint.params.w;
    let base = pow(rgb, vec3(1.0 / GLINT_GAMMA));
    return pow(min(base + add * add, vec3(1.0)), vec3(GLINT_GAMMA));
}

struct Vertex {
    @location(0) position: vec2<f32>,
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
    out.clip_position = vec4(vertex.position, 0.0, 1.0);
    out.uv = vertex.uv;
    out.color = vertex.color;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    if in.uv.x >= UNIHEX_UV_TEST {
        let uv = vec2(in.uv.x - UNIHEX_UV_BIAS, in.uv.y);
        let a = in.color.a * textureSampleLevel(unihex_texture, unihex_sampler, uv, 0.0).r;
        return vec4(in.color.rgb * a, a);
    }
    var uv = in.uv;
    let glinting = uv.y >= GLINT_UV_TEST;
    if glinting {
        uv.y -= GLINT_UV_BIAS;
    }
    let texel = textureSampleLevel(atlas_texture, atlas_sampler, uv, 0.0);
    if in.color.a < 0.0 {
        return vec4(texel.rgb * in.color.rgb, 0.0);
    }
    let color = in.color * texel;
    var rgb = color.rgb;
    if glinting && glint.params.w > 0.0 {
        rgb = add_glint(rgb, in.clip_position.xy);
    }
    return vec4(rgb * color.a, color.a);
}

fn srgb_encode(c: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3(0.0031308));
}
fn srgb_decode(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
}

@fragment
fn vignette_fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSampleLevel(atlas_texture, atlas_sampler, in.uv, 0.0);
    let src = srgb_encode(texel.rgb) * srgb_encode(in.color.rgb);
    return vec4(1.0 - srgb_decode(1.0 - src), 0.0);
}
