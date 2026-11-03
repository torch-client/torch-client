use std::f32::consts::PI;

use azalea_registry::builtin::EntityKind::{self, *};
use bevy::math::{Mat4, Quat, Vec3};

use super::humanoid::armor;
use super::objects::thrown::item_key;
use crate::blockentities::models;
use crate::blockentities::render::skull;
use crate::entities::geom::{BakedModel, LayerDef, PartState};
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{
    Anchor, AnchorHook, Anchors, HeadTransforms, RenderSpec, TexturePath, hand_stack, is_skull,
};
use crate::util::mth::DEG_TO_RAD;

const VILLAGER_HEAD: HeadTransforms = HeadTransforms {
    y_offset: -0.117_187_5,
    skull_y_offset: -0.074_218_75,
    horizontal_scale: 1.0,
};

const PIGLIN_HEAD: HeadTransforms = HeadTransforms {
    y_offset: 0.0,
    skull_y_offset: 0.0,
    horizontal_scale: 1.001_953_1,
};

const BODIES: &[(&str, AnchorHook)] = &[
    ("zombie", humanoid),
    ("zombie_baby", humanoid),
    ("husk", humanoid),
    ("husk_baby", humanoid),
    ("drowned", humanoid),
    ("drowned_baby", humanoid),
    ("giant", humanoid),
    ("zombie_villager", humanoid),
    ("zombie_villager_baby", humanoid),
    ("skeleton", skeleton),
    ("wither_skeleton", skeleton),
    ("stray", skeleton),
    ("bogged", skeleton),
    ("parched", skeleton),
    ("piglin", humanoid),
    ("piglin_baby", humanoid),
    ("zombified_piglin", humanoid),
    ("zombified_piglin_baby", humanoid),
    ("armor_stand", humanoid),
    ("armor_stand_small", humanoid),
    ("pillager", humanoid),
    ("vindicator", humanoid),
    ("evoker", humanoid),
    ("illusioner", humanoid),
    ("vex", vex),
    ("allay", allay),
    ("copper_golem", copper_golem),
    ("villager", villager),
    ("villager_baby", villager),
    ("wandering_trader", villager),
    ("witch", witch),
    ("fox", fox),
    ("fox_baby", fox),
    ("panda", panda),
    ("panda_baby", panda),
    ("dolphin", dolphin),
    ("dolphin_baby", dolphin),
];

pub fn register(registry: &mut Registry) {
    for &(name, hook) in BODIES {
        let spec = registry.specs.iter_mut().find(|s| s.name == name);
        debug_assert!(spec.is_some(), "no body spec named {name}");
        if let Some(spec) = spec {
            spec.anchors = Some(hook);
        }
    }

    hands(
        registry,
        &[
            Zombie,
            Husk,
            Drowned,
            Giant,
            ZombieVillager,
            Skeleton,
            WitherSkeleton,
            Stray,
            Bogged,
            Parched,
            Piglin,
            PiglinBrute,
            ZombifiedPiglin,
            ArmorStand,
            Pillager,
            Vex,
            Allay,
            CopperGolem,
        ],
        ["held_right", "held_left"],
        crate::entities::always_visible,
    );
    hands(
        registry,
        &[Vindicator],
        ["held_right_aggressive", "held_left_aggressive"],
        |st| st.extras.aggressive,
    );
    hands(
        registry,
        &[Evoker],
        ["held_right_casting", "held_left_casting"],
        |st| st.extras.spell != 0,
    );
    hands(
        registry,
        &[Illusioner],
        ["held_right_illusioner", "held_left_illusioner"],
        |st| st.extras.spell != 0 || st.extras.aggressive,
    );

    head(
        registry,
        &[
            Zombie,
            Husk,
            Drowned,
            Skeleton,
            WitherSkeleton,
            Stray,
            Bogged,
            Parched,
            ArmorStand,
            Pillager,
            Vindicator,
            Evoker,
            Illusioner,
            CopperGolem,
            WanderingTrader,
        ],
        ["head_item", "head_dragon", "head_piglin"],
        HeadTransforms::DEFAULT,
    );
    head(
        registry,
        &[ZombieVillager, Villager],
        [
            "head_item_villager",
            "head_dragon_villager",
            "head_piglin_villager",
        ],
        VILLAGER_HEAD,
    );
    head(
        registry,
        &[Piglin, PiglinBrute, ZombifiedPiglin],
        [
            "head_item_piglin",
            "head_dragon_piglin",
            "head_piglin_piglin",
        ],
        PIGLIN_HEAD,
    );

    registry.add_many(
        &[Villager, WanderingTrader, Witch, Fox, Dolphin],
        carry_spec("carried_item", |st| !st.extras.main_hand.is_empty()),
    );
    registry.add_many(
        &[Panda],
        carry_spec("carried_item_panda", |st| {
            !st.extras.main_hand.is_empty() && st.extras.sitting && !st.extras.scared
        }),
    );
}

fn hands(
    registry: &mut Registry,
    kinds: &[EntityKind],
    names: [&'static str; 2],
    when: fn(&EntityState) -> bool,
) {
    fn right_item(st: &EntityState) -> TexturePath {
        item_key(hand_stack(st, true))
    }
    fn left_item(st: &EntityState) -> TexturePath {
        item_key(hand_stack(st, false))
    }
    let [right_name, left_name] = names;
    let right = anchored(
        right_name,
        right_item,
        "thirdperson_righthand",
        Anchor::Hand { right: true },
        when,
    );
    registry.add_many(kinds, right);
    let left = anchored(
        left_name,
        left_item,
        "thirdperson_lefthand",
        Anchor::Hand { right: false },
        when,
    );
    registry.add_many(kinds, left);
}

fn head(
    registry: &mut Registry,
    kinds: &[EntityKind],
    names: [&'static str; 3],
    t: HeadTransforms,
) {
    let [item, dragon, piglin] = names;
    let spec = anchored(
        item,
        |st| item_key(&st.extras.head),
        "head",
        Anchor::Head(t),
        |st| head_item_drawn(st.extras.head.id),
    );
    registry.add_many(kinds, spec);

    registry.add_many(
        kinds,
        worn_skull(
            dragon,
            models::dragon_head,
            |_| skull::texture_for_type("dragon").into(),
            |m, p, st| skull::pose_for_type("dragon", m, p, worn_head_animation(st)),
            |st| st.extras.head.id == "dragon_head",
            t,
        ),
    );
    registry.add_many(
        kinds,
        worn_skull(
            piglin,
            models::piglin_head,
            |_| skull::texture_for_type("piglin").into(),
            |m, p, st| skull::pose_for_type("piglin", m, p, worn_head_animation(st)),
            |st| st.extras.head.id == "piglin_head",
            t,
        ),
    );
}

fn worn_skull(
    name: &'static str,
    layer: fn() -> LayerDef,
    texture: fn(&EntityState) -> TexturePath,
    setup: fn(&BakedModel, &mut [PartState], &EntityState),
    visible: fn(&EntityState) -> bool,
    t: HeadTransforms,
) -> RenderSpec {
    RenderSpec::new(name, layer, texture, setup)
        .with_anchor(Anchor::Head(t))
        .with_visible(visible)
        .ignoring_invisibility()
}

pub(crate) fn head_item_drawn(id: &str) -> bool {
    !id.is_empty()
        && !animated_skull(id)
        && (is_skull(id) || !armor::has_layer(Some(id), armor::LAYER_HUMANOID))
}

pub(crate) fn animated_skull(id: &str) -> bool {
    is_skull(id) && skull::animates(skull::skull_type(id))
}

fn worn_head_animation(st: &EntityState) -> f32 {
    st.walk_pos
}

fn carry_spec(name: &'static str, when: fn(&EntityState) -> bool) -> RenderSpec {
    anchored(
        name,
        |st| item_key(&st.extras.main_hand),
        "ground",
        Anchor::Carry,
        when,
    )
}

fn anchored(
    name: &'static str,
    item: fn(&EntityState) -> TexturePath,
    display: &'static str,
    anchor: Anchor,
    when: fn(&EntityState) -> bool,
) -> RenderSpec {
    RenderSpec::item(name, item, display, crate::entities::no_inner)
        .with_anchor(anchor)
        .with_visible(when)
        .ignoring_invisibility()
}

fn humanoid(model: &BakedModel, s: &[PartState], _st: &EntityState) -> Anchors {
    Anchors {
        hands: [
            model.chain(s, model.id("right_arm")),
            model.chain(s, model.id("left_arm")),
        ],
        head: model.chain(s, model.id("head")),
        ..Anchors::default()
    }
}

fn skeleton(model: &BakedModel, s: &[PartState], st: &EntityState) -> Anchors {
    let arm = |name: &str, offset: f32| {
        let id = model.id(name);
        let mut part = s[id];
        part.x += offset;
        model.chain_with(s, id, part.transform().to_matrix())
    };
    Anchors {
        hands: [arm("right_arm", 1.0), arm("left_arm", -1.0)],
        ..humanoid(model, s, st)
    }
}

fn vex(model: &BakedModel, s: &[PartState], _st: &EntityState) -> Anchors {
    let arm = |name, x| {
        model.chain(s, model.id(name))
            * Mat4::from_scale(Vec3::splat(0.55))
            * Mat4::from_translation(Vec3::new(x, -0.156_25, 0.078_125))
    };
    Anchors {
        hands: [arm("right_arm", 0.046_875), arm("left_arm", -0.046_875)],
        ..Anchors::default()
    }
}

fn allay(model: &BakedModel, s: &[PartState], _st: &EntityState) -> Anchors {
    let hand = model.chain(s, model.id("body"))
        * Mat4::from_translation(Vec3::new(0.0, 0.0625, 0.1875))
        * Mat4::from_rotation_x(s[model.id("right_arm")].x_rot)
        * Mat4::from_scale(Vec3::splat(0.7))
        * Mat4::from_translation(Vec3::new(0.0625, 0.0, 0.0));
    Anchors {
        hands: [hand; 2],
        ..Anchors::default()
    }
}

fn copper_golem(model: &BakedModel, s: &[PartState], st: &EntityState) -> Anchors {
    let idle = st.extras.copper_golem_state == 0;
    let arm = |name, right: bool| {
        let chain = model.chain(s, model.id(name));
        if idle {
            let deg: f32 = if right { -90.0 } else { 90.0 };
            chain
                * Mat4::from_rotation_y(deg * DEG_TO_RAD)
                * Mat4::from_translation(Vec3::new(0.0, 0.0, 0.125))
        } else {
            chain
                * Mat4::from_scale(Vec3::splat(0.55))
                * Mat4::from_translation(Vec3::new(-0.125, 0.3125, -0.1875))
        }
    };
    Anchors {
        hands: [arm("right_arm", true), arm("left_arm", false)],
        head: model.chain(s, model.id("head"))
            * Mat4::from_scale_rotation_translation(
                Vec3::splat(1.0625),
                Quat::IDENTITY,
                Vec3::new(0.0, 0.125, 0.0),
            ),
        ..Anchors::default()
    }
}

fn crossed_arms(model: &BakedModel, s: &[PartState]) -> Mat4 {
    model.chain(s, model.id("arms"))
        * Mat4::from_rotation_x(0.75)
        * Mat4::from_scale(Vec3::splat(1.07))
        * Mat4::from_translation(Vec3::new(0.0, 0.13, -0.34))
        * Mat4::from_rotation_x(PI)
}

fn villager(model: &BakedModel, s: &[PartState], _st: &EntityState) -> Anchors {
    Anchors {
        head: model.chain(s, model.id("head")),
        carry: crossed_arms(model, s),
        ..Anchors::default()
    }
}

fn witch(model: &BakedModel, s: &[PartState], st: &EntityState) -> Anchors {
    let carry = if st.extras.main_hand.id == "potion" {
        model.chain(s, model.id("nose"))
            * Mat4::from_translation(Vec3::new(0.0625, 0.25, 0.0))
            * Mat4::from_rotation_z(PI)
            * Mat4::from_rotation_x(140.0 * DEG_TO_RAD)
            * Mat4::from_rotation_z(10.0 * DEG_TO_RAD)
            * Mat4::from_rotation_x(PI)
    } else {
        crossed_arms(model, s)
    };
    Anchors {
        carry,
        ..Anchors::default()
    }
}

fn fox(model: &BakedModel, s: &[PartState], st: &EntityState) -> Anchors {
    let head = &s[model.id("head")];
    let baby = st.extras.is_baby;
    let sleeping = st.extras.sleeping;
    let mut carry = Mat4::from_translation(Vec3::new(head.x, head.y, head.z) / 16.0);
    if baby {
        carry *= Mat4::from_scale(Vec3::splat(0.75));
    }
    carry *= Mat4::from_rotation_z(st.extras.head_roll_angle)
        * Mat4::from_rotation_y(st.y_rot * DEG_TO_RAD)
        * Mat4::from_rotation_x(st.x_rot * DEG_TO_RAD);
    let shift = match (baby, sleeping) {
        (true, true) => Vec3::new(0.4, 0.26, 0.15),
        (true, false) => Vec3::new(0.06, 0.26, -0.5),
        (false, true) => Vec3::new(0.46, 0.26, 0.22),
        (false, false) => Vec3::new(0.06, 0.27, -0.5),
    };
    carry *= Mat4::from_translation(shift) * Mat4::from_rotation_x(90.0 * DEG_TO_RAD);
    if sleeping {
        carry *= Mat4::from_rotation_z(90.0 * DEG_TO_RAD);
    }
    Anchors {
        carry,
        ..Anchors::default()
    }
}

fn panda(_model: &BakedModel, _s: &[PartState], st: &EntityState) -> Anchors {
    let (mut y, mut z) = (1.4, -0.6);
    if st.extras.eating {
        let wobble = (st.age_ticks * 0.6).sin();
        z -= 0.2 * wobble + 0.2;
        y -= 0.09 * wobble;
    }
    Anchors {
        carry: Mat4::from_translation(Vec3::new(0.1, y, z)),
        ..Anchors::default()
    }
}

fn dolphin(_model: &BakedModel, _s: &[PartState], st: &EntityState) -> Anchors {
    let pitch = st.x_rot.abs() / 60.0;
    let offset = if st.x_rot < 0.0 {
        Vec3::new(0.0, 1.0 - pitch * 0.5, -1.0 + pitch * 0.5)
    } else {
        Vec3::new(0.0, 1.0 + pitch * 0.8, -1.0 + pitch * 0.2)
    };
    Anchors {
        carry: Mat4::from_translation(offset),
        ..Anchors::default()
    }
}

#[cfg(test)]
mod tests {
    use crate::entities::registry::Registry;
    use crate::entities::state::EntityState;

    #[test]
    fn every_anchor_hook_finds_its_parts() {
        let registry = Registry::build();
        let specs = &registry.specs;
        let anchored = registry.registered_kinds().flat_map(|kind| {
            registry
                .specs_for(kind)
                .iter()
                .map(move |&i| (kind, &specs[i]))
        });
        for (kind, spec) in anchored {
            let (Some(hook), Some(geom)) = (spec.anchors, spec.model()) else {
                continue;
            };
            let model = crate::entities::geom::bake(&(geom.layer)());
            let mut parts = model.rest_pose();
            for flag in [false, true] {
                let mut st = EntityState::new(0, kind);
                st.extras.eating = flag;
                let shared = st.extras.shared_mut();
                shared.is_baby = flag;
                shared.sleeping = flag;
                shared.copper_golem_state = u8::from(flag);
                if flag {
                    shared.main_hand.id = "potion";
                }
                (geom.setup)(&model, &mut parts, &st);
                let anchors = hook(&model, &parts, &st);
                assert!(anchors.hands[0].is_finite(), "{}", spec.name);
                assert!(anchors.carry.is_finite(), "{}", spec.name);
            }
        }
    }

    #[test]
    fn worn_skulls_pose_and_split_from_the_item() {
        let registry = Registry::build();
        let mut seen = 0;
        for spec in registry
            .specs
            .iter()
            .filter(|s| s.name.starts_with("head_dragon") || s.name.starts_with("head_piglin"))
        {
            let geom = spec.model().expect("a worn skull is a model");
            let model = crate::entities::geom::bake(&(geom.layer)());
            let mut parts = model.rest_pose();
            let mut st = EntityState::new(0, azalea_registry::builtin::EntityKind::Zombie);
            st.walk_pos = 2.5;
            (geom.setup)(&model, &mut parts, &st);
            seen += 1;
        }
        assert_eq!(seen, 6);
        assert!(!super::head_item_drawn("dragon_head"));
        assert!(!super::head_item_drawn("piglin_head"));
        assert!(super::head_item_drawn("zombie_head"));
        assert!(super::head_item_drawn("carved_pumpkin"));
    }
}
