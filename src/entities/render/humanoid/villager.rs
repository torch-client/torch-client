use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::geom::{BakedModel, LayerDef, PartState};
use crate::entities::models::humanoid::{villager, zombie};
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

use super::armor;
use super::zombie::{is_adult, is_baby};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Villager,
        RenderSpec::new(
            "villager",
            villager::villager_layer,
            villager_texture,
            villager::setup_anim,
        )
        .with_visible(is_adult),
    );
    registry.add(
        EntityKind::Villager,
        RenderSpec::new(
            "villager_baby",
            villager::baby_villager_layer,
            villager_texture,
            villager::setup_anim,
        )
        .with_visible(is_baby),
    );
    register_profession_layers(
        registry,
        EntityKind::Villager,
        ProfessionLayers {
            adult: villager::villager_layer,
            adult_no_hat: villager::villager_no_hat_layer,
            baby: villager::baby_villager_layer,
            baby_no_hat: villager::baby_villager_no_hat_layer,
            setup: villager::setup_anim,
            textures: [
                villager_type_texture,
                villager_baby_type_texture,
                villager_profession_texture,
                villager_level_texture,
            ],
            visible: [
                villager_adult_hatted,
                villager_adult_hatless,
                villager_baby_hatted,
                villager_baby_hatless,
                villager_has_profession,
                villager_has_level,
            ],
            names: [
                "villager_type",
                "villager_type_no_hat",
                "villager_baby_type",
                "villager_baby_type_no_hat",
                "villager_profession",
                "villager_profession_level",
            ],
        },
    );

    registry.add(
        EntityKind::WanderingTrader,
        RenderSpec::new(
            "wandering_trader",
            villager::villager_layer,
            wandering_trader_texture,
            villager::setup_anim,
        ),
    );

    registry.add(
        EntityKind::Witch,
        RenderSpec::new(
            "witch",
            villager::witch_layer,
            witch_texture,
            villager::witch_setup_anim,
        ),
    );

    registry.add(
        EntityKind::ZombieVillager,
        RenderSpec::new(
            "zombie_villager",
            zombie::zombie_villager_layer,
            zombie_villager_texture,
            zombie::setup_anim,
        )
        .with_visible(is_adult),
    );
    registry.add(
        EntityKind::ZombieVillager,
        RenderSpec::new(
            "zombie_villager_baby",
            zombie::baby_zombie_villager_layer,
            zombie_villager_texture,
            zombie::setup_anim,
        )
        .with_visible(is_baby),
    );
    armor::register_set(
        registry,
        &[EntityKind::ZombieVillager],
        [
            "zombie_villager_helmet",
            "zombie_villager_chestplate",
            "zombie_villager_leggings",
            "zombie_villager_boots",
        ],
        armor::ZOMBIE_VILLAGER,
        zombie::setup_anim,
        armor::ADULT,
    );
    register_profession_layers(
        registry,
        EntityKind::ZombieVillager,
        ProfessionLayers {
            adult: zombie::zombie_villager_layer,
            adult_no_hat: zombie::zombie_villager_no_hat_layer,
            baby: zombie::baby_zombie_villager_layer,
            baby_no_hat: zombie::baby_zombie_villager_no_hat_layer,
            setup: zombie::setup_anim,
            textures: [
                zombie_villager_type_texture,
                zombie_villager_baby_type_texture,
                zombie_villager_profession_texture,
                zombie_villager_level_texture,
            ],
            visible: [
                zombie_adult_hatted,
                zombie_adult_hatless,
                zombie_baby_hatted,
                zombie_baby_hatless,
                zombie_has_profession,
                zombie_has_level,
            ],
            names: [
                "zombie_villager_type",
                "zombie_villager_type_no_hat",
                "zombie_villager_baby_type",
                "zombie_villager_baby_type_no_hat",
                "zombie_villager_profession",
                "zombie_villager_profession_level",
            ],
        },
    );
}

fn villager_texture(st: &EntityState) -> String {
    if st.extras.is_baby {
        "entity/villager/villager_baby".to_string()
    } else {
        "entity/villager/villager".to_string()
    }
}

fn wandering_trader_texture(_st: &EntityState) -> String {
    "entity/wandering_trader/wandering_trader".to_string()
}

fn witch_texture(_st: &EntityState) -> String {
    "entity/witch/witch".to_string()
}

fn zombie_villager_texture(st: &EntityState) -> String {
    if st.extras.is_baby {
        "entity/zombie_villager/zombie_villager_baby".to_string()
    } else {
        "entity/zombie_villager/zombie_villager".to_string()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Hat {
    None,
    Partial,
    Full,
}

const FULL_HAT_TYPES: [&str; 2] = ["desert", "snow"];

const FULL_HAT_PROFESSIONS: [&str; 5] =
    ["farmer", "fisherman", "fletcher", "librarian", "shepherd"];

const PARTIAL_HAT_PROFESSIONS: [&str; 1] = ["butcher"];

const LEVEL_LOCATIONS: [&str; 5] = ["stone", "iron", "gold", "emerald", "diamond"];

fn villager_type(st: &EntityState) -> &str {
    match &st.extras.villager_kind {
        Some(id) => id.rsplit(':').next().unwrap_or("plains"),
        None => "plains",
    }
}

fn profession(st: &EntityState) -> &str {
    match &st.extras.villager_profession {
        Some(id) => id.rsplit(':').next().unwrap_or("none"),
        None => "none",
    }
}

fn type_hat(st: &EntityState, zombie_villager: bool) -> Hat {
    if zombie_villager {
        Hat::None
    } else if FULL_HAT_TYPES.contains(&villager_type(st)) {
        Hat::Full
    } else {
        Hat::None
    }
}

fn profession_hat(st: &EntityState) -> Hat {
    let p = profession(st);
    if FULL_HAT_PROFESSIONS.contains(&p) {
        Hat::Full
    } else if PARTIAL_HAT_PROFESSIONS.contains(&p) {
        Hat::Partial
    } else {
        Hat::None
    }
}

fn type_hat_visible(st: &EntityState, zombie_villager: bool) -> bool {
    let profession = profession_hat(st);
    profession == Hat::None
        || (profession == Hat::Partial && type_hat(st, zombie_villager) != Hat::Full)
}

struct ProfessionLayers {
    adult: fn() -> LayerDef,
    adult_no_hat: fn() -> LayerDef,
    baby: fn() -> LayerDef,
    baby_no_hat: fn() -> LayerDef,
    setup: fn(&BakedModel, &mut [PartState], &EntityState),
    textures: [fn(&EntityState) -> String; 4],
    visible: [fn(&EntityState) -> bool; 6],
    names: [&'static str; 6],
}

fn register_profession_layers(registry: &mut Registry, kind: EntityKind, l: ProfessionLayers) {
    let specs: [(
        &'static str,
        fn() -> LayerDef,
        fn(&EntityState) -> String,
        i32,
    ); 6] = [
        (l.names[0], l.adult, l.textures[0], 1),
        (l.names[1], l.adult_no_hat, l.textures[0], 1),
        (l.names[2], l.baby, l.textures[1], 1),
        (l.names[3], l.baby_no_hat, l.textures[1], 1),
        (l.names[4], l.adult, l.textures[2], 2),
        (l.names[5], l.adult, l.textures[3], 3),
    ];
    for (i, (name, layer, texture, depth_bias)) in specs.into_iter().enumerate() {
        registry.add(
            kind,
            RenderSpec::new(name, layer, texture, l.setup)
                .with_visible(l.visible[i])
                .with_depth_bias(depth_bias),
        );
    }
}

fn villager_type_texture(st: &EntityState) -> String {
    format!("entity/villager/type/{}", villager_type(st))
}

fn villager_baby_type_texture(st: &EntityState) -> String {
    format!("entity/villager/baby/{}", villager_type(st))
}

fn villager_profession_texture(st: &EntityState) -> String {
    format!("entity/villager/profession/{}", profession_or_farmer(st))
}

fn villager_level_texture(st: &EntityState) -> String {
    format!("entity/villager/profession_level/{}", level(st))
}

fn zombie_villager_type_texture(st: &EntityState) -> String {
    format!("entity/zombie_villager/type/{}", villager_type(st))
}

fn zombie_villager_baby_type_texture(st: &EntityState) -> String {
    format!("entity/zombie_villager/baby/{}", villager_type(st))
}

fn zombie_villager_profession_texture(st: &EntityState) -> String {
    format!(
        "entity/zombie_villager/profession/{}",
        profession_or_farmer(st)
    )
}

fn zombie_villager_level_texture(st: &EntityState) -> String {
    format!("entity/zombie_villager/profession_level/{}", level(st))
}

fn profession_or_farmer(st: &EntityState) -> &str {
    match profession(st) {
        "none" => "farmer",
        other => other,
    }
}

fn level(st: &EntityState) -> &'static str {
    let level = st
        .extras
        .villager_level
        .clamp(1, LEVEL_LOCATIONS.len() as u32);
    LEVEL_LOCATIONS[level as usize - 1]
}

fn villager_adult_hatted(st: &EntityState) -> bool {
    !st.extras.is_baby && type_hat_visible(st, false)
}
fn villager_adult_hatless(st: &EntityState) -> bool {
    !st.extras.is_baby && !type_hat_visible(st, false)
}
fn villager_baby_hatted(st: &EntityState) -> bool {
    st.extras.is_baby && type_hat_visible(st, false)
}
fn villager_baby_hatless(st: &EntityState) -> bool {
    st.extras.is_baby && !type_hat_visible(st, false)
}
fn villager_has_profession(st: &EntityState) -> bool {
    !st.extras.is_baby && profession(st) != "none"
}
fn villager_has_level(st: &EntityState) -> bool {
    villager_has_profession(st) && profession(st) != "nitwit"
}

fn zombie_adult_hatted(st: &EntityState) -> bool {
    !st.extras.is_baby && type_hat_visible(st, true)
}
fn zombie_adult_hatless(st: &EntityState) -> bool {
    !st.extras.is_baby && !type_hat_visible(st, true)
}
fn zombie_baby_hatted(st: &EntityState) -> bool {
    st.extras.is_baby && type_hat_visible(st, true)
}
fn zombie_baby_hatless(st: &EntityState) -> bool {
    st.extras.is_baby && !type_hat_visible(st, true)
}
fn zombie_has_profession(st: &EntityState) -> bool {
    !st.extras.is_baby && profession(st) != "none"
}
fn zombie_has_level(st: &EntityState) -> bool {
    zombie_has_profession(st) && profession(st) != "nitwit"
}
