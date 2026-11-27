struct torch_terrain_in {
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
    @location(0) position: vec4<i32>,
    @location(1) uv: vec2<f32>,
    @location(2) light: vec4<f32>,
    @location(3) color: vec4<f32>,
}

@group(2) @binding(0) var torch_pack_quads: texture_2d<u32>;
const torch_quad_row: u32 = 2048u;

const torch_unmapped: u32 = 65535u;

fn torch_pack_quad(torch_vertex: u32) -> vec4<u32> {
    let torch_quad = torch_vertex >> 2u;
    let torch_texel = vec2<i32>(i32(torch_quad % torch_quad_row), i32(torch_quad / torch_quad_row));
    return textureLoad(torch_pack_quads, torch_texel, 0);
}

fn torch_snorm3(torch_word: u32) -> vec3<f32> {
    let torch_bytes = vec3<i32>(
        bitcast<i32>(torch_word << 24u) >> 24u,
        bitcast<i32>(torch_word << 16u) >> 24u,
        bitcast<i32>(torch_word << 8u) >> 24u,
    );
    return max(vec3<f32>(torch_bytes) / 127.0, vec3<f32>(-1.0));
}

fn torch_quad_normal(torch_word: u32) -> vec3<f32> {
    let torch_n = torch_snorm3(torch_word);
    if dot(torch_n, torch_n) > 0.25 {
        return normalize(torch_n);
    }
    return vec3<f32>(0.0, 1.0, 0.0);
}

fn torch_quad_tangent(torch_word: u32) -> vec4<f32> {
    let torch_t = torch_snorm3(torch_word);
    if dot(torch_t, torch_t) > 0.25 {
        return vec4<f32>(normalize(torch_t), select(1.0, -1.0, bitcast<i32>(torch_word) < 0));
    }
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}

fn torch_decode_tint(torch_rgb: vec3<f32>) -> vec3<f32> {
    let torch_k = torch_rgb * (255.0 / 128.0);
    return torch_k * torch_k;
}

fn torch_terrain_decode(torch_in: torch_terrain_in) -> torch_terrain_varyings {
    let torch_q = torch_pack_quad(torch_in.vertex);
    let torch_local = vec3<f32>(torch_in.position.xyz) / 256.0;
    let torch_offset = torch_section_origin(torch_in.instance, torch_in.position.w) - frame.camera.block;
    let torch_position = vec3<f32>(torch_offset) - frame.camera.fraction + torch_local;
    let torch_id = torch_q.w & 65535u;
    let torch_material = select(torch_id, 0u, torch_id == torch_unmapped);
    let torch_cell = vec3<f32>(vec3<u32>(torch_q.w >> 16u, torch_q.w >> 20u, torch_q.w >> 24u) & vec3<u32>(15u));
    let torch_mid_block = torch_cell + 0.5 - torch_local;
    let torch_mid_uv = vec2<f32>(f32(torch_q.z & 65535u), f32(torch_q.z >> 16u)) / 65535.0;
    let torch_normal = torch_quad_normal(torch_q.x);
    let torch_light = torch_in.light.xy * (255.0 / 240.0);
    let torch_moved = torch_position + torch_displace(VertexInput(
        torch_position,
        torch_in.uv,
        torch_mid_uv,
        torch_normal,
        torch_light,
        torch_material,
        torch_mid_block,
    ));

    var torch_out: torch_terrain_varyings;
    torch_out.position = torch_moved;
    torch_out.uv = torch_in.uv;
    torch_out.mid_uv = torch_mid_uv;
    torch_out.color = vec4<f32>(torch_decode_tint(torch_in.color.rgb), torch_in.color.a);
    torch_out.light = torch_light;
    torch_out.normal = torch_normal;
    torch_out.tangent = torch_quad_tangent(torch_q.y);
    torch_out.mid_block = torch_mid_block;
    torch_out.torch_material = torch_material;
    torch_out.torch_emission = f32(torch_q.w >> 28u) / 15.0;
    return torch_out;
}

fn torch_lightmap(torch_level: vec2<f32>) -> vec3<f32> {
    let torch_coord = clamp(torch_level / 256.0 + 0.5 / 16.0, vec2<f32>(0.5 / 16.0), vec2<f32>(15.5 / 16.0));
    return textureSampleLevel(lightmap, linear_clamp, torch_coord, 0.0).rgb;
}

fn torch_terrain_lit(torch_v: torch_terrain_varyings, torch_light: vec4<f32>) -> torch_terrain_varyings {
    let torch_level = torch_light.xy * 255.0;
    let torch_floor = frame.sky.light_floor;
    var torch_out = torch_v;
    torch_out.block_lit = max(torch_lightmap(vec2<f32>(torch_level.x, 0.0)) - torch_floor, vec3<f32>(0.0));
    torch_out.sky_lit = max(torch_lightmap(vec2<f32>(0.0, torch_level.y)) - torch_floor, vec3<f32>(0.0));
    return torch_out;
}

fn torch_terrain_base(torch_v: torch_terrain_varyings) -> Surface {
    let torch_texel = textureSample(block_atlas, nearest_clamp, torch_v.uv);
    return Surface(
        vec4<f32>(torch_texel.rgb * torch_v.color.rgb, torch_texel.a),
        torch_v.normal,
        torch_v.light,
        torch_v.torch_emission,
        1.0,
        0.0,
        torch_v.torch_material,
        vec4<f32>(0.0),
    );
}
