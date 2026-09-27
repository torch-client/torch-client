use std::collections::HashMap;
use std::sync::OnceLock;

pub const MAX_PATTERNS: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BannerLayer {
    pub asset: Box<str>,
    pub color: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BannerData {
    pub layers: Vec<BannerLayer>,
}

pub const TEXTURE_DIFFUSE: [u32; 16] = [
    16383998, 16351261, 13061821, 3847130, 16701501, 8439583, 15961002, 4673362, 10329495, 1481884,
    8991416, 3949738, 8606770, 6192150, 11546150, 1908001,
];

pub const NAMES: [&str; 16] = [
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

pub fn color_by_name(name: &str) -> u8 {
    NAMES.iter().position(|n| *n == name).unwrap_or(0) as u8
}

pub fn tint(color: u8) -> [f32; 4] {
    let packed = TEXTURE_DIFFUSE[(color as usize).min(15)];
    [
        crate::util::mth::srgb_to_linear(((packed >> 16) & 0xff) as f32 / 255.0),
        crate::util::mth::srgb_to_linear(((packed >> 8) & 0xff) as f32 / 255.0),
        crate::util::mth::srgb_to_linear((packed & 0xff) as f32 / 255.0),
        1.0,
    ]
}

pub fn base_color_of(block: &str) -> u8 {
    let name = block
        .strip_suffix("_wall_banner")
        .or_else(|| block.strip_suffix("_banner"))
        .unwrap_or("white");
    color_by_name(name)
}

pub fn is_wall(block: &str) -> bool {
    block.ends_with("_wall_banner")
}

fn assets() -> &'static HashMap<Box<str>, Box<str>> {
    static ASSETS: OnceLock<HashMap<Box<str>, Box<str>>> = OnceLock::new();
    ASSETS.get_or_init(|| {
        crate::util::datapack::entries("banner_pattern")
            .into_iter()
            .filter_map(|(id, json)| {
                let asset = crate::util::datapack::bare_id(json.get("asset_id")?)?;
                Some((Box::from(id.as_str()), Box::from(asset)))
            })
            .collect()
    })
}

pub fn asset_for(pattern: &str) -> Box<str> {
    assets()
        .get(pattern)
        .cloned()
        .unwrap_or_else(|| Box::from(pattern))
}

const KEY_SEP: char = '!';
const LAYER_SEP: char = ',';
const COLOR_SEP: char = '.';

pub fn model_key(item: &str, layers: &[BannerLayer]) -> String {
    let mut key = String::with_capacity(item.len() + layers.len() * 12);
    key.push_str(item);
    for (i, layer) in layers.iter().enumerate() {
        key.push(if i == 0 { KEY_SEP } else { LAYER_SEP });
        key.push_str(&layer.asset);
        key.push(COLOR_SEP);
        let color = layer.color.min(15);
        key.push((b'0' + color / 10) as char);
        key.push((b'0' + color % 10) as char);
    }
    key
}

pub fn key_item(key: &str) -> &str {
    key.split_once(KEY_SEP).map_or(key, |(item, _)| item)
}

pub fn is_model_key(key: &str) -> bool {
    key.contains(KEY_SEP)
}

pub fn parse_model_key(key: &str) -> Option<(&str, Vec<BannerLayer>)> {
    let (item, rest) = key.split_once(KEY_SEP)?;
    let mut layers = Vec::new();
    for part in rest.split(LAYER_SEP) {
        let (asset, color) = part.rsplit_once(COLOR_SEP)?;
        layers.push(BannerLayer {
            asset: Box::from(asset),
            color: color.parse::<u8>().ok()?.min(15),
        });
        if layers.len() == MAX_PATTERNS {
            break;
        }
    }
    (!layers.is_empty()).then_some((item, layers))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(asset: &str, color: u8) -> BannerLayer {
        BannerLayer {
            asset: Box::from(asset),
            color,
        }
    }

    #[test]
    fn model_keys_round_trip() {
        let layers = [layer("creeper", 15), layer("border", 4)];
        let key = model_key("red_banner", &layers);
        assert_eq!(key, "red_banner!creeper.15,border.04");
        let (item, back) = parse_model_key(&key).unwrap();
        assert_eq!(item, "red_banner");
        assert_eq!(back, layers);
    }

    #[test]
    fn an_unpatterned_banner_keys_as_itself() {
        assert_eq!(model_key("white_banner", &[]), "white_banner");
        assert_eq!(parse_model_key("white_banner"), None);
        assert_eq!(parse_model_key("diamond_sword"), None);
    }

    #[test]
    fn a_dotted_asset_id_keeps_its_dots() {
        let key = model_key("gray_banner", &[layer("mod.pattern.one", 7)]);
        let (_, back) = parse_model_key(&key).unwrap();
        assert_eq!(back, [layer("mod.pattern.one", 7)]);
    }

    #[test]
    fn a_banner_block_names_its_base_colour() {
        assert_eq!(base_color_of("white_banner"), 0);
        assert_eq!(base_color_of("red_wall_banner"), 14);
        assert_eq!(base_color_of("light_gray_banner"), 8);
        assert_eq!(base_color_of("black_wall_banner"), 15);
    }

    #[test]
    fn a_wall_banner_is_not_a_standing_one() {
        assert!(is_wall("cyan_wall_banner"));
        assert!(!is_wall("cyan_banner"));
        assert_eq!(base_color_of("cyan_wall_banner"), 9);
    }

    #[test]
    fn the_diffuse_column_is_not_the_text_column() {
        assert_eq!(TEXTURE_DIFFUSE[14], 11546150);
        assert_eq!(crate::blockentities::feed::dye_text_color("red"), 16711680);
    }

    #[test]
    fn a_dye_index_tints_its_own_colour() {
        assert_eq!(tint(15), [29.0 / 255.0, 29.0 / 255.0, 33.0 / 255.0, 1.0]);
        assert_eq!(tint(0)[0], 249.0 / 255.0);
    }
}
