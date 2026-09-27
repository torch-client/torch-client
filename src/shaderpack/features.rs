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
        ALL.into_iter().find(|f| f.name() == name)
    }

    pub(crate) fn absence(self) -> &'static str {
        match self {
            Feature::TessellationShaders => "this renderer has no tessellation stage",
            Feature::ComputeShaders | Feature::CustomImages | Feature::Ssbo => {
                if COMPUTE {
                    "not implemented yet"
                } else {
                    "the browser build uses WebGL2, which has no compute shaders"
                }
            }
            _ => "not implemented yet",
        }
    }
}

const COMPUTE: bool = !cfg!(all(target_arch = "wasm32", not(feature = "webgpu")));

pub(crate) fn supported(feature: Feature) -> bool {
    match feature {
        Feature::TessellationShaders => false,

        Feature::ComputeShaders | Feature::CustomImages | Feature::Ssbo => false,

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

pub(crate) fn mc_version() -> u32 {
    let mut parts = crate::ASSET_VERSION
        .split('.')
        .map(|p| p.parse().unwrap_or(0));
    let major: u32 = parts.next().unwrap_or(0);
    let minor: u32 = parts.next().unwrap_or(0);
    let patch: u32 = parts.next().unwrap_or(0);
    major * 10000 + minor * 100 + patch
}

pub(crate) fn base_defines() -> Defines {
    let mut defines = Defines::new();
    defines.define("MC_VERSION", mc_version().to_string());
    for feature in ALL {
        if supported(feature) {
            defines.define(format!("IRIS_FEATURE_{}", feature.name()), "");
        }
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
        let encode = |v: &str| {
            let mut p = v.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
            p.next().unwrap_or(0) * 10000 + p.next().unwrap_or(0) * 100 + p.next().unwrap_or(0)
        };
        assert_eq!(encode("1.21.4"), 12104);
        assert_eq!(encode("1.21"), 12100);
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
        for feature in ALL {
            let name = format!("IRIS_FEATURE_{}", feature.name());
            assert_eq!(defines.is_defined(&name), supported(feature), "{name}");
        }
    }
}
