use crate::entities::registry::Registry;

pub mod allay;
pub mod axolotl;
pub mod bat;
pub mod bee;
pub mod dolphin;
pub mod fish;
pub mod frog;
pub mod ghast;
pub mod nautilus;
pub mod parrot;
pub mod root;
pub mod squid;

pub fn register(registry: &mut Registry) {
    allay::register(registry);
    axolotl::register(registry);
    bat::register(registry);
    bee::register(registry);
    dolphin::register(registry);
    fish::register(registry);
    frog::register(registry);
    ghast::register(registry);
    nautilus::register(registry);
    parrot::register(registry);
    squid::register(registry);
}

#[cfg(test)]
mod tests {
    use azalea_registry::builtin::EntityKind;

    use crate::entities::Geom;
    use crate::entities::registry::Registry;
    use crate::entities::state::EntityState;

    fn model_geom(spec: &crate::entities::RenderSpec) -> &crate::entities::ModelGeom {
        match &spec.geom {
            Geom::Model(geom) => geom,
            Geom::Item(_) | Geom::Block(_) | Geom::Built(_) => {
                panic!("{} is not a Geom::Model spec", spec.name)
            }
        }
    }

    #[test]
    fn every_aquatic_kind_draws_its_layers() {
        let registry = Registry::build();
        let expected = [
            (EntityKind::Squid, 2),
            (EntityKind::GlowSquid, 2),
            (EntityKind::Dolphin, 2),
            (EntityKind::Cod, 1),
            (EntityKind::Salmon, 3),
            (EntityKind::TropicalFish, 4),
            (EntityKind::Pufferfish, 3),
            (EntityKind::Axolotl, 2),
            (EntityKind::Tadpole, 1),
            (EntityKind::Frog, 1),
            (EntityKind::Nautilus, 4),
            (EntityKind::ZombieNautilus, 4),
            (EntityKind::Bat, 1),
            (EntityKind::Bee, 2),
            (EntityKind::Parrot, 1),
            (EntityKind::Allay, 1),
            (EntityKind::Ghast, 1),
            (EntityKind::HappyGhast, 6),
        ];
        for (kind, count) in expected {
            assert_eq!(registry.specs_for(kind).len(), count, "{kind:?}");
        }
    }

    #[test]
    fn exactly_one_body_is_visible_per_size_and_puff_state() {
        let registry = Registry::build();
        let visible = |kind: EntityKind, st: &EntityState| {
            registry
                .specs_for(kind)
                .iter()
                .filter(|&&i| (registry.specs[i].visible)(st))
                .count()
        };

        for baby in [false, true] {
            let mut st = EntityState::new(0, EntityKind::Squid);
            st.extras.shared_mut().is_baby = baby;
            assert_eq!(visible(EntityKind::Squid, &st), 1);
            assert_eq!(visible(EntityKind::GlowSquid, &st), 1);
            assert_eq!(visible(EntityKind::Dolphin, &st), 1);
            assert_eq!(visible(EntityKind::Axolotl, &st), 1);
            assert_eq!(visible(EntityKind::Bee, &st), 1);
            assert_eq!(visible(EntityKind::Nautilus, &st), 1);
            assert_eq!(visible(EntityKind::HappyGhast, &st), 1);
        }

        for id in -1..4 {
            let mut st = EntityState::new(0, EntityKind::Salmon);
            st.extras.shared_mut().variant_id = id;
            assert_eq!(visible(EntityKind::Salmon, &st), 1, "salmon variant {id}");
        }

        for puff in -1..4 {
            let mut st = EntityState::new(0, EntityKind::Pufferfish);
            st.extras.shared_mut().puff_state = puff;
            assert_eq!(visible(EntityKind::Pufferfish, &st), 1, "puff {puff}");
        }

        for base in 0..2 {
            let mut st = EntityState::new(0, EntityKind::TropicalFish);
            st.extras.shared_mut().variant_id = base;
            assert_eq!(visible(EntityKind::TropicalFish, &st), 2, "base {base}");
        }

        for variant in ["minecraft:temperate", "minecraft:warm"] {
            let mut st = EntityState::new(0, EntityKind::ZombieNautilus);
            st.extras.shared_mut().variant = Some(variant.to_string());
            assert_eq!(visible(EntityKind::ZombieNautilus, &st), 1, "{variant}");
        }
    }

    #[test]
    fn every_tropical_fish_pattern_texture_exists() {
        let registry = Registry::build();
        for base in 0..2 {
            for index in 0..6 {
                let mut st = EntityState::new(0, EntityKind::TropicalFish);
                st.extras.shared_mut().variant_id = base | index << 8;
                for &i in registry.specs_for(EntityKind::TropicalFish) {
                    let spec = &registry.specs[i];
                    if !(spec.visible)(&st) {
                        continue;
                    }
                    let path = (model_geom(spec).texture)(&st);
                    let file = crate::assets_root().join(format!("textures/{path}.png"));
                    assert!(file.exists(), "{path} missing for {base}/{index}");
                }
            }
        }
    }

    #[test]
    fn every_aquatic_variant_texture_exists() {
        let registry = Registry::build();
        let check = |kind: EntityKind, st: &EntityState| {
            for &i in registry.specs_for(kind) {
                let path = (model_geom(&registry.specs[i]).texture)(st);
                let file = crate::assets_root().join(format!("textures/{path}.png"));
                assert!(file.exists(), "{path} missing");
            }
        };

        for id in 0..5 {
            for baby in [false, true] {
                let mut st = EntityState::new(0, EntityKind::Axolotl);
                st.extras.shared_mut().variant_id = id;
                st.extras.shared_mut().is_baby = baby;
                check(EntityKind::Axolotl, &st);
            }
            let mut st = EntityState::new(0, EntityKind::Parrot);
            st.extras.shared_mut().variant_id = id;
            check(EntityKind::Parrot, &st);
        }

        for material in [
            "minecraft:copper",
            "minecraft:iron",
            "minecraft:gold",
            "minecraft:diamond",
            "minecraft:netherite",
        ] {
            let mut st = EntityState::new(0, EntityKind::Nautilus);
            st.extras.shared_mut().body_armor = Some(material);
            st.extras.shared_mut().saddled = true;
            check(EntityKind::Nautilus, &st);
        }

        for harness in [
            "minecraft:white_harness",
            "minecraft:orange_harness",
            "minecraft:magenta_harness",
            "minecraft:light_blue_harness",
            "minecraft:yellow_harness",
            "minecraft:lime_harness",
            "minecraft:pink_harness",
            "minecraft:gray_harness",
            "minecraft:light_gray_harness",
            "minecraft:cyan_harness",
            "minecraft:purple_harness",
            "minecraft:blue_harness",
            "minecraft:brown_harness",
            "minecraft:green_harness",
            "minecraft:red_harness",
            "minecraft:black_harness",
        ] {
            let mut st = EntityState::new(0, EntityKind::HappyGhast);
            st.extras.shared_mut().body_armor = Some(harness);
            check(EntityKind::HappyGhast, &st);
        }

        for variant in ["cold", "temperate", "warm"] {
            let mut st = EntityState::new(0, EntityKind::Frog);
            st.extras.shared_mut().variant = Some(format!("minecraft:{variant}"));
            check(EntityKind::Frog, &st);
        }

        for angry in [false, true] {
            for nectar in [false, true] {
                for baby in [false, true] {
                    let mut st = EntityState::new(0, EntityKind::Bee);
                    st.extras.shared_mut().bee_angry = angry;
                    st.extras.shared_mut().has_nectar = nectar;
                    st.extras.shared_mut().is_baby = baby;
                    check(EntityKind::Bee, &st);
                }
            }
        }

        for charged in [false, true] {
            let mut st = EntityState::new(0, EntityKind::Ghast);
            st.extras.shared_mut().charged = charged;
            check(EntityKind::Ghast, &st);
        }
    }

    #[test]
    fn every_setup_anim_finds_its_parts() {
        let registry = Registry::build();
        let kinds = [
            EntityKind::Squid,
            EntityKind::GlowSquid,
            EntityKind::Dolphin,
            EntityKind::Cod,
            EntityKind::Salmon,
            EntityKind::TropicalFish,
            EntityKind::Pufferfish,
            EntityKind::Axolotl,
            EntityKind::Tadpole,
            EntityKind::Frog,
            EntityKind::Nautilus,
            EntityKind::ZombieNautilus,
            EntityKind::Bat,
            EntityKind::Bee,
            EntityKind::Parrot,
            EntityKind::Allay,
            EntityKind::Ghast,
            EntityKind::HappyGhast,
        ];
        for kind in kinds {
            for &i in registry.specs_for(kind) {
                let spec = &registry.specs[i];
                let geom = model_geom(spec);
                let model = crate::entities::geom::bake(&(geom.layer)());
                let mut parts = model.rest_pose();
                for flag in [false, true] {
                    let mut st = EntityState::new(0, kind);
                    st.age_ticks = 7.5;
                    st.walk_speed = 0.5;
                    st.walk_pos = 3.0;
                    st.is_in_water = flag;
                    st.extras.shared_mut().is_baby = flag;
                    st.extras.shared_mut().resting = flag;
                    st.extras.shared_mut().dancing = flag;
                    st.extras.spinning = flag;
                    st.extras.shared_mut().moving = flag;
                    st.extras.on_ground = flag;
                    st.extras.shared_mut().is_ridden = flag;
                    st.extras.bee_roll = if flag { 0.5 } else { 0.0 };
                    st.extras.jump_progress = if flag { 0.5 } else { 0.0 };
                    st.extras.playing_dead_factor = if flag { 1.0 } else { 0.0 };
                    st.extras.on_ground_factor = if flag { 1.0 } else { 0.0 };
                    st.extras.moving_factor = if flag { 1.0 } else { 0.0 };
                    st.extras.shared_mut().body_armor = flag.then_some("minecraft:copper");
                    for pose in [
                        crate::entities::Pose::Standing,
                        crate::entities::Pose::Croaking,
                        crate::entities::Pose::UsingTongue,
                    ] {
                        st.pose = pose;
                        for parrot_pose in 0..5 {
                            st.extras.parrot_pose = parrot_pose;
                            model.reset_pose(&mut parts);
                            (geom.setup)(&model, &mut parts, &st);
                        }
                    }
                }
            }
        }
    }
}
