@vertex
fn torch_terrain_vertex(torch_in: torch_terrain_in) -> torch_terrain_varyings {
    var torch_out = torch_terrain_lit(torch_terrain_decode(torch_in), torch_in.light);
    torch_out.clip = frame.camera.projection * (frame.camera.view * vec4<f32>(torch_out.position, 1.0));
    return torch_out;
}
