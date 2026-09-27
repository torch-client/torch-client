use std::path::PathBuf;

use serde_json::Value;

pub(crate) fn dir(registry: &str) -> PathBuf {
    crate::datapack_root().join(registry)
}

pub(crate) fn entries(registry: &str) -> Vec<(String, Value)> {
    let mut out: Vec<(String, Value)> = crate::platform::assets::read_dir(dir(registry))
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .filter_map(|path| {
            let id = path.file_stem()?.to_str()?.to_owned();
            let text = crate::platform::assets::read_to_string(&path)?;
            let json = serde_json::from_str(&text).ok()?;
            Some((id, json))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

pub(crate) fn entry(registry: &str, id: &str) -> Option<Value> {
    let path = dir(registry).join(format!("{id}.json"));
    let text = crate::platform::assets::read_to_string(path)?;
    serde_json::from_str(&text).ok()
}

pub(crate) fn hex_color(value: &Value) -> Option<u32> {
    let text = value.as_str()?.strip_prefix('#')?;
    match text.len() {
        6 => Some(0xff00_0000 | u32::from_str_radix(text, 16).ok()?),
        8 => u32::from_str_radix(text, 16).ok(),
        _ => None,
    }
}

pub(crate) fn hex_rgb(value: &Value) -> Option<u32> {
    Some(hex_color(value)? & 0x00ff_ffff)
}

pub(crate) fn bare_id(value: &Value) -> Option<&str> {
    let text = value.as_str()?;
    Some(text.strip_prefix("minecraft:").unwrap_or(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_hex_digits_read_as_fully_opaque() {
        assert_eq!(hex_color(&Value::from("#302821")), Some(0xff30_2821));
        assert_eq!(hex_color(&Value::from("#7a7aff")), Some(0xff7a_7aff));
        assert_eq!(hex_color(&Value::from("#000000")), Some(0xff00_0000));
    }

    #[test]
    fn eight_hex_digits_keep_their_alpha() {
        assert_eq!(hex_color(&Value::from("#ccffffff")), Some(0xccff_ffff));
        assert_eq!(hex_color(&Value::from("#00000000")), Some(0));
    }

    #[test]
    fn the_rgb_reader_drops_the_alpha_from_either_width() {
        assert_eq!(hex_rgb(&Value::from("#0a0a0a")), Some(0x0a0a0a));
        assert_eq!(hex_rgb(&Value::from("#ccffffff")), Some(0xffffff));
    }

    #[test]
    fn anything_that_is_not_six_or_eight_hex_digits_is_not_a_colour() {
        assert_eq!(hex_color(&Value::from("302821")), None);
        assert_eq!(hex_color(&Value::from("#30282")), None);
        assert_eq!(hex_color(&Value::from("#3028210")), None);
        assert_eq!(hex_color(&Value::from("#zzzzzz")), None);
        assert_eq!(hex_color(&Value::from(3021345)), None);
    }

    #[test]
    fn an_id_reads_the_same_with_or_without_the_namespace() {
        assert_eq!(bare_id(&Value::from("minecraft:day")), Some("day"));
        assert_eq!(bare_id(&Value::from("day")), Some("day"));
        assert_eq!(bare_id(&Value::from(7)), None);
    }
}
