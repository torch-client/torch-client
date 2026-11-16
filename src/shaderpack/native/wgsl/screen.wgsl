struct torch_screen_varyings {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn torch_screen_vertex(@builtin(vertex_index) torch_index: u32) -> torch_screen_varyings {
    let torch_corner = vec2<f32>(f32((torch_index << 1u) & 2u), f32(torch_index & 2u));
    var torch_out: torch_screen_varyings;
    torch_out.clip = vec4<f32>(torch_corner * 2.0 - 1.0, 0.0, 1.0);
    torch_out.uv = vec2<f32>(torch_corner.x, 1.0 - torch_corner.y);
    return torch_out;
}
