mod build;

use bevy::prelude::*;
use bevy::render::render_resource::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindingResource, Buffer, BufferBinding,
    ComputePipeline, RenderPipeline, Sampler, TextureView, TextureViewId,
};
use bevy::render::renderer::RenderDevice;

use super::compile::EntityKind;

pub(crate) use build::prepare_plan;

#[derive(Clone, Copy, Default)]
struct Flips(u16);

impl Flips {
    fn current(self, index: u8) -> usize {
        ((self.0 >> index) & 1) as usize
    }

    fn other(self, index: u8) -> usize {
        1 - self.current(index)
    }

    fn flip(&mut self, buffers: &[u8]) {
        for buffer in buffers {
            self.0 ^= 1 << buffer;
        }
    }
}

pub(super) struct Attachment {
    pub(super) target: u8,
    pub(super) view: TextureView,
}

pub(super) struct ScreenPass {
    pub(super) pipeline: RenderPipeline,
    pub(super) bind_group: BindGroup,
    pub(super) attachments: Vec<Attachment>,
    pub(super) mipmaps: Vec<(u8, usize)>,
}

pub(super) struct GeometryPass {
    pub(super) pipeline: RenderPipeline,
    pub(super) bind_group: BindGroup,
    pub(super) attachments: Vec<Attachment>,
    pub(super) stream: u32,
    pub(super) clears: u16,
}

#[derive(Clone)]
enum Owned {
    Buffer(Buffer),
    Range(Buffer, u64, u64),
    View(TextureView),
    Sampler(Sampler),
}

pub(super) struct BindTemplate {
    label: String,
    layout: BindGroupLayout,
    entries: Vec<(u32, Owned)>,
    entity_textures: Vec<u32>,
}

impl BindTemplate {
    pub(super) fn create(&self, device: &RenderDevice, texture: Option<&TextureView>) -> BindGroup {
        let entries: Vec<BindGroupEntry> = self
            .entries
            .iter()
            .map(|(binding, resource)| BindGroupEntry {
                binding: *binding,
                resource: match (resource, texture) {
                    (Owned::View(_), Some(texture)) if self.entity_textures.contains(binding) => {
                        BindingResource::TextureView(texture)
                    }
                    (Owned::View(view), _) => BindingResource::TextureView(view),
                    (Owned::Buffer(buffer), _) => buffer.as_entire_binding(),
                    (Owned::Range(buffer, offset, size), _) => {
                        BindingResource::Buffer(BufferBinding {
                            buffer,
                            offset: *offset,
                            size: std::num::NonZeroU64::new(*size),
                        })
                    }
                    (Owned::Sampler(sampler), _) => BindingResource::Sampler(sampler),
                },
            })
            .collect();
        device.create_bind_group(self.label.as_str(), &self.layout, &entries)
    }
}

pub(super) struct EntityPass {
    pub(super) opaque: RenderPipeline,
    pub(super) blend: RenderPipeline,
    pub(super) attachments: Vec<Attachment>,
    pub(super) template: BindTemplate,
    pub(super) groups: std::collections::HashMap<Option<AssetId<Image>>, BindGroup>,
}

pub(super) struct ShadowPass {
    pub(super) pipeline: RenderPipeline,
    pub(super) cutout: RenderPipeline,
    pub(super) bind_group: BindGroup,
    pub(super) attachments: Vec<Attachment>,
    pub(super) copy_depth: bool,
    pub(super) depth_source: Option<BindGroup>,
}

pub(super) struct ComputePass {
    pub(super) variants: [ComputeVariant; 2],
    pub(super) groups: [u32; 3],
}

#[derive(Clone)]
pub(super) struct ComputeVariant {
    pub(super) pipeline: ComputePipeline,
    pub(super) bind_group: BindGroup,
    pub(super) snapshots: u16,
}

pub(super) enum StepPass {
    Compute(ComputePass),
    Screen(ScreenPass),
}

pub(super) struct FinalPass {
    pub(super) bind_group: BindGroup,
    pub(super) mipmaps: Vec<(u8, usize)>,
}

pub(super) struct Clear {
    pub(super) attachments: Vec<Attachment>,
}

pub(super) struct Plan {
    pub(super) setup: Vec<ComputePass>,
    pub(super) stages: [Vec<StepPass>; 4],
    pub(super) shadow: Option<ShadowPass>,
    pub(super) shadowcomp: Vec<ComputePass>,
    pub(super) entities: [Option<EntityPass>; EntityKind::COUNT],
    pub(super) clears: Vec<Clear>,
    pub(super) full_clears: Vec<Clear>,
    pub(super) opaque: Vec<GeometryPass>,
    pub(super) translucent: Vec<GeometryPass>,
    pub(super) final_computes: Vec<ComputePass>,
    pub(super) final_pass: Option<FinalPass>,
    pub(super) blit: Option<BindGroup>,
    end: Flips,
    pub(super) copy_depth: bool,
    pub(super) chains: Vec<[Option<super::mipmaps::MipChain>; 2]>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct PlanKey {
    generation: u32,
    size: UVec2,
    atlas: TextureViewId,
    lightmap: TextureViewId,
}

impl Plan {
    pub(super) fn record_mipmaps(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        mipmaps: &[(u8, usize)],
    ) {
        for &(target, half) in mipmaps {
            if let Some(chain) = &self.chains[target as usize][half] {
                chain.record(encoder);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flips_alternate_per_written_target() {
        let mut flips = Flips::default();
        assert_eq!((flips.current(3), flips.other(3)), (0, 1));
        flips.flip(&[3]);
        assert_eq!((flips.current(3), flips.other(3)), (1, 0));
        assert_eq!(flips.current(0), 0, "an unwritten target does not move");
        flips.flip(&[3, 0]);
        assert_eq!((flips.current(3), flips.current(0)), (0, 1));
    }
}
