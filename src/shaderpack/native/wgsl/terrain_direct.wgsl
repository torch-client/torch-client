const torch_origin_row: i32 = 256;
@group(1) @binding(5) var torch_origins: texture_2d<i32>;

fn torch_section_origin(torch_instance: u32, torch_slot_lane: i32) -> vec3<i32> {
    let torch_slot = torch_slot_lane & 0xFFFF;
    let torch_texel = vec2<i32>(torch_slot % torch_origin_row, torch_slot / torch_origin_row);
    return textureLoad(torch_origins, torch_texel, 0).xyz;
}
