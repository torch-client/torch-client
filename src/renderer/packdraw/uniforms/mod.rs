use bevy::math::{Mat4, Vec4};
use bevy::render::render_resource::{Buffer, BufferDescriptor, BufferUsages};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bytemuck::{Pod, Zeroable};

use crate::shaderpack::expressions::{CustomType, Customs};
use crate::shaderpack::pipeline::UniformLayout;

mod builtins;
mod values;

pub(crate) use builtins::resolve_input;
use builtins::{Builtin, Value, builtin};
pub(crate) use values::{FrameValues, History};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub(crate) struct BlockHead {
    pub(crate) model_view: Mat4,
    pub(crate) projection: Mat4,
    pub(crate) model_view_projection: Mat4,
    pub(crate) camera: Vec4,
    pub(crate) screen: Vec4,
    pub(crate) time: Vec4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MatrixSet {
    Camera,
    Screen,
    Shadow,
    Hand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Builtin(Builtin),
    Custom(usize, CustomType),
}

fn source(name: &str, ty: &str, customs: &Customs) -> Option<Source> {
    customs
        .entries
        .iter()
        .position(|c| c.uniform && c.name == name && c.ty.glsl() == ty)
        .map(|i| Source::Custom(i, customs.entries[i].ty))
        .or_else(|| builtin(name, ty).map(Source::Builtin))
}

pub(crate) fn unsupplied(layout: &UniformLayout, customs: &Customs) -> Vec<String> {
    layout
        .members
        .iter()
        .filter(|m| {
            !m.name.starts_with("iris_")
                && m.initial.is_none()
                && source(&m.name, &m.ty, customs).is_none()
        })
        .map(|m| format!("{} {}", m.ty, m.name))
        .collect()
}

pub(crate) struct PackUniforms {
    pub(crate) buffer: Buffer,
    bytes: Vec<u8>,
    sources: Vec<(u32, Source)>,
    blocks: Vec<Block>,
}

struct Block {
    offset: u32,
    size: u32,
    set: MatrixSet,
}

pub(crate) struct BlockSpec<'a> {
    pub(crate) layout: &'a UniformLayout,
    pub(crate) set: MatrixSet,
    pub(crate) render_stage: i32,
}

impl PackUniforms {
    pub(crate) fn new<'a>(
        device: &RenderDevice,
        label: &str,
        specs: impl IntoIterator<Item = BlockSpec<'a>>,
        customs: &Customs,
    ) -> PackUniforms {
        let align = device.limits().min_uniform_buffer_offset_alignment.max(1);
        let mut blocks = Vec::new();
        let mut sources = Vec::new();
        let mut stages = Vec::new();
        let mut initials = Vec::new();
        let mut end = 0u32;
        for spec in specs {
            let offset = end.next_multiple_of(align);
            end = offset + spec.layout.size;
            for member in &spec.layout.members {
                match source(&member.name, &member.ty, customs) {
                    Some(Source::Builtin(Builtin::RenderStage)) => {
                        stages.push((offset + member.offset, spec.render_stage))
                    }
                    Some(source) => sources.push((offset + member.offset, source)),
                    None => initials.extend(
                        member
                            .initial
                            .iter()
                            .map(|words| (offset + member.offset, words)),
                    ),
                }
            }
            blocks.push(Block {
                offset,
                size: spec.layout.size,
                set: spec.set,
            });
        }
        let mut bytes = vec![0; end.max(4) as usize];
        for (at, stage) in stages {
            Value::Int(stage).write(&mut bytes[at as usize..]);
        }
        for (at, words) in initials {
            let words = bytemuck::cast_slice::<u32, u8>(words);
            bytes[at as usize..at as usize + words.len()].copy_from_slice(words);
        }
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some(label),
            size: bytes.len() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        PackUniforms {
            buffer,
            bytes,
            sources,
            blocks,
        }
    }

    pub(crate) fn range(&self, index: usize) -> (u64, u64) {
        let block = &self.blocks[index];
        (u64::from(block.offset), u64::from(block.size))
    }

    pub(crate) fn write(&mut self, values: &FrameValues, customs: &[f32], queue: &RenderQueue) {
        for block in &self.blocks {
            let head = values.head(block.set);
            let head = bytemuck::bytes_of(&head);
            let at = block.offset as usize;
            self.bytes[at..at + head.len()].copy_from_slice(head);
        }
        for (offset, source) in &self.sources {
            let value = match *source {
                Source::Builtin(b) => values.get(b),
                Source::Custom(i, CustomType::Float) => Value::Float(customs[i]),
                Source::Custom(i, CustomType::Int | CustomType::Bool) => {
                    Value::Int(customs[i] as i32)
                }
            };
            value.write(&mut self.bytes[*offset as usize..]);
        }
        queue.write_buffer(&self.buffer, 0, &self.bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaderpack::transform::UNIFORMS;

    #[test]
    fn the_block_head_matches_the_glsl_it_mirrors() {
        let layout = crate::shaderpack::pipeline::describe(&Default::default()).uniforms;
        assert_eq!(layout.members.len(), UNIFORMS.len());
        let names: Vec<&str> = layout.members.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "iris_ModelViewMatrix",
                "iris_ProjectionMatrix",
                "iris_ModelViewProjectionMatrix",
                "iris_CameraPosition",
                "iris_ScreenSize",
                "iris_Time",
            ]
        );
        assert_eq!(std::mem::size_of::<BlockHead>() as u32, layout.size);
    }
}
