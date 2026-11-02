#define_import_path client::glint

#import bevy_pbr::mesh_view_bindings::globals

const GLINT_GAMMA: f32 = 2.2;

fn add_glint(
    rgb: vec3<f32>,
    uv: vec2<f32>,
    scale: vec2<f32>,
    strength: f32,
    glint_texture: texture_2d<f32>,
    glint_sampler: sampler,
) -> vec3<f32> {
    let rot = bitcast<f32>(#{GLINT_ROTATION_BITS}u);
    let period_u = bitcast<f32>(#{GLINT_PERIOD_U_BITS}u);
    let period_v = bitcast<f32>(#{GLINT_PERIOD_V_BITS}u);
    let offset = vec2(-fract(globals.time / period_u), fract(globals.time / period_v));
    let c = cos(rot);
    let s = sin(rot);
    let p = uv * scale;
    let g = offset + vec2(c * p.x - s * p.y, s * p.x + c * p.y);
    let texel = textureSampleLevel(glint_texture, glint_sampler, g, 0.0).rgb;
    let add = pow(texel, vec3(1.0 / GLINT_GAMMA)) * strength;
    let base = pow(rgb, vec3(1.0 / GLINT_GAMMA));
    let sum = base + add * add;
    return pow(select(min(sum, vec3(1.0)), sum, base > vec3(1.0)), vec3(GLINT_GAMMA));
}
