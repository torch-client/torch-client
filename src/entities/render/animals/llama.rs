use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::models::animals::llama;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

const KINDS: [EntityKind; 2] = [EntityKind::Llama, EntityKind::TraderLlama];

pub fn register(registry: &mut Registry) {
    registry.add_many(
        &KINDS,
        RenderSpec::new("llama", llama::layer, texture, llama::setup_anim)
            .with_visible(|st| !st.extras.is_baby),
    );
    registry.add_many(
        &KINDS,
        RenderSpec::new("llama_baby", llama::baby_layer, texture, llama::setup_anim)
            .with_visible(|st| st.extras.is_baby),
    );
    registry.add_many(
        &KINDS,
        RenderSpec::new(
            "llama_decor",
            llama::decor_layer,
            decor_texture,
            llama::setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby && decor_texture_name(st).is_some()),
    );
    registry.add_many(
        &KINDS,
        RenderSpec::new(
            "llama_baby_decor",
            llama::baby_decor_layer,
            decor_texture,
            llama::setup_anim,
        )
        .with_visible(|st| st.extras.is_baby && st.kind == EntityKind::TraderLlama),
    );
}

const VARIANTS: [&str; 4] = ["creamy", "white", "brown", "gray"];

fn texture(st: &EntityState) -> String {
    let variant = VARIANTS[(st.extras.variant_id.clamp(0, 3)) as usize];
    if st.extras.is_baby {
        format!("entity/llama/llama_{variant}_baby")
    } else {
        format!("entity/llama/llama_{variant}")
    }
}

fn decor_texture_name(st: &EntityState) -> Option<String> {
    if let Some(item) = &st.extras.body_armor {
        let id = item.rsplit(':').next().unwrap_or("");
        if let Some(color) = id.strip_suffix("_carpet") {
            return Some(color.to_string());
        }
    }
    if st.kind == EntityKind::TraderLlama {
        return Some(
            if st.extras.is_baby {
                "trader_llama_baby"
            } else {
                "trader_llama"
            }
            .to_string(),
        );
    }
    None
}

fn decor_texture(st: &EntityState) -> String {
    let name = decor_texture_name(st).unwrap_or_else(|| "white".to_string());
    format!("entity/equipment/llama_body/{name}")
}
