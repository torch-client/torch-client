# Torch Client

[Torch](https://torchclient.com) is a Minecraft client rebuilt from the ground up in Rust. No JVM, a third of the memory, and twice the frame performance of the vanilla Java client.

**Supported versions:** `26.1.1` and `1.21.11`. One build joins both; there is no separate install per version.

> [!WARNING]
> Torch is in early development. Some features are missing and things will break. [Report a bug or suggest a feature](https://github.com/torch-client/torch-client/issues/new/choose). Or join [the discord server](https://discord.gg/qqXSHEYXaq)


<img width="960" height="526" alt="preview" src="https://github.com/user-attachments/assets/7139ff66-3257-4841-b810-75796659f7b3" />

Measured on 26.1.1 at a 32 chunk render distance. Results vary by system.

| | Torch | Java client |
| --- | --- | --- |
| Frame rate | 400 FPS | 180 FPS |
| Memory usage | 700 MB | 2.3 GB |
| Startup time | ~1 s | ~4 s |

## Download

Builds for Linux and Windows are on the [releases page](https://github.com/torch-client/torch-client/releases).

## Build from source

```sh
git clone https://github.com/torch-client/client.git
cd client
cargo build --release
```

See [CONTRIBUTING.md](.github/CONTRIBUTING.md#feature-system) to find out more about the projects cargo features

[Suggest a feature](https://github.com/torch-client/torch-client/issues/new/choose)

## Linux persistence

Override the ```MC_CLIENT_CONFIG_DIR``` env var (default value ```/tmp/torch-client```) to properly persist in a folder.

## FAQ

<details>
<summary>How do I enable shaders?</summary>

Go to:

`Video Settings -> Shaders -> Toggle On`

</details>

<details>
<summary>Why can't I use the click GUI?</summary>

Make sure you downloaded a release with `clickgui` in the file name. Builds without it do not include the click GUI.

</details>

## License

[GNU Affero General Public License v3.0 or later](LICENSE)

This project vendors modified source from:
- azalea: [vendor/azalea/LICENSE.md](vendor/azalea/LICENSE.md)
- bevy_camera: [vendor/bevy_camera/LICENSE-MIT](vendor/bevy_camera/LICENSE-MIT) or [vendor/bevy_camera/LICENSE-APACHE](vendor/bevy_camera/LICENSE-APACHE)
- simdnbt: [vendor/simdnbt/LICENSE](vendor/simdnbt/LICENSE)

Torch is not affiliated with Mojang Studios or Microsoft.

