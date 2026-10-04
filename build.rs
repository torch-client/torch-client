fn main() {
    println!("cargo::rustc-check-cfg=cfg(packed_assets)");

    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if arch == "wasm32" || os == "android" {
        println!("cargo::rustc-cfg=packed_assets");
    }

    println!("cargo::rustc-check-cfg=cfg(resource_packs)");
    if std::env::var_os("CARGO_FEATURE_RESOURCEPACKS").is_some()
        && arch != "wasm32"
        && os != "android"
    {
        println!("cargo::rustc-cfg=resource_packs");
    }
}
