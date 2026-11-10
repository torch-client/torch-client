use wasm_bindgen::JsCast;

#[cfg(feature = "asset_download")]
pub(crate) fn save(name: &str, bytes: &[u8], mime: &str) -> Result<(), String> {
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
        .map_err(|e| super::describe_js(&e))?;
    save_blob(name, &blob)
}

pub(crate) fn save_blob(name: &str, blob: &web_sys::Blob) -> Result<(), String> {
    let js = |e: wasm_bindgen::JsValue| super::describe_js(&e);
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let url = web_sys::Url::create_object_url_with_blob(blob).map_err(js)?;

    let anchor: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(js)?
        .dyn_into()
        .map_err(|_| "not an anchor")?;
    anchor.set_href(&url);
    anchor.set_download(name);
    anchor.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}
