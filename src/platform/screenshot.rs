#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn dir() -> std::path::PathBuf {
    crate::platform::storage::dir().join("screenshots")
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn save(image: &image::RgbImage) -> Result<String, String> {
    use image::ImageEncoder;

    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| format!("could not encode the image: {e}"))?;

    let dir = dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("could not create {}: {e}", dir.display()))?;

    let (name, path) = free_name(&dir);
    std::fs::write(&path, &png).map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(name)
}

#[cfg(not(target_arch = "wasm32"))]
fn free_name(dir: &std::path::Path) -> (String, std::path::PathBuf) {
    let stamp = timestamp(epoch_secs());
    let mut n = 0u32;
    loop {
        let name = if n == 0 {
            format!("{stamp}.png")
        } else {
            format!("{stamp}_{n}.png")
        };
        let path = dir.join(&name);
        if !path.exists() {
            return (name, path);
        }
        n += 1;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn epoch_secs() -> i64 {
    crate::platform::time::SystemTime::now()
        .duration_since(crate::platform::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(not(target_arch = "wasm32"))]
fn timestamp(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + i64::from(month <= 2);

    format!("{year:04}-{month:02}-{day:02}_{hour:02}.{minute:02}.{second:02}")
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn save(_image: &image::RgbImage) -> Result<String, String> {
    Err("the web build cannot copy screenshots to the clipboard yet".to_string())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn the_calendar_matches_known_instants() {
        let cases = [
            (0, "1970-01-01_00.00.00"),
            (1, "1970-01-01_00.00.01"),
            (86_399, "1970-01-01_23.59.59"),
            (86_400, "1970-01-02_00.00.00"),
            (951_782_400, "2000-02-29_00.00.00"),
            (951_868_800, "2000-03-01_00.00.00"),
            (4_107_542_400, "2100-03-01_00.00.00"),
            (-2_208_988_800, "1900-01-01_00.00.00"),
            (1_234_567_890, "2009-02-13_23.31.30"),
        ];
        for (secs, want) in cases {
            assert_eq!(timestamp(secs), want, "{secs}");
        }
    }

    #[test]
    fn names_sort_in_time_order() {
        let mut previous = String::new();
        for day in 0..(365 * 8) {
            let stamp = timestamp(day * 86_400 + 3661);
            assert_eq!(stamp.len(), 19, "{stamp}");
            assert!(stamp > previous, "{stamp} does not follow {previous}");
            previous = stamp;
        }
    }
}
