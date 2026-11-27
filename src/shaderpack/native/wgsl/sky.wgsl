@vertex
fn torch_sky_vertex(torch_in: torch_mesh_in) -> torch_sky_varyings {
    let torch_m = torch_mesh_decode(torch_in);
    var torch_out: torch_sky_varyings;
    torch_out.clip = torch_clip(torch_m.position);
    torch_out.position = torch_m.position;
    torch_out.part = (torch_in.flags >> torch_sky_part_shift) & 255u;
    torch_out.torch_uv = torch_m.uv;
    torch_out.torch_color = torch_m.color;
    torch_out.torch_normal = torch_m.normal;
    torch_out.torch_light = torch_m.light;
    return torch_out;
}

fn torch_sky_base(torch_v: torch_sky_varyings) -> Surface {
    return Surface(
        torch_mesh_texel(torch_v.torch_uv) * torch_v.torch_color,
        torch_v.torch_normal,
        torch_v.torch_light,
        1.0,
        1.0,
        0.0,
        0u,
        vec4<f32>(0.0),
    );
}
