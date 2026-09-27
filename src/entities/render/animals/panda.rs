use azalea_registry::builtin::EntityKind;
use bevy::prelude::{Quat, Transform};

use crate::entities::models::animals::panda;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{RenderSpec, RootPose, default_root};
use crate::util::mth::lerp;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Panda,
        RenderSpec::new("panda", panda::layer, texture, panda::setup_anim)
            .with_root(root)
            .with_visible(|st| !st.extras.is_baby),
    );
    registry.add(
        EntityKind::Panda,
        RenderSpec::new(
            "panda_baby",
            panda::baby_layer,
            texture,
            panda::baby_setup_anim,
        )
        .with_root(root)
        .with_visible(|st| st.extras.is_baby),
    );
}

const GENES: [&str; 7] = [
    "normal",
    "lazy",
    "worried",
    "playful",
    "brown",
    "weak",
    "aggressive",
];

fn gene(st: &EntityState) -> &'static str {
    GENES[(st.extras.variant_id.clamp(0, 6)) as usize]
}

fn texture(st: &EntityState) -> String {
    let gene = gene(st);
    if st.extras.is_baby {
        if gene == "normal" {
            "entity/panda/panda_baby".to_string()
        } else {
            format!("entity/panda/{gene}_panda_baby")
        }
    } else if gene == "normal" {
        "entity/panda/panda".to_string()
    } else {
        format!("entity/panda/panda_{gene}")
    }
}

fn frac(x: f32) -> f32 {
    x - x.floor()
}

fn angle_of(this_angle: f32, next_angle: f32, next_roll_pos: i32, t: f32, threshold: f32) -> f32 {
    if (next_roll_pos as f32) < threshold {
        lerp(t, this_angle, next_angle)
    } else {
        this_angle
    }
}

fn root(st: &EntityState) -> RootPose {
    let mut pose = default_root(st);
    let e = &st.extras;
    let mut extra = Transform::IDENTITY;

    if e.roll_time > 0.0 {
        let t = frac(e.roll_time);
        let roll_pos = e.roll_time.floor() as i32;
        let next = roll_pos + 1;
        let y = if e.is_baby { 0.3 } else { 0.8 };
        if (roll_pos as f32) < 8.0 {
            let this_angle = 90.0 * roll_pos as f32 / 7.0;
            let next_angle = 90.0 * next as f32 / 7.0;
            let angle = angle_of(this_angle, next_angle, next, t, 8.0);
            extra = extra * Transform::from_xyz(0.0, (y + 0.2) * (angle / 90.0), 0.0);
            extra = extra * Transform::from_rotation(Quat::from_rotation_x((-angle).to_radians()));
        } else if (roll_pos as f32) < 16.0 {
            let this_angle = 90.0 + 90.0 * (roll_pos as f32 - 8.0) / 7.0;
            let next_angle = 90.0 + 90.0 * (next as f32 - 8.0) / 7.0;
            let angle = angle_of(this_angle, next_angle, next, t, 16.0);
            extra =
                extra * Transform::from_xyz(0.0, y + 0.2 + (y - 0.2) * (angle - 90.0) / 90.0, 0.0);
            extra = extra * Transform::from_rotation(Quat::from_rotation_x((-angle).to_radians()));
        } else if (roll_pos as f32) < 24.0 {
            let this_angle = 180.0 + 90.0 * (roll_pos as f32 - 16.0) / 7.0;
            let next_angle = 180.0 + 90.0 * (next as f32 - 16.0) / 7.0;
            let angle = angle_of(this_angle, next_angle, next, t, 24.0);
            extra = extra * Transform::from_xyz(0.0, y + y * (270.0 - angle) / 90.0, 0.0);
            extra = extra * Transform::from_rotation(Quat::from_rotation_x((-angle).to_radians()));
        } else if roll_pos < 32 {
            let this_angle = 270.0 + 90.0 * (roll_pos as f32 - 24.0) / 7.0;
            let next_angle = 270.0 + 90.0 * (next as f32 - 24.0) / 7.0;
            let angle = angle_of(this_angle, next_angle, next, t, 32.0);
            extra = extra * Transform::from_xyz(0.0, y * ((360.0 - angle) / 90.0), 0.0);
            extra = extra * Transform::from_rotation(Quat::from_rotation_x((-angle).to_radians()));
        }
    }

    let sit = e.sit_amount;
    if sit > 0.0 {
        extra = extra * Transform::from_xyz(0.0, 0.8 * sit, 0.0);
        extra = extra
            * Transform::from_rotation(Quat::from_rotation_x(
                lerp(sit, st.x_rot, st.x_rot + 90.0).to_radians(),
            ));
        extra = extra * Transform::from_xyz(0.0, -1.0 * sit, 0.0);
        if e.scared {
            let wobble = ((st.age_ticks * 1.25) as f64).cos() as f32 * std::f32::consts::PI * 0.05;
            extra = extra * Transform::from_rotation(Quat::from_rotation_y(wobble.to_radians()));
            if e.is_baby {
                extra = extra * Transform::from_xyz(0.0, 0.8, 0.55);
            }
        }
    }

    let lie = e.lie_on_back_amount;
    if lie > 0.0 {
        let y = if e.is_baby { 0.5 } else { 1.3 };
        extra = extra * Transform::from_xyz(0.0, y * lie, 0.0);
        extra = extra
            * Transform::from_rotation(Quat::from_rotation_x(
                lerp(lie, st.x_rot, st.x_rot + 180.0).to_radians(),
            ));
    }

    pose.extra_offset += pose.extra_rotation * extra.translation;
    pose.extra_rotation *= extra.rotation;
    pose
}
