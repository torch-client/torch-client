use std::collections::HashMap;

use super::directives::SampleKind;

const MAX_IMAGES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImageFormat {
    R8Ui,
    R32Ui,
    R32I,
    R32F,
    Rg32F,
    Rgba8,
    Rgba8Ui,
    Rgba16F,
    Rgba16Ui,
    Rgba32F,
    Rgba32Ui,
}

impl ImageFormat {
    const ALL: [ImageFormat; 11] = [
        ImageFormat::R8Ui,
        ImageFormat::R32Ui,
        ImageFormat::R32I,
        ImageFormat::R32F,
        ImageFormat::Rg32F,
        ImageFormat::Rgba8,
        ImageFormat::Rgba8Ui,
        ImageFormat::Rgba16F,
        ImageFormat::Rgba16Ui,
        ImageFormat::Rgba32F,
        ImageFormat::Rgba32Ui,
    ];

    pub(crate) fn glsl(self) -> &'static str {
        match self {
            ImageFormat::R8Ui => "r8ui",
            ImageFormat::R32Ui => "r32ui",
            ImageFormat::R32I => "r32i",
            ImageFormat::R32F => "r32f",
            ImageFormat::Rg32F => "rg32f",
            ImageFormat::Rgba8 => "rgba8",
            ImageFormat::Rgba8Ui => "rgba8ui",
            ImageFormat::Rgba16F => "rgba16f",
            ImageFormat::Rgba16Ui => "rgba16ui",
            ImageFormat::Rgba32F => "rgba32f",
            ImageFormat::Rgba32Ui => "rgba32ui",
        }
    }

    fn parse(name: &str) -> Option<ImageFormat> {
        ImageFormat::ALL
            .into_iter()
            .find(|f| f.glsl().eq_ignore_ascii_case(name))
    }

    pub(crate) fn sample_kind(self) -> SampleKind {
        match self {
            ImageFormat::R8Ui
            | ImageFormat::R32Ui
            | ImageFormat::Rgba8Ui
            | ImageFormat::Rgba16Ui
            | ImageFormat::Rgba32Ui => SampleKind::Uint,
            ImageFormat::R32I => SampleKind::Sint,
            ImageFormat::Rgba8 | ImageFormat::Rgba16F => SampleKind::Float,
            ImageFormat::R32F | ImageFormat::Rg32F | ImageFormat::Rgba32F => {
                SampleKind::UnfilterableFloat
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ImageSize {
    Fixed {
        width: u32,
        height: u32,
        depth: u32,
        dimension: Dimension,
    },
    Relative {
        width: f32,
        height: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dimension {
    D1,
    D2,
    D3,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CustomImage {
    pub(crate) name: String,
    pub(crate) sampler: Option<String>,
    pub(crate) format: ImageFormat,
    pub(crate) clear: bool,
    pub(crate) size: ImageSize,
}

impl CustomImage {
    pub(crate) fn dimension(&self) -> Dimension {
        match self.size {
            ImageSize::Fixed { dimension, .. } => dimension,
            ImageSize::Relative { .. } => Dimension::D2,
        }
    }
}

pub(crate) fn read(
    properties: &HashMap<String, String>,
    notes: &mut Vec<String>,
) -> Vec<CustomImage> {
    let mut keys: Vec<(&str, &str)> = properties
        .iter()
        .filter_map(|(key, value)| Some((key.strip_prefix("image.")?, value.as_str())))
        .collect();
    keys.sort_unstable();
    let mut out = Vec::new();
    for (name, value) in keys {
        if out.len() == MAX_IMAGES {
            notes.push(format!("image.{name}: more than {MAX_IMAGES} images"));
            continue;
        }
        match parse(name, value) {
            Ok(image) => out.push(image),
            Err(why) => notes.push(format!("image.{name}={value}: {why}")),
        }
    }
    out
}

fn parse(name: &str, value: &str) -> Result<CustomImage, String> {
    let parts: Vec<&str> = value.split_whitespace().collect();
    if parts.len() < 7 {
        return Err("too few fields".into());
    }
    let format = ImageFormat::parse(parts[2])
        .ok_or_else(|| format!("{} is not a format this client can store", parts[2]))?;
    let flag = |s: &str| s.eq_ignore_ascii_case("true");
    let number = |s: &str| {
        s.parse::<u32>()
            .ok()
            .filter(|n| (1..=16384).contains(n))
            .ok_or_else(|| format!("{s} is not a size"))
    };
    let size = if flag(parts[5]) {
        let scale = |s: &str| {
            s.parse::<f32>()
                .ok()
                .filter(|v| *v > 0.0)
                .ok_or_else(|| format!("{s} is not a scale"))
        };
        match parts.get(6..8) {
            Some([w, h]) => ImageSize::Relative {
                width: scale(w)?,
                height: scale(h)?,
            },
            _ => return Err("a relative image needs two scales".into()),
        }
    } else {
        match &parts[6..] {
            [w] => ImageSize::Fixed {
                width: number(w)?,
                height: 1,
                depth: 1,
                dimension: Dimension::D1,
            },
            [w, h] => ImageSize::Fixed {
                width: number(w)?,
                height: number(h)?,
                depth: 1,
                dimension: Dimension::D2,
            },
            [w, h, d] => ImageSize::Fixed {
                width: number(w)?,
                height: number(h)?,
                depth: number(d)?,
                dimension: Dimension::D3,
            },
            _ => return Err("an image has one, two or three sizes".into()),
        }
    };
    Ok(CustomImage {
        name: name.to_owned(),
        sampler: (parts[0] != "none").then(|| parts[0].to_owned()),
        format,
        clear: flag(parts[4]),
        size,
    })
}

const MAX_BUFFER_INDEX: u8 = 12;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BufferObject {
    pub(crate) index: u8,
    pub(crate) size: u64,
    pub(crate) relative: Option<[f32; 2]>,
}

impl BufferObject {
    pub(crate) fn bytes(&self, screen: [u32; 2]) -> u64 {
        match self.relative {
            None => self.size,
            Some([x, y]) => {
                ((screen[0] as f32 * x) as u64).max(1)
                    * ((screen[1] as f32 * y) as u64).max(1)
                    * self.size
            }
        }
    }
}

pub(crate) fn buffers(
    properties: &HashMap<String, String>,
    notes: &mut Vec<String>,
) -> Vec<BufferObject> {
    let mut out: Vec<BufferObject> = Vec::new();
    for (key, value) in properties {
        let Some(index) = key.strip_prefix("bufferObject.") else {
            continue;
        };
        let parts: Vec<&str> = value.split_whitespace().collect();
        let parsed = (|| {
            let index = index
                .parse::<u8>()
                .ok()
                .filter(|i| *i <= MAX_BUFFER_INDEX)?;
            let size = parts.first()?.parse::<u64>().ok().filter(|s| *s > 0)?;
            let relative = match parts.len() {
                0..=2 => None,
                _ if parts[1].eq_ignore_ascii_case("true") => {
                    Some([parts.get(2)?.parse().ok()?, parts.get(3)?.parse().ok()?])
                }
                _ => None,
            };
            Some(BufferObject {
                index,
                size,
                relative,
            })
        })();
        match parsed {
            Some(buffer) => out.push(buffer),
            None => notes.push(format!("{key}={value}: not a buffer this client can make")),
        }
    }
    out.sort_by_key(|b| b.index);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solas_images_are_read() {
        let props = HashMap::from([
            (
                "image.voxel_img".to_string(),
                "voxelSampler red_integer r8ui unsigned_int true false 128 64 128".to_string(),
            ),
            (
                "image.floodfill_img".to_string(),
                "floodfillSampler rgba rgba16f half_float false false 128 64 128".to_string(),
            ),
            (
                "image.screen_img".to_string(),
                "none rgba rgba8 unsigned_byte false true 0.5 0.5".to_string(),
            ),
            (
                "image.bad".to_string(),
                "x rgb rgb9_e5 float true false 4 4".to_string(),
            ),
        ]);
        let mut notes = Vec::new();
        let images = read(&props, &mut notes);
        let names: Vec<&str> = images.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["floodfill_img", "screen_img", "voxel_img"]);
        assert_eq!(notes.len(), 1, "{notes:?}");
        let voxel = &images[2];
        assert_eq!(voxel.sampler.as_deref(), Some("voxelSampler"));
        assert_eq!(voxel.format, ImageFormat::R8Ui);
        assert!(voxel.clear);
        assert_eq!(
            voxel.size,
            ImageSize::Fixed {
                width: 128,
                height: 64,
                depth: 128,
                dimension: Dimension::D3
            }
        );
        assert_eq!(images[1].sampler, None);
        assert_eq!(
            images[1].size,
            ImageSize::Relative {
                width: 0.5,
                height: 0.5
            }
        );
    }

    #[test]
    fn buffer_objects_are_read() {
        let props = HashMap::from([
            ("bufferObject.0".to_string(), "4096".to_string()),
            ("bufferObject.3".to_string(), "16 true 0.5 0.5".to_string()),
            ("bufferObject.13".to_string(), "16".to_string()),
        ]);
        let mut notes = Vec::new();
        let buffers = buffers(&props, &mut notes);
        assert_eq!(buffers.len(), 2);
        assert_eq!(notes.len(), 1);
        assert_eq!(buffers[0].bytes([1920, 1080]), 4096);
        assert_eq!(buffers[1].bytes([1920, 1080]), 960 * 540 * 16);
    }
}
