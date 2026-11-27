#import client::vanilla_fog::srgb_decode

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var glow_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var glow_sampler: sampler;

struct Vertex {
    @location(0) position: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4(vertex.position.xy, 0.0, 1.0);
    return out;
}

fn load(q: vec2<i32>, hi: vec2<i32>) -> vec4<f32> {
    return textureLoad(glow_texture, clamp(q, vec2<i32>(0, 0), hi), 0);
}

fn sobel(c: vec2<i32>, hi: vec2<i32>) -> vec4<f32> {
    let o = load(c, hi);
    let left = load(c - vec2<i32>(1, 0), hi);
    let right = load(c + vec2<i32>(1, 0), hi);
    let up = load(c - vec2<i32>(0, 1), hi);
    let down = load(c + vec2<i32>(0, 1), hi);
    let a = clamp(
        abs(o.a - left.a) + abs(o.a - right.a) + abs(o.a - up.a) + abs(o.a - down.a),
        0.0,
        1.0,
    );
    let rgb = 0.2 * (o.rgb * o.a + left.rgb * left.a + right.rgb * right.a
        + up.rgb * up.a + down.rgb * down.a);
    return vec4(rgb, a);
}

fn block_uniform(p: vec2<i32>) -> bool {
    let size = vec2<f32>(textureDimensions(glow_texture));
    var lo = 1.0;
    var top = 0.0;
    for (var y = 0; y < 4; y++) {
        for (var x = 0; x < 4; x++) {
            let corner = vec2<f32>(p + vec2<i32>(2 * x - 2, 2 * y - 2));
            let a = textureSampleLevel(glow_texture, glow_sampler, corner / size, 0.0).a;
            lo = min(lo, a);
            top = max(top, a);
        }
    }
    return top < 0.125 || lo > 0.875;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.clip_position.xy);
    if block_uniform(p) {
        discard;
    }
    let hi = vec2<i32>(textureDimensions(glow_texture)) - vec2<i32>(1, 1);

    var v_rgb = vec3(0.0);
    var v_a = 0.0;
    for (var j = -2; j <= 2; j++) {
        let row = clamp(p.y + j, 0, hi.y);
        var h_rgb = vec3(0.0);
        var h_a = 0.0;
        for (var i = -2; i <= 2; i++) {
            let s = sobel(vec2<i32>(clamp(p.x + i, 0, hi.x), row), hi);
            h_rgb += s.rgb;
            h_a += s.a;
        }
        v_rgb += h_rgb * 0.2;
        v_a += min(0.5 * h_a, 1.0);
    }
    let alpha = min(0.5 * v_a, 1.0);
    if alpha <= 0.0 {
        discard;
    }
    return vec4(srgb_decode(v_rgb * 0.2), alpha);
}
