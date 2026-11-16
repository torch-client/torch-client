#ifdef WEBGL2
#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}
#import client::unlit::{unlit_base_color, unlit_finish}
#else
#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
    forward_io::{VertexOutput, FragmentOutput},
}
#endif
#import client::glint::add_glint

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> hand_glint: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var glint_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var glint_sampler: sampler;

fn with_glint(in: VertexOutput, color: vec4<f32>) -> vec4<f32> {
#ifdef VERTEX_UVS_A
    if hand_glint.z > 0.0 {
        let rgb = add_glint(
            color.rgb,
            in.uv,
            hand_glint.xy,
            hand_glint.z,
            glint_texture,
            glint_sampler,
        );
        return vec4(rgb, color.a);
    }
#endif
    return color;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var out: FragmentOutput;
#ifdef WEBGL2
    out.color = unlit_finish(in, with_glint(in, unlit_base_color(in)));
#else
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    pbr_input.material.base_color = with_glint(in, pbr_input.material.base_color);

    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}
