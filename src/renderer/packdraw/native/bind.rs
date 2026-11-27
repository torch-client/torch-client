use std::num::NonZeroU64;

use bevy::render::render_resource::{
    BindGroupLayout, BindGroupLayoutEntry, BindingType, BufferBindingType, SamplerBindingType,
    ShaderStages, TextureSampleType, TextureView, TextureViewDimension,
};
use bevy::render::renderer::RenderDevice;

use super::super::plan::{BindTemplate, Owned};
use super::super::targets::Target;
use super::render::Install;
use super::walk::Walk;
use crate::shaderpack::native::api::{
    self, Binding, BindingKind, Resource as Global, SampleKind, ViewDimension,
};
use crate::shaderpack::native::build::{Built, BuiltPass};

pub(super) fn pass_layout(
    device: &RenderDevice,
    built: &Built,
    pass: &BuiltPass,
) -> Result<BindGroupLayout, String> {
    let mut entries = Vec::with_capacity(pass.bindings.len());
    for binding in &pass.bindings {
        entries.push(BindGroupLayoutEntry {
            binding: binding.binding,
            visibility: ShaderStages::VERTEX_FRAGMENT,
            ty: binding_type(built, binding).ok_or_else(|| {
                format!(
                    "pass `{}` uses `{}`, which this build does not bind yet",
                    pass.id, binding.name
                )
            })?,
            count: None,
        });
    }
    Ok(device.create_bind_group_layout(gpu_label!("native pass"), &entries))
}

pub(super) fn binding_type(built: &Built, binding: &Binding) -> Option<BindingType> {
    Some(match binding.kind {
        BindingKind::Uniform => BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(size_of::<api::Frame>() as u64),
        },
        BindingKind::Sampler if binding.resource == Global::NearestClamp => {
            BindingType::Sampler(SamplerBindingType::NonFiltering)
        }
        BindingKind::Sampler => BindingType::Sampler(SamplerBindingType::Filtering),
        BindingKind::ComparisonSampler => BindingType::Sampler(SamplerBindingType::Comparison),
        BindingKind::Texture(kind) => BindingType::Texture {
            sample_type: match kind {
                SampleKind::Float => TextureSampleType::Float {
                    filterable: filterable(built, binding.resource),
                },
                SampleKind::Uint => TextureSampleType::Uint,
                SampleKind::Depth => TextureSampleType::Depth,
            },
            view_dimension: match binding.dimension {
                ViewDimension::D2 => TextureViewDimension::D2,
                ViewDimension::D2Array => TextureViewDimension::D2Array,
            },
            multisampled: false,
        },
        BindingKind::StorageTexture => return None,
    })
}

fn filterable(built: &Built, resource: Global) -> bool {
    match resource {
        Global::Target(t) | Global::TargetPrevious(t) => built
            .targets
            .get(t)
            .is_some_and(|t| t.format.info().filterable),
        Global::SceneDepth => false,
        _ => true,
    }
}

pub(super) struct Bindables<'a> {
    pub(super) install: &'a Install,
    pub(super) targets: &'a [Vec<Target>],
    pub(super) walk: Walk,
    pub(super) scene_color: Option<&'a TextureView>,
    pub(super) scene_depth: Option<&'a TextureView>,
    pub(super) atlas: Option<&'a TextureView>,
    pub(super) lightmap: Option<&'a TextureView>,
}

impl Bindables<'_> {
    pub(super) fn template(
        &self,
        label: &str,
        layout: &BindGroupLayout,
        bindings: &[Binding],
    ) -> Option<BindTemplate> {
        let install = self.install;
        let samplers = &install.samplers;
        let view = |view: Option<&TextureView>| Some(Owned::View(view?.clone()));
        let target = |t: usize, which: usize| {
            Some(Owned::View(self.targets.get(t)?.get(which)?.view.clone()))
        };
        let mut entity_textures = Vec::new();
        let mut entries = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let owned = match binding.resource {
                Global::Frame => Some(Owned::Buffer(install.uniform.clone())),
                Global::LinearClamp => Some(Owned::Sampler(samplers.linear_clamp.clone())),
                Global::LinearRepeat => Some(Owned::Sampler(samplers.linear_repeat.clone())),
                Global::NearestClamp => Some(Owned::Sampler(samplers.nearest_clamp.clone())),
                Global::SceneColor => view(self.scene_color),
                Global::SceneDepth => view(self.scene_depth),
                Global::BlockAtlas => view(self.atlas),
                Global::Lightmap => view(self.lightmap),
                Global::Noise => view(install.textures.noise.as_ref().map(|t| &t.view)),
                Global::EntityTexture => {
                    entity_textures.push(binding.binding);
                    view(install.textures.white.as_ref().map(|t| &t.view))
                }
                Global::Target(t) => target(t, self.walk.current(t)),
                Global::TargetPrevious(t) => target(t, self.walk.previous(t)),
                Global::ShadowCompare => install.shadow.compare.clone().map(Owned::Sampler),
                Global::ShadowDepth => view(install.shadow.depth.as_ref()),
                Global::ShadowColor => view(install.shadow.color.as_ref()),
                Global::Texture(_) | Global::TargetOut(_) => None,
            };
            entries.push((binding.binding, owned?));
        }
        Some(BindTemplate::new(
            label.to_owned(),
            layout.clone(),
            entries,
            entity_textures,
        ))
    }
}
