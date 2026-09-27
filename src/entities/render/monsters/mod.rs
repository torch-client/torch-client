use crate::entities::registry::Registry;

pub mod blaze;
pub mod breeze;
pub mod copper_golem;
pub mod creaking;
pub mod creeper;
pub mod dragon;
pub mod enderman;
pub mod guardian;
pub mod iron_golem;
pub mod phantom;
pub mod ravager;
pub mod shulker;
pub mod silverfish;
pub mod slime;
pub mod snow_golem;
pub mod spider;
pub mod warden;
pub mod wither;

pub fn register(registry: &mut Registry) {
    blaze::register(registry);
    breeze::register(registry);
    copper_golem::register(registry);
    creaking::register(registry);
    creeper::register(registry);
    dragon::register(registry);
    enderman::register(registry);
    guardian::register(registry);
    iron_golem::register(registry);
    phantom::register(registry);
    ravager::register(registry);
    shulker::register(registry);
    silverfish::register(registry);
    slime::register(registry);
    snow_golem::register(registry);
    spider::register(registry);
    warden::register(registry);
    wither::register(registry);
}

#[cfg(test)]
mod tests {
    use azalea_registry::builtin::EntityKind;

    use crate::entities::registry::Registry;
    use crate::entities::state::{Direction, EntityState, Pose};
    use crate::entities::{CameraView, Geom};

    fn model_geom(spec: &crate::entities::RenderSpec) -> &crate::entities::ModelGeom {
        match &spec.geom {
            Geom::Model(geom) => geom,
            Geom::Item(_) | Geom::Block(_) | Geom::Built(_) => {
                panic!("{} is not a Geom::Model spec", spec.name)
            }
        }
    }

    const EXPECTED: [(EntityKind, usize); 22] = [
        (EntityKind::Creeper, 2),
        (EntityKind::Spider, 2),
        (EntityKind::CaveSpider, 2),
        (EntityKind::Silverfish, 1),
        (EntityKind::Endermite, 1),
        (EntityKind::Enderman, 2),
        (EntityKind::Blaze, 1),
        (EntityKind::Slime, 2),
        (EntityKind::MagmaCube, 1),
        (EntityKind::Guardian, 1),
        (EntityKind::ElderGuardian, 1),
        (EntityKind::Shulker, 1),
        (EntityKind::Ravager, 1),
        (EntityKind::Wither, 2),
        (EntityKind::EnderDragon, 2),
        (EntityKind::Breeze, 3),
        (EntityKind::Creaking, 2),
        (EntityKind::Warden, 6),
        (EntityKind::IronGolem, 2),
        (EntityKind::SnowGolem, 1),
        (EntityKind::CopperGolem, 2),
        (EntityKind::Phantom, 2),
    ];

    #[test]
    fn every_monster_kind_draws_its_layers() {
        let registry = Registry::build();
        for (kind, count) in EXPECTED {
            assert_eq!(registry.specs_for(kind).len(), count, "{kind:?}");
        }
    }

    #[test]
    fn every_setup_anim_finds_its_parts() {
        let registry = Registry::build();
        for (kind, _) in EXPECTED {
            for &i in registry.specs_for(kind) {
                let spec = &registry.specs[i];
                let geom = model_geom(spec);
                let model = crate::entities::geom::bake(&(geom.layer)());
                let mut parts = model.rest_pose();
                let camera = CameraView::default();
                for flag in [false, true] {
                    let mut st = EntityState::new(0, kind);
                    st.age_ticks = 7.5;
                    st.walk_speed = 0.5;
                    st.walk_pos = 3.0;
                    st.attack_time = if flag { 0.5 } else { 0.0 };
                    st.death_time = if flag { 4.0 } else { 0.0 };
                    st.swing_left = flag;
                    st.extras.squish = if flag { 0.6 } else { 0.0 };
                    st.extras.peek = if flag { 0.8 } else { 0.0 };
                    st.extras.spikes_animation = if flag { 1.0 } else { 0.0 };
                    st.extras.tail_animation = if flag { 2.0 } else { 0.0 };
                    st.extras.attack_ticks = if flag { 7.0 } else { 0.0 };
                    st.extras.stunned_ticks = if flag { 20.0 } else { 0.0 };
                    st.extras.roar_animation = if flag { 0.5 } else { 0.0 };
                    st.extras.offer_flower_ticks = if flag { 30 } else { 0 };
                    st.extras.shared_mut().invulnerable_ticks = if flag { 100.0 } else { 0.0 };
                    st.extras.flap_time = if flag { 3.0 } else { 0.0 };
                    st.extras.tendril_animation = if flag { 1.0 } else { 0.0 };
                    st.extras.heart_animation = if flag { 1.0 } else { 0.0 };
                    st.extras.shared_mut().can_move = flag;
                    st.extras.shared_mut().tearing_down = flag;
                    st.extras.shared_mut().creepy = flag;
                    st.extras.shared_mut().carried_block =
                        flag.then(|| "minecraft:grass_block".to_string());
                    st.extras.shared_mut().main_hand.id = if flag { "copper_ingot" } else { "" };
                    for pose in [
                        Pose::Standing,
                        Pose::Emerging,
                        Pose::Digging,
                        Pose::Roaring,
                        Pose::Sniffing,
                        Pose::Shooting,
                        Pose::Inhaling,
                        Pose::Sliding,
                        Pose::LongJumping,
                    ] {
                        st.pose = pose;
                        for state in 0..5u8 {
                            st.extras.shared_mut().copper_golem_state = state;
                            model.reset_pose(&mut parts);
                            (geom.setup)(&model, &mut parts, &st);
                            let _ = spec.root.pose(&st, &camera);
                            let _ = (spec.visible)(&st);
                            let _ = (spec.tint)(&st);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_monster_state_texture_exists() {
        let registry = Registry::build();
        let check = |kind: EntityKind, st: &EntityState| {
            for &i in registry.specs_for(kind) {
                let path = (model_geom(&registry.specs[i]).texture)(st);
                let file = crate::assets_root().join(format!("textures/{path}.png"));
                assert!(file.exists(), "{path} missing for {kind:?}");
            }
        };

        for color in 0..=16u8 {
            let mut st = EntityState::new(0, EntityKind::Shulker);
            st.extras.shared_mut().color = color;
            check(EntityKind::Shulker, &st);
        }
        for ticks in [0, 3, 7, 82, 220] {
            let mut st = EntityState::new(0, EntityKind::Wither);
            st.extras.shared_mut().invulnerable_ticks = ticks as f32;
            check(EntityKind::Wither, &st);
        }
        for level in 0..4u8 {
            let mut st = EntityState::new(0, EntityKind::IronGolem);
            st.extras.shared_mut().crackiness = level;
            check(EntityKind::IronGolem, &st);
        }
        for weathering in 0..4u8 {
            let mut st = EntityState::new(0, EntityKind::CopperGolem);
            st.extras.shared_mut().weather_state = weathering;
            check(EntityKind::CopperGolem, &st);
        }
    }

    #[test]
    fn a_floor_shulker_is_not_rotated_off_its_base() {
        let registry = Registry::build();
        let spec_ids = registry.specs_for(EntityKind::Shulker);
        let spec = &registry.specs[spec_ids[0]];

        let camera = CameraView::default();
        let mut st = EntityState::new(0, EntityKind::Shulker);
        st.extras.shared_mut().attach_face = Direction::Down;
        let pose = spec.root.pose(&st, &camera);
        assert!(
            pose.extra_rotation
                .angle_between(bevy::prelude::Quat::IDENTITY)
                < 1e-5
        );
        assert!(pose.extra_offset.length() < 1e-5);

        st.extras.shared_mut().attach_face = Direction::Up;
        let pose = spec.root.pose(&st, &camera);
        let mapped = pose.extra_rotation * bevy::prelude::Vec3::ZERO + pose.extra_offset;
        assert!((mapped.y - 1.0).abs() < 1e-5, "{mapped:?}");
    }
}
