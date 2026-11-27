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

pub(crate) const MAX_FORMAT_LIST: usize = 4;

#[derive(Clone, Copy, Debug)]
pub(crate) struct FormatInfo {
    pub(crate) format: Format,
    pub(crate) name: &'static str,
    pub(crate) class: Class,
    pub(crate) baseline: bool,
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
    info(
        Format::Rgba16Float,
        "rgba16float",
        Class::Float,
        false,
        true,
    ),
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
            Format::Rgba16Float,
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
            Format::Rgba16Float,
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
        loads_packs: true,
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
            Format::Rgba16Float,
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
    pub(crate) format_lists: Vec<Vec<Format>>,
    pub(crate) sampled_textures: u32,
}

impl Needs {
    pub(crate) fn declared_formats(&self) -> Vec<Format> {
        let mut out: Vec<Format> = self
            .format_lists
            .iter()
            .filter_map(|list| list.last().copied())
            .filter(|f| !f.info().baseline)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }
}

impl Tier {
    pub(crate) fn renders(&self, format: Format) -> bool {
        format.info().baseline || self.formats.contains(&format)
    }

    pub(crate) fn runs(&self, needs: &Needs) -> bool {
        (!needs.compute || self.compute)
            && (!needs.storage_textures || self.storage_textures)
            && needs.outputs <= self.max_outputs
            && needs.sampled_textures <= self.max_sampled_textures
            && needs
                .format_lists
                .iter()
                .all(|list| list.iter().any(|&f| self.renders(f)))
    }
}

pub(crate) fn tiers_for(needs: &Needs) -> Vec<TierName> {
    TIERS
        .iter()
        .filter(|tier| tier.runs(needs))
        .map(|tier| tier.name)
        .collect()
}

pub(crate) fn running() -> TierName {
    if cfg!(all(target_arch = "wasm32", not(feature = "webgpu"))) {
        TierName::WebGl2
    } else if cfg!(target_arch = "wasm32") {
        TierName::WebGpu
    } else if cfg!(target_os = "android") {
        TierName::Android
    } else {
        TierName::Desktop
    }
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
        assert_eq!(baseline, ["rgba8unorm", "depth"]);
    }

    #[test]
    fn rgba16float_is_in_every_floor_but_webgl2() {
        let with: Vec<TierName> = TIERS
            .iter()
            .filter(|t| t.renders(Format::Rgba16Float))
            .map(|t| t.name)
            .collect();
        assert_eq!(
            with,
            [TierName::Desktop, TierName::WebGpu, TierName::Android]
        );
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
    fn desktop_and_webgl2_load_packs() {
        let loading: Vec<TierName> = TIERS
            .iter()
            .filter(|t| t.loads_packs)
            .map(|t| t.name)
            .collect();
        assert_eq!(loading, [TierName::Desktop, TierName::WebGl2]);
    }

    #[test]
    fn a_native_test_build_is_desktop() {
        assert_eq!(running(), TierName::Desktop);
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
            format_lists: vec![vec![Format::Rg11b10Ufloat]],
            ..Needs::default()
        };
        assert_eq!(tiers_for(&needs), [TierName::Desktop]);
        let needs = Needs {
            outputs: 9,
            ..Needs::default()
        };
        assert!(tiers_for(&needs).is_empty());
    }

    #[test]
    fn a_format_list_runs_where_one_entry_renders() {
        let hdr = Needs {
            outputs: 1,
            format_lists: vec![vec![Format::Rgba16Float]],
            ..Needs::default()
        };
        assert_eq!(
            tiers_for(&hdr),
            [TierName::Desktop, TierName::WebGpu, TierName::Android]
        );
        assert_eq!(hdr.declared_formats(), [Format::Rgba16Float]);

        let list = vec![Format::Rgba16Float, Format::Rgba8Unorm];
        let fallback = Needs {
            outputs: 1,
            format_lists: vec![list],
            ..Needs::default()
        };
        assert_eq!(
            tiers_for(&fallback),
            [
                TierName::Desktop,
                TierName::WebGpu,
                TierName::WebGl2,
                TierName::Android
            ]
        );
        assert!(fallback.declared_formats().is_empty());

        let list = vec![Format::Rgba16Float, Format::Rg11b10Ufloat];
        let both = Needs {
            outputs: 1,
            format_lists: vec![list],
            ..Needs::default()
        };
        assert_eq!(
            tiers_for(&both),
            [TierName::Desktop, TierName::WebGpu, TierName::Android]
        );
        assert_eq!(both.declared_formats(), [Format::Rg11b10Ufloat]);
    }
}
