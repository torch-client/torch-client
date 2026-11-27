@vertex
fn torch_hand_vertex(torch_in: torch_mesh_in) -> torch_entity_varyings {
    let torch_m = torch_mesh_decode(torch_in);
    var torch_out: torch_entity_varyings;
    torch_out.clip = frame.camera.hand_projection * vec4<f32>(torch_m.position, 1.0);
    torch_out.position = (frame.camera.view_inverse * vec4<f32>(torch_m.position, 1.0)).xyz;
    torch_out.uv = torch_m.uv;
    torch_out.color = torch_m.color;
    torch_out.light = torch_m.light;
    torch_out.normal = (frame.camera.view_inverse * vec4<f32>(torch_m.normal, 0.0)).xyz;
    torch_out.overlay = torch_m.overlay;
    return torch_out;
}
