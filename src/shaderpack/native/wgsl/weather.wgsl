const torch_snow: u32 = 2u;

@vertex
fn torch_weather_vertex(torch_in: torch_mesh_in) -> torch_weather_varyings {
    let torch_m = torch_mesh_decode(torch_in);
    var torch_out: torch_weather_varyings;
    torch_out.clip = torch_clip(torch_m.position);
    torch_out.position = torch_m.position;
    torch_out.uv = torch_m.uv;
    torch_out.color = torch_m.color;
    torch_out.light = torch_m.light;
    torch_out.snow = select(0u, 1u, (torch_in.flags & torch_snow) != 0u);
    torch_out.torch_normal = torch_m.normal;
    return torch_out;
}

fn torch_weather_base(torch_v: torch_weather_varyings) -> Surface {
    return Surface(
        torch_mesh_texel(torch_v.uv) * torch_v.color,
        torch_v.torch_normal,
        torch_v.light,
        0.0,
        1.0,
        0.0,
        0u,
        vec4<f32>(0.0),
    );
}
