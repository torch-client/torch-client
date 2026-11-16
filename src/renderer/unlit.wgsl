#define_import_path client::unlit

#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings as view_bindings,
    pbr_bindings,
    pbr_functions,
    pbr_types,
}

#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping::{tone_mapping, screen_space_dither}
#import bevy_render::maths::powsafe
#endif

fn unlit_base_color(in: VertexOutput) -> vec4<f32> {
#ifdef VERTEX_COLORS
    var color = in.color * pbr_bindings::material.base_color;
#else
    var color = pbr_bindings::material.base_color;
#endif

#ifdef VERTEX_UVS_A
    if (pbr_bindings::material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u {
        let uv = (pbr_bindings::material.uv_transform * vec3(in.uv, 1.0)).xy;
        color *= textureSampleBias(
            pbr_bindings::base_color_texture,
            pbr_bindings::base_color_sampler,
            uv,
            view_bindings::view.mip_bias,
        );
    }
#endif

    return pbr_functions::alpha_discard(pbr_bindings::material, color);
}

fn unlit_finish(in: VertexOutput, input_color: vec4<f32>) -> vec4<f32> {
    var color = input_color;

#ifdef DISTANCE_FOG
    if (pbr_bindings::material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_FOG_ENABLED_BIT) != 0u {
        color = pbr_functions::apply_fog(
            view_bindings::fog,
            color,
            in.world_position.xyz,
            view_bindings::view.world_position.xyz,
            in.position.xy,
        );
    }
#endif

#ifdef TONEMAP_IN_SHADER
#ifdef TONEMAP_METHOD_NONE
    color = vec4(max(color.rgb, vec3(0.0)), color.a);
#else
    color = tone_mapping(color, view_bindings::view.color_grading);
#endif
#ifdef DEBAND_DITHER
    var rgb = color.rgb;
    rgb = powsafe(rgb, 1.0 / 2.2);
    rgb += screen_space_dither(in.position.xy);
    rgb = powsafe(rgb, 2.2);
    color = vec4(rgb, color.a);
#endif
#endif

#ifdef PREMULTIPLY_ALPHA
    color = pbr_functions::premultiply_alpha(pbr_bindings::material.flags, color);
#endif

    return color;
}
