use crate::gui::painter::Painter;
use crate::session::SlotStack;
use std::sync::OnceLock;

pub fn no_item_required() -> &'static [Box<str>] {
    static PATTERNS: OnceLock<Box<[Box<str>]>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        let Some(json) = crate::util::datapack::entry("tags/banner_pattern", "no_item_required")
        else {
            eprintln!("[loom] no banner_pattern/no_item_required tag; the pattern grid is empty");
            return Box::from([]);
        };
        let out: Vec<Box<str>> = json
            .get("values")
            .and_then(|v| v.as_array())
            .map(|v| {
                v.iter()
                    .filter_map(|e| e.as_str())
                    .map(|e| Box::from(e.trim_start_matches("minecraft:")))
                    .collect()
            })
            .unwrap_or_default();
        out.into_boxed_slice()
    })
}

pub fn swatch(p: &mut Painter, pattern: &str, x: f32, y: f32) {
    draw_layer(p, pattern, x, y, 5.0, 10.0, 0xFFFFFFFF);
}

pub fn preview(p: &mut Painter, banner: Option<&SlotStack>, x: f32, y: f32) {
    let Some(banner) = banner else { return };
    draw_layer(p, "base", x, y, 20.0, 40.0, base_color(banner.item));
}

fn draw_layer(p: &mut Painter, pattern: &str, x: f32, y: f32, w: f32, h: f32, argb: u32) {
    let mut key = [0u8; 64];
    let Some(name) = sprite_key(&mut key, pattern) else {
        return;
    };
    if p.atlas.sprite(name).is_none() {
        return;
    }
    p.sprite_tinted(name, x, y, w, h, argb);
}

fn sprite_key<'a>(buf: &'a mut [u8; 64], pattern: &str) -> Option<&'a str> {
    const PREFIX: &[u8] = b"banner/";
    let end = PREFIX.len() + pattern.len();
    if end > buf.len() {
        return None;
    }
    buf[..PREFIX.len()].copy_from_slice(PREFIX);
    buf[PREFIX.len()..end].copy_from_slice(pattern.as_bytes());
    std::str::from_utf8(&buf[..end]).ok()
}

fn base_color(item: &str) -> u32 {
    use crate::entities::render::animals::dye::TEXTURE_DIFFUSE;
    const NAMES: [&str; 16] = [
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
    let colour = item.strip_suffix("_banner").unwrap_or("white");
    let idx = NAMES.iter().position(|n| *n == colour).unwrap_or(0);
    0xFF00_0000 | TEXTURE_DIFFUSE[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_banner_id_resolves_to_its_dye_colour() {
        assert_eq!(base_color("red_banner"), 0xFF00_0000 | 11546150);
        assert_eq!(base_color("black_banner"), 0xFF00_0000 | 1908001);
        assert_eq!(base_color("stone"), 0xFF00_0000 | 16383998);
    }

    #[test]
    fn a_sprite_key_is_built_without_allocating() {
        let mut buf = [0u8; 64];
        assert_eq!(sprite_key(&mut buf, "cross"), Some("banner/cross"));
        let mut buf = [0u8; 64];
        assert_eq!(sprite_key(&mut buf, &"x".repeat(64)), None);
    }
}
