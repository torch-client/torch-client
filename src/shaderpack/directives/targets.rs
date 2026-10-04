pub(crate) const MAX_COLOR_TARGETS: usize = 16;

pub(crate) const MAX_SHADOW_COLORS: usize = 8;

const LEGACY_TARGETS: [&str; 8] = [
    "gcolor",
    "gdepth",
    "gnormal",
    "composite",
    "gaux1",
    "gaux2",
    "gaux3",
    "gaux4",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SampleKind {
    Float,
    UnfilterableFloat,
    Uint,
    Sint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TargetFormat {
    R8Unorm,
    Rg8Unorm,
    Rgba8Unorm,
    R8Snorm,
    Rg8Snorm,
    Rgba8Snorm,
    R16Float,
    Rg16Float,
    Rgba16Float,
    R16Unorm,
    Rg16Unorm,
    Rgba16Unorm,
    R16Snorm,
    Rg16Snorm,
    Rgba16Snorm,
    Rg11b10Ufloat,
    R32Float,
    Rg32Float,
    Rgba32Float,
    Rgb10a2Unorm,
    R8Uint,
    Rg8Uint,
    Rgba8Uint,
    R8Sint,
    Rg8Sint,
    Rgba8Sint,
    R16Uint,
    Rg16Uint,
    Rgba16Uint,
    R16Sint,
    Rg16Sint,
    Rgba16Sint,
    R32Uint,
    Rg32Uint,
    Rgba32Uint,
    R32Sint,
    Rg32Sint,
    Rgba32Sint,
    Rgb10a2Uint,
}

impl TargetFormat {
    pub(crate) fn parse(name: &str) -> Option<TargetFormat> {
        use TargetFormat as F;
        Some(match name {
            "RGBA" | "RGB8" | "RGBA8" | "RGBA2" | "RGBA4" | "RGB5_A1" | "RGB565" | "R3_G3_B2" => {
                F::Rgba8Unorm
            }
            "R8" => F::R8Unorm,
            "RG8" => F::Rg8Unorm,
            "R8_SNORM" => F::R8Snorm,
            "RG8_SNORM" => F::Rg8Snorm,
            "RGB8_SNORM" | "RGBA8_SNORM" => F::Rgba8Snorm,
            "R16F" => F::R16Float,
            "RG16F" => F::Rg16Float,
            "RGB16F" | "RGBA16F" | "RGB9_E5" => F::Rgba16Float,
            "R16" => F::R16Unorm,
            "RG16" => F::Rg16Unorm,
            "RGB16" | "RGBA16" => F::Rgba16Unorm,
            "R16_SNORM" => F::R16Snorm,
            "RG16_SNORM" => F::Rg16Snorm,
            "RGB16_SNORM" | "RGBA16_SNORM" => F::Rgba16Snorm,
            "R11F_G11F_B10F" => F::Rg11b10Ufloat,
            "R32F" => F::R32Float,
            "RG32F" => F::Rg32Float,
            "RGB32F" | "RGBA32F" => F::Rgba32Float,
            "RGB10_A2" => F::Rgb10a2Unorm,
            "RGB10_A2UI" => F::Rgb10a2Uint,
            "R8UI" => F::R8Uint,
            "RG8UI" => F::Rg8Uint,
            "RGB8UI" | "RGBA8UI" => F::Rgba8Uint,
            "R8I" => F::R8Sint,
            "RG8I" => F::Rg8Sint,
            "RGB8I" | "RGBA8I" => F::Rgba8Sint,
            "R16UI" => F::R16Uint,
            "RG16UI" => F::Rg16Uint,
            "RGB16UI" | "RGBA16UI" => F::Rgba16Uint,
            "R16I" => F::R16Sint,
            "RG16I" => F::Rg16Sint,
            "RGB16I" | "RGBA16I" => F::Rgba16Sint,
            "R32UI" => F::R32Uint,
            "RG32UI" => F::Rg32Uint,
            "RGB32UI" | "RGBA32UI" => F::Rgba32Uint,
            "R32I" => F::R32Sint,
            "RG32I" => F::Rg32Sint,
            "RGB32I" | "RGBA32I" => F::Rgba32Sint,
            _ => return None,
        })
    }

    pub(crate) fn sample_kind(self) -> SampleKind {
        use TargetFormat as F;
        match self {
            F::R32Float | F::Rg32Float | F::Rgba32Float => SampleKind::UnfilterableFloat,
            F::R8Uint
            | F::Rg8Uint
            | F::Rgba8Uint
            | F::R16Uint
            | F::Rg16Uint
            | F::Rgba16Uint
            | F::R32Uint
            | F::Rg32Uint
            | F::Rgba32Uint
            | F::Rgb10a2Uint => SampleKind::Uint,
            F::R8Sint
            | F::Rg8Sint
            | F::Rgba8Sint
            | F::R16Sint
            | F::Rg16Sint
            | F::Rgba16Sint
            | F::R32Sint
            | F::Rg32Sint
            | F::Rgba32Sint => SampleKind::Sint,
            _ => SampleKind::Float,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TargetSettings {
    pub(crate) format: TargetFormat,
    pub(crate) clear: bool,
    pub(super) clear_color: Option<[f32; 4]>,
    pub(crate) size: Option<[SizeAxis; 2]>,
}

impl Default for TargetSettings {
    fn default() -> Self {
        TargetSettings {
            format: TargetFormat::Rgba8Unorm,
            clear: true,
            clear_color: None,
            size: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum SizeAxis {
    Relative(f32),
    Fixed(u32),
}

impl SizeAxis {
    fn parse(text: &str) -> Option<SizeAxis> {
        if text.contains('.') {
            text.parse::<f32>()
                .ok()
                .filter(|f| f.is_finite() && *f > 0.0)
                .map(SizeAxis::Relative)
        } else {
            text.parse::<u32>()
                .ok()
                .filter(|n| *n > 0)
                .map(SizeAxis::Fixed)
        }
    }

    fn resolve(self, screen: u32) -> u32 {
        match self {
            SizeAxis::Relative(fraction) => ((screen as f32 * fraction) as u32).max(1),
            SizeAxis::Fixed(texels) => texels,
        }
    }
}

impl TargetSettings {
    pub(crate) fn resolve_size(&self, screen: [u32; 2]) -> [u32; 2] {
        match self.size {
            Some([x, y]) => [x.resolve(screen[0]), y.resolve(screen[1])],
            None => screen,
        }
    }

    pub(crate) fn clear_color(&self, index: usize, fog: [f32; 4]) -> [f32; 4] {
        self.clear_color.unwrap_or(match index {
            0 => fog,
            1 => [1.0; 4],
            _ => [0.0; 4],
        })
    }

    pub(crate) fn shadow_clear_color(&self) -> [f32; 4] {
        self.clear_color.unwrap_or([1.0; 4])
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Targets {
    pub(crate) settings: [TargetSettings; MAX_COLOR_TARGETS],
    pub(crate) shadow: [TargetSettings; MAX_SHADOW_COLORS],
}

impl Default for Targets {
    fn default() -> Self {
        Targets {
            settings: [TargetSettings::default(); MAX_COLOR_TARGETS],
            shadow: [TargetSettings::default(); MAX_SHADOW_COLORS],
        }
    }
}

pub(crate) fn shadow_index(name: &str) -> Option<usize> {
    if name == "shadowcolor" {
        return Some(0);
    }
    let number = name.strip_prefix("shadowcolor")?;
    number
        .parse::<usize>()
        .ok()
        .filter(|n| *n < MAX_SHADOW_COLORS && number == n.to_string())
}

pub(crate) fn target_index(name: &str) -> Option<usize> {
    if let Some(number) = name.strip_prefix("colortex") {
        return number
            .parse::<usize>()
            .ok()
            .filter(|n| *n < MAX_COLOR_TARGETS && number == n.to_string());
    }
    LEGACY_TARGETS.iter().position(|legacy| *legacy == name)
}

pub(crate) fn read_sizes<'a>(
    targets: &mut Targets,
    keys: impl Iterator<Item = (&'a String, &'a String)>,
    notes: &mut Vec<String>,
) {
    for (key, value) in keys {
        let Some(name) = key.strip_prefix("size.buffer.") else {
            continue;
        };
        let Some(index) = target_index(name) else {
            notes.push(format!("{key}: no such target"));
            continue;
        };
        let axes: Vec<Option<SizeAxis>> = value.split_whitespace().map(SizeAxis::parse).collect();
        match axes[..] {
            [Some(x), Some(y)] => targets.settings[index].size = Some([x, y]),
            _ => notes.push(format!("{key}: \"{value}\" is not two sizes")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sized(pairs: &[(&str, &str)]) -> (Targets, Vec<String>) {
        let keys: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        let mut targets = Targets::default();
        let mut notes = Vec::new();
        read_sizes(&mut targets, keys.iter().map(|(k, v)| (k, v)), &mut notes);
        (targets, notes)
    }

    #[test]
    fn relative_sizes_scale_the_screen_and_truncate() {
        let (targets, notes) = sized(&[
            ("size.buffer.colortex6", "0.33 0.33"),
            ("size.buffer.colortex9", "0.1 0.1"),
        ]);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(targets.settings[6].resolve_size([1920, 1002]), [633, 330]);
        assert_eq!(targets.settings[9].resolve_size([1920, 1002]), [192, 100]);
        assert_eq!(targets.settings[0].resolve_size([1920, 1002]), [1920, 1002]);
    }

    #[test]
    fn a_value_without_a_point_is_texels_and_legacy_names_work() {
        let (targets, _) = sized(&[("size.buffer.gaux3", "256 0.5")]);
        assert_eq!(targets.settings[6].resolve_size([1000, 800]), [256, 400]);
    }

    #[test]
    fn a_tiny_fraction_keeps_one_texel() {
        let (targets, _) = sized(&[("size.buffer.colortex2", "0.0001 0.0001")]);
        assert_eq!(targets.settings[2].resolve_size([640, 480]), [1, 1]);
    }

    #[test]
    fn malformed_keys_are_noted_and_ignored() {
        let (targets, notes) = sized(&[
            ("size.buffer.colortex99", "0.5 0.5"),
            ("size.buffer.colortex3", "0.5"),
            ("size.buffer.colortex4", "half 0.5"),
            ("size.buffer.colortex5", "0 1"),
        ]);
        assert_eq!(notes.len(), 4, "{notes:?}");
        assert!(targets.settings.iter().all(|s| s.size.is_none()));
    }
}
