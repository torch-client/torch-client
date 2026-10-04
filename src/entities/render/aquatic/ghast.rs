use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::aquatic::ghast;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Ghast,
        RenderSpec::new(
            "ghast",
            ghast::ghast_layer,
            ghast_texture,
            ghast::ghast_setup,
        ),
    );

    registry.add(
        EntityKind::HappyGhast,
        RenderSpec::new(
            "happy_ghast",
            ghast::happy_ghast_layer,
            happy_ghast_texture,
            ghast::happy_ghast_setup,
        )
        .with_visible(is_adult),
    );
    registry.add(
        EntityKind::HappyGhast,
        RenderSpec::new(
            "happy_ghast_baby",
            ghast::happy_ghast_baby_layer,
            happy_ghast_baby_texture,
            ghast::happy_ghast_setup,
        )
        .with_visible(is_baby),
    );
    registry.add(
        EntityKind::HappyGhast,
        RenderSpec::new(
            "happy_ghast_harness",
            ghast::harness_layer,
            harness_texture,
            ghast::harness_setup,
        )
        .with_visible(shows_adult_harness),
    );
    registry.add(
        EntityKind::HappyGhast,
        RenderSpec::new(
            "happy_ghast_baby_harness",
            ghast::baby_harness_layer,
            harness_texture,
            ghast::harness_setup,
        )
        .with_visible(shows_baby_harness),
    );
    registry.add(
        EntityKind::HappyGhast,
        RenderSpec::new(
            "happy_ghast_ropes",
            ghast::happy_ghast_ropes_layer,
            ropes_texture,
            ghast::happy_ghast_setup,
        )
        .with_visible(shows_adult_ropes),
    );
    registry.add(
        EntityKind::HappyGhast,
        RenderSpec::new(
            "happy_ghast_baby_ropes",
            ghast::happy_ghast_baby_ropes_layer,
            ropes_texture,
            ghast::happy_ghast_setup,
        )
        .with_visible(shows_baby_ropes),
    );
}

fn is_adult(st: &EntityState) -> bool {
    !st.extras.is_baby
}

fn is_baby(st: &EntityState) -> bool {
    st.extras.is_baby
}

fn ghast_texture(st: &EntityState) -> TexturePath {
    if st.extras.charged {
        "entity/ghast/ghast_shooting".into()
    } else {
        "entity/ghast/ghast".into()
    }
}

fn happy_ghast_texture(_st: &EntityState) -> TexturePath {
    "entity/ghast/happy_ghast".into()
}

fn happy_ghast_baby_texture(_st: &EntityState) -> TexturePath {
    "entity/ghast/happy_ghast_baby".into()
}

fn shows_adult_harness(st: &EntityState) -> bool {
    !st.extras.is_baby && st.extras.body_armor.is_some()
}

fn shows_baby_harness(st: &EntityState) -> bool {
    st.extras.is_baby && st.extras.body_armor.is_some()
}

fn shows_adult_ropes(st: &EntityState) -> bool {
    shows_adult_harness(st) && st.extras.ridden
}

fn shows_baby_ropes(st: &EntityState) -> bool {
    shows_baby_harness(st) && st.extras.ridden
}

fn ropes_texture(_st: &EntityState) -> TexturePath {
    "entity/ghast/happy_ghast_ropes".into()
}

fn harness_texture(st: &EntityState) -> TexturePath {
    const COLORS: [&str; 16] = [
        "white",
        "orange",
        "magenta",
        "light_blue",
        "yellow",
        "lime",
        "pink",
        "gray",
        "light_gray",
        "cyan",
        "purple",
        "blue",
        "brown",
        "green",
        "red",
        "black",
    ];
    let asset = match &st.extras.body_armor {
        Some(id) => id.rsplit(':').next().unwrap_or("white_harness"),
        None => "white_harness",
    };
    let color = asset.strip_suffix("_harness").unwrap_or("white");
    let color = if COLORS.contains(&color) {
        color
    } else {
        "white"
    };
    format!("entity/equipment/happy_ghast_body/{color}_harness").into()
}
