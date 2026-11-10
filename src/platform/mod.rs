pub(crate) mod address;
pub(crate) mod assets;
pub(crate) mod cli;
pub(crate) mod clipboard;
#[cfg(all(
    target_arch = "wasm32",
    any(feature = "asset_download", feature = "screen_record")
))]
pub(crate) mod download;
pub(crate) mod env;
pub(crate) mod executor;
pub(crate) mod fullscreen;
#[cfg(any(
    target_arch = "wasm32",
    feature = "asset_download",
    feature = "skins",
    feature = "update_checker",
    resource_packs,
    lua_mods
))]
pub(crate) mod http;
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub(crate) mod icon;
#[cfg(target_os = "android")]
pub(crate) mod java;
pub(crate) mod keyboard;
#[cfg(target_os = "android")]
pub(crate) mod logcat;
pub(crate) mod notify;
pub(crate) mod orientation;
#[cfg(any(
    feature = "shader_support",
    resource_packs,
    lua_mods,
    all(target_arch = "wasm32", eagler_skins)
))]
pub(crate) mod packfs;
pub(crate) mod pointer;
#[cfg(all(target_arch = "wasm32", feature = "screen_record"))]
pub(crate) mod record;
pub(crate) mod screenshot;
pub(crate) mod storage;
pub(crate) mod time;
#[cfg(all(
    target_arch = "wasm32",
    any(feature = "shader_support", resource_packs, eagler_skins, lua_mods)
))]
pub(crate) mod upload;
pub(crate) mod url;

#[cfg(target_arch = "wasm32")]
pub(crate) fn describe_js(value: &wasm_bindgen::JsValue) -> String {
    value
        .as_string()
        .or_else(|| {
            js_sys::Reflect::get(value, &"message".into())
                .ok()?
                .as_string()
        })
        .unwrap_or_else(|| format!("{value:?}"))
}
