@vertex
fn torch_cloud_vertex(torch_in: torch_mesh_in) -> torch_cloud_varyings {
    let torch_m = torch_mesh_decode(torch_in);
    var torch_out: torch_cloud_varyings;
    torch_out.clip = torch_clip(torch_m.position);
    torch_out.position = torch_m.position;
    torch_out.normal = torch_m.normal;
    torch_out.color = torch_m.color;
    torch_out.torch_uv = torch_m.uv;
    torch_out.torch_light = torch_m.light;
    return torch_out;
}

fn torch_cloud_base(torch_v: torch_cloud_varyings) -> Surface {
    return Surface(
        torch_mesh_texel(torch_v.torch_uv) * torch_v.color,
        torch_v.normal,
        torch_v.torch_light,
        0.0,
        1.0,
        0.0,
        0u,
        vec4<f32>(0.0),
    );
}
