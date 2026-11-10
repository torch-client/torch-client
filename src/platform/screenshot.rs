#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn dir() -> std::path::PathBuf {
    crate::platform::storage::dir().join("screenshots")
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn store(
    image: &image::RgbImage,
    report: impl FnOnce(Result<String, String>) + 'static,
) {
    report(save(image).map(|name| format!("Captured screenshot as {name}")));
}

fn encode_png(image: &image::RgbImage) -> Result<Vec<u8>, String> {
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
    Ok(png)
}

#[cfg(not(target_arch = "wasm32"))]
fn save(image: &image::RgbImage) -> Result<String, String> {
    let png = encode_png(image)?;

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

#[cfg(any(not(target_arch = "wasm32"), feature = "screen_record"))]
pub(crate) fn epoch_secs() -> i64 {
    crate::platform::time::SystemTime::now()
        .duration_since(crate::platform::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(any(not(target_arch = "wasm32"), feature = "screen_record"))]
pub(crate) fn timestamp(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let (year, month, day) = crate::util::calendar::civil_from_days(days);

    format!("{year:04}-{month:02}-{day:02}_{hour:02}.{minute:02}.{second:02}")
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn store(
    image: &image::RgbImage,
    report: impl FnOnce(Result<String, String>) + 'static,
) {
    let promise = match clipboard_write(image) {
        Ok(promise) => promise,
        Err(e) => return report(Err(e)),
    };
    wasm_bindgen_futures::spawn_local(async move {
        report(
            wasm_bindgen_futures::JsFuture::from(promise)
                .await
                .map(|_| "Copied screenshot to the clipboard".to_string())
                .map_err(|e| {
                    format!(
                        "the browser refused the clipboard write: {}",
                        super::describe_js(&e)
                    )
                }),
        );
    });
}

#[cfg(target_arch = "wasm32")]
fn clipboard_write(image: &image::RgbImage) -> Result<js_sys::Promise, String> {
    let js = |e: wasm_bindgen::JsValue| super::describe_js(&e);
    let png = encode_png(image)?;

    let options = web_sys::BlobPropertyBag::new();
    options.set_type("image/png");
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(png.as_slice()));
    let blob =
        web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options).map_err(js)?;

    let record = js_sys::Object::new();
    js_sys::Reflect::set(&record, &"image/png".into(), &blob).map_err(js)?;
    let item = web_sys::ClipboardItem::new_with_record_from_str_to_blob_promise(&record)
        .map_err(|e| format!("this browser cannot copy images: {}", js(e)))?;

    let window = web_sys::window().ok_or("no window")?;
    Ok(window
        .navigator()
        .clipboard()
        .write(&js_sys::Array::of1(&item)))
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
