@group(1) @binding(5) var<storage, read> torch_slot_table: array<vec4<i32>>;
const torch_slot_words: u32 = 6u;

fn torch_section_origin(torch_instance: u32, torch_slot_lane: i32) -> vec3<i32> {
    return torch_slot_table[torch_instance * torch_slot_words + 2u].xyz;
}
