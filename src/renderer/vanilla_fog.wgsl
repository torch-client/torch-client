#define_import_path client::vanilla_fog

#ifdef DISTANCE_FOG
#import bevy_pbr::mesh_view_bindings::fog
#endif

fn srgb_encode(c: vec3<f32>) -> vec3<f32> {
    let low = c * 12.92;
    let high = 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055;
    return select(high, low, c <= vec3(0.0031308));
}

fn srgb_decode(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3(2.4));
    return select(high, low, c <= vec3(0.04045));
}

fn vanilla_fog_ramp(distance: f32, start: f32, end: f32) -> f32 {
    if distance <= start {
        return 0.0;
    } else if distance >= end {
        return 1.0;
    }
    return (distance - start) / (end - start);
}

#ifdef DISTANCE_FOG
fn vanilla_fog(color: vec4<f32>, rel: vec3<f32>) -> vec4<f32> {
    let spherical = length(rel);
    let cylindrical = max(length(rel.xz), abs(rel.y));
    let environmental = fog.directional_light_color.xy;
    let value = max(
        vanilla_fog_ramp(spherical, environmental.x, environmental.y),
        vanilla_fog_ramp(cylindrical, fog.be.x, fog.be.y),
    );
    let mixed = mix(srgb_encode(color.rgb), srgb_encode(fog.base_color.rgb), value * fog.base_color.a);
    return vec4(srgb_decode(mixed), color.a);
}
#endif
