struct torch_overlay_varyings {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn torch_overlay_vertex(torch_in: torch_mesh_in) -> torch_overlay_varyings {
    let torch_m = torch_mesh_decode(torch_in);
    var torch_out: torch_overlay_varyings;
    torch_out.clip = torch_clip(torch_m.position);
    torch_out.uv = torch_m.uv;
    torch_out.color = torch_m.color;
    return torch_out;
}

@fragment
fn torch_overlay_cracks_fragment(torch_v: torch_overlay_varyings) -> @location(0) vec4<f32> {
    let torch_color = torch_mesh_texel(torch_v.uv) * torch_v.color;
    if torch_color.a < 0.1 {
        discard;
    }
    return torch_color;
}

fn torch_overlay_linear(torch_c: vec3<f32>) -> vec3<f32> {
    let torch_low = torch_c / 12.92;
    let torch_high = pow((torch_c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(torch_high, torch_low, torch_c <= vec3<f32>(0.04045));
}

@fragment
fn torch_overlay_line_fragment(torch_v: torch_overlay_varyings) -> @location(0) vec4<f32> {
    return vec4<f32>(torch_overlay_linear(torch_v.color.rgb), torch_v.color.a);
}
