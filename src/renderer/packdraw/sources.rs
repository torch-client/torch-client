use bevy::render::render_resource::TextureViewDimension;

use crate::shaderpack::directives;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Depth {
    All,
    Opaque,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Stub {
    White2d,
    Zero3d,
    FlatNormal,
    Black2d,
    Uint3d,
    Uint2d,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TextureSource {
    Color(u8),
    Depth(Depth),
    DepthNoHand,
    Noise,
    Atlas,
    Lightmap,
    Stub(Stub),
    Custom(u8),
    ShadowDepth(Depth),
    ShadowColor(u8),
    EntityTexture,
    Image {
        index: u8,
        dimension: TextureViewDimension,
    },
}

impl TextureSource {
    pub(crate) fn resolve(name: &str, ty: &str) -> Result<TextureSource, String> {
        let source = match name {
            "depthtex0" | "gdepthtex" => TextureSource::Depth(Depth::All),
            "depthtex1" => TextureSource::Depth(Depth::Opaque),
            "depthtex2" => TextureSource::DepthNoHand,
            "noisetex" => TextureSource::Noise,
            "gtexture" | "tex" | "texture" => TextureSource::Atlas,
            "lightmap" => TextureSource::Lightmap,
            "normals" => TextureSource::Stub(Stub::FlatNormal),
            "specular" => TextureSource::Stub(Stub::Black2d),
            "shadowtex0" | "shadow" | "watershadow" => TextureSource::ShadowDepth(Depth::All),
            "shadowtex1" => TextureSource::ShadowDepth(Depth::Opaque),
            name if directives::shadow_index(name).is_some() => {
                TextureSource::ShadowColor(directives::shadow_index(name).expect("matched") as u8)
            }
            _ => match directives::target_index(name) {
                Some(index) => TextureSource::Color(index as u8),
                None => match ty {
                    "sampler2D" => TextureSource::Stub(Stub::White2d),
                    "sampler3D" => TextureSource::Stub(Stub::Zero3d),
                    "usampler3D" => TextureSource::Stub(Stub::Uint3d),
                    "usampler2D" => TextureSource::Stub(Stub::Uint2d),
                    _ => return Err(format!("{ty} {name} is not supplied yet")),
                },
            },
        };
        let expects_2d = !source.is_3d();
        let is_2d = matches!(ty, "sampler2D" | "usampler2D" | "isampler2D");
        if expects_2d != is_2d {
            return Err(format!(
                "{ty} {name}: this client supplies it as a different kind of texture"
            ));
        }
        Ok(source)
    }

    pub(crate) fn image(
        index: u8,
        dimension: crate::shaderpack::customimages::Dimension,
        name: &str,
        ty: &str,
    ) -> Result<TextureSource, String> {
        use crate::shaderpack::customimages::Dimension;
        let shape = crate::shaderpack::transform::untyped(ty);
        let matches = match dimension {
            Dimension::D1 => shape == "sampler1D",
            Dimension::D2 => shape == "sampler2D",
            Dimension::D3 => shape == "sampler3D",
        };
        if matches {
            let dimension = match dimension {
                Dimension::D1 => TextureViewDimension::D1,
                Dimension::D2 => TextureViewDimension::D2,
                Dimension::D3 => TextureViewDimension::D3,
            };
            Ok(TextureSource::Image { index, dimension })
        } else {
            Err(format!("{ty} {name}: the image it reads is {dimension:?}"))
        }
    }

    pub(crate) fn entity(name: &str, ty: &str) -> Result<TextureSource, String> {
        if ty == "sampler2D" {
            Ok(TextureSource::EntityTexture)
        } else {
            Err(format!("{ty} {name}: an entity's texture is 2D"))
        }
    }

    pub(crate) fn custom(index: u8, name: &str, ty: &str) -> Result<TextureSource, String> {
        if ty == "sampler2D" {
            Ok(TextureSource::Custom(index))
        } else {
            Err(format!("{ty} {name}: the pack supplies a 2D image for it"))
        }
    }

    pub(crate) fn sample_kind(self, targets: &directives::Targets) -> directives::SampleKind {
        use directives::SampleKind;
        match self {
            TextureSource::Color(index) => targets.settings[index as usize].format.sample_kind(),
            TextureSource::ShadowColor(index) => {
                targets.shadow[index as usize].format.sample_kind()
            }
            TextureSource::Depth(_)
            | TextureSource::DepthNoHand
            | TextureSource::ShadowDepth(_) => SampleKind::UnfilterableFloat,
            TextureSource::Stub(Stub::Uint3d | Stub::Uint2d) => SampleKind::Uint,
            TextureSource::Image { .. } => SampleKind::UnfilterableFloat,
            TextureSource::Noise
            | TextureSource::Atlas
            | TextureSource::Lightmap
            | TextureSource::Custom(_)
            | TextureSource::EntityTexture
            | TextureSource::Stub(
                Stub::White2d | Stub::Zero3d | Stub::FlatNormal | Stub::Black2d,
            ) => SampleKind::Float,
        }
    }

    pub(crate) fn view_dimension(self) -> TextureViewDimension {
        match self {
            TextureSource::Image { dimension, .. } => dimension,
            TextureSource::Stub(Stub::Zero3d | Stub::Uint3d) => TextureViewDimension::D3,
            _ => TextureViewDimension::D2,
        }
    }

    pub(crate) fn is_3d(self) -> bool {
        self.view_dimension() == TextureViewDimension::D3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_resolve_to_their_sources() {
        assert_eq!(
            TextureSource::resolve("colortex4", "sampler2D"),
            Ok(TextureSource::Color(4))
        );
        assert_eq!(
            TextureSource::resolve("gaux1", "sampler2D"),
            Ok(TextureSource::Color(4))
        );
        assert_eq!(
            TextureSource::resolve("depthtex1", "sampler2D"),
            Ok(TextureSource::Depth(Depth::Opaque))
        );
        assert_eq!(
            TextureSource::resolve("gtexture", "sampler2D"),
            Ok(TextureSource::Atlas)
        );
        assert_eq!(
            TextureSource::resolve("shadowtex0", "sampler2D"),
            Ok(TextureSource::ShadowDepth(Depth::All))
        );
        assert_eq!(
            TextureSource::resolve("shadowcolor1", "sampler2D"),
            Ok(TextureSource::ShadowColor(1))
        );
        assert_eq!(
            TextureSource::resolve("voxelSampler", "usampler3D"),
            Ok(TextureSource::Stub(Stub::Uint3d))
        );
        assert!(TextureSource::resolve("colortex4", "sampler3D").is_err());
        assert!(TextureSource::resolve("shadow", "sampler2DShadow").is_err());
    }
}
