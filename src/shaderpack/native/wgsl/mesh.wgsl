struct torch_mesh_in {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) model0: vec4<f32>,
    @location(5) model1: vec4<f32>,
    @location(6) model2: vec4<f32>,
    @location(7) tint: vec4<f32>,
    @location(8) overlay: vec4<f32>,
    @location(9) light: vec2<f32>,
    @location(14) flags: u32,
}

const torch_shade_in_alpha: u32 = 1u;

const torch_sky_part_shift: u32 = 8u;

struct torch_mesh {
    position: vec3<f32>,
    normal: vec3<f32>,
    uv: vec2<f32>,
    color: vec4<f32>,
    light: vec2<f32>,
    overlay: vec4<f32>,
}

fn torch_mesh_affine(torch_in: torch_mesh_in, torch_v: vec4<f32>) -> vec3<f32> {
    return vec3<f32>(dot(torch_in.model0, torch_v), dot(torch_in.model1, torch_v), dot(torch_in.model2, torch_v));
}

fn torch_unit_or_up(torch_n: vec3<f32>) -> vec3<f32> {
    if dot(torch_n, torch_n) > 1e-8 {
        return normalize(torch_n);
    }
    return vec3<f32>(0.0, 1.0, 0.0);
}

fn torch_mesh_decode(torch_in: torch_mesh_in) -> torch_mesh {
    var torch_m: torch_mesh;
    torch_m.position = torch_mesh_affine(torch_in, vec4<f32>(torch_in.position, 1.0));
    torch_m.normal = torch_unit_or_up(torch_mesh_affine(torch_in, vec4<f32>(torch_in.normal, 0.0)));
    torch_m.uv = torch_in.uv;
    var torch_color = torch_in.color;
    if (torch_in.flags & torch_shade_in_alpha) != 0u {
        torch_color.a = 1.0;
    }
    torch_m.color = torch_color * torch_in.tint;
    torch_m.light = torch_in.light / 240.0;
    torch_m.overlay = torch_in.overlay;
    return torch_m;
}

fn torch_clip(torch_position: vec3<f32>) -> vec4<f32> {
    return frame.camera.projection * (frame.camera.view * vec4<f32>(torch_position, 1.0));
}

fn torch_mesh_texel(torch_uv: vec2<f32>) -> vec4<f32> {
    return textureSample(entity_texture, nearest_clamp, torch_uv);
}
