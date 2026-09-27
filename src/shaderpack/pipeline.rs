use super::transform::{Interface, Sampler};

const ALIGNMENT: u32 = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UniformMember {
    pub(crate) name: &'static str,
    pub(crate) offset: u32,
    pub(crate) size: u32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct UniformLayout {
    pub(crate) members: Vec<UniformMember>,
    pub(crate) size: u32,
}

impl UniformLayout {
    pub(crate) fn offset(&self, name: &str) -> Option<u32> {
        self.members
            .iter()
            .find(|m| m.name == name)
            .map(|m| m.offset)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Binding {
    Uniform {
        binding: u32,
        size: u32,
    },
    Texture {
        binding: u32,
        dimension: String,
        name: String,
    },
    Sampler {
        binding: u32,
        name: String,
    },
}

impl Binding {
    pub(crate) fn binding(&self) -> u32 {
        match self {
            Binding::Uniform { binding, .. }
            | Binding::Texture { binding, .. }
            | Binding::Sampler { binding, .. } => *binding,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Format {
    Float32x2,
    Float32x3,
    Float32x4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Attribute {
    pub(crate) location: u32,
    pub(crate) format: Format,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Layout {
    pub(crate) bindings: Vec<Binding>,
    pub(crate) attributes: Vec<Attribute>,
    pub(crate) uniforms: UniformLayout,
    pub(crate) targets: u32,
}

pub(crate) fn describe(interface: &Interface) -> Layout {
    Layout {
        bindings: bindings(interface),
        attributes: interface
            .attributes
            .iter()
            .map(|location| Attribute {
                location: *location,
                format: format_of(*location),
            })
            .collect(),
        uniforms: uniform_layout(),
        targets: interface.outputs.max(1),
    }
}

fn bindings(interface: &Interface) -> Vec<Binding> {
    let layout = uniform_layout();
    let mut out = vec![Binding::Uniform {
        binding: super::transform::UNIFORM_BINDING,
        size: layout.size,
    }];
    for Sampler {
        name,
        dimension,
        texture_binding,
        sampler_binding,
    } in &interface.samplers
    {
        out.push(Binding::Texture {
            binding: *texture_binding,
            dimension: dimension.clone(),
            name: name.clone(),
        });
        out.push(Binding::Sampler {
            binding: *sampler_binding,
            name: format!("{name}_sampler"),
        });
    }
    out.sort_by_key(Binding::binding);
    out
}

fn uniform_layout() -> UniformLayout {
    let mut members = Vec::new();
    let mut offset = 0;
    for (name, ty) in super::transform::UNIFORMS {
        let size = size_of_type(ty);
        members.push(UniformMember { name, offset, size });
        offset += size;
    }
    UniformLayout {
        members,
        size: offset,
    }
}

fn size_of_type(ty: &str) -> u32 {
    let raw: u32 = match ty {
        "mat4" => 64,
        "mat3" => 48,
        "vec4" | "ivec4" => 16,
        "vec3" | "ivec3" => 12,
        "vec2" | "ivec2" => 8,
        _ => 4,
    };
    raw.div_ceil(ALIGNMENT) * ALIGNMENT
}

fn format_of(location: u32) -> Format {
    match location {
        0 | 4 | 5 | 7 | 8 => Format::Float32x3,
        1 | 2 | 6 => Format::Float32x2,
        _ => Format::Float32x4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaderpack::transform::{self, Stage};

    #[test]
    fn every_uniform_is_aligned_and_offsets_are_cumulative() {
        let layout = uniform_layout();
        assert!(!layout.members.is_empty());
        let mut expected = 0;
        for member in &layout.members {
            assert_eq!(member.offset, expected, "{}", member.name);
            assert_eq!(member.offset % ALIGNMENT, 0, "{}", member.name);
            assert_eq!(member.size % ALIGNMENT, 0, "{}", member.name);
            expected += member.size;
        }
        assert_eq!(layout.size, expected);
        assert!(layout.size >= 128, "{}", layout.size);
    }

    #[test]
    fn a_uniform_resolves_to_one_offset() {
        let layout = uniform_layout();
        assert_eq!(layout.offset("iris_ModelViewMatrix"), Some(0));
        assert_eq!(layout.offset("iris_ProjectionMatrix"), Some(64));
        assert_eq!(layout.offset("nothing_of_the_sort"), None);
    }

    #[test]
    fn the_minimal_pack_describes_a_buildable_pipeline() {
        let vsh = "#version 130\nattribute vec3 mc_Entity;\nvarying vec2 texcoord;\nvarying vec4 tint;\n\
                   void main() { gl_Position = ftransform(); texcoord = (gl_TextureMatrix[0] * gl_MultiTexCoord0).xy; tint = gl_Color; }\n";
        let fsh = "#version 130\nuniform sampler2D gtexture;\nvarying vec2 texcoord;\nvarying vec4 tint;\n\
                   void main() { gl_FragData[0] = texture2D(gtexture, texcoord) * tint; }\n";
        let shared = transform::varyings(vsh);
        let set = 3;

        let vertex = describe(&transform::transform(vsh, Stage::Vertex, &shared, set).interface);
        assert_eq!(
            vertex.attributes,
            [
                Attribute {
                    location: 0,
                    format: Format::Float32x3
                },
                Attribute {
                    location: 1,
                    format: Format::Float32x2
                },
                Attribute {
                    location: 3,
                    format: Format::Float32x4
                },
            ]
        );

        let fragment =
            describe(&transform::transform(fsh, Stage::Fragment, &shared, set).interface);
        assert_eq!(fragment.targets, 1);
        let numbers: Vec<u32> = fragment.bindings.iter().map(Binding::binding).collect();
        assert_eq!(numbers, [0, 1, 2]);
        assert!(
            matches!(fragment.bindings[1], Binding::Texture { ref dimension, .. } if dimension == "2D")
        );
        assert!(
            matches!(fragment.bindings[2], Binding::Sampler { ref name, .. } if name == "gtexture_sampler")
        );
    }

    #[test]
    fn a_pass_always_has_at_least_one_target() {
        assert_eq!(describe(&Interface::default()).targets, 1);
    }
}
