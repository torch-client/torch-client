@group(3) @binding(0) var<uniform> torch_shadow_draw: vec4<u32>;

fn torch_shadow_clip(torch_position: vec3<f32>) -> vec4<f32> {
    return frame.shadow.cascades[min(torch_shadow_draw.x, 3u)] * vec4<f32>(torch_position, 1.0);
}

@vertex
fn torch_shadow_terrain_vertex(torch_in: torch_terrain_in) -> torch_terrain_varyings {
    var torch_out = torch_shadow_terrain_lit(torch_terrain_decode(torch_in), torch_in.light);
    torch_out.clip = torch_shadow_clip(torch_out.position);
    return torch_out;
}

struct torch_shadow_mesh_varyings {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn torch_shadow_entity_vertex(torch_in: torch_mesh_in) -> torch_shadow_mesh_varyings {
    let torch_m = torch_mesh_decode(torch_in);
    var torch_out: torch_shadow_mesh_varyings;
    torch_out.clip = torch_shadow_clip(torch_m.position);
    torch_out.uv = torch_m.uv;
    torch_out.color = torch_m.color;
    return torch_out;
}
