#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view

struct SkyParams {
    sun: vec4<f32>,
    tuning: vec4<f32>,
    sky_color: vec4<f32>,
    fog_color: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: SkyParams;

const DENSITY_FALLOFF: f32 = 0.45;

const SCATTER_GAIN: f32 = 0.55;

const SCATTER_TINT: vec3<f32> = vec3(1.00, 0.45, 0.20);

const MIE_GAIN: f32 = 0.35;

const GLARE_SHARPNESS: f32 = 220.0;

const GROUND_DARKEN: f32 = 0.14;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let dir = normalize(in.world_position.xyz - view.world_position);
    let up = dir.y;
    let to_sun = dot(dir, params.sun.xyz);

    let sun_visibility = params.tuning.x;
    let day_height = params.sun.w;

    let zenith = params.sky_color.rgb;
    let horizon = params.fog_color.rgb;

    let climb = pow(saturate(up), DENSITY_FALLOFF);
    var color = mix(horizon, zenith, climb);

    let toward_sun = saturate(to_sun);
    let toward_sun2 = toward_sun * toward_sun;
    let toward_sun4 = toward_sun2 * toward_sun2;
    let toward_sun8 = toward_sun4 * toward_sun4;

    let low_sun = 1.0 - saturate(day_height);
    let band = toward_sun4 * (1.0 - saturate(up)) * low_sun * sun_visibility;
    color = mix(color, SCATTER_TINT * max(horizon, vec3(0.35)), band * SCATTER_GAIN);

    let halo = toward_sun8 * sun_visibility;
    color += horizon * halo * MIE_GAIN;

    color *= mix(1.0, GROUND_DARKEN, saturate(-up * 3.0));

    let glare = pow(saturate(to_sun), GLARE_SHARPNESS)
        * sun_visibility
        * saturate(up * 4.0 + 0.2);
    color += vec3(1.0, 0.95, 0.85) * glare * params.tuning.y;

    return vec4(color, 1.0);
}
