# Contributing

## Issues

Use the Bug, Crash or Suggestion form on the issues page. Search existing issues first.

## Building

```sh
cargo build --release --features desktop             # desktop
tools/build_web.sh --harden --features web           # browser
tools/build_android.sh --features android            # Android APK
```

## Building from source

1. Install [rustup](https://rustup.rs). `rust-toolchain.toml` pins `nightly-2026-04-11` and rustup fetches it on the first build.
2. Clone the repository and run `cargo build --release` for desktop. The binary is `target/release/torch-client`.
3. For the browser, add the target with `rustup target add wasm32-unknown-unknown`, then run `tools/build_web.sh`. It writes `web-output/default/site/`, which you can serve with `python3 -m http.server 8080 --directory web-output/default/site`.
4. For Android, run `tools/build_android.sh`. It needs a JDK, the Android NDK and SDK build-tools; the script pins the exact versions. The APK lands in `dist/`.

Optional features are listed under [Feature system](#feature-system). If your change touches code behind a feature, build with that feature on (for example `--features builtin_shaders`), since code behind a feature that is off is not compiled at all.

## Pull requests

- Keep each PR to one change.
- For anything large you want to work on, preferably open an issue first before writing the code describing what you will add.
- Run `rustfmt` and match the style of the surrounding code.
- Keep changes under `vendor/` small.
- No including Mojang's assets in the source tree.

## Feature system

Parts of the client are Cargo features. These will be cleaned up in the future as they become more solid.

```sh
cargo build --release --features builtin_shaders,audio
tools/build_web.sh --features webgpu
```

| Feature | Default | What it does |
| --- | --- | --- |
| `asset_download` | on | Downloads the game's assets from Mojang on first run. |
| `multiversion` | on | Lets the client join servers on other protocol versions. |
| `full_font` | on | Draws every Unicode character instead of a missing-glyph box. |
| `skins` | on | Player skins and capes. Without it, everyone gets one of the built-in default skins. |
| `online_mode` | on | Microsoft account sign-in, for servers that require it. |
| `resourcepacks` | on | Vanilla resource packs from folders and zips. Not on Android. |
| `update_checker` | off | Checks GitHub once a day for a newer release and shows an update line on the title screen. Desktop only. |
| `self_update` | off | Offers that release on a full screen, then downloads, verifies and installs it and restarts. Pulls in `update_checker` and `asset_download`. x86_64 Windows and Linux only. |
| `builtin_shaders` | off | Built-in shaders: sun shadows, reflective water, waving foliage, height fog and bloom. |
| `shader_support` | off | Loads Iris/OptiFine-style shader packs from a `shaders/` folder. |
| `audio` | off | Sound. Downloads about 332 MB of sounds. Desktop only by itself; the browser and Android take `small_audio`. |
| `eagler` | off | Joins Eaglercraft servers over `wss://`. |
| `webgpu` | off | Browser build only. Uses WebGPU instead of WebGL2, with no fallback. |
| `mobile_ui` | off | On-screen touch controls. The Android build turns this on itself. |
| `click_gui` | off | A Right Shift menu with a switch and settings for every module. |
| `lua_mods` | off | Modules written in Luau, loaded from a `mods/` folder and switched from the click GUI, with a Mods tab to reload them. Not on Android. Turns on `click_gui`. |
| `hud_editor` | on | A screen for dragging, scaling and hiding HUD elements, saved to `hud.json`. |
| `tutorial` | on | Adds an introductory tutorial. |
| `render_scale` | on | A Render Scale slider for drawing the world below the window's resolution. |
| `microsoft_web` | on | The sign-in transport for the browser build. Turns on `online_mode`; adds nothing elsewhere. |
| `explore_modrinth` | off | Searches Modrinth for resource packs and downloads them. Not on Android. Turns on `resourcepacks`. |
| `benchmark_feature` | off | A Benchmark button on Options > Game. |
| `screen_record` | off | A Record button on the pause menu. Browser only. |
| `small_audio` | off | Browser and Android only. A 236 file sound set. The Android build turns this on itself. |
| `native_shaders` | off | The client's own shader pack format. Turns on `shader_support`. |
| `seed_cracker` | off | Recovers a world's seed. Turns on `click_gui`. |
| `ui_utils` | off | UI Utils' container buttons. Turns on `click_gui`. |
| `module_panel` | off | The module list as one window sized for touch. Turns on `click_gui`. |
| `profiling` | off | Span profiler. Press F9 in game to save a flamegraph. |
| `deadlock-detection` | off | Names the threads involved when the client freezes on a lock. |
| `alloc_diag` | off | Heap usage rows on the F3 screen. |
| `budget` | off | Per-frame timing breakdown. |

Each release build is one preset, on top of the default features:

| Preset | Build |
| --- | --- |
| `desktop` | Windows and Linux |
| `desktop_clickgui` | Windows and Linux with the click GUI |
| `web` | Browser |
| `web_mobile` | Browser on a phone |
| `android` | Android |

To build without the default features, add `--no-default-features` and list the ones you want.
