use std::collections::HashMap;
use std::sync::Arc;

use azalea_registry::builtin::EntityKind;
use serde_json::Value;

use crate::entities::geom::{BakedModel, LayerDef, PartState};
use crate::entities::models::humanoid::armor;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{RenderSpec, TexturePath};
use crate::util::reload::Reloadable;

pub type ArmorLayers = [fn() -> LayerDef; 4];

pub const HUMANOID: ArmorLayers = [
    armor::humanoid_helmet,
    armor::humanoid_chestplate,
    armor::humanoid_leggings,
    armor::humanoid_boots,
];

pub const HUSK: ArmorLayers = [
    armor::husk_helmet,
    armor::husk_chestplate,
    armor::husk_leggings,
    armor::husk_boots,
];

pub const GIANT: ArmorLayers = [
    armor::giant_helmet,
    armor::giant_chestplate,
    armor::giant_leggings,
    armor::giant_boots,
];

pub const PIGLIN: ArmorLayers = [
    armor::piglin_helmet,
    armor::piglin_chestplate,
    armor::piglin_leggings,
    armor::piglin_boots,
];

pub const ZOMBIE_VILLAGER: ArmorLayers = [
    armor::zombie_villager_helmet,
    armor::zombie_villager_chestplate,
    armor::zombie_villager_leggings,
    armor::zombie_villager_boots,
];

pub const ARMOR_STAND: ArmorLayers = [
    armor::armor_stand_helmet,
    armor::armor_stand_chestplate,
    armor::armor_stand_leggings,
    armor::armor_stand_boots,
];

pub const ARMOR_STAND_SMALL: ArmorLayers = [
    armor::armor_stand_small_helmet,
    armor::armor_stand_small_chestplate,
    armor::armor_stand_small_leggings,
    armor::armor_stand_small_boots,
];

#[allow(clippy::too_many_arguments)]
pub fn register_set(
    registry: &mut Registry,
    kinds: &[EntityKind],
    names: [&'static str; 4],
    layers: ArmorLayers,
    setup: fn(&BakedModel, &mut [PartState], &EntityState),
    visible: [fn(&EntityState) -> bool; 4],
) {
    const TEXTURES: [fn(&EntityState) -> TexturePath; 4] = [
        helmet_texture,
        chestplate_texture,
        leggings_texture,
        boots_texture,
    ];
    const FOIL: [fn(&EntityState) -> bool; 4] =
        [helmet_foil, chestplate_foil, leggings_foil, boots_foil];
    for slot in 0..4 {
        registry.add_many(
            kinds,
            RenderSpec::new(names[slot], layers[slot], TEXTURES[slot], setup)
                .with_visible(visible[slot])
                .with_armor_glint(FOIL[slot])
                .ignoring_invisibility(),
        );
    }
    super::elytra::register(registry, kinds);
}

pub(crate) fn equipment_asset(item: Option<&str>) -> &'static str {
    let Some(id) = item else {
        return "iron";
    };
    let id = id.rsplit(':').next().unwrap_or(id);
    match id {
        "turtle_helmet" => return "turtle_scute",
        "elytra" => return "elytra",
        _ => {}
    }
    let (material, piece) = id.split_once('_').unwrap_or((id, ""));
    if !matches!(
        piece,
        "helmet" | "chestplate" | "leggings" | "boots" | "horse_armor" | "nautilus_armor"
    ) {
        return NO_ASSET;
    }
    match material {
        "leather" => "leather",
        "chainmail" => "chainmail",
        "iron" => "iron",
        "gold" | "golden" => "gold",
        "diamond" => "diamond",
        "netherite" => "netherite",
        "copper" => "copper",
        _ => NO_ASSET,
    }
}

const NO_ASSET: &str = "";

struct EquipmentLayer {
    texture: String,
    dye: Option<u32>,
}

struct EquipmentAsset {
    layers: HashMap<String, Vec<EquipmentLayer>>,
}

impl EquipmentAsset {
    fn layers(&self, layer_type: &str) -> &[EquipmentLayer] {
        self.layers.get(layer_type).map_or(&[], Vec::as_slice)
    }
}

pub(crate) const LAYER_HUMANOID: &str = "humanoid";
pub(crate) const LAYER_HUMANOID_LEGGINGS: &str = "humanoid_leggings";
pub(crate) const LAYER_WINGS: &str = "wings";
pub(crate) const LAYER_HORSE_BODY: &str = "horse_body";
pub(crate) const LAYER_NAUTILUS_BODY: &str = "nautilus_body";

const LAYER_TYPES: [&str; 5] = [
    LAYER_HUMANOID,
    LAYER_HUMANOID_LEGGINGS,
    LAYER_WINGS,
    LAYER_HORSE_BODY,
    LAYER_NAUTILUS_BODY,
];

pub(crate) const STACK_PREFIX: &str = "equipment_stack/";

static ASSETS: Reloadable<HashMap<String, EquipmentAsset>> = Reloadable::new(load_equipment_assets);

pub(crate) fn reset() {
    ASSETS.reset();
}

fn equipment_assets() -> Arc<HashMap<String, EquipmentAsset>> {
    ASSETS.get()
}

fn load_equipment_assets() -> HashMap<String, EquipmentAsset> {
    {
        let dir = crate::assets_root().join("equipment");
        let mut out = HashMap::new();
        for path in crate::platform::assets::read_dir(&dir) {
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some(text) = crate::platform::assets::read_to_string(&path) else {
                continue;
            };
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let layers = json.get("layers");
            let read_layer = |name: &str| -> Vec<EquipmentLayer> {
                let Some(entries) = layers.and_then(|l| l.get(name)).and_then(|v| v.as_array())
                else {
                    return Vec::new();
                };
                entries
                    .iter()
                    .filter_map(|entry| {
                        Some(EquipmentLayer {
                            texture: crate::util::datapack::bare_id(entry.get("texture")?)
                                .map(str::to_owned)?,
                            dye: entry.get("dyeable").map(|d| {
                                match d.get("color_when_undyed").and_then(Value::as_i64) {
                                    Some(argb) => argb as u32 & 0x00FF_FFFF,
                                    None => 0x00FF_FFFF,
                                }
                            }),
                        })
                    })
                    .collect()
            };
            out.insert(
                id.to_owned(),
                EquipmentAsset {
                    layers: LAYER_TYPES
                        .iter()
                        .filter_map(|name| {
                            let list = read_layer(name);
                            (!list.is_empty()).then(|| ((*name).to_owned(), list))
                        })
                        .collect(),
                },
            );
        }
        crate::log_info!("equipment", "{} equipment assets loaded", out.len());
        out
    }
}

pub(crate) fn slot_texture(item: Option<&str>, layer_type: &str) -> String {
    let asset = equipment_asset(item);
    let assets = equipment_assets();
    let Some(layers) = assets.get(asset).map(|a| a.layers(layer_type)) else {
        return format!("entity/equipment/{layer_type}/{asset}");
    };
    match layers {
        [only] if only.dye.is_none() => {
            format!("entity/equipment/{layer_type}/{}", only.texture)
        }
        [] => format!("entity/equipment/{layer_type}/{asset}"),
        _ => format!("{STACK_PREFIX}{layer_type}/{asset}"),
    }
}

pub(crate) fn has_layer(item: Option<&str>, layer_type: &str) -> bool {
    let Some(item) = item else { return false };
    equipment_assets()
        .get(equipment_asset(Some(item)))
        .is_some_and(|asset| !asset.layers(layer_type).is_empty())
}

pub(crate) fn humanoid_texture(item: Option<&str>) -> String {
    slot_texture(item, LAYER_HUMANOID)
}

pub(crate) fn leggings_layer_texture(item: Option<&str>) -> String {
    slot_texture(item, LAYER_HUMANOID_LEGGINGS)
}

pub(crate) fn stack(rest: &str) -> Option<image::RgbaImage> {
    let (layer_type, asset) = rest.split_once('/')?;
    let assets = equipment_assets();
    let layers = assets.get(asset)?.layers(layer_type);
    let (first, others) = layers.split_first()?;

    let sheet = |layer: &EquipmentLayer| {
        let file = crate::assets_root().join(format!(
            "textures/entity/equipment/{layer_type}/{}.png",
            layer.texture
        ));
        crate::platform::assets::open_image(&file)
            .ok()
            .map(|img| img.to_rgba8())
    };

    let mut base = sheet(first)?;
    if let Some(dye) = first.dye {
        tint(&mut base, dye);
    }
    for layer in others {
        let Some(mut over) = sheet(layer) else {
            crate::log_warn!(
                "equipment",
                "layer not found: {layer_type}/{}",
                layer.texture
            );
            continue;
        };
        if over.dimensions() != base.dimensions() {
            crate::log_warn!(
                "equipment",
                "layer {layer_type}/{} is {:?}, base is {:?}",
                layer.texture,
                over.dimensions(),
                base.dimensions()
            );
            continue;
        }
        if let Some(dye) = layer.dye {
            tint(&mut over, dye);
        }
        over_onto(&mut base, &over);
    }
    Some(base)
}

fn tint(image: &mut image::RgbaImage, rgb: u32) {
    let channel = [(rgb >> 16) & 0xFF, (rgb >> 8) & 0xFF, rgb & 0xFF];
    for pixel in image.pixels_mut() {
        for i in 0..3 {
            pixel[i] = ((pixel[i] as u32 * channel[i]) / 255) as u8;
        }
    }
}

fn over_onto(under: &mut image::RgbaImage, over: &image::RgbaImage) {
    for (dst, src) in under.pixels_mut().zip(over.pixels()) {
        let a = src[3] as u32;
        if a == 0 {
            continue;
        }
        if a == 255 {
            *dst = *src;
            continue;
        }
        for i in 0..3 {
            dst[i] = ((src[i] as u32 * a + dst[i] as u32 * (255 - a)) / 255) as u8;
        }
        dst[3] = (a + dst[3] as u32 * (255 - a) / 255).min(255) as u8;
    }
}

fn helmet_texture(st: &EntityState) -> TexturePath {
    humanoid_texture(st.extras.helmet.as_deref()).into()
}

fn chestplate_texture(st: &EntityState) -> TexturePath {
    humanoid_texture(st.extras.chestplate.as_deref()).into()
}

fn leggings_texture(st: &EntityState) -> TexturePath {
    leggings_layer_texture(st.extras.leggings.as_deref()).into()
}

fn boots_texture(st: &EntityState) -> TexturePath {
    humanoid_texture(st.extras.boots.as_deref()).into()
}

fn helmet_foil(st: &EntityState) -> bool {
    st.extras.armor_foil[0]
}

pub(crate) fn chestplate_foil(st: &EntityState) -> bool {
    st.extras.armor_foil[1]
}

fn leggings_foil(st: &EntityState) -> bool {
    st.extras.armor_foil[2]
}

fn boots_foil(st: &EntityState) -> bool {
    st.extras.armor_foil[3]
}

pub const FILLED: [fn(&EntityState) -> bool; 4] =
    [has_helmet, has_chestplate, has_leggings, has_boots];

pub const ADULT: [fn(&EntityState) -> bool; 4] =
    [adult_helmet, adult_chestplate, adult_leggings, adult_boots];

pub const BIG_STAND: [fn(&EntityState) -> bool; 4] =
    [big_helmet, big_chestplate, big_leggings, big_boots];

pub const SMALL_STAND: [fn(&EntityState) -> bool; 4] =
    [small_helmet, small_chestplate, small_leggings, small_boots];

fn has_helmet(st: &EntityState) -> bool {
    has_layer(st.extras.helmet.as_deref(), LAYER_HUMANOID)
}
fn has_chestplate(st: &EntityState) -> bool {
    has_layer(st.extras.chestplate.as_deref(), LAYER_HUMANOID)
}
fn has_leggings(st: &EntityState) -> bool {
    has_layer(st.extras.leggings.as_deref(), LAYER_HUMANOID_LEGGINGS)
}
fn has_boots(st: &EntityState) -> bool {
    has_layer(st.extras.boots.as_deref(), LAYER_HUMANOID)
}

fn adult_helmet(st: &EntityState) -> bool {
    !st.extras.is_baby && has_helmet(st)
}
fn adult_chestplate(st: &EntityState) -> bool {
    !st.extras.is_baby && has_chestplate(st)
}
fn adult_leggings(st: &EntityState) -> bool {
    !st.extras.is_baby && has_leggings(st)
}
fn adult_boots(st: &EntityState) -> bool {
    !st.extras.is_baby && has_boots(st)
}

fn big_helmet(st: &EntityState) -> bool {
    !st.extras.small && has_helmet(st)
}
fn big_chestplate(st: &EntityState) -> bool {
    !st.extras.small && has_chestplate(st)
}
fn big_leggings(st: &EntityState) -> bool {
    !st.extras.small && has_leggings(st)
}
fn big_boots(st: &EntityState) -> bool {
    !st.extras.small && has_boots(st)
}

fn small_helmet(st: &EntityState) -> bool {
    st.extras.small && has_helmet(st)
}
fn small_chestplate(st: &EntityState) -> bool {
    st.extras.small && has_chestplate(st)
}
fn small_leggings(st: &EntityState) -> bool {
    st.extras.small && has_leggings(st)
}
fn small_boots(st: &EntityState) -> bool {
    st.extras.small && has_boots(st)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(rgba: [u8; 4]) -> image::RgbaImage {
        image::RgbaImage::from_pixel(1, 1, image::Rgba(rgba))
    }

    #[test]
    fn a_dye_multiplies_rgb_and_leaves_alpha_alone() {
        let mut img = pixel([255, 128, 64, 200]);
        tint(&mut img, 0x00A0_6540);
        assert_eq!(img.get_pixel(0, 0).0, [160, 50, 16, 200]);
    }

    #[test]
    fn a_white_dye_changes_nothing() {
        let mut img = pixel([13, 200, 7, 255]);
        tint(&mut img, 0x00FF_FFFF);
        assert_eq!(img.get_pixel(0, 0).0, [13, 200, 7, 255]);
    }

    #[test]
    fn an_opaque_overlay_pixel_replaces_and_a_clear_one_does_not() {
        let mut under = pixel([10, 20, 30, 255]);
        over_onto(&mut under, &pixel([1, 2, 3, 255]));
        assert_eq!(under.get_pixel(0, 0).0, [1, 2, 3, 255]);

        let mut under = pixel([10, 20, 30, 255]);
        over_onto(&mut under, &pixel([1, 2, 3, 0]));
        assert_eq!(under.get_pixel(0, 0).0, [10, 20, 30, 255]);
    }

    #[test]
    fn a_partly_transparent_overlay_pixel_blends_toward_it() {
        let mut under = pixel([0, 0, 0, 255]);
        over_onto(&mut under, &pixel([200, 100, 50, 128]));
        let got = under.get_pixel(0, 0).0;
        assert_eq!([got[0], got[1], got[2]], [100, 50, 25]);
        assert_eq!(got[3], 255);
    }

    #[test]
    fn only_a_layered_or_dyed_asset_takes_a_stack_name() {
        assert!(!humanoid_texture(Some("iron_helmet")).starts_with(STACK_PREFIX));
        assert!(!humanoid_texture(None).starts_with(STACK_PREFIX));
        if equipment_assets().contains_key("leather") {
            assert_eq!(
                humanoid_texture(Some("leather_boots")),
                format!("{STACK_PREFIX}humanoid/leather")
            );
            assert_eq!(
                leggings_layer_texture(Some("leather_leggings")),
                format!("{STACK_PREFIX}humanoid_leggings/leather")
            );
            assert!(stack("humanoid/leather").is_some());
        }
    }

    #[test]
    fn every_vanilla_armour_material_resolves_to_its_equipment_asset() {
        for (item, asset) in [
            ("minecraft:iron_helmet", "iron"),
            ("golden_boots", "gold"),
            ("chainmail_chestplate", "chainmail"),
            ("netherite_leggings", "netherite"),
            ("copper_helmet", "copper"),
            ("leather_boots", "leather"),
            ("turtle_helmet", "turtle_scute"),
            ("minecraft:elytra", "elytra"),
            ("iron_chestplate", "iron"),
            ("carved_pumpkin", NO_ASSET),
            ("zombie_head", NO_ASSET),
            ("golden_apple", NO_ASSET),
            ("minecraft:golden_horse_armor", "gold"),
            ("leather_horse_armor", "leather"),
            ("minecraft:iron_nautilus_armor", "iron"),
            ("golden_nautilus_armor", "gold"),
            ("white_carpet", NO_ASSET),
        ] {
            assert_eq!(equipment_asset(Some(item)), asset, "{item}");
        }
        assert_eq!(equipment_asset(None), "iron");
    }

    #[test]
    fn an_elytra_is_wings_and_not_a_chestplate() {
        if !equipment_assets().contains_key("elytra") {
            return;
        }
        assert!(has_layer(Some("minecraft:elytra"), LAYER_WINGS));
        assert!(!has_layer(Some("minecraft:elytra"), LAYER_HUMANOID));
        assert!(!has_layer(Some("carved_pumpkin"), LAYER_HUMANOID));
    }
}
