#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::keyframe;
use crate::entities::models::monsters::warden_anim;
use crate::entities::state::{EntityState, Pose};
use crate::util::mth::DEG_TO_RAD;

const DEFAULT_ARM_X_Y: f32 = 13.0;
const DEFAULT_ARM_Z: f32 = 1.0;

type Keep = fn(&str) -> bool;

fn base_mesh(keep: Keep) -> MeshDef {
    let cubes = |name: &str, list: CubeList| if keep(name) { list } else { CubeList::new() };

    let mut mesh = MeshDef::new();
    let bone = mesh
        .root()
        .child("bone", CubeList::new(), PartPose::offset(0.0, 24.0, 0.0));
    let body = bone.child(
        "body",
        cubes(
            "body",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(-9.0, -13.0, -4.0, 18.0, 21.0, 11.0),
        ),
        PartPose::offset(0.0, -21.0, 0.0),
    );
    body.child(
        "right_ribcage",
        cubes(
            "right_ribcage",
            CubeList::new()
                .tex_offs(90, 11)
                .add_box(-2.0, -11.0, -0.1, 9.0, 21.0, 0.0),
        ),
        PartPose::offset(-7.0, -2.0, -4.0),
    );
    body.child(
        "left_ribcage",
        cubes(
            "left_ribcage",
            CubeList::new()
                .tex_offs(90, 11)
                .mirror()
                .add_box(-7.0, -11.0, -0.1, 9.0, 21.0, 0.0)
                .mirror_if(false),
        ),
        PartPose::offset(7.0, -2.0, -4.0),
    );
    let head = body.child(
        "head",
        cubes(
            "head",
            CubeList::new()
                .tex_offs(0, 32)
                .add_box(-8.0, -16.0, -5.0, 16.0, 16.0, 10.0),
        ),
        PartPose::offset(0.0, -13.0, 0.0),
    );
    head.child(
        "right_tendril",
        cubes(
            "right_tendril",
            CubeList::new()
                .tex_offs(52, 32)
                .add_box(-16.0, -13.0, 0.0, 16.0, 16.0, 0.0),
        ),
        PartPose::offset(-8.0, -12.0, 0.0),
    );
    head.child(
        "left_tendril",
        cubes(
            "left_tendril",
            CubeList::new()
                .tex_offs(58, 0)
                .add_box(0.0, -13.0, 0.0, 16.0, 16.0, 0.0),
        ),
        PartPose::offset(8.0, -12.0, 0.0),
    );
    body.child(
        "right_arm",
        cubes(
            "right_arm",
            CubeList::new()
                .tex_offs(44, 50)
                .add_box(-4.0, 0.0, -4.0, 8.0, 28.0, 8.0),
        ),
        PartPose::offset(-DEFAULT_ARM_X_Y, -DEFAULT_ARM_X_Y, DEFAULT_ARM_Z),
    );
    body.child(
        "left_arm",
        cubes(
            "left_arm",
            CubeList::new()
                .tex_offs(0, 58)
                .add_box(-4.0, 0.0, -4.0, 8.0, 28.0, 8.0),
        ),
        PartPose::offset(DEFAULT_ARM_X_Y, -DEFAULT_ARM_X_Y, DEFAULT_ARM_Z),
    );
    bone.child(
        "right_leg",
        cubes(
            "right_leg",
            CubeList::new()
                .tex_offs(76, 48)
                .add_box(-3.1, 0.0, -3.0, 6.0, 13.0, 6.0),
        ),
        PartPose::offset(-5.9, -13.0, 0.0),
    );
    bone.child(
        "left_leg",
        cubes(
            "left_leg",
            CubeList::new()
                .tex_offs(76, 76)
                .add_box(-2.9, 0.0, -3.0, 6.0, 13.0, 6.0),
        ),
        PartPose::offset(5.9, -13.0, 0.0),
    );
    mesh
}

pub fn layer() -> LayerDef {
    LayerDef::create(base_mesh(|_| true), 128, 128)
}

pub fn tendrils_layer() -> LayerDef {
    LayerDef::create(
        base_mesh(|name| matches!(name, "left_tendril" | "right_tendril")),
        128,
        128,
    )
}

pub fn heart_layer() -> LayerDef {
    LayerDef::create(base_mesh(|name| name == "body"), 128, 128)
}

pub fn bioluminescent_layer() -> LayerDef {
    LayerDef::create(
        base_mesh(|name| {
            matches!(
                name,
                "head" | "left_arm" | "right_arm" | "left_leg" | "right_leg"
            )
        }),
        128,
        128,
    )
}

pub fn pulsating_spots_layer() -> LayerDef {
    LayerDef::create(
        base_mesh(|name| {
            matches!(
                name,
                "body" | "head" | "left_arm" | "right_arm" | "left_leg" | "right_leg"
            )
        }),
        128,
        128,
    )
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let head = model.id("head");
    let body = model.id("body");
    let left_arm = model.id("left_arm");
    let right_arm = model.id("right_arm");
    let left_leg = model.id("left_leg");
    let right_leg = model.id("right_leg");

    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;

    let speed_modifier = (3.0 * st.walk_speed).min(0.5);
    let adjusted_pos = st.walk_pos * 0.8662;
    let cosine = adjusted_pos.cos();
    let sine = adjusted_pos.sin();
    let capped = speed_modifier.min(0.35);
    parts[head].z_rot += 0.3 * sine * speed_modifier;
    parts[head].x_rot += 1.2 * (adjusted_pos + 1.5707964).cos() * capped;
    parts[body].z_rot = 0.1 * sine * speed_modifier;
    parts[body].x_rot = 1.0 * cosine * capped;
    parts[left_leg].x_rot = 1.0 * cosine * speed_modifier;
    parts[right_leg].x_rot = 1.0 * (adjusted_pos + 3.1415927).cos() * speed_modifier;
    parts[left_arm].x_rot = -(0.8 * cosine * speed_modifier);
    parts[left_arm].z_rot = 0.0;
    parts[right_arm].x_rot = -(0.8 * sine * speed_modifier);
    parts[right_arm].z_rot = 0.0;
    parts[left_arm].y_rot = 0.0;
    parts[left_arm].set_pos(DEFAULT_ARM_X_Y, -DEFAULT_ARM_X_Y, DEFAULT_ARM_Z);
    parts[right_arm].y_rot = 0.0;
    parts[right_arm].set_pos(-DEFAULT_ARM_X_Y, -DEFAULT_ARM_X_Y, DEFAULT_ARM_Z);

    let scaled_age = st.age_ticks * 0.1;
    let wobble_cos = scaled_age.cos();
    let wobble_sin = scaled_age.sin();
    parts[head].z_rot += 0.06 * wobble_cos;
    parts[head].x_rot += 0.06 * wobble_sin;
    parts[body].z_rot += 0.025 * wobble_sin;
    parts[body].x_rot += 0.025 * wobble_cos;

    let tendril_x_rot = st.extras.tendril_animation
        * ((st.age_ticks as f64 * 2.25).cos() * std::f64::consts::PI * 0.100_000_001_490_116_12)
            as f32;
    parts[model.id("left_tendril")].x_rot = tendril_x_rot;
    parts[model.id("right_tendril")].x_rot = -tendril_x_rot;

    let seconds = st.age_ticks / 20.0;
    if st.attack_time > 0.0 {
        let animation = &warden_anim::WARDEN_ATTACK;
        keyframe::apply(
            animation,
            model,
            parts,
            st.attack_time * animation.length_seconds,
            1.0,
        );
    }
    let one_shot = match st.pose {
        Pose::Emerging => Some(&warden_anim::WARDEN_EMERGE),
        Pose::Digging => Some(&warden_anim::WARDEN_DIG),
        Pose::Roaring => Some(&warden_anim::WARDEN_ROAR),
        Pose::Sniffing => Some(&warden_anim::WARDEN_SNIFF),
        Pose::Shooting => Some(&warden_anim::WARDEN_SONIC_BOOM),
        _ => None,
    };
    if let Some(animation) = one_shot {
        keyframe::apply(
            animation,
            model,
            parts,
            seconds % animation.length_seconds,
            1.0,
        );
    }
}
