use std::collections::HashMap;

use azalea_core::registry_holder::RegistryHolder;
use azalea_inventory::components::Trim;
use serde_json::Value;

use crate::util::reload::Reloadable;

pub(crate) const TRIM_PREFIX: &str = "armor_trim/";

const DECAL: char = '+';

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
    let (pattern, decal) = match &trim.pattern {
        Holder::Direct(pattern) => (pattern.asset_id.to_string(), pattern.decal),
        Holder::Reference(pattern) => {
            let (name, entry) = entry(registries?, "trim_pattern", pattern.protocol_id())?;
            entry
                .and_then(|entry| {
                    let asset_id = entry.string("asset_id")?.to_str().into_owned();
                    Some((
                        asset_id,
                        entry.byte("decal").is_some_and(|decal| decal != 0),
                    ))
                })
                .or_else(|| {
                    let json = (name.namespace() == "minecraft")
                        .then(|| crate::util::datapack::entry("trim_pattern", name.path()))??;
                    let asset_id = json.get("asset_id")?.as_str()?.to_owned();
                    let decal = json.get("decal").and_then(Value::as_bool).unwrap_or(false);
                    Some((asset_id, decal))
                })?
        }
    };
    let pattern = azalea::Identifier::new(pattern);
    let mut key = format!("{}:{}_{suffix}", pattern.namespace(), pattern.path());
    if decal {
        key.push(DECAL);
        key.push_str(asset);
    }
    Some(crate::util::intern(key))
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
    let Some((trim, asset)) = trim.rsplit_once(DECAL) else {
        return sprite(layer_type, trim);
    };
    let image = sprite(layer_type, trim)?;
    match super::armor::stack(&format!("{layer_type}/{asset}")) {
        Some(base) => Some(mask_decal(image, &base)),
        None => Some(image),
    }
}

fn mask_decal(sprite: image::RgbaImage, base: &image::RgbaImage) -> image::RgbaImage {
    let cutoff = crate::renderer::entity_material::ALPHA_CUTOUT;
    let (bw, bh) = base.dimensions();
    if bw == 0 || bh == 0 {
        return sprite;
    }
    let (w, h) = (sprite.width().max(bw), sprite.height().max(bh));
    let mut sprite = if (w, h) == sprite.dimensions() {
        sprite
    } else {
        image::imageops::resize(&sprite, w, h, image::imageops::FilterType::Nearest)
    };
    let at = |i: u32, size: u32, other: u32| {
        (((2 * u64::from(i) + 1) * u64::from(other)) / (2 * u64::from(size))) as u32
    };
    for (x, y, pixel) in sprite.enumerate_pixels_mut() {
        let under = base.get_pixel(at(x, w, bw).min(bw - 1), at(y, h, bh).min(bh - 1));
        if f32::from(under[3]) / 255.0 < cutoff {
            pixel[3] = 0;
        }
    }
    sprite
}

fn sprite(layer_type: &str, trim: &str) -> Option<image::RgbaImage> {
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

    #[test]
    fn a_decal_is_cut_to_the_texels_its_base_keeps() {
        let red = image::Rgba([200, 0, 0, 255]);
        let kept = image::Rgba([0, 0, 0, 26]);
        let dropped = image::Rgba([0, 0, 0, 25]);

        let mut base = image::RgbaImage::from_pixel(2, 1, kept);
        base.put_pixel(1, 0, dropped);
        let out = mask_decal(image::RgbaImage::from_pixel(2, 1, red), &base);
        assert_eq!(out.dimensions(), (2, 1));
        assert_eq!(out.get_pixel(0, 0).0, red.0);
        assert_eq!(out.get_pixel(1, 0)[3], 0);

        let out = mask_decal(image::RgbaImage::from_pixel(4, 2, red), &base);
        assert_eq!(out.dimensions(), (4, 2));
        for y in 0..2 {
            assert_eq!(out.get_pixel(0, y).0, red.0);
            assert_eq!(out.get_pixel(1, y).0, red.0);
            assert_eq!(out.get_pixel(2, y)[3], 0);
            assert_eq!(out.get_pixel(3, y)[3], 0);
        }

        let mut base = image::RgbaImage::from_pixel(4, 2, kept);
        base.put_pixel(0, 0, dropped);
        let out = mask_decal(image::RgbaImage::from_pixel(2, 1, red), &base);
        assert_eq!(out.dimensions(), (4, 2));
        assert_eq!(out.get_pixel(0, 0)[3], 0);
        for (x, y) in [(1, 0), (0, 1), (1, 1), (3, 1)] {
            assert_eq!(out.get_pixel(x, y).0, red.0);
        }

        let base = image::RgbaImage::from_pixel(1, 1, kept);
        let out = mask_decal(image::RgbaImage::new(1, 1), &base);
        assert_eq!(out.get_pixel(0, 0)[3], 0);
    }
}
