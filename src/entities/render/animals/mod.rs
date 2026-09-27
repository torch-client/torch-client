use crate::entities::registry::Registry;

pub mod armadillo;
pub mod camel;
pub mod cat;
pub mod chicken;
pub mod cow;
pub mod donkey;
pub mod dye;
pub mod fox;
pub mod goat;
pub mod hoglin;
pub mod horse;
pub mod llama;
pub mod mooshroom;
pub mod ocelot;
pub mod panda;
pub mod pig;
pub mod polar_bear;
pub mod rabbit;
pub mod sheep;
pub mod sniffer;
pub mod strider;
pub mod turtle;
pub mod undead_horse;
pub mod wolf;

pub fn register(registry: &mut Registry) {
    armadillo::register(registry);
    camel::register(registry);
    cat::register(registry);
    chicken::register(registry);
    cow::register(registry);
    donkey::register(registry);
    fox::register(registry);
    goat::register(registry);
    hoglin::register(registry);
    horse::register(registry);
    llama::register(registry);
    mooshroom::register(registry);
    ocelot::register(registry);
    panda::register(registry);
    pig::register(registry);
    polar_bear::register(registry);
    rabbit::register(registry);
    sheep::register(registry);
    sniffer::register(registry);
    strider::register(registry);
    turtle::register(registry);
    undead_horse::register(registry);
    wolf::register(registry);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::geom::bake;
    use crate::entities::{EntityState, Geom};
    use azalea_registry::builtin::EntityKind;

    #[test]
    fn every_animal_setup_finds_the_parts_it_writes() {
        const KINDS: [EntityKind; 28] = [
            EntityKind::Pig,
            EntityKind::Cow,
            EntityKind::Mooshroom,
            EntityKind::Sheep,
            EntityKind::Chicken,
            EntityKind::Rabbit,
            EntityKind::Fox,
            EntityKind::Wolf,
            EntityKind::Cat,
            EntityKind::Ocelot,
            EntityKind::Panda,
            EntityKind::PolarBear,
            EntityKind::Llama,
            EntityKind::TraderLlama,
            EntityKind::Horse,
            EntityKind::Donkey,
            EntityKind::Mule,
            EntityKind::SkeletonHorse,
            EntityKind::ZombieHorse,
            EntityKind::Camel,
            EntityKind::CamelHusk,
            EntityKind::Goat,
            EntityKind::Hoglin,
            EntityKind::Zoglin,
            EntityKind::Strider,
            EntityKind::Armadillo,
            EntityKind::Sniffer,
            EntityKind::Turtle,
        ];
        let registry = Registry::build();
        let mut indices: Vec<usize> = KINDS
            .iter()
            .flat_map(|kind| registry.specs_for(*kind).to_vec())
            .collect();
        indices.sort_unstable();
        indices.dedup();
        assert!(!indices.is_empty());
        for spec in indices.into_iter().map(|i| &registry.specs[i]) {
            let geom = match &spec.geom {
                Geom::Model(geom) => geom,
                Geom::Item(_) | Geom::Block(_) | Geom::Built(_) => {
                    panic!("{} is not a Geom::Model spec", spec.name)
                }
            };
            let model = bake(&(geom.layer)());
            let mut parts = model.rest_pose();
            for baby in [false, true] {
                for flag in [false, true] {
                    let mut st = EntityState::new(0, EntityKind::Pig);
                    st.extras.shared_mut().is_baby = baby;
                    st.extras.shared_mut().sitting = flag;
                    st.extras.shared_mut().sleeping = flag;
                    st.extras.shared_mut().crouching = flag;
                    st.extras.shared_mut().sprinting = flag;
                    st.extras.shared_mut().pouncing = flag;
                    st.extras.shared_mut().faceplanted = flag;
                    st.extras.shared_mut().has_chest = flag;
                    st.extras.shared_mut().has_egg = flag;
                    st.extras.shared_mut().on_land = flag;
                    st.extras.shared_mut().ridden = flag;
                    st.extras.shared_mut().dash = flag;
                    st.extras.shared_mut().armadillo_state = u8::from(flag);
                    st.extras.shared_mut().sniffer_state = if flag { 4 } else { 0 };
                    st.extras.lie_down_amount = if flag { 1.0 } else { 0.0 };
                    st.extras.sit_amount = if flag { 1.0 } else { 0.0 };
                    model.reset_pose(&mut parts);
                    (geom.setup)(&model, &mut parts, &st);
                }
            }
        }
    }
}
