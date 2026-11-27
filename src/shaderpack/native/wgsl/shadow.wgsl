fn torch_shadow_view_depth(torch_position: vec3<f32>) -> f32 {
    return -(frame.camera.view * vec4<f32>(torch_position, 0.0)).z;
}

fn shadow_cascade(position: vec3<f32>) -> u32 {
    let torch_count = min(frame.shadow.count, 4u);
    if torch_count == 0u {
        return 0u;
    }
    let torch_depth = torch_shadow_view_depth(position);
    var torch_cascade = 0u;
    while torch_cascade + 1u < torch_count && torch_depth > frame.shadow.far[torch_cascade] {
        torch_cascade += 1u;
    }
    return torch_cascade;
}

fn shadow_light(position: vec3<f32>, normal: vec3<f32>) -> f32 {
    if min(frame.shadow.count, 4u) == 0u || torch_shadow_view_depth(position) > frame.shadow.distance {
        return 1.0;
    }
    let torch_cascade = shadow_cascade(position);
    let torch_moved = position + normal * frame.shadow.texel[torch_cascade];
    let torch_clip = frame.shadow.cascades[torch_cascade] * vec4<f32>(torch_moved, 1.0);
    let torch_ndc = torch_clip.xyz / torch_clip.w;
    let torch_uv = vec2<f32>(torch_ndc.x * 0.5 + 0.5, 0.5 - torch_ndc.y * 0.5);
    let torch_inside = all(torch_uv >= vec2<f32>(0.0)) && all(torch_uv <= vec2<f32>(1.0))
        && torch_ndc.z >= 0.0 && torch_ndc.z <= 1.0;
    if !torch_inside {
        return 1.0;
    }
    return textureSampleCompareLevel(shadow_depth, shadow_compare, torch_uv, torch_cascade, torch_ndc.z);
}
