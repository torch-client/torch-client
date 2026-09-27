use bevy::math::{EulerRot, Quat, Vec3};

use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

fn rotate_by(part: &mut PartState, rotation: Quat) {
    let old = Quat::from_rotation_z(part.z_rot)
        * Quat::from_rotation_y(part.y_rot)
        * Quat::from_rotation_x(part.x_rot);
    let (z, y, x) = (old * rotation).to_euler(EulerRot::ZYX);
    part.set_rotation(x, y, z);
}

pub fn end_crystal_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        let glass_cube = CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -4.0, -4.0, 8.0, 8.0, 8.0);
        let outer = root.child(
            "outer_glass",
            glass_cube.clone(),
            PartPose::offset(0.0, 24.0, 0.0),
        );
        let inner = outer.child("inner_glass", glass_cube, PartPose::ZERO.with_scale(0.875));
        inner.child(
            "cube",
            CubeList::new()
                .tex_offs(32, 0)
                .add_box(-4.0, -4.0, -4.0, 8.0, 8.0, 8.0),
            PartPose::ZERO.with_scale(0.765_625),
        );
        root.child(
            "base",
            CubeList::new()
                .tex_offs(0, 16)
                .add_box(-6.0, 0.0, -6.0, 12.0, 4.0, 12.0),
            PartPose::ZERO,
        );
    }
    LayerDef::create(mesh, 64, 32)
}

pub fn crystal_y(time_in_ticks: f32) -> f32 {
    let mut hh = (time_in_ticks * 0.2).sin() / 2.0 + 0.5;
    hh = (hh * hh + hh) * 0.4;
    hh - 1.4
}

pub fn end_crystal_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    const SIN_45: f32 = std::f32::consts::FRAC_1_SQRT_2;
    const TILT: f32 = 1.047_197_6;

    let base = model.id("base");
    let outer = model.id("outer_glass");
    let inner = model.id("inner_glass");
    let cube = model.id("cube");

    parts[base].visible = st.extras.shows_bottom;
    let animation_speed = st.age_ticks * 3.0;
    let crystal_y = crystal_y(st.age_ticks) * 16.0;
    parts[outer].y += crystal_y / 2.0;

    let axis = Vec3::new(SIN_45, 0.0, SIN_45).normalize();
    let tilt = Quat::from_axis_angle(axis, TILT);
    rotate_by(
        &mut parts[outer],
        Quat::from_rotation_y(animation_speed * DEG_TO_RAD) * tilt,
    );
    let inner_rotation = tilt * Quat::from_rotation_y(animation_speed * DEG_TO_RAD);
    rotate_by(&mut parts[inner], inner_rotation);
    rotate_by(&mut parts[cube], inner_rotation);
}

pub fn evoker_fangs_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    {
        let base = mesh.root().child(
            "base",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(0.0, 0.0, 0.0, 10.0, 12.0, 10.0),
            PartPose::offset(-5.0, 24.0, -5.0),
        );
        let jaw = CubeList::new()
            .tex_offs(40, 0)
            .add_box(0.0, 0.0, 0.0, 4.0, 14.0, 8.0);
        base.child(
            "upper_jaw",
            jaw.clone(),
            PartPose::offset_rotation(6.5, 0.0, 1.0, 0.0, 0.0, 2.042_035),
        );
        base.child(
            "lower_jaw",
            jaw,
            PartPose::offset_rotation(3.5, 0.0, 9.0, 0.0, 3.141_592_7, 4.241_150_4),
        );
    }
    LayerDef::create(mesh, 64, 32)
}

pub fn evoker_fangs_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    const PI: f32 = 3.141_592_7;
    let bite_progress = st.extras.bite_progress;
    let mut bite_amount = (bite_progress * 2.0).min(1.0);
    bite_amount = 1.0 - bite_amount * bite_amount * bite_amount;

    let base = model.id("base");
    let upper = model.id("upper_jaw");
    let lower = model.id("lower_jaw");
    parts[upper].z_rot = PI - bite_amount * 0.35 * PI;
    parts[lower].z_rot = PI + bite_amount * 0.35 * PI;
    parts[base].y -= (bite_progress + (bite_progress * 2.7).sin()) * 7.2;

    let mut pre_scale = 1.0;
    if bite_progress > 0.9 {
        pre_scale *= (1.0 - bite_progress) / 0.1;
    }
    parts[0].y = 24.0 - 20.0 * pre_scale;
    parts[0].x_scale = pre_scale;
    parts[0].y_scale = pre_scale;
    parts[0].z_scale = pre_scale;
}

pub fn leash_knot_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    mesh.root().child(
        "knot",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-3.0, -8.0, -3.0, 6.0, 8.0, 6.0),
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 32, 32)
}

pub fn shield_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        root.child(
            "plate",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(-6.0, -11.0, -2.0, 12.0, 22.0, 1.0),
            PartPose::ZERO,
        );
        root.child(
            "handle",
            CubeList::new()
                .tex_offs(26, 0)
                .add_box(-1.0, -3.0, -1.0, 2.0, 6.0, 6.0),
            PartPose::ZERO,
        );
    }
    LayerDef::create(mesh, 64, 64)
}

pub fn no_anim(_model: &BakedModel, _parts: &mut [PartState], _st: &EntityState) {}
