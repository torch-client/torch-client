fn main() {
    println!("cargo::rustc-check-cfg=cfg(packed_assets)");

    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if arch == "wasm32" || os == "android" {
        println!("cargo::rustc-cfg=packed_assets");
    }

    println!("cargo::rustc-check-cfg=cfg(resource_packs)");
    println!("cargo::rustc-check-cfg=cfg(explore_modrinth)");
    if std::env::var_os("CARGO_FEATURE_RESOURCEPACKS").is_some() && os != "android" {
        println!("cargo::rustc-cfg=resource_packs");
        if std::env::var_os("CARGO_FEATURE_EXPLORE_MODRINTH").is_some() {
            println!("cargo::rustc-cfg=explore_modrinth");
        }
    }

    println!("cargo::rustc-check-cfg=cfg(lua_mods)");
    if std::env::var_os("CARGO_FEATURE_LUA_MODS").is_some() && os != "android" {
        println!("cargo::rustc-cfg=lua_mods");
    }

    println!("cargo::rustc-check-cfg=cfg(self_update)");
    if std::env::var_os("CARGO_FEATURE_SELF_UPDATE").is_some()
        && (os == "linux" || os == "windows")
        && arch == "x86_64"
    {
        println!("cargo::rustc-cfg=self_update");
    }

    println!("cargo::rustc-check-cfg=cfg(eagler_skins)");
    if std::env::var_os("CARGO_FEATURE_SKINS").is_some()
        && (std::env::var_os("CARGO_FEATURE_EAGLER").is_some() || arch == "wasm32")
    {
        println!("cargo::rustc-cfg=eagler_skins");
    }

    if os == "windows" {
        winresource::WindowsResource::new()
            .set_icon("windows/icon.ico")
            .compile()
            .expect("embedding windows/icon.ico");
    }
}
