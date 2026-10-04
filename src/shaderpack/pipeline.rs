use super::transform::declarations::{PackUniform, std140_size};
use super::transform::{Interface, Sampler};

const ALIGNMENT: u32 = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UniformMember {
    pub(crate) name: String,
    pub(crate) ty: String,
    pub(crate) offset: u32,
    pub(crate) size: u32,
    pub(crate) initial: Option<Vec<u32>>,
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
        ty: String,
        name: String,
    },
    Sampler {
        binding: u32,
        name: String,
    },
    Image {
        binding: u32,
        ty: String,
        name: String,
        vertex: bool,
        access: super::transform::ImageAccess,
    },
    Buffer {
        binding: u32,
        index: u8,
        read_only: bool,
        vertex: bool,
    },
}

impl Binding {
    pub(crate) fn binding(&self) -> u32 {
        match self {
            Binding::Uniform { binding, .. }
            | Binding::Texture { binding, .. }
            | Binding::Sampler { binding, .. }
            | Binding::Image { binding, .. }
            | Binding::Buffer { binding, .. } => *binding,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Format {
    Float32x2,
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
    let uniforms = uniform_layout(&interface.uniforms);
    Layout {
        bindings: bindings(interface, uniforms.size),
        attributes: interface
            .attributes
            .iter()
            .map(|location| Attribute {
                location: *location,
                format: format_of(*location),
            })
            .collect(),
        uniforms,
        targets: interface.outputs.max(1),
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Program<'a> {
    Draw {
        vertex: &'a Interface,
        fragment: &'a Interface,
    },
    Compute(&'a Interface),
}

pub(crate) fn describe_program(program: Program<'_>) -> Layout {
    let no_vertex = Interface::default();
    let (vertex, fragment) = match program {
        Program::Draw { vertex, fragment } => (vertex, fragment),
        Program::Compute(stage) => (&no_vertex, stage),
    };
    debug_assert!(
        vertex.uniforms.is_empty() || vertex.uniforms == fragment.uniforms,
        "the two stages lay the block out differently"
    );
    let mut samplers = vertex.samplers.clone();
    for sampler in &fragment.samplers {
        if !samplers.iter().any(|s| s.name == sampler.name) {
            samplers.push(sampler.clone());
        }
    }
    let mut images = vertex.images.clone();
    for image in &fragment.images {
        match images.iter_mut().find(|i| i.name == image.name) {
            Some(known) => known.access = known.access.union(image.access),
            None => images.push(image.clone()),
        }
    }
    let mut buffers = vertex.buffers.clone();
    for buffer in &fragment.buffers {
        match buffers.iter_mut().find(|b| b.index == buffer.index) {
            Some(known) => known.read_only &= buffer.read_only,
            None => buffers.push(buffer.clone()),
        }
    }
    let mut layout = describe(&Interface {
        samplers,
        attributes: vertex.attributes.clone(),
        outputs: fragment.outputs,
        uniforms: fragment.uniforms.clone(),
        images,
        buffers,
    });
    for binding in &mut layout.bindings {
        match binding {
            Binding::Image {
                name, vertex: used, ..
            } => *used = vertex.images.iter().any(|i| i.name == *name),
            Binding::Buffer {
                index,
                vertex: used,
                ..
            } => *used = vertex.buffers.iter().any(|b| b.index == *index),
            _ => {}
        }
    }
    layout
}

fn bindings(interface: &Interface, uniform_size: u32) -> Vec<Binding> {
    let mut out = vec![Binding::Uniform {
        binding: super::transform::UNIFORM_BINDING,
        size: uniform_size,
    }];
    for Sampler {
        name,
        ty,
        texture_binding,
        sampler_binding,
        ..
    } in &interface.samplers
    {
        out.push(Binding::Texture {
            binding: *texture_binding,
            ty: ty.clone(),
            name: name.clone(),
        });
        out.push(Binding::Sampler {
            binding: *sampler_binding,
            name: format!("{name}_sampler"),
        });
    }
    for image in &interface.images {
        out.push(Binding::Image {
            binding: image.binding,
            ty: image.ty.clone(),
            name: image.name.clone(),
            vertex: false,
            access: image.access,
        });
    }
    for buffer in &interface.buffers {
        out.push(Binding::Buffer {
            binding: buffer.binding,
            index: buffer.index,
            read_only: buffer.read_only,
            vertex: false,
        });
    }
    out.sort_by_key(Binding::binding);
    out
}

fn uniform_layout(pack: &[PackUniform]) -> UniformLayout {
    let mut members = Vec::new();
    let mut offset = 0;
    for (name, ty) in super::transform::UNIFORMS {
        let size = std140_size(ty).expect("this crate's own members all have a layout");
        members.push(UniformMember {
            name: (*name).to_owned(),
            ty: (*ty).to_owned(),
            offset,
            size,
            initial: None,
        });
        offset += size;
    }
    for uniform in pack {
        let Ok(size) = uniform.size() else { continue };
        members.push(UniformMember {
            name: uniform.name.clone(),
            ty: uniform.ty.clone(),
            offset,
            size,
            initial: uniform.initial.clone(),
        });
        offset += size;
    }
    debug_assert!(
        members
            .iter()
            .all(|m| m.offset % ALIGNMENT == 0 && m.size % ALIGNMENT == 0)
    );
    UniformLayout {
        members,
        size: offset,
    }
}

fn format_of(location: u32) -> Format {
    super::transform::PACK_ATTRIBUTES
        .iter()
        .find(|(_, at, _)| *at == location)
        .map_or(Format::Float32x4, |(_, _, format)| *format)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaderpack::transform::{self, Stage};

    #[test]
    fn every_uniform_is_aligned_and_offsets_are_cumulative() {
        let layout = uniform_layout(&[]);
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
        let layout = uniform_layout(&[]);
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
        let shared = transform::Shared::of(vsh, fsh);
        let target = transform::Target {
            vertex_prelude: "",
            ..transform::TEST_TARGET
        };

        let vertex =
            describe(&transform::transform(vsh, Stage::Vertex, &shared, &target).interface);
        assert!(vertex.attributes.is_empty(), "{:?}", vertex.attributes);

        let fragment =
            describe(&transform::transform(fsh, Stage::Fragment, &shared, &target).interface);
        assert_eq!(fragment.targets, 1);
        let numbers: Vec<u32> = fragment.bindings.iter().map(Binding::binding).collect();
        assert_eq!(numbers, [0, 1, 2]);
        assert!(
            matches!(fragment.bindings[1], Binding::Texture { ref ty, .. } if ty == "sampler2D")
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
