use bevy::math::{DVec3, Mat4, Vec3, Vec4};

use super::super::inputs::Inputs;
use super::uniform::toward;
use crate::shaderpack::native::api::{FrameShadow, MAX_CASCADES};

const DEPTH_REACH: f32 = 1.75;

const SECTION: f32 = 16.0;

const ANCHOR_GRID: f64 = 2.0;

pub(super) fn depth_reach(distance: f32) -> f32 {
    distance * DEPTH_REACH + SECTION
}

pub(super) fn toward_light(inputs: &Inputs) -> Vec3 {
    let sun = Vec3::from_array(toward(inputs.sun_angle));
    if sun.y >= 0.0 {
        sun
    } else {
        Vec3::from_array(toward(inputs.moon_angle))
    }
}

fn light_view(toward_light: Vec3) -> Mat4 {
    let up = if toward_light.z.abs() < 0.9 {
        Vec3::Z
    } else {
        Vec3::X
    };
    Mat4::look_to_rh(Vec3::ZERO, -toward_light, up)
}

fn reversed_box(half: f32, reach: f32) -> Mat4 {
    Mat4::from_cols(
        Vec4::new(1.0 / half, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0 / half, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 0.5 / reach, 0.0),
        Vec4::new(0.0, 0.0, 0.5, 1.0),
    )
}

pub(super) fn fit(
    toward_light: Vec3,
    camera: DVec3,
    resolution: u32,
    distance: f32,
    count: usize,
) -> FrameShadow {
    let count = count.clamp(1, MAX_CASCADES);
    let offset = (camera % ANCHOR_GRID - DVec3::splat(ANCHOR_GRID / 2.0)).as_vec3();
    let view = light_view(toward_light) * Mat4::from_translation(offset);
    let reach = depth_reach(distance);
    let mut shadow = FrameShadow {
        view: view.to_cols_array_2d(),
        resolution: resolution as f32,
        distance,
        count: count as u32,
        ..none()
    };
    for c in 0..count {
        let half = distance * (c + 1) as f32 / count as f32;
        let projection = reversed_box(half, reach);
        shadow.cascades[c] = (projection * view).to_cols_array_2d();
        shadow.far[c] = half;
        shadow.texel[c] = 2.0 * half / resolution.max(1) as f32;
        shadow.projection = projection.to_cols_array_2d();
    }
    shadow
}

pub(super) fn none() -> FrameShadow {
    let identity = Mat4::IDENTITY.to_cols_array_2d();
    FrameShadow {
        view: identity,
        projection: identity,
        cascades: [identity; MAX_CASCADES],
        ..FrameShadow::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(shadow: &FrameShadow, point: Vec3) -> Vec3 {
        let m = Mat4::from_cols_array_2d(&shadow.cascades[0]);
        let p = m * point.extend(1.0);
        p.truncate() / p.w
    }

    #[test]
    fn noon_looks_down_with_reversed_depth() {
        let shadow = fit(Vec3::Y, DVec3::ONE, 2048, 128.0, 1);
        let at = clip(&shadow, Vec3::ZERO);
        assert!(
            at.truncate().length() < 1e-4 && (at.z - 0.5).abs() < 1e-4,
            "{at}"
        );
        assert!(clip(&shadow, Vec3::new(0.0, 10.0, 0.0)).z > at.z);
        assert!((clip(&shadow, Vec3::new(128.0, 0.0, 0.0)).x.abs() - 1.0).abs() < 1e-3);
        assert_eq!(
            (shadow.count, shadow.far[0], shadow.texel[0]),
            (1, 128.0, 0.125)
        );
        let product =
            Mat4::from_cols_array_2d(&shadow.projection) * Mat4::from_cols_array_2d(&shadow.view);
        assert!(product.abs_diff_eq(Mat4::from_cols_array_2d(&shadow.cascades[0]), 1e-6));
    }

    #[test]
    fn the_map_holds_still_inside_a_grid_cell() {
        let light = Vec3::new(-0.6, 0.8, 0.0);
        let world = DVec3::new(1000.3, 70.0, -20.0);
        let at = |camera: DVec3| {
            clip(
                &fit(light, camera, 2048, 128.0, 1),
                (world - camera).as_vec3(),
            )
        };
        let a = at(DVec3::new(1000.1, 70.2, -20.5));
        let b = at(DVec3::new(1001.7, 71.9, -21.9));
        assert!(a.abs_diff_eq(b, 1e-4), "{a} {b}");
    }
}
