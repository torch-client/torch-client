#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
    forward_io::{VertexOutput, FragmentOutput},
}
#import client::glint::add_glint

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> hand_glint: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var glint_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var glint_sampler: sampler;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef VERTEX_UVS_A
    if hand_glint.z > 0.0 {
        let rgb = add_glint(
            pbr_input.material.base_color.rgb,
            in.uv,
            hand_glint.xy,
            hand_glint.z,
            glint_texture,
            glint_sampler,
        );
        pbr_input.material.base_color = vec4(rgb, pbr_input.material.base_color.a);
    }
#endif

    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
