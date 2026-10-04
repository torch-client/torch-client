use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::aquatic::nautilus;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Nautilus,
        RenderSpec::new(
            "nautilus",
            nautilus::nautilus_layer,
            adult_texture,
            nautilus::setup_anim,
        )
        .with_visible(is_adult),
    );
    registry.add(
        EntityKind::Nautilus,
        RenderSpec::new(
            "nautilus_baby",
            nautilus::baby_layer,
            baby_texture,
            nautilus::setup_anim,
        )
        .with_visible(is_baby),
    );
    registry.add(
        EntityKind::Nautilus,
        RenderSpec::new(
            "nautilus_armor",
            nautilus::armor_layer,
            body_armor_texture,
            nautilus::setup_anim,
        )
        .with_visible(shows_body_armor),
    );
    registry.add(
        EntityKind::Nautilus,
        RenderSpec::new(
            "nautilus_saddle",
            nautilus::saddle_layer,
            saddle_texture,
            nautilus::setup_anim,
        )
        .with_visible(shows_saddle),
    );

    registry.add(
        EntityKind::ZombieNautilus,
        RenderSpec::new(
            "zombie_nautilus",
            nautilus::nautilus_layer,
            zombie_texture,
            nautilus::setup_anim,
        )
        .with_visible(is_plain_zombie),
    );
    registry.add(
        EntityKind::ZombieNautilus,
        RenderSpec::new(
            "zombie_nautilus_coral",
            nautilus::zombie_coral_layer,
            zombie_texture,
            nautilus::zombie_coral_setup,
        )
        .with_visible(is_coral_zombie),
    );
    registry.add(
        EntityKind::ZombieNautilus,
        RenderSpec::new(
            "zombie_nautilus_armor",
            nautilus::armor_layer,
            body_armor_texture,
            nautilus::setup_anim,
        )
        .with_visible(shows_body_armor),
    );
    registry.add(
        EntityKind::ZombieNautilus,
        RenderSpec::new(
            "zombie_nautilus_saddle",
            nautilus::saddle_layer,
            saddle_texture,
            nautilus::setup_anim,
        )
        .with_visible(shows_saddle),
    );
}

fn is_adult(st: &EntityState) -> bool {
    !st.extras.is_baby
}

fn is_baby(st: &EntityState) -> bool {
    st.extras.is_baby
}

fn adult_texture(_st: &EntityState) -> TexturePath {
    "entity/nautilus/nautilus".into()
}

fn baby_texture(_st: &EntityState) -> TexturePath {
    "entity/nautilus/nautilus_baby".into()
}

fn variant(st: &EntityState) -> &str {
    match &st.extras.variant {
        Some(id) => id.rsplit(':').next().unwrap_or("temperate"),
        None => "temperate",
    }
}

fn is_coral_zombie(st: &EntityState) -> bool {
    variant(st) == "warm"
}

fn is_plain_zombie(st: &EntityState) -> bool {
    !is_coral_zombie(st)
}

fn zombie_texture(st: &EntityState) -> TexturePath {
    if is_coral_zombie(st) {
        "entity/nautilus/zombie_nautilus_coral".into()
    } else {
        "entity/nautilus/zombie_nautilus".into()
    }
}

fn shows_body_armor(st: &EntityState) -> bool {
    !st.extras.is_baby && st.extras.body_armor.is_some()
}

fn body_armor_texture(st: &EntityState) -> TexturePath {
    let asset = match &st.extras.body_armor {
        Some(id) => id.rsplit(':').next().unwrap_or("copper"),
        None => "copper",
    };
    match asset {
        "diamond" => "entity/equipment/nautilus_body/diamond".into(),
        "gold" => "entity/equipment/nautilus_body/gold".into(),
        "iron" => "entity/equipment/nautilus_body/iron".into(),
        "netherite" => "entity/equipment/nautilus_body/netherite".into(),
        _ => "entity/equipment/nautilus_body/copper".into(),
    }
}

fn shows_saddle(st: &EntityState) -> bool {
    !st.extras.is_baby && st.extras.saddled
}

fn saddle_texture(_st: &EntityState) -> TexturePath {
    "entity/equipment/nautilus_saddle/saddle".into()
}
