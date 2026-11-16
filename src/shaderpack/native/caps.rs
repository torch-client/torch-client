#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Format {
    R8Unorm,
    Rg8Unorm,
    Rgba8Unorm,
    Rgb10a2Unorm,
    Rg11b10Ufloat,
    R16Float,
    Rg16Float,
    Rgba16Float,
    R32Float,
    Rg32Float,
    Rgba32Float,
    R32Uint,
    Rg32Uint,
    Rgba32Uint,
    Depth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    Float,
    Uint,
    Depth,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FormatInfo {
    pub(crate) format: Format,
    pub(crate) name: &'static str,
    pub(crate) class: Class,
    pub(crate) baseline: bool,
    #[allow(
        dead_code,
        reason = "phase 2 of docs/SHADER_FORMAT.md binds targets, and picks their sampler by this"
    )]
    pub(crate) filterable: bool,
}

pub(crate) const FORMATS: &[FormatInfo] = &[
    info(Format::R8Unorm, "r8unorm", Class::Float, false, true),
    info(Format::Rg8Unorm, "rg8unorm", Class::Float, false, true),
    info(Format::Rgba8Unorm, "rgba8unorm", Class::Float, true, true),
    info(
        Format::Rgb10a2Unorm,
        "rgb10a2unorm",
        Class::Float,
        false,
        true,
    ),
    info(
        Format::Rg11b10Ufloat,
        "rg11b10ufloat",
        Class::Float,
        false,
        true,
    ),
    info(Format::R16Float, "r16float", Class::Float, false, true),
    info(Format::Rg16Float, "rg16float", Class::Float, false, true),
    info(Format::Rgba16Float, "rgba16float", Class::Float, true, true),
    info(Format::R32Float, "r32float", Class::Float, false, false),
    info(Format::Rg32Float, "rg32float", Class::Float, false, false),
    info(
        Format::Rgba32Float,
        "rgba32float",
        Class::Float,
        false,
        false,
    ),
    info(Format::R32Uint, "r32uint", Class::Uint, false, false),
    info(Format::Rg32Uint, "rg32uint", Class::Uint, false, false),
    info(Format::Rgba32Uint, "rgba32uint", Class::Uint, false, false),
    info(Format::Depth, "depth", Class::Depth, true, false),
];

const fn info(
    format: Format,
    name: &'static str,
    class: Class,
    baseline: bool,
    filterable: bool,
) -> FormatInfo {
    FormatInfo {
        format,
        name,
        class,
        baseline,
        filterable,
    }
}

impl Format {
    pub(crate) fn parse(name: &str) -> Option<Format> {
        FORMATS.iter().find(|f| f.name == name).map(|f| f.format)
    }

    pub(crate) fn info(self) -> &'static FormatInfo {
        FORMATS
            .iter()
            .find(|f| f.format == self)
            .expect("every format has a row in FORMATS")
    }

    pub(crate) fn name(self) -> &'static str {
        self.info().name
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum TierName {
    Desktop,
    WebGpu,
    WebGl2,
    Android,
}

impl TierName {
    pub(crate) fn name(self) -> &'static str {
        match self {
            TierName::Desktop => "desktop",
            TierName::WebGpu => "webgpu",
            TierName::WebGl2 => "webgl2",
            TierName::Android => "android",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Tier {
    pub(crate) name: TierName,
    pub(crate) loads_packs: bool,
    pub(crate) compute: bool,
    pub(crate) storage_textures: bool,
    pub(crate) max_outputs: u32,
    pub(crate) max_sampled_textures: u32,
    pub(crate) formats: &'static [Format],
}

pub(crate) const TIERS: &[Tier] = &[
    Tier {
        name: TierName::Desktop,
        loads_packs: true,
        compute: true,
        storage_textures: true,
        max_outputs: 8,
        max_sampled_textures: 16,
        formats: &[
            Format::R8Unorm,
            Format::Rg8Unorm,
            Format::Rgb10a2Unorm,
            Format::Rg11b10Ufloat,
            Format::R16Float,
            Format::Rg16Float,
            Format::R32Float,
            Format::Rg32Float,
            Format::Rgba32Float,
            Format::R32Uint,
            Format::Rg32Uint,
            Format::Rgba32Uint,
        ],
    },
    Tier {
        name: TierName::WebGpu,
        loads_packs: false,
        compute: true,
        storage_textures: true,
        max_outputs: 8,
        max_sampled_textures: 16,
        formats: &[
            Format::R8Unorm,
            Format::Rg8Unorm,
            Format::Rgb10a2Unorm,
            Format::R16Float,
            Format::Rg16Float,
            Format::R32Float,
            Format::Rg32Float,
            Format::Rgba32Float,
            Format::R32Uint,
            Format::Rg32Uint,
            Format::Rgba32Uint,
        ],
    },
    Tier {
        name: TierName::WebGl2,
        loads_packs: false,
        compute: false,
        storage_textures: false,
        max_outputs: 4,
        max_sampled_textures: 16,
        formats: &[
            Format::R8Unorm,
            Format::Rg8Unorm,
            Format::Rgb10a2Unorm,
            Format::R32Uint,
            Format::Rg32Uint,
            Format::Rgba32Uint,
        ],
    },
    Tier {
        name: TierName::Android,
        loads_packs: false,
        compute: true,
        storage_textures: true,
        max_outputs: 4,
        max_sampled_textures: 16,
        formats: &[
            Format::R8Unorm,
            Format::Rg8Unorm,
            Format::Rgb10a2Unorm,
            Format::R16Float,
            Format::Rg16Float,
            Format::R32Float,
            Format::Rg32Float,
            Format::Rgba32Float,
            Format::R32Uint,
            Format::Rg32Uint,
            Format::Rgba32Uint,
        ],
    },
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Needs {
    pub(crate) compute: bool,
    pub(crate) storage_textures: bool,
    pub(crate) outputs: u32,
    pub(crate) formats: Vec<Format>,
    pub(crate) sampled_textures: u32,
}

impl Tier {
    pub(crate) fn runs(&self, needs: &Needs) -> bool {
        (!needs.compute || self.compute)
            && (!needs.storage_textures || self.storage_textures)
            && needs.outputs <= self.max_outputs
            && needs.sampled_textures <= self.max_sampled_textures
            && needs
                .formats
                .iter()
                .all(|f| f.info().baseline || self.formats.contains(f))
    }
}

pub(crate) fn tiers_for(needs: &Needs) -> Vec<TierName> {
    TIERS
        .iter()
        .filter(|tier| tier.runs(needs))
        .map(|tier| tier.name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_format_has_one_row_and_names_are_distinct() {
        for row in FORMATS {
            assert_eq!(FORMATS.iter().filter(|r| r.format == row.format).count(), 1);
            assert_eq!(FORMATS.iter().filter(|r| r.name == row.name).count(), 1);
            assert_eq!(Format::parse(row.name), Some(row.format));
        }
        assert_eq!(Format::parse("rgba8"), None);
    }

    #[test]
    fn the_baseline_is_what_the_spec_names() {
        let baseline: Vec<&str> = FORMATS
            .iter()
            .filter(|f| f.baseline)
            .map(|f| f.name)
            .collect();
        assert_eq!(baseline, ["rgba8unorm", "rgba16float", "depth"]);
    }

    #[test]
    fn tier_formats_are_never_baseline() {
        for tier in TIERS {
            assert!(
                tier.formats.iter().all(|f| !f.info().baseline),
                "{:?}",
                tier.name
            );
        }
    }

    #[test]
    fn compute_rules_out_webgl2() {
        let needs = Needs {
            compute: true,
            outputs: 1,
            ..Needs::default()
        };
        assert_eq!(
            tiers_for(&needs),
            [TierName::Desktop, TierName::WebGpu, TierName::Android]
        );
    }

    #[test]
    fn outputs_and_formats_narrow_the_list() {
        let needs = Needs {
            outputs: 6,
            ..Needs::default()
        };
        assert_eq!(tiers_for(&needs), [TierName::Desktop, TierName::WebGpu]);
        let needs = Needs {
            outputs: 1,
            formats: vec![Format::Rg11b10Ufloat],
            ..Needs::default()
        };
        assert_eq!(tiers_for(&needs), [TierName::Desktop]);
        let needs = Needs {
            outputs: 9,
            ..Needs::default()
        };
        assert!(tiers_for(&needs).is_empty());
    }
}
