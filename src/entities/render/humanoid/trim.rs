use std::collections::HashMap;

use azalea_core::registry_holder::RegistryHolder;
use azalea_inventory::components::Trim;
use serde_json::Value;

use crate::util::reload::Reloadable;

pub(crate) const TRIM_PREFIX: &str = "armor_trim/";

pub(crate) fn resolve(
    stack: &azalea_inventory::ItemStack,
    item: &str,
    registries: Option<&RegistryHolder>,
) -> Option<&'static str> {
    use azalea_registry::{DataRegistry, Holder};

    let trim = stack.get_component::<Trim>()?;
    let asset = super::armor::equipment_asset(Some(item));
    let suffix = match &trim.material {
        Holder::Direct(material) => {
            let group = &material.assets;
            group
                .override_armor_assets
                .iter()
                .find(|(key, _)| key.namespace() == "minecraft" && key.path() == asset)
                .map_or(&group.assert_name.suffix, |(_, info)| &info.suffix)
                .clone()
        }
        Holder::Reference(material) => {
            let (name, entry) = entry(registries?, "trim_material", material.protocol_id())?;
            let from_registry = entry.and_then(|entry| {
                let base = entry.string("asset_name")?.to_str().into_owned();
                let over = entry.compound("override_armor_assets").and_then(|map| {
                    map.iter()
                        .find(|(key, _)| bare(&key.to_str()) == asset)
                        .and_then(|(_, value)| Some(value.string()?.to_str().into_owned()))
                });
                Some(over.unwrap_or(base))
            });
            match from_registry {
                Some(suffix) => suffix,
                None => {
                    let json = (name.namespace() == "minecraft")
                        .then(|| crate::util::datapack::entry("trim_material", name.path()))??;
                    let over = json
                        .get("override_armor_assets")
                        .and_then(Value::as_object)
                        .and_then(|map| map.iter().find(|(key, _)| bare(key) == asset))
                        .and_then(|(_, value)| value.as_str());
                    over.or_else(|| json.get("asset_name")?.as_str())?
                        .to_owned()
                }
            }
        }
    };
    let pattern = match &trim.pattern {
        Holder::Direct(pattern) => pattern.asset_id.to_string(),
        Holder::Reference(pattern) => {
            let (name, entry) = entry(registries?, "trim_pattern", pattern.protocol_id())?;
            entry
                .and_then(|entry| Some(entry.string("asset_id")?.to_str().into_owned()))
                .or_else(|| {
                    let json = (name.namespace() == "minecraft")
                        .then(|| crate::util::datapack::entry("trim_pattern", name.path()))??;
                    json.get("asset_id")?.as_str().map(str::to_owned)
                })?
        }
    };
    let pattern = azalea::Identifier::new(pattern);
    Some(crate::util::intern(format!(
        "{}:{}_{suffix}",
        pattern.namespace(),
        pattern.path()
    )))
}

fn entry<'a>(
    registries: &'a RegistryHolder,
    registry: &str,
    protocol_id: u32,
) -> Option<(
    &'a azalea::Identifier,
    Option<&'a simdnbt::owned::NbtCompound>,
)> {
    let (name, data) = registries
        .extra
        .get(&azalea::Identifier::new(registry))?
        .map
        .get_index(protocol_id as usize)?;
    Some((name, (!data.is_empty()).then_some(data)))
}

fn bare(id: &str) -> &str {
    id.strip_prefix("minecraft:").unwrap_or(id)
}

pub(crate) fn texture_name(trim: &str, layer_type: &str) -> String {
    format!("{TRIM_PREFIX}{layer_type}/{trim}")
}

struct Permutations {
    textures: Vec<String>,
    key: String,
    permutations: Vec<(String, String)>,
    separator: String,
}

static SOURCES: Reloadable<Vec<Permutations>> = Reloadable::new(read_sources);

pub(crate) fn reset() {
    SOURCES.reset();
}

fn read_sources() -> Vec<Permutations> {
    let mut out = Vec::new();
    let path = crate::assets_root().join("atlases/armor_trims.json");
    for text in crate::platform::assets::read_stack(path) {
        let Ok(doc) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let sources = doc.get("sources").and_then(Value::as_array);
        for source in sources.into_iter().flatten() {
            if source.get("type").and_then(Value::as_str).map(bare) != Some("paletted_permutations")
            {
                continue;
            }
            let id = |value: &Value| value.as_str().map(|s| bare(s).to_owned());
            let Some(key) = source.get("palette_key").and_then(id) else {
                continue;
            };
            out.push(Permutations {
                textures: source
                    .get("textures")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(id)
                    .collect(),
                key,
                permutations: source
                    .get("permutations")
                    .and_then(Value::as_object)
                    .into_iter()
                    .flatten()
                    .filter_map(|(suffix, palette)| Some((suffix.clone(), id(palette)?)))
                    .collect(),
                separator: source
                    .get("separator")
                    .and_then(Value::as_str)
                    .unwrap_or("_")
                    .to_owned(),
            });
        }
    }
    crate::log_info!("equipment", "{} armour trim permutation sources", out.len());
    out
}

fn open(id: &str) -> Option<image::RgbaImage> {
    let file = crate::assets_root().join(format!("textures/{id}.png"));
    crate::platform::assets::open_image(&file)
        .ok()
        .map(|img| img.to_rgba8())
}

pub(crate) fn permuted(rest: &str) -> Option<image::RgbaImage> {
    let (layer_type, trim) = rest.split_once('/')?;
    let (namespace, name) = trim.split_once(':')?;
    if namespace != "minecraft" {
        return None;
    }
    let sprite = format!("trims/entity/{layer_type}/{name}");
    if let Some(image) = open(&sprite) {
        return Some(image);
    }
    let sources = SOURCES.get();
    let (texture, key, palette) = sources.iter().find_map(|source| {
        source.textures.iter().find_map(|texture| {
            let suffix = sprite
                .strip_prefix(texture.as_str())?
                .strip_prefix(source.separator.as_str())?;
            let (_, palette) = source
                .permutations
                .iter()
                .find(|(name, _)| name == suffix)?;
            Some((texture, &source.key, palette))
        })
    })?;
    let mut image = open(texture)?;
    let (keys, values) = (open(key)?, open(palette)?);
    if keys.len() != values.len() {
        crate::log_warn!("equipment", "palette {palette} does not match {key}");
        return None;
    }
    let mut mapping: HashMap<[u8; 3], [u8; 4]> = HashMap::new();
    for (key, value) in keys.pixels().zip(values.pixels()) {
        if key[3] != 0 {
            mapping.insert([key[0], key[1], key[2]], value.0);
        }
    }
    for pixel in image.pixels_mut() {
        let alpha = pixel[3];
        if alpha == 0 {
            continue;
        }
        let rgb = [pixel[0], pixel[1], pixel[2]];
        let value = mapping
            .get(&rgb)
            .copied()
            .unwrap_or([rgb[0], rgb[1], rgb[2], 255]);
        let alpha = (u32::from(alpha) * u32::from(value[3]) / 255) as u8;
        pixel.0 = [value[0], value[1], value[2], alpha];
    }
    Some(image)
}

pub(crate) fn cut_over(base: &mut image::RgbaImage, trim: &image::RgbaImage) {
    let cutoff = crate::renderer::entity_material::ALPHA_CUTOUT;
    for (dst, src) in base.pixels_mut().zip(trim.pixels()) {
        if f32::from(src[3]) / 255.0 >= cutoff {
            *dst = *src;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_vanilla_trim_sprite_permutes() {
        if open("trims/color_palettes/trim_palette").is_none() {
            return;
        }
        let image = permuted("humanoid_leggings/minecraft:sentry_iron").expect("permutes");
        let base = open("trims/entity/humanoid_leggings/sentry").expect("base sheet");
        assert_eq!(image.dimensions(), base.dimensions());
        assert!(permuted("humanoid/minecraft:sentry_iron_darker").is_some());
        assert!(permuted("humanoid/minecraft:sentry_no_such_material").is_none());
    }

    #[test]
    fn the_cutout_keeps_a_texel_at_the_threshold_and_drops_one_below() {
        let mut base = image::RgbaImage::from_pixel(2, 1, image::Rgba([1, 2, 3, 255]));
        let mut trim = image::RgbaImage::new(2, 1);
        trim.put_pixel(0, 0, image::Rgba([9, 9, 9, 26]));
        trim.put_pixel(1, 0, image::Rgba([9, 9, 9, 25]));
        cut_over(&mut base, &trim);
        assert_eq!(base.get_pixel(0, 0).0, [9, 9, 9, 26]);
        assert_eq!(base.get_pixel(1, 0).0, [1, 2, 3, 255]);
    }
}
