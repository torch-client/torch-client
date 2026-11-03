#import client::vanilla_fog::srgb_decode

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var glow_texture: texture_2d<f32>;

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

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let hi = vec2<i32>(textureDimensions(glow_texture)) - vec2<i32>(1, 1);
    let p = vec2<i32>(in.clip_position.xy);

    var block: array<vec4<f32>, 49>;
    for (var y = 0; y < 7; y++) {
        for (var x = 0; x < 7; x++) {
            if (x == 0 || x == 6) && (y == 0 || y == 6) {
                continue;
            }
            let at = clamp(p + vec2<i32>(x - 3, y - 3), vec2<i32>(0, 0), hi);
            block[y * 7 + x] = textureLoad(glow_texture, at, 0);
        }
    }

    var v_rgb = vec3(0.0);
    var v_a = 0.0;
    for (var j = -2; j <= 2; j++) {
        let r = clamp(p.y + j, 0, hi.y) - p.y + 3;
        var h_rgb = vec3(0.0);
        var h_a = 0.0;
        for (var i = -2; i <= 2; i++) {
            let c = (clamp(p.x + i, 0, hi.x) - p.x + 3) + r * 7;
            let o = block[c];
            let left = block[c - 1];
            let right = block[c + 1];
            let up = block[c - 7];
            let down = block[c + 7];
            h_a += clamp(
                abs(o.a - left.a) + abs(o.a - right.a) + abs(o.a - up.a) + abs(o.a - down.a),
                0.0,
                1.0,
            );
            h_rgb += 0.2 * (o.rgb * o.a + left.rgb * left.a + right.rgb * right.a
                + up.rgb * up.a + down.rgb * down.a);
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
