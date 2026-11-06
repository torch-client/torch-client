#![cfg_attr(target_arch = "wasm32", feature(alloc_error_hook))]

#[cfg(feature = "audio")]
mod audio;
mod blockentities;
mod blocks;
mod client;
mod diag;
#[cfg(any(feature = "eagler", target_arch = "wasm32"))]
mod eagler;
mod entities;
mod generated_items;
mod gui;
mod items;
mod lighting;
#[cfg(feature = "mobile_ui")]
mod mobile;
mod modules;
mod platform;
mod play;
#[cfg(feature = "multiversion")]
mod protocol;
mod renderer;
#[cfg(resource_packs)]
mod resourcepacks;
mod session;
#[cfg(feature = "shader_support")]
mod shaderpack;
mod text;
mod util;

#[cfg(feature = "alloc_diag")]
#[global_allocator]
static ALLOC: diag::alloc::Counting = diag::alloc::Counting;

#[macro_export]
macro_rules! prof_span {
    ($name:expr) => {
        #[cfg(feature = "profiling")]
        let _prof_span = {
            static PROF_SPAN_ID: ::std::sync::OnceLock<u32> = ::std::sync::OnceLock::new();
            $crate::diag::profiling::span_cached(&PROF_SPAN_ID, $name)
        };
    };
}

use crate::session::SharedMutex;
use azalea_protocol::packets::PROTOCOL_VERSION;
use client::worker::JobQueue;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

pub(crate) static SHARED: OnceLock<Arc<SharedMutex>> = OnceLock::new();
pub(crate) static CHUNK_Q: OnceLock<Arc<JobQueue>> = OnceLock::new();

#[allow(
    dead_code,
    reason = "read only where an asset set can be missing, which is the asset_download builds"
)]
const ASSET_PROBE: &str = "font/default.json";

pub(crate) const ASSET_VERSION: &str = "26.1.1";

pub(crate) fn assets_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| match std::env::var_os("MINECRAFT_ASSETS") {
        Some(p) => PathBuf::from(p),
        None => pack_root().join("assets").join("minecraft"),
    })
}

fn pack_root() -> PathBuf {
    #[cfg(feature = "asset_download")]
    {
        client::assets::pack_root(ASSET_VERSION)
    }
    #[cfg(not(feature = "asset_download"))]
    {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/minecraft-26.1.1")
    }
}

pub(crate) fn datapack_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| match std::env::var_os("MINECRAFT_DATA") {
        Some(p) => PathBuf::from(p),
        None => pack_root().join("data").join("minecraft"),
    })
}

fn assets_present() -> bool {
    #[cfg(feature = "asset_download")]
    {
        platform::assets::read(assets_root().join(ASSET_PROBE)).is_some()
    }
    #[cfg(not(feature = "asset_download"))]
    {
        true
    }
}

pub(crate) fn textures_dir() -> PathBuf {
    match std::env::var_os("MINECRAFT_TEXTURES") {
        Some(p) => PathBuf::from(p),
        None => assets_root().join("textures"),
    }
}

#[cfg(any(
    feature = "online_mode",
    feature = "eagler",
    all(explore_modrinth, not(target_arch = "wasm32")),
    all(feature = "asset_download", not(target_arch = "wasm32"))
))]
use rustls as tls;

#[cfg(any(
    feature = "online_mode",
    feature = "eagler",
    all(explore_modrinth, not(target_arch = "wasm32")),
    all(feature = "asset_download", not(target_arch = "wasm32"))
))]
pub(crate) fn install_crypto_provider() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let _ = tls::crypto::ring::default_provider().install_default();
    });
}

#[cfg(all(
    target_os = "android",
    any(feature = "online_mode", feature = "asset_download", feature = "skins")
))]
pub(crate) fn android_tls_config() -> Option<rustls::ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let provider = std::sync::Arc::new(rustls::crypto::ring::default_provider());
    match rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
    {
        Ok(builder) => Some(builder.with_root_certificates(roots).with_no_client_auth()),
        Err(e) => {
            log_warn!("net", "no TLS config for Android: {e}");
            None
        }
    }
}

#[cfg(not(any(
    feature = "online_mode",
    feature = "eagler",
    all(explore_modrinth, not(target_arch = "wasm32")),
    all(feature = "asset_download", not(target_arch = "wasm32"))
)))]
pub(crate) fn install_crypto_provider() {}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn start_web(assets: Vec<u8>, username: String, address: String) -> Result<(), String> {
    console_error_panic_hook::set_once();
    diag::panic_report::install_alloc_error_hook();
    step("start_web entered (build: breadcrumbs)");

    platform::keyboard::install_devtools_passthrough();

    #[cfg(feature = "webgpu")]
    require_webgpu()?;

    if assets.is_empty() && cfg!(feature = "asset_download") {
        step("no stored assets; the download screen will ask");
    } else {
        step(&format!("unpacking {} bytes of assets", assets.len()));
        client::mesh_worker::spawn(&assets);
        load_assets(assets)?;
    }

    step("assets ready, starting the client");
    start(
        Some(username.trim().to_string()).filter(|u| platform::cli::valid_username(u)),
        Some(address).filter(|a| !a.is_empty()),
    )
    .map_err(|e| e.to_string())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn start_mesh_worker(assets: Vec<u8>) {
    client::mesh_worker::mirror::start(assets);
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn bake_block_lookups() {
    let textures_dir = textures_dir().to_string_lossy().into_owned();
    let textures = renderer::Textures::blocks(&textures_dir);
    let atlas = renderer::build_block_atlas(&textures, &textures_dir, false);
    atlas.tables.publish();
    blocks::install(blocks::Blocks::load(atlas.sprites));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn take_web_assets() -> Option<Vec<u8>> {
    client::assets::take_installed()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn seed_mod_file(path: String, bytes: Option<Vec<u8>>) {
    #[cfg(lua_mods)]
    platform::packfs::seed(&path, bytes);
    #[cfg(not(lua_mods))]
    let _ = (path, bytes);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn ships_assets() -> bool {
    !cfg!(feature = "asset_download")
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn asset_cache_key() -> String {
    let magic = std::str::from_utf8(platform::assets::ARCHIVE_MAGIC).unwrap_or("archive");
    format!("{magic}-{ASSET_VERSION}")
}

#[cfg(all(target_arch = "wasm32", feature = "webgpu"))]
fn require_webgpu() -> Result<(), String> {
    use wasm_bindgen::JsValue;

    let window = web_sys::window().ok_or_else(|| {
        "no `window`: this build has to run in a browser tab, not a worker".to_string()
    })?;
    let navigator = JsValue::from(window.navigator());
    let gpu = js_sys::Reflect::get(&navigator, &JsValue::from_str("gpu"))
        .map_err(|_| "could not read `navigator.gpu`".to_string())?;
    if gpu.is_undefined() || gpu.is_null() {
        return Err(
            "this browser has no WebGPU (`navigator.gpu` is missing), and this \
                    build has no WebGL2 path compiled into it. Use the WebGL2 site \
                    instead. On a phone, WebGPU needs a recent Chrome or Edge; Firefox \
                    and iOS Safari are still catching up. Serving over https:// or \
                    localhost is also required."
                .to_string(),
        );
    }
    step("navigator.gpu present");
    Ok(())
}

pub fn start(username: Option<String>, address: Option<String>) -> eyre::Result<()> {
    run(platform::cli::Args {
        username,
        address,
        access_token: None,
    })
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
pub fn start_from_cli() -> eyre::Result<()> {
    run(platform::cli::parse())
}

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
pub fn load_assets(packed: Vec<u8>) -> Result<(), String> {
    platform::assets::archive::load(packed)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: bevy::android::android_activity::AndroidApp) {
    platform::logcat::redirect_stdio();
    step("android_main entered");
    let _ = bevy::android::ANDROID_APP.set(app.clone());
    platform::fullscreen::android_init();

    match platform::assets::android::read_apk_archive(&app) {
        Ok(packed) => {
            step(&format!(
                "unpacking {} bytes of bundled assets",
                packed.len()
            ));
            if let Err(e) = load_assets(packed) {
                return step(&format!("assets refused: {e}"));
            }
        }
        Err(_) if cfg!(feature = "asset_download") => {
            step("no bundled assets; mounting a downloaded set or asking for one");
        }
        Err(e) => return step(&format!("no assets in the apk: {e}")),
    }

    platform::assets::android::apply_env(&app);

    let (username, address) = platform::assets::android::launch_target(&app);
    step(&format!(
        "assets ready, starting the client (address: {})",
        address
            .as_deref()
            .unwrap_or("none, opening the title screen")
    ));
    if let Err(e) = start(username, address) {
        step(&format!("client exited: {e}"));
    }

    step("event loop ended, finishing the activity");
    finish_and_exit(&app);
}

#[cfg(target_os = "android")]
pub(crate) fn finish_and_exit(app: &bevy::android::android_activity::AndroidApp) -> ! {
    unsafe extern "C" {
        fn ANativeActivity_finish(activity: *mut core::ffi::c_void);
    }
    unsafe { ANativeActivity_finish(app.activity_as_ptr()) };
    std::thread::sleep(std::time::Duration::from_millis(100));
    std::process::exit(0);
}

#[cfg(target_arch = "wasm32")]
pub fn step(message: &str) {
    web_sys::console::log_1(&format!("[mc] {message}").into());
}

#[cfg(target_os = "android")]
pub fn step(message: &str) {
    platform::logcat::write(message);
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
pub fn step(_message: &str) {}

fn authenticate(args: &platform::cli::Args) -> Option<String> {
    #[cfg(all(feature = "online_mode", not(target_arch = "wasm32")))]
    {
        let token = args.access_token.clone()?;
        let name = match client::auth::sign_in_with_token(token) {
            Ok(name) => name,
            Err(message) => {
                eprintln!("{message}");
                std::process::exit(1);
            }
        };
        if let Some(asked) = &args.username
            && !asked.eq_ignore_ascii_case(&name)
        {
            eprintln!(
                "The access token belongs to {name}, but --username asked for {asked}. The token \
                 decides the name; drop --username, or pass the one the token belongs to."
            );
            std::process::exit(2);
        }
        Some(name)
    }
    #[cfg(not(all(feature = "online_mode", not(target_arch = "wasm32"))))]
    {
        let _ = args;
        None
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn create_drop_folders() {
    #[allow(unused_mut)]
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    #[cfg(lua_mods)]
    dirs.push(modules::script::folder());
    #[cfg(lua_mods)]
    dirs.push(platform::storage::dir().join(modules::script::MOD_DATA));
    #[cfg(resource_packs)]
    dirs.push(resourcepacks::dir());
    #[cfg(feature = "shader_support")]
    dirs.push(shaderpack::discover::dir());
    for dir in dirs {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log_warn!("storage", "could not create {}: {e}", dir.display());
        }
    }
}

fn run(args: platform::cli::Args) -> eyre::Result<()> {
    diag::alloc::label_thread(diag::alloc::Site::Render);
    diag::alloc::init_task_pools();
    diag::panic_report::install_panic_hook();
    client::bot::install_exit_signal_handlers();
    install_crypto_provider();
    let cli_account = authenticate(&args);
    let online = cli_account.is_some();
    let account = cli_account
        .or_else(|| args.username.clone())
        .or_else(gui::accountlist::active_username);
    let needs_account = account.is_none();
    if let Some(name) = account {
        client::bot::set_username(name);
    }
    log_info!(
        "net",
        "Torch Client ({} mode)",
        if online { "online" } else { "offline" }
    );
    log_info!("net", "protocol {PROTOCOL_VERSION} (MC 26.1.2)");
    if needs_account {
        log_info!("net", "no account yet: opening the account screen");
    } else {
        log_info!("net", "joining as {}", client::bot::username());
    }

    #[cfg(not(target_arch = "wasm32"))]
    create_drop_folders();

    {
        let threads = std::env::var("MC_POOL_THREADS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(4);
        bevy::tasks::ComputeTaskPool::get_or_init(|| {
            bevy::tasks::TaskPoolBuilder::new()
                .num_threads(threads)
                .thread_name("Compute Task Pool".to_string())
                .build()
        });
        log_info!("net", "compute pool: {threads} threads");
        step("compute pool ready");
    }

    #[cfg(feature = "asset_download")]
    {
        let started = platform::time::Instant::now();
        if client::assets::mount() {
            log_info!(
                "assets",
                "mounted the installed set in {:.0} ms",
                started.elapsed().as_secs_f32() * 1000.0
            );
            step("mounted the downloaded assets");
        }
    }

    let assets_root = assets_root();
    let textures_dir = textures_dir().to_string_lossy().into_owned();
    log_info!("assets", "root {}", assets_root.display());
    log_info!("assets", "textures {textures_dir}");

    let have_assets = assets_present();
    #[cfg(all(feature = "update_checker", not(target_arch = "wasm32")))]
    if have_assets {
        client::update_checker::spawn();
    }
    #[cfg(resource_packs)]
    if have_assets {
        resourcepacks::mount();
        #[cfg(feature = "audio")]
        audio::packs_changed();
    }
    #[cfg(lua_mods)]
    if have_assets {
        modules::script::init();
    }
    let (atlas_image, gui_atlas) = if have_assets {
        step("building the atlases");
        let (block, gui) = renderer::reload::build_atlases().install();
        (block, Arc::new(gui))
    } else {
        log_info!("assets", "no asset set found; opening the download screen");
        step("no assets: building the boot atlas");
        (
            renderer::placeholder_atlas(),
            Arc::new(gui::atlas::GuiAtlas::boot()),
        )
    };

    let address = args.address;

    let shared = Arc::new(SharedMutex::default());
    SHARED.set(shared.clone()).ok();
    CHUNK_Q
        .set(client::worker::spawn_chunk_workers(shared.clone()))
        .ok();
    step("starting workers");
    client::worldsync::start_light_thread();
    client::worldsync::install_block_change_hook();
    #[cfg(feature = "render_lod")]
    client::lod::cache::install();
    if crate::diag::trace_enabled()
        && azalea_protocol::packet_trace::set_sent_packet_hook(Box::new(|p| {
            if let Some(name) = p.name {
                crate::diag::note_sent_packet(name);
            }
        }))
        .is_err()
    {
        log_warn!(
            "net",
            "the serverbound packet trace could not be installed because something else              already owns the hook; the opening trace will show only what the server sends."
        );
    }

    diag::start_watchdog();

    let startup = if !have_assets {
        #[cfg(feature = "asset_download")]
        {
            gui::render::StartupState {
                screen: gui::Screen::AssetDownload,
                first_run: true,
                pending_connect: address,
                after_download: if needs_account {
                    gui::profile::entry_screen()
                } else {
                    gui::Screen::Title
                },
            }
        }
        #[cfg(not(feature = "asset_download"))]
        {
            unreachable!()
        }
    } else if needs_account {
        gui::render::StartupState {
            screen: gui::profile::entry_screen(),
            first_run: true,
            pending_connect: address,
            ..Default::default()
        }
    } else {
        gui::render::StartupState {
            screen: match address {
                Some(address) => {
                    client::bot::start_bot(address);
                    gui::Screen::None
                }
                None => gui::Screen::Title,
            },
            ..Default::default()
        }
    };

    step("handing off to the renderer");
    renderer::run(shared, atlas_image, gui_atlas, startup, have_assets);
    Ok(())
}
