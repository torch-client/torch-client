use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;

use super::scaling;
use crate::util::mth::DEG_TO_RAD;
use crate::util::mth::lerp;

const DEG_30: f32 = 0.5235988;
const DEG_15: f32 = 0.2617994;
const DEG_125: f32 = 2.1816616;
const DEG_60: f32 = 1.0471976;
const DEG_45: f32 = 0.7853982;

pub const DONKEY_SCALE: f32 = 0.87;
pub const MULE_SCALE: f32 = 0.92;
const LIVING_HORSE_SCALE: f32 = 1.1;

fn body_mesh(g: Grow) -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let body = root.child(
        "body",
        CubeList::new().tex_offs(0, 32).add_box_grow(
            -5.0,
            -8.0,
            -17.0,
            10.0,
            10.0,
            22.0,
            Grow::all(0.05),
        ),
        PartPose::offset(0.0, 11.0, 5.0),
    );
    body.child(
        "tail",
        CubeList::new()
            .tex_offs(42, 36)
            .add_box_grow(-1.5, 0.0, 0.0, 3.0, 14.0, 4.0, g),
        PartPose::offset_rotation(0.0, -5.0, 2.0, DEG_30, 0.0, 0.0),
    );
    let head_parts = root.child(
        "head_parts",
        CubeList::new()
            .tex_offs(0, 35)
            .add_box(-2.05, -6.0, -2.0, 4.0, 12.0, 7.0),
        PartPose::offset_rotation(0.0, 4.0, -12.0, DEG_30, 0.0, 0.0),
    );
    let head = head_parts.child(
        "head",
        CubeList::new()
            .tex_offs(0, 13)
            .add_box_grow(-3.0, -11.0, -2.0, 6.0, 5.0, 7.0, g),
        PartPose::ZERO,
    );
    head.child(
        "left_ear",
        CubeList::new().tex_offs(19, 16).add_box_grow(
            0.55,
            -13.0,
            4.0,
            2.0,
            3.0,
            1.0,
            Grow::all(-0.001),
        ),
        PartPose::ZERO,
    );
    head.child(
        "right_ear",
        CubeList::new().tex_offs(19, 16).add_box_grow(
            -2.55,
            -13.0,
            4.0,
            2.0,
            3.0,
            1.0,
            Grow::all(-0.001),
        ),
        PartPose::ZERO,
    );
    let head_parts = root.get("head_parts");
    head_parts.child(
        "mane",
        CubeList::new()
            .tex_offs(56, 36)
            .add_box_grow(-1.0, -11.0, 5.01, 2.0, 16.0, 2.0, g),
        PartPose::ZERO,
    );
    head_parts.child(
        "upper_mouth",
        CubeList::new()
            .tex_offs(0, 25)
            .add_box_grow(-2.0, -11.0, -7.0, 4.0, 5.0, 5.0, g),
        PartPose::ZERO,
    );
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(48, 21)
            .mirror()
            .add_box_grow(-3.0, -1.01, -1.0, 4.0, 11.0, 4.0, g),
        PartPose::offset(4.0, 14.0, 7.0),
    );
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(48, 21)
            .add_box_grow(-1.0, -1.01, -1.0, 4.0, 11.0, 4.0, g),
        PartPose::offset(-4.0, 14.0, 7.0),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(48, 21)
            .mirror()
            .add_box_grow(-3.0, -1.01, -1.9, 4.0, 11.0, 4.0, g),
        PartPose::offset(4.0, 14.0, -10.0),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(48, 21)
            .add_box_grow(-1.0, -1.01, -1.9, 4.0, 11.0, 4.0, g),
        PartPose::offset(-4.0, 14.0, -10.0),
    );
    mesh
}

fn saddle_mesh() -> MeshDef {
    let mut mesh = body_mesh(Grow::NONE);
    let root = mesh.root();
    root.get("body").child(
        "saddle",
        CubeList::new().tex_offs(26, 0).add_box_grow(
            -5.0,
            -8.0,
            -9.0,
            10.0,
            9.0,
            9.0,
            Grow::all(0.5),
        ),
        PartPose::ZERO,
    );
    let head_parts = root.get("head_parts");
    head_parts.child(
        "left_saddle_mouth",
        CubeList::new()
            .tex_offs(29, 5)
            .add_box(2.0, -9.0, -6.0, 1.0, 2.0, 2.0),
        PartPose::ZERO,
    );
    head_parts.child(
        "right_saddle_mouth",
        CubeList::new()
            .tex_offs(29, 5)
            .add_box(-3.0, -9.0, -6.0, 1.0, 2.0, 2.0),
        PartPose::ZERO,
    );
    head_parts.child(
        "left_saddle_line",
        CubeList::new()
            .tex_offs(32, 2)
            .add_box(3.1, -6.0, -8.0, 0.0, 3.0, 16.0),
        PartPose::rotation(-DEG_30, 0.0, 0.0),
    );
    head_parts.child(
        "right_saddle_line",
        CubeList::new()
            .tex_offs(32, 2)
            .add_box(-3.1, -6.0, -8.0, 0.0, 3.0, 16.0),
        PartPose::rotation(-DEG_30, 0.0, 0.0),
    );
    head_parts.child(
        "head_saddle",
        CubeList::new().tex_offs(1, 1).add_box_grow(
            -3.0,
            -11.0,
            -1.9,
            6.0,
            5.0,
            6.0,
            Grow::all(0.22),
        ),
        PartPose::ZERO,
    );
    head_parts.child(
        "mouth_saddle_wrap",
        CubeList::new().tex_offs(19, 0).add_box_grow(
            -2.0,
            -11.0,
            -4.0,
            4.0,
            5.0,
            2.0,
            Grow::all(0.2),
        ),
        PartPose::ZERO,
    );
    mesh
}

fn donkey_modify(mesh: &mut MeshDef) {
    let root = mesh.root();
    let chest = CubeList::new()
        .tex_offs(26, 21)
        .add_box(-4.0, 0.0, -2.0, 8.0, 8.0, 3.0);
    let body = root.get("body");
    body.child(
        "left_chest",
        chest.clone(),
        PartPose::offset_rotation(6.0, -8.0, 0.0, 0.0, -std::f32::consts::FRAC_PI_2, 0.0),
    );
    body.child(
        "right_chest",
        chest,
        PartPose::offset_rotation(-6.0, -8.0, 0.0, 0.0, std::f32::consts::FRAC_PI_2, 0.0),
    );
    let head = root.get("head_parts/head");
    let ear = CubeList::new()
        .tex_offs(0, 12)
        .add_box(-1.0, -7.0, 0.0, 2.0, 7.0, 1.0);
    head.child(
        "left_ear",
        ear.clone(),
        PartPose::offset_rotation(1.25, -10.0, 4.0, DEG_15, 0.0, DEG_15),
    );
    head.child(
        "right_ear",
        ear,
        PartPose::offset_rotation(-1.25, -10.0, 4.0, DEG_15, 0.0, -DEG_15),
    );
}

fn baby_horse_mesh(g: Grow) -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let body = root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 13)
            .add_box_grow(-4.0, -3.5, -7.0, 8.0, 7.0, 14.0, g),
        PartPose::offset(0.0, 12.5, 0.0),
    );
    body.child(
        "tail",
        CubeList::new()
            .tex_offs(24, 34)
            .add_box_grow(-1.5, -1.5, -1.0, 3.0, 3.0, 8.0, g),
        PartPose::offset_rotation(0.0, -1.0, 7.0, -0.7418, 0.0, 0.0),
    );
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(12, 46)
            .add_box_grow(-1.5, -1.0, -1.5, 3.0, 9.0, 3.0, g),
        PartPose::offset(2.4, 16.0, 5.4),
    );
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(0, 46)
            .add_box_grow(-1.5, -1.0, -1.5, 3.0, 9.0, 3.0, g),
        PartPose::offset(-2.4, 16.0, 5.4),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(12, 34)
            .add_box_grow(-1.5, -1.0, -1.5, 3.0, 9.0, 3.0, g),
        PartPose::offset(2.4, 16.0, -5.4),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(0, 34)
            .add_box_grow(-1.5, -1.0, -1.5, 3.0, 9.0, 3.0, g),
        PartPose::offset(-2.4, 16.0, -5.4),
    );
    let neck = root.child(
        "head_parts",
        CubeList::new()
            .tex_offs(30, 0)
            .add_box_grow(-2.0, -6.0, -2.0, 4.0, 8.0, 4.0, g),
        PartPose::offset_rotation(0.0, 10.0, -6.0, 0.6109, 0.0, 0.0),
    );
    let head = neck.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(-3.0, -3.9484, -6.705, 6.0, 4.0, 9.0, g),
        PartPose::offset(0.0, -6.0516, -0.2951),
    );
    head.child(
        "left_ear",
        CubeList::new()
            .tex_offs(0, 4)
            .add_box_grow(-1.0, -2.5, -0.8, 2.0, 3.0, 1.0, g),
        PartPose::offset_rotation(2.0, -4.2484, 1.9451, 0.0, 0.0, 0.2618),
    );
    head.child(
        "right_ear",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(-1.0, -2.5, -0.5, 2.0, 3.0, 1.0, g),
        PartPose::offset_rotation(-2.0, -4.2484, 1.645, 0.0, 0.0, -0.2618),
    );
    mesh
}

pub fn baby_donkey_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let body = root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 13)
            .add_box(-5.0, -3.0, -7.0, 8.0, 6.0, 14.0),
        PartPose::offset(1.0, 14.0, 0.0),
    );
    let tail = body.child("tail", CubeList::new(), PartPose::offset(0.0, -1.5, 6.5));
    tail.child(
        "tail_r1",
        CubeList::new()
            .tex_offs(24, 33)
            .add_box(-2.5, -1.0, -0.5, 3.0, 3.0, 8.0),
        PartPose::offset_rotation(0.0, 0.0, 0.0, -0.7418, 0.0, 0.0),
    );
    let body = root.get("body");
    body.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(12, 44)
            .add_box(-2.5, -1.5, -1.5, 3.0, 8.0, 3.0),
        PartPose::offset(2.25, 3.5, 5.25),
    );
    body.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(0, 44)
            .add_box(-2.5, -1.5, -1.5, 3.0, 8.0, 3.0),
        PartPose::offset(-2.4, 3.5, 5.4),
    );
    body.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(12, 33)
            .add_box(-2.5, -1.5, -1.5, 3.0, 8.0, 3.0),
        PartPose::offset(2.4, 3.5, -5.3),
    );
    body.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(0, 33)
            .add_box(-2.5, -1.5, -1.5, 3.0, 8.0, 3.0),
        PartPose::offset(-2.4, 3.5, -5.4),
    );
    let neck = body.child(
        "head_parts",
        CubeList::new(),
        PartPose::offset(0.0, -3.0, -5.0),
    );
    neck.child(
        "neck_r1",
        CubeList::new()
            .tex_offs(30, 9)
            .add_box(-3.0, -6.0, -3.0, 4.0, 8.0, 4.0),
        PartPose::offset_rotation(0.0, 0.0, 0.0, 0.3927, 0.0, 0.0),
    );
    let head = neck.child("head", CubeList::new(), PartPose::offset(0.0, -5.0, -3.0));
    head.child(
        "head_r1",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -3.6, -8.4, 6.0, 4.0, 9.0),
        PartPose::offset_rotation(0.0, -1.0, 1.0, 0.3927, 0.0, 0.0),
    );
    head.child(
        "left_ear",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-2.0, -6.5, -0.3, 2.0, 7.0, 1.0),
        PartPose::offset_rotation(2.0, -3.5, -1.0, 0.48, 0.0, 0.48),
    );
    head.child(
        "right_ear",
        CubeList::new()
            .tex_offs(22, 0)
            .mirror()
            .add_box(-2.0, -6.5, -0.3, 2.0, 7.0, 1.0)
            .mirror_if(false),
        PartPose::offset_rotation(-2.0, -3.5, -1.0, 0.48, 0.0, -0.48),
    );
    let body = root.get("body");
    body.child(
        "right_chest",
        CubeList::new(),
        PartPose::offset(-1.0, 10.0, 0.0),
    );
    body.child(
        "left_chest",
        CubeList::new(),
        PartPose::offset(-1.0, 10.0, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn horse_layer() -> LayerDef {
    LayerDef::create(scaling(body_mesh(Grow::NONE), LIVING_HORSE_SCALE), 64, 64)
}

pub fn horse_armor_layer() -> LayerDef {
    LayerDef::create(
        scaling(body_mesh(Grow::all(0.1)), LIVING_HORSE_SCALE),
        64,
        64,
    )
}

pub fn horse_saddle_layer() -> LayerDef {
    LayerDef::create(scaling(saddle_mesh(), LIVING_HORSE_SCALE), 64, 64)
}

pub fn baby_horse_layer() -> LayerDef {
    LayerDef::create(baby_horse_mesh(Grow::NONE), 64, 64)
}

pub fn undead_horse_layer() -> LayerDef {
    LayerDef::create(body_mesh(Grow::NONE), 64, 64)
}

pub fn undead_horse_armor_layer() -> LayerDef {
    LayerDef::create(body_mesh(Grow::all(0.1)), 64, 64)
}

pub fn undead_horse_saddle_layer() -> LayerDef {
    LayerDef::create(saddle_mesh(), 64, 64)
}

pub fn donkey_layer() -> LayerDef {
    let mut mesh = body_mesh(Grow::NONE);
    donkey_modify(&mut mesh);
    LayerDef::create(scaling(mesh, DONKEY_SCALE), 64, 64)
}

pub fn donkey_saddle_layer() -> LayerDef {
    let mut mesh = saddle_mesh();
    donkey_modify(&mut mesh);
    LayerDef::create(scaling(mesh, DONKEY_SCALE), 64, 64)
}

pub fn mule_layer() -> LayerDef {
    let mut mesh = body_mesh(Grow::NONE);
    donkey_modify(&mut mesh);
    LayerDef::create(scaling(mesh, MULE_SCALE), 64, 64)
}

pub fn mule_saddle_layer() -> LayerDef {
    let mut mesh = saddle_mesh();
    donkey_modify(&mut mesh);
    LayerDef::create(scaling(mesh, MULE_SCALE), 64, 64)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HeadPlacement {
    Adult,
    BabyHorse,
    BabyDonkey,
}

struct Traits {
    leg_stand_angle: f32,
    leg_standing_y_offset: f32,
    leg_standing_z_offset: f32,
    leg_standing_x_rot_offset: f32,
    tail_x_rot_offset: f32,
    head_placement: HeadPlacement,
    offset_hind_legs_when_standing: bool,
}

const ADULT: Traits = Traits {
    leg_stand_angle: DEG_15,
    leg_standing_y_offset: 12.0,
    leg_standing_z_offset: 4.0,
    leg_standing_x_rot_offset: -DEG_60,
    tail_x_rot_offset: 0.0,
    head_placement: HeadPlacement::Adult,
    offset_hind_legs_when_standing: false,
};

const BABY_HORSE: Traits = Traits {
    leg_stand_angle: DEG_15,
    leg_standing_y_offset: 4.0,
    leg_standing_z_offset: 0.0,
    leg_standing_x_rot_offset: -DEG_60,
    tail_x_rot_offset: -std::f32::consts::FRAC_PI_2,
    head_placement: HeadPlacement::BabyHorse,
    offset_hind_legs_when_standing: false,
};

const BABY_DONKEY: Traits = Traits {
    leg_stand_angle: DEG_60,
    leg_standing_y_offset: 1.0,
    leg_standing_z_offset: 0.5,
    leg_standing_x_rot_offset: 0.0,
    tail_x_rot_offset: -DEG_45,
    head_placement: HeadPlacement::BabyDonkey,
    offset_hind_legs_when_standing: true,
};

struct Ids {
    body: usize,
    head_parts: usize,
    tail: usize,
    left_hind: usize,
    right_hind: usize,
    left_front: usize,
    right_front: usize,
}

impl Ids {
    fn new(model: &BakedModel) -> Ids {
        Ids {
            body: model.id("body"),
            head_parts: model.id("head_parts"),
            tail: model.id("tail"),
            left_hind: model.id("left_hind_leg"),
            right_hind: model.id("right_hind_leg"),
            left_front: model.id("left_front_leg"),
            right_front: model.id("right_front_leg"),
        }
    }
}

fn setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState, t: &Traits) {
    let id = Ids::new(model);
    let e = &st.extras;
    let clamped_y_rot = st.y_rot.clamp(-20.0, 20.0);
    let mut head_rot_x_rad = st.x_rot * DEG_TO_RAD;
    let speed = st.walk_speed;
    let pos = st.walk_pos;
    if speed > 0.2 {
        head_rot_x_rad += (pos * 0.8).cos() * 0.15 * speed;
    }

    let eating = e.eat_animation;
    let standing = e.stand_animation;
    let i_standing = 1.0 - standing;
    let feeding = e.feeding_animation;
    let age = st.age_ticks;

    parts[id.head_parts].y_rot = clamped_y_rot * DEG_TO_RAD;
    let water_multiplier = if st.is_in_water { 0.2 } else { 1.0 };
    let leg_anim_1 = (water_multiplier * pos * 0.6662 + std::f32::consts::PI).cos();
    let leg_x_rot_anim = leg_anim_1 * 0.8 * speed;
    let base_head_angle =
        (1.0 - standing.max(eating)) * (DEG_30 + head_rot_x_rad + feeding * age.sin() * 0.05);
    parts[id.head_parts].x_rot = standing * (DEG_15 + head_rot_x_rad)
        + eating * (DEG_125 + age.sin() * 0.05)
        + base_head_angle;
    parts[id.head_parts].y_rot = standing * clamped_y_rot * DEG_TO_RAD
        + (1.0 - standing.max(eating)) * parts[id.head_parts].y_rot;
    head_parts_placement(parts, &id, t, eating, standing);

    parts[id.body].x_rot = standing * -DEG_45 + i_standing * parts[id.body].x_rot;
    parts[id.left_front].y -= t.leg_standing_y_offset * standing;
    parts[id.left_front].z += t.leg_standing_z_offset * standing;
    parts[id.right_front].y = parts[id.left_front].y;
    parts[id.right_front].z = parts[id.left_front].z;
    let stand_angle = t.leg_stand_angle * standing;
    let bob = (age * 0.6 + std::f32::consts::PI).cos();
    let x_rot_offset = t.leg_standing_x_rot_offset;
    let r_leg_rot = (x_rot_offset + bob) * standing + leg_x_rot_anim * i_standing;
    let l_leg_rot = (x_rot_offset - bob) * standing - leg_x_rot_anim * i_standing;
    parts[id.left_hind].x_rot = stand_angle - leg_anim_1 * 0.5 * speed * i_standing;
    parts[id.right_hind].x_rot = stand_angle + leg_anim_1 * 0.5 * speed * i_standing;
    parts[id.left_front].x_rot = r_leg_rot;
    parts[id.right_front].x_rot = l_leg_rot;
    if t.offset_hind_legs_when_standing {
        parts[id.left_hind].y = lerp(standing, parts[id.left_hind].y, -0.3);
        parts[id.right_hind].y = lerp(standing, parts[id.left_hind].y, -0.3);
    }

    let age_scale = st.age_scale;
    parts[id.tail].x_rot = t.tail_x_rot_offset + DEG_30 + speed * 0.75;
    parts[id.tail].y += speed * age_scale;
    parts[id.tail].z += speed * 2.0 * age_scale;
    parts[id.tail].y_rot = if e.rearing { (age * 0.7).cos() } else { 0.0 };
}

fn head_parts_placement(parts: &mut [PartState], id: &Ids, t: &Traits, eating: f32, standing: f32) {
    match t.head_placement {
        HeadPlacement::Adult => {
            parts[id.head_parts].y += lerp(eating, lerp(standing, 0.0, -8.0), 7.0);
            parts[id.head_parts].z = lerp(standing, parts[id.head_parts].z, -4.0);
        }
        HeadPlacement::BabyHorse => {
            parts[id.head_parts].y += lerp(eating, lerp(standing, 0.0, -2.0), 2.0);
            parts[id.head_parts].z = lerp(standing, parts[id.head_parts].z, -4.0);
        }
        HeadPlacement::BabyDonkey => {
            parts[id.head_parts].y = lerp(eating, parts[id.head_parts].y, -1.2);
            parts[id.head_parts].z = lerp(standing, parts[id.head_parts].z, -3.6);
        }
    }
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, &ADULT);
}

pub fn baby_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, &BABY_HORSE);
}

pub fn donkey_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, &ADULT);
    chests(model, parts, st);
}

pub fn baby_donkey_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, &BABY_DONKEY);
    let head_rot_x_rad = -30.0 * DEG_TO_RAD;
    let eating = st.extras.eat_animation;
    let standing = st.extras.stand_animation;
    let feeding = st.extras.feeding_animation;
    let age = st.age_ticks;
    let base_head_angle =
        (1.0 - standing.max(eating)) * (DEG_30 + head_rot_x_rad + feeding * age.sin() * 0.05);
    parts[model.id("head_parts")].x_rot = standing * (DEG_15 + head_rot_x_rad)
        + eating * (std::f32::consts::FRAC_PI_2 + age.sin() * 0.05)
        + base_head_angle;
    chests(model, parts, st);
}

pub fn saddle_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, &ADULT);
    for name in ["left_saddle_line", "right_saddle_line"] {
        parts[model.id(name)].visible = st.extras.ridden;
    }
}

fn chests(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    if let Some(left) = model.find("left_chest") {
        parts[left].visible = st.extras.has_chest;
    }
    if let Some(right) = model.find("right_chest") {
        parts[right].visible = st.extras.has_chest;
    }
}
