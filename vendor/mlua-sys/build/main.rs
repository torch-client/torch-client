use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const LUAU_VERSION: &str = "0.709";

const LIBRARIES: &[(&str, &[&str])] = &[
    ("Ast", &["Ast/include"]),
    ("Common", &["Common/include"]),
    ("Compiler", &["Compiler/include", "Ast/include"]),
    ("Config", &["Config/include", "Ast/include", "Compiler/include", "VM/include"]),
    ("Custom", &["VM/include", "VM/src"]),
    ("Require", &["Require/include", "Ast/include", "Config/include", "VM/include"]),
    ("VM", &["VM/include"]),
];

fn main() {
    let unsupported = [
        ("lua51", cfg!(feature = "lua51")),
        ("lua52", cfg!(feature = "lua52")),
        ("lua53", cfg!(feature = "lua53")),
        ("lua54", cfg!(feature = "lua54")),
        ("lua55", cfg!(feature = "lua55")),
        ("luajit", cfg!(feature = "luajit")),
        ("external", cfg!(feature = "external")),
        ("module", cfg!(feature = "module")),
        ("luau-codegen", cfg!(feature = "luau-codegen")),
    ];
    for (name, enabled) in unsupported {
        if enabled {
            panic!("vendor/mlua-sys builds only Luau, without codegen; the `{name}` feature is not supported");
        }
    }
    if !cfg!(feature = "luau") {
        panic!("vendor/mlua-sys builds only Luau; enable the `luau` feature");
    }

    println!("cargo:rerun-if-changed=build");
    println!("cargo:rerun-if-changed=luau");
    println!("cargo:rerun-if-changed=shim");

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let target = env::var("TARGET").unwrap();
    let host = env::var("HOST").unwrap();
    let wasm32 = env::var("CARGO_CFG_TARGET_ARCH").unwrap() == "wasm32";
    let luau = manifest_dir.join("luau");
    let shim = manifest_dir.join("shim");

    let wasm = wasm32.then(|| WasmToolchain::find(&host));

    let mut base = cc::Build::new();
    base.cpp(true)
        .std("c++17")
        .warnings(false)
        .cargo_metadata(false)
        .define("LUAI_MAXCSTACK", "1000000")
        .define("LUA_VECTOR_SIZE", if cfg!(feature = "luau-vector4") { "4" } else { "3" })
        .define("LUA_API", "extern \"C\"")
        .include(luau.join("Common/include"));

    if target.ends_with("emscripten") {
        base.flag_if_supported("-fexceptions");
    }

    if cfg!(debug_assertions) {
        base.define("LUAU_ENABLE_ASSERT", None);
    } else if !wasm32 {
        base.flag_if_supported("-fno-math-errno");
    }

    if let Some(wasm) = &wasm {
        wasm32_cxx(&mut base, wasm, &shim);
    }

    let mut objects = Vec::new();
    for (name, includes) in LIBRARIES {
        let mut build = base.clone();
        for include in *includes {
            build.include(luau.join(include));
        }
        if *name == "Compiler" {
            build.define("LUACODE_API", "extern \"C\"");
        }
        add_sorted(&mut build, &luau.join(name).join("src"), "cpp");
        objects.extend(build.compile_intermediates());
    }

    if let Some(wasm) = &wasm {
        let mut cxx = base.clone();
        cxx.file(shim.join("runtime.cpp"));
        objects.extend(cxx.compile_intermediates());

        let mut c = cc::Build::new();
        c.cpp(false)
            .std("c11")
            .warnings(false)
            .cargo_metadata(false)
            .compiler(&wasm.clang)
            .archiver(&wasm.ar)
            .no_default_flags(true)
            .flag("--target=wasm32-unknown-unknown")
            .flag("-O2")
            .flag("-ffunction-sections")
            .flag("-fdata-sections")
            .flag("-fno-math-errno")
            .flag("-nostdlibinc")
            .flag("-isystem")
            .flag(shim.join("include"))
            .file(shim.join("runtime.c"))
            .file(shim.join("printf.c"));
        objects.extend(c.compile_intermediates());
    }

    let mut archive = base.clone();
    archive.objects(&objects).out_dir(&out_dir).compile("luau");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=luau");
    if !wasm32 {
        if let Some(stdlib) = cpp_link_stdlib(&target, &host) {
            println!("cargo:rustc-link-lib={stdlib}");
        }
    }
    println!("cargo:rustc-env=LUAU_VERSION={LUAU_VERSION}");
}

fn wasm32_cxx(build: &mut cc::Build, wasm: &WasmToolchain, shim: &Path) {
    build
        .compiler(&wasm.clangxx)
        .archiver(&wasm.ar)
        .no_default_flags(true)
        .flag("--target=wasm32-unknown-unknown")
        .flag("-O2")
        .flag("-ffunction-sections")
        .flag("-fdata-sections")
        .flag("-fno-exceptions")
        .flag("-fno-rtti")
        .flag("-fno-math-errno")
        .flag("-nostdlibinc");
    for dir in &wasm.libcxx_include {
        build.flag("-isystem").flag(dir);
    }
    build
        .flag("-isystem")
        .flag(shim.join("include"))
        .define("_LIBCPP_HAS_NO_THREADS", None)
        .define("_LIBCPP_HAS_NO_WIDE_CHARACTERS", None)
        .define("_LIBCPP_HAS_NO_LOCALIZATION", None)
        .define("LUA_USE_LONGJMP", "1")
        .define("LUAU_NO_EXCEPTIONS", None)
        .flag("-mllvm")
        .flag("-enable-emscripten-sjlj");
}

struct WasmToolchain {
    clang: PathBuf,
    clangxx: PathBuf,
    ar: PathBuf,
    libcxx_include: Vec<PathBuf>,
}

impl WasmToolchain {
    fn find(host: &str) -> Self {
        for var in [
            "WASM_CLANG",
            "WASM_LIBCXX_INCLUDE",
            "WASI_SDK_PATH",
            "PATH",
            "ANDROID_NDK_HOME",
            "ANDROID_NDK_ROOT",
            "ANDROID_HOME",
            "ANDROID_SDK_ROOT",
            "HOME",
        ] {
            println!("cargo:rerun-if-env-changed={var}");
        }
        let var = |name: &str| env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);

        let (given, sysroot_includes): (PathBuf, Vec<PathBuf>) = if let Some(given) = var("WASM_CLANG") {
            (given, Vec::new())
        } else if let Some(sdk) = var("WASI_SDK_PATH") {
            let sysroot = sdk.join("share/wasi-sysroot/include");
            (sdk.join("bin").join(exe("clang++")), vec![sysroot.join("c++/v1"), sysroot.join("wasm32-wasi/c++/v1")])
        } else if let Some(found) = on_path("clang++") {
            (found, Vec::new())
        } else if let Some(ndk) = ndk_dir(&var) {
            let host_tag = if host.contains("apple") {
                "darwin-x86_64"
            } else if host.contains("windows") {
                "windows-x86_64"
            } else {
                "linux-x86_64"
            };
            let prebuilt = ndk.join("toolchains/llvm/prebuilt").join(host_tag);
            (prebuilt.join("bin").join(exe("clang++")), vec![prebuilt.join("sysroot/usr/include/c++/v1")])
        } else {
            panic!(
                "building Luau for wasm32 needs a clang with the wasm32 backend, and none was found. \
                 Install clang and libc++ headers (Debian/Ubuntu: `apt install clang libc++-dev`; \
                 or a wasi-sdk and set WASI_SDK_PATH), or set WASM_CLANG to a clang binary"
            );
        };
        if !given.is_file() {
            panic!("{} is not a file; WASM_CLANG, WASI_SDK_PATH or the NDK named it", given.display());
        }

        let bin = given.parent().map(Path::to_path_buf).unwrap_or_default();
        let stem = given.file_name().and_then(|n| n.to_str()).unwrap_or("").trim_end_matches(".exe");
        let suffix = stem.trim_start_matches("clang++").trim_start_matches("clang").to_string();
        let sibling = |name: &str| bin.join(exe(&format!("{name}{suffix}")));
        let (clang, clangxx) = if stem.starts_with("clang++") {
            (Some(sibling("clang")).filter(|p| p.is_file()).unwrap_or_else(|| given.clone()), given.clone())
        } else {
            (given.clone(), Some(sibling("clang++")).filter(|p| p.is_file()).unwrap_or_else(|| given.clone()))
        };
        let ar = Some(sibling("llvm-ar"))
            .filter(|p| p.is_file())
            .or_else(|| first_file(&bin, &["llvm-ar", "llvm-ar-20", "llvm-ar-19", "llvm-ar-18", "llvm-ar-17", "llvm-ar-16", "llvm-ar-15"]))
            .or_else(|| on_path("llvm-ar"))
            .unwrap_or_else(|| {
                panic!("no llvm-ar beside {} or on PATH; a wasm archive needs LLVM's ar", clangxx.display())
            });

        let targets = std::process::Command::new(&clangxx).arg("--print-targets").output();
        let has_wasm = targets.map(|out| String::from_utf8_lossy(&out.stdout).contains("wasm32")).unwrap_or(false);
        if !has_wasm {
            panic!("{} has no wasm32 backend; point WASM_CLANG at an upstream, distro or wasi-sdk clang", clangxx.display());
        }

        let libcxx_include = match var("WASM_LIBCXX_INCLUDE") {
            Some(given) => env::split_paths(&given).collect(),
            None => libcxx_dirs(&clangxx, &sysroot_includes),
        };
        if libcxx_include.is_empty() {
            panic!(
                "no libc++ headers found near {}; install them (Debian/Ubuntu: `apt install libc++-dev`) \
                 or set WASM_LIBCXX_INCLUDE to the folder that holds <string> and <vector>",
                clangxx.display()
            );
        }

        Self { clang, clangxx, ar, libcxx_include }
    }
}

fn libcxx_dirs(clangxx: &Path, named: &[PathBuf]) -> Vec<PathBuf> {
    let real = fs::canonicalize(clangxx).unwrap_or_else(|_| clangxx.to_path_buf());
    let mut prefixes: Vec<PathBuf> = Vec::new();
    if let Some(bin) = real.parent() {
        if let Some(prefix) = bin.parent() {
            prefixes.push(prefix.to_path_buf());
        }
    }
    if let Ok(out) = std::process::Command::new(clangxx).arg("-print-resource-dir").output() {
        let resource = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
        if let Some(prefix) = resource.ancestors().nth(3) {
            prefixes.push(prefix.to_path_buf());
        }
    }

    let mut candidates: Vec<PathBuf> = named.to_vec();
    for prefix in &prefixes {
        candidates.push(prefix.join("include/c++/v1"));
    }
    let generic = candidates.iter().find(|dir| dir.join("string").is_file()).cloned();
    let Some(generic) = generic else { return Vec::new() };

    let mut dirs = vec![generic.clone()];
    if !generic.join("__config_site").is_file() {
        let include = generic.parent().and_then(Path::parent).map(Path::to_path_buf);
        let site = include.and_then(|include| {
            fs::read_dir(include).ok()?.filter_map(|e| e.ok()).map(|e| e.path().join("c++/v1")).find(|d| d.join("__config_site").is_file())
        });
        if let Some(site) = site {
            dirs.push(site);
        }
    }
    dirs
}

fn ndk_dir(var: &dyn Fn(&str) -> Option<PathBuf>) -> Option<PathBuf> {
    var("ANDROID_NDK_HOME").or_else(|| var("ANDROID_NDK_ROOT")).or_else(|| {
        let sdk = var("ANDROID_HOME")
            .or_else(|| var("ANDROID_SDK_ROOT"))
            .or_else(|| var("HOME").map(|home| home.join("Android/Sdk")))?;
        newest_version_dir(&sdk.join("ndk"))
    })
}

fn first_file(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    names.iter().map(|name| dir.join(exe(name))).find(|path| path.is_file())
}

fn on_path(name: &str) -> Option<PathBuf> {
    let versions = ["", "-20", "-19", "-18", "-17", "-16", "-15"];
    let paths = env::var_os("PATH")?;
    for dir in env::split_paths(&paths) {
        for suffix in versions {
            let candidate = dir.join(exe(&format!("{name}{suffix}")));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn newest_version_dir(dir: &Path) -> Option<PathBuf> {
    let version = |name: &str| -> Option<Vec<u64>> { name.split('.').map(|part| part.parse().ok()).collect() };
    fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| Some((version(entry.file_name().to_str()?)?, entry.path())))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, path)| path)
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn add_sorted(build: &mut cc::Build, dir: &Path, ext: &str) {
    let mut sources: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension() == Some(ext.as_ref()))
        .collect();
    sources.sort();
    build.files(sources);
}

fn cpp_link_stdlib(target: &str, host: &str) -> Option<String> {
    let kind = if host == target { "HOST" } else { "TARGET" };
    let from_env = env::var(format!("CXXSTDLIB_{target}"))
        .or_else(|_| env::var(format!("CXXSTDLIB_{}", target.replace('-', "_"))))
        .or_else(|_| env::var(format!("{kind}_CXXSTDLIB")))
        .or_else(|_| env::var("CXXSTDLIB"))
        .ok();
    if from_env.is_some() {
        return from_env;
    }

    if target.contains("msvc") {
        None
    } else if target.contains("apple") || target.contains("freebsd") || target.contains("openbsd") {
        Some("c++".to_string())
    } else if target.contains("android") {
        Some("c++_shared".to_string())
    } else {
        Some("stdc++".to_string())
    }
}
