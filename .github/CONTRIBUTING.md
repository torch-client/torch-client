# Contributing

## Issues

Use the Bug, Crash or Suggestion form on the issues page. Search existing issues first.

## Building

```sh
cargo build --release          # desktop
tools/build_web.sh             # browser
tools/build_android.sh         # Android APK
```

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
| `builtin_shaders` | off | Built-in shaders: sun shadows, reflective water, waving foliage, height fog and bloom. |
| `shader_support` | off | Loads Iris/OptiFine-style shader packs from a `shaders/` folder. |
| `audio` | off | Sound. Desktop only; downloads about 332 MB of sounds. |
| `eagler` | off | Joins Eaglercraft servers over `wss://`. |
| `webgpu` | off | Browser build only. Uses WebGPU instead of WebGL2, with no fallback. |
| `mobile_ui` | off | On-screen touch controls. The Android build turns this on itself. |
| `click_gui` | off | A Right Shift menu. Layout only; its settings do nothing yet. |
| `profiling` | off | Span profiler. Press F9 in game to save a flamegraph. |
| `deadlock-detection` | off | Names the threads involved when the client freezes on a lock. |
| `alloc_diag` | off | Heap usage rows on the F3 screen. |
| `budget` | off | Per-frame timing breakdown. |

To build without the default features, add `--no-default-features` and list the ones you want.
