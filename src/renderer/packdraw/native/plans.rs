use std::collections::HashMap;

use bevy::render::render_resource::{BindGroup, TextureFormat, TextureView, TextureViewId};
use bevy::render::renderer::RenderDevice;

use super::super::compile::EntityKind;
use super::super::entities::EntityPasses;
use super::super::plan::{Attachment, EntityPass};
use super::bind::Bindables;
use super::overlay::{self, OverlayPipelines};
use super::render::{Install, Pipelines, Sized};
use super::walk::{Walk, walk_frame};

pub(super) struct WorldPlans {
    pub(super) key: [TextureViewId; 2],
    pub(super) format: TextureFormat,
    pub(super) plans: Vec<WorldPlan>,
    pub(super) entity_passes: Vec<EntityPasses>,
}

pub(super) struct WorldPlan {
    pub(super) start: u32,
    pub(super) groups: Vec<GroupPlan>,
    pub(super) shadow: Option<BindGroup>,
}

pub(super) struct GroupPlan {
    pub(super) bind_group: BindGroup,
    pub(super) attachments: Vec<Attachment>,
}

impl WorldPlans {
    pub(super) fn find(&self, start: u32) -> Option<usize> {
        self.plans.iter().position(|plan| plan.start == start)
    }
}

pub(super) struct PlanViews<'a> {
    pub(super) atlas: &'a TextureView,
    pub(super) lightmap: &'a TextureView,
    pub(super) key: [TextureViewId; 2],
    pub(super) format: TextureFormat,
    pub(super) overlays: &'a OverlayPipelines,
}

pub(super) fn make(
    device: &RenderDevice,
    install: &Install,
    pipelines: &Pipelines,
    sized: &Sized,
    views: PlanViews<'_>,
) -> Option<WorldPlans> {
    let geometry = install.geometry.as_ref()?;
    let world = sized.world.as_ref()?;
    let built = &install.installed.built;
    let end = walk_frame(built, 0).end;
    let starts = if end == 0 { vec![0] } else { vec![0, end] };
    let bindables = |walk: Walk| Bindables {
        install,
        targets: &sized.targets,
        walk,
        scene_color: None,
        scene_depth: world.depth_copy.as_ref().map(|copy| &copy.view),
        atlas: Some(views.atlas),
        lightmap: Some(views.lightmap),
    };
    let mut plans = Vec::with_capacity(starts.len());
    let mut entity_passes = Vec::with_capacity(starts.len());
    for start in starts {
        let walked = walk_frame(built, start);
        let attachments = |index: usize| -> Vec<Attachment> {
            let walk = walked.groups[index];
            walk.group_writes(&built.targets, &built.geometry[index])
                .into_iter()
                .map(|(t, which)| Attachment {
                    target: t as u8,
                    view: sized.targets[t][which].view.clone(),
                })
                .collect()
        };
        let mut groups = Vec::with_capacity(built.geometry.len());
        for (index, group) in built.geometry.iter().enumerate() {
            let template = bindables(walked.groups[index]).template(
                gpu_label!("native geometry"),
                &geometry.layouts[index],
                &group.bindings,
            )?;
            groups.push(GroupPlan {
                bind_group: template.create(device, None),
                attachments: attachments(index),
            });
        }
        let mut passes: EntityPasses = std::array::from_fn(|at| {
            let draw = geometry.kinds[at]?;
            let (opaque, blend) = pipelines.groups.get(draw.group)?.entities[at].clone()?;
            let group = &built.geometry[draw.group];
            let template = bindables(walked.groups[draw.group]).template(
                gpu_label!("native entities"),
                &geometry.layouts[draw.group],
                &group.bindings,
            )?;
            Some(EntityPass {
                opaque,
                blend,
                attachments: attachments(draw.group),
                template,
                groups: HashMap::new(),
            })
        });
        for (kind, (opaque, blend)) in overlay::KINDS
            .into_iter()
            .zip(views.overlays.iter().cloned())
        {
            let template = bindables(Walk::new(start)).template(
                gpu_label!("native overlays"),
                &geometry.overlay.layout,
                &geometry.overlay.bindings,
            )?;
            passes[kind.index()] = Some(EntityPass {
                opaque,
                blend,
                attachments: Vec::new(),
                template,
                groups: HashMap::new(),
            });
        }
        let shadow = match (&install.shadow.pass, &pipelines.shadow, &built.shadow) {
            (Some(pass), Some(shadow_pipelines), Some(shadow)) => {
                let template = bindables(walked.shadow).template(
                    gpu_label!("native shadow"),
                    &pass.globals,
                    &shadow.bindings,
                )?;
                let (opaque, blend) = shadow_pipelines.entities.clone();
                let group = template.create(device, None);
                passes[EntityKind::ShadowCaster.index()] = Some(EntityPass {
                    opaque,
                    blend,
                    attachments: Vec::new(),
                    template,
                    groups: HashMap::new(),
                });
                Some(group)
            }
            (Some(_), _, _) => return None,
            _ => None,
        };
        plans.push(WorldPlan {
            start,
            groups,
            shadow,
        });
        entity_passes.push(passes);
    }
    Some(WorldPlans {
        key: views.key,
        format: views.format,
        plans,
        entity_passes,
    })
}
