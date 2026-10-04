use super::preprocess::Defines;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Feature {
    SeparateHardwareSamplers,
    HigherShadowcolor,
    CustomImages,
    PerBufferBlending,
    ComputeShaders,
    TessellationShaders,
    EntityTranslucent,
    ReversedCulling,
    BlockEmissionAttribute,
    CanDisableWeather,
    Ssbo,
    FadeVariable,
    TextureFiltering,
}

pub(crate) const ALL: [Feature; 13] = [
    Feature::SeparateHardwareSamplers,
    Feature::HigherShadowcolor,
    Feature::CustomImages,
    Feature::PerBufferBlending,
    Feature::ComputeShaders,
    Feature::TessellationShaders,
    Feature::EntityTranslucent,
    Feature::ReversedCulling,
    Feature::BlockEmissionAttribute,
    Feature::CanDisableWeather,
    Feature::Ssbo,
    Feature::FadeVariable,
    Feature::TextureFiltering,
];

impl Feature {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Feature::SeparateHardwareSamplers => "SEPARATE_HARDWARE_SAMPLERS",
            Feature::HigherShadowcolor => "HIGHER_SHADOWCOLOR",
            Feature::CustomImages => "CUSTOM_IMAGES",
            Feature::PerBufferBlending => "PER_BUFFER_BLENDING",
            Feature::ComputeShaders => "COMPUTE_SHADERS",
            Feature::TessellationShaders => "TESSELLATION_SHADERS",
            Feature::EntityTranslucent => "ENTITY_TRANSLUCENT",
            Feature::ReversedCulling => "REVERSED_CULLING",
            Feature::BlockEmissionAttribute => "BLOCK_EMISSION_ATTRIBUTE",
            Feature::CanDisableWeather => "CAN_DISABLE_WEATHER",
            Feature::Ssbo => "SSBO",
            Feature::FadeVariable => "FADE_VARIABLE",
            Feature::TextureFiltering => "TEXTURE_FILTERING",
        }
    }

    pub(crate) fn parse(name: &str) -> Option<Feature> {
        if name.eq_ignore_ascii_case("TESSELATION_SHADERS") {
            return Some(Feature::TessellationShaders);
        }
        ALL.into_iter()
            .find(|f| f.name().eq_ignore_ascii_case(name))
    }

    pub(crate) fn absence(self) -> &'static str {
        match self {
            Feature::TessellationShaders => "this renderer has no tessellation stage",
            Feature::ComputeShaders | Feature::CustomImages | Feature::Ssbo => {
                "the browser build uses WebGL2, which has no compute shaders"
            }
            _ => "not implemented yet",
        }
    }
}

const COMPUTE: bool = !cfg!(all(target_arch = "wasm32", not(feature = "webgpu")));

pub(crate) fn supported(feature: Feature) -> bool {
    match feature {
        Feature::TessellationShaders => false,

        Feature::ComputeShaders | Feature::CustomImages | Feature::Ssbo => COMPUTE,

        Feature::SeparateHardwareSamplers
        | Feature::HigherShadowcolor
        | Feature::PerBufferBlending
        | Feature::EntityTranslucent
        | Feature::ReversedCulling
        | Feature::BlockEmissionAttribute
        | Feature::CanDisableWeather
        | Feature::FadeVariable
        | Feature::TextureFiltering => false,
    }
}

fn mc_version() -> u32 {
    encode_version(crate::ASSET_VERSION)
}

fn encode_version(version: &str) -> u32 {
    let mut parts = version.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);
    let patch = parts.next().unwrap_or(0);
    major * 10000 + minor * 100 + patch
}

const GL_VERSION: u32 = super::transform::TARGET_VERSION;

const MIPMAP_LEVEL: &str = "4";

const OS_DEFINE: &str = if cfg!(target_os = "windows") {
    "MC_OS_WINDOWS"
} else if cfg!(target_os = "macos") {
    "MC_OS_MAC"
} else if cfg!(target_os = "linux") {
    "MC_OS_LINUX"
} else {
    "MC_OS_UNKNOWN"
};

const DH_BLOCKS: &[&str] = &[
    "UNKNOWN",
    "LEAVES",
    "STONE",
    "WOOD",
    "METAL",
    "DIRT",
    "LAVA",
    "DEEPSLATE",
    "SNOW",
    "SAND",
    "TERRACOTTA",
    "NETHER_STONE",
    "WATER",
    "GRASS",
    "AIR",
    "ILLUMINATED",
];

pub(crate) fn base_defines() -> Defines {
    let mut defines = Defines::new();
    defines.define("MC_VERSION", mc_version().to_string());
    defines.define("MC_MIPMAP_LEVEL", MIPMAP_LEVEL);
    defines.define("MC_GL_VERSION", GL_VERSION.to_string());
    defines.define("MC_GLSL_VERSION", GL_VERSION.to_string());
    defines.define(OS_DEFINE, "");
    defines.define("IS_IRIS", "");
    defines.define("MC_RENDER_QUALITY", "1.0");
    defines.define("MC_SHADOW_QUALITY", "1.0");
    defines.define("MC_HAND_DEPTH", "0.125");
    for (id, name) in DH_BLOCKS.iter().enumerate() {
        defines.define(format!("DH_BLOCK_{name}"), id.to_string());
    }
    for (id, name) in RENDER_STAGES.iter().enumerate() {
        defines.define(format!("MC_RENDER_STAGE_{name}"), id.to_string());
    }
    defines
}

const RENDER_STAGES: [&str; 24] = [
    "NONE",
    "SKY",
    "SUNSET",
    "CUSTOM_SKY",
    "SUN",
    "MOON",
    "STARS",
    "VOID",
    "TERRAIN_SOLID",
    "TERRAIN_CUTOUT_MIPPED",
    "TERRAIN_CUTOUT",
    "ENTITIES",
    "BLOCK_ENTITIES",
    "DESTROY",
    "OUTLINE",
    "DEBUG",
    "HAND_SOLID",
    "TERRAIN_TRANSLUCENT",
    "TRIPWIRE",
    "PARTICLES",
    "CLOUDS",
    "RAIN_SNOW",
    "WORLD_BORDER",
    "HAND_TRANSLUCENT",
];

pub(crate) fn render_stage(name: &str) -> i32 {
    RENDER_STAGES
        .iter()
        .position(|n| *n == name)
        .map_or(0, |i| i as i32)
}

pub(crate) fn properties_defines() -> Defines {
    let mut defines = base_defines();
    for feature in ALL.into_iter().filter(|f| supported(*f)) {
        defines.define(format!("IRIS_FEATURE_{}", feature.name()), "");
    }
    defines
}

pub(crate) fn program_defines<'a>(optional: impl Iterator<Item = &'a str>) -> Defines {
    let mut defines = base_defines();
    for feature in optional
        .filter_map(Feature::parse)
        .filter(|f| supported(*f))
    {
        defines.define(format!("IRIS_FEATURE_{}", feature.name()), "");
    }
    defines
}

pub(crate) fn unmet<'a>(required: impl Iterator<Item = &'a str>) -> Vec<String> {
    required
        .filter_map(|name| match Feature::parse(name) {
            Some(feature) if supported(feature) => None,
            Some(feature) => Some(format!("{name} ({})", feature.absence())),
            None => Some(format!("{name} (unknown to this client)")),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_is_encoded_the_way_packs_compare_it() {
        assert_eq!(encode_version("1.21.4"), 12104);
        assert_eq!(encode_version("1.21"), 12100);
        assert!(mc_version() > 0);
    }

    #[test]
    fn every_flag_round_trips_through_its_name() {
        for feature in ALL {
            assert_eq!(Feature::parse(feature.name()), Some(feature));
        }
        assert_eq!(Feature::parse("NOT_A_FLAG"), None);
    }

    #[test]
    fn unmet_names_what_is_missing_and_why() {
        let missing = unmet(["TESSELLATION_SHADERS", "NOT_A_FLAG"].into_iter());
        assert_eq!(missing.len(), 2);
        assert!(missing[0].contains("no tessellation stage"), "{missing:?}");
        assert!(missing[1].contains("unknown"), "{missing:?}");
    }

    #[test]
    fn base_defines_carry_the_version_and_only_supported_flags() {
        let defines = base_defines();
        assert_eq!(
            defines.get("MC_VERSION"),
            Some(mc_version().to_string().as_str())
        );
        let properties = properties_defines();
        let program = program_defines(["CUSTOM_IMAGES", "ssbo"].into_iter());
        for feature in ALL {
            let name = format!("IRIS_FEATURE_{}", feature.name());
            assert!(!defines.is_defined(&name), "{name}");
            assert_eq!(properties.is_defined(&name), supported(feature), "{name}");
            let listed = matches!(feature, Feature::CustomImages | Feature::Ssbo);
            assert_eq!(
                program.is_defined(&name),
                listed && supported(feature),
                "{name}"
            );
        }
    }

    #[test]
    fn a_flag_name_is_read_in_any_case_and_under_the_old_spelling() {
        assert_eq!(Feature::parse("custom_images"), Some(Feature::CustomImages));
        assert_eq!(
            Feature::parse("TESSELATION_SHADERS"),
            Some(Feature::TessellationShaders)
        );
        assert_eq!(Feature::parse("NOT_A_FLAG"), None);
    }
}
