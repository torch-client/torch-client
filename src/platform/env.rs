pub(crate) struct Var {
    pub name: &'static str,
    pub what: &'static str,
    pub default: &'static str,
    pub enabled: bool,
}

const ALL: &[Var] = &[
    Var {
        name: "MINECRAFT_ASSETS",
        what: "directory the game's assets are read from",
        default: if cfg!(feature = "asset_download") {
            "the downloaded asset set"
        } else {
            "reference/minecraft-26.1.1/assets/minecraft in the checkout"
        },
        enabled: true,
    },
    Var {
        name: "MINECRAFT_DATA",
        what: "directory the vanilla datapack is read from",
        default: if cfg!(feature = "asset_download") {
            "the downloaded asset set"
        } else {
            "reference/minecraft-26.1.1/data/minecraft in the checkout"
        },
        enabled: true,
    },
    Var {
        name: "MINECRAFT_TEXTURES",
        what: "block and item texture directory",
        default: "textures/ under the asset root",
        enabled: true,
    },
    Var {
        name: "MC_CLIENT_CONFIG_DIR",
        what: "where options, keybinds, the server list and the accounts are written",
        default: if cfg!(target_os = "android") {
            "the package's own data directory"
        } else if cfg!(target_os = "windows") {
            "torch-client\\config under %LOCALAPPDATA%"
        } else {
            "/tmp/torch-client"
        },
        enabled: !cfg!(target_arch = "wasm32"),
    },
    Var {
        name: "MC_ASSET_DIR",
        what: "where downloaded assets are stored",
        default: if cfg!(target_os = "windows") {
            "torch-client\\assets under %LOCALAPPDATA%"
        } else {
            "torch-client/assets under the system temporary directory"
        },
        enabled: cfg!(all(feature = "asset_download", not(target_arch = "wasm32"))),
    },
    Var {
        name: "MC_POOL_THREADS",
        what: "threads in the compute task pool",
        default: "4",
        enabled: true,
    },
    Var {
        name: "MC_EXEC",
        what: "`mt` puts bevy's multi-threaded executor back",
        default: "single-threaded",
        enabled: true,
    },
    Var {
        name: "MC_PAR",
        what: "how wide the per-entity passes fan out",
        default: "4",
        enabled: true,
    },
    Var {
        name: "MC_PROTOCOL",
        what: "protocol number to speak on the wire, overriding what the server says \
               (on the web build, read from the page's own `?MC_PROTOCOL=` query string instead)",
        default: "asked for with a status ping before each join",
        enabled: cfg!(feature = "multiversion"),
    },
    Var {
        name: "MC_MAX_FED",
        what: "entities handed to the world per tick",
        default: "10000",
        enabled: true,
    },
    Var {
        name: "MC_MAX_SIMULATED",
        what: "entities given client-side physics per tick",
        default: "12000",
        enabled: true,
    },
    Var {
        name: "MC_MAX_ENTITIES",
        what: "entities drawn at once",
        default: "10000",
        enabled: true,
    },
    Var {
        name: "MC_COLLIDE_CHECK",
        what: "cross-check the fast collision path against the general one",
        default: "off",
        enabled: true,
    },
    Var {
        name: "MC_NO_OCCLUSION",
        what: "draw every loaded section instead of culling occluded ones",
        default: "off",
        enabled: true,
    },
    Var {
        name: "MC_NO_CUTOUT",
        what: "draw the opaque terrain with no alpha test, which is faster and \
               wrong on screen: it is here to measure what the cutout costs",
        default: "off",
        enabled: true,
    },
    Var {
        name: "MC_UPLOAD_VERTS",
        what: "terrain vertices uploaded per frame",
        default: if cfg!(target_arch = "wasm32") {
            "30000"
        } else {
            "120000"
        },
        enabled: true,
    },
    Var {
        name: "MC_UPLOAD_SECTIONS",
        what: "terrain sections uploaded per frame",
        default: if cfg!(target_arch = "wasm32") {
            "24"
        } else {
            "128"
        },
        enabled: true,
    },
    Var {
        name: "MC_MESH_THREADS",
        what: "mesh worker threads",
        default: "2",
        enabled: !cfg!(target_arch = "wasm32"),
    },
    Var {
        name: "MC_MESH_BUDGET_MS",
        what: "milliseconds spent draining finished mesh jobs per call",
        default: "4",
        enabled: true,
    },
    Var {
        name: "MC_TERRAIN_DIRECT",
        what: "force the direct terrain tier (one draw per run of sections) on a device that has the indirect one",
        default: "off",
        enabled: true,
    },
    Var {
        name: "MC_SHADERPACK_PASSES",
        what: "run only this many of a shader pack's full-screen passes (deferred, composite, final, in order) and show colortex0 as it stands, to find which pass misbehaves",
        default: "every pass",
        enabled: cfg!(feature = "shader_support"),
    },
    Var {
        name: "MC_SHAFTS",
        what: "volumetric light shafts in the post-processing chain",
        default: "off",
        enabled: cfg!(all(
            feature = "builtin_shaders",
            not(target_arch = "wasm32")
        )),
    },
    Var {
        name: "MC_AUTO_LOOK",
        what: "degrees per second the camera turns on its own, for capture runs",
        default: "0",
        enabled: true,
    },
    Var {
        name: "MC_EXIT_AFTER_MS",
        what: "quit this many milliseconds after startup",
        default: "never quits",
        enabled: true,
    },
    Var {
        name: "MC_DEBUG",
        what: "comma-separated log tags to print, or `all`",
        default: "none",
        enabled: true,
    },
    Var {
        name: "MC_DEBUG_CHUNKS",
        what: "older spelling of MC_DEBUG=chunks, on if set to anything",
        default: "off",
        enabled: true,
    },
    Var {
        name: "MC_DEBUG_LIGHT",
        what: "older spelling of MC_DEBUG=light, on if set to anything",
        default: "off",
        enabled: true,
    },
    Var {
        name: "MC_TRACE",
        what: "log every packet that crosses the connection",
        default: "off",
        enabled: true,
    },
    Var {
        name: "MC_INV_TRACE",
        what: "log every inventory menu change",
        default: "off",
        enabled: true,
    },
    Var {
        name: "MC_BOT_KILL_SECS",
        what: "seconds a stalled bot thread gets before the watchdog tears it down, 0 to never",
        default: "5",
        enabled: true,
    },
    Var {
        name: "MC_CHUNK_RATE_MIN",
        what: "floor on the chunks per tick this client asks the server to stream at, 0 for \
               upstream azalea's behaviour",
        default: "1",
        enabled: true,
    },
    Var {
        name: "AZALEA_DO_NOT_CUT_OFF_PACKET_LOGS",
        what: "print whole packet bodies in the trace log instead of the first 500 bytes",
        default: "off",
        enabled: true,
    },
    Var {
        name: "MC_PROF_MAX_RECORDS",
        what: "span records kept before the profiler stops recording",
        default: "2000000",
        enabled: cfg!(feature = "profiling"),
    },
    Var {
        name: "MC_PROF_WINDOW_MS",
        what: "length of one F9 capture window in milliseconds",
        default: "1000",
        enabled: cfg!(feature = "profiling"),
    },
    Var {
        name: "MC_PROF_TAG",
        what: "name used in the flamegraph file this build writes",
        default: "cap",
        enabled: cfg!(feature = "profiling"),
    },
    Var {
        name: "MC_PROF_AT",
        what: "milliseconds after startup to capture at, comma separated",
        default: "none",
        enabled: cfg!(feature = "profiling"),
    },
];

pub(crate) fn table() -> impl Iterator<Item = &'static Var> {
    ALL.iter().filter(|v| v.enabled)
}

#[cfg(test)]
mod tests {
    use super::ALL;

    const UNLISTED: &[&str] = &["MC_GUI_PREVIEW_DIR"];

    #[test]
    fn the_table_covers_every_variable_the_crate_reads() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut missing = Vec::new();
        for path in rust_files(&src) {
            let text = std::fs::read_to_string(&path).expect("read a source file");
            for name in read_names(&text) {
                if UNLISTED.contains(&name.as_str())
                    || ALL.iter().any(|v| v.name == name)
                    || missing.contains(&name)
                {
                    continue;
                }
                missing.push(name);
            }
        }
        assert!(
            missing.is_empty(),
            "these variables are read but not described in platform::env: {missing:?}"
        );
    }

    fn read_names(text: &str) -> Vec<String> {
        let mut names = Vec::new();
        for call in ["env::var(\"", "env::var_os(\""] {
            let mut rest = text;
            while let Some(at) = rest.find(call) {
                rest = &rest[at + call.len()..];
                let Some(end) = rest.find('"') else { break };
                let name = &rest[..end];
                if name.starts_with("MC_") || name.starts_with("MINECRAFT_") {
                    names.push(name.to_string());
                }
            }
        }
        names
    }

    fn rust_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut found = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return found;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                found.extend(rust_files(&path));
            } else if path.extension().is_some_and(|e| e == "rs") {
                found.push(path);
            }
        }
        found
    }
}
