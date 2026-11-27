use std::sync::Arc;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

use super::compile::EntityKind;
use super::entities::EntityOwner;
use super::frame::{Install, PackRender, Stage};
use super::install::{CompiledPack, rejected};
#[cfg(feature = "native_shaders")]
use super::native::NativeRender;
use super::pipelines::Building;
use crate::renderer::packvertex::BlockIds;

#[derive(Clone, Copy)]
pub(super) struct ViewClaim {
    pub(super) hand: bool,
}

#[derive(SystemParam)]
pub(crate) struct ViewPacks<'w> {
    glsl: Option<Res<'w, PackRender>>,
    #[cfg(feature = "native_shaders")]
    native: Option<Res<'w, NativeRender>>,
}

impl ViewPacks<'_> {
    pub(super) fn claim(&self, format: TextureFormat) -> Option<ViewClaim> {
        #[cfg(feature = "native_shaders")]
        if let Some(claim) = self
            .native
            .as_deref()
            .and_then(|native| native.view_claim(format))
        {
            return Some(claim);
        }
        self.glsl.as_deref()?.view_claim(format)
    }
}

impl PackRender {
    fn view_claim(&self, format: TextureFormat) -> Option<ViewClaim> {
        self.drawable(format)?;
        let install = self.install.as_deref()?;
        Some(ViewClaim {
            hand: install.entities.draws_hand(),
        })
    }
}

#[derive(Clone, Copy)]
pub(super) struct PackVertices<'a> {
    pub(super) block_ids: Option<&'a Arc<BlockIds>>,
    pub(super) player_shadow: bool,
}

impl CompiledPack {
    pub(super) fn vertices(&self) -> Option<PackVertices<'_>> {
        let installed = &*self.installed;
        if rejected(installed.generation) {
            return None;
        }
        let casters = installed.shadow_casters;
        Some(PackVertices {
            block_ids: installed.block_ids.as_ref(),
            player_shadow: installed.entities[EntityKind::ShadowCaster.index()].is_some()
                && (casters.player || casters.entities),
        })
    }
}

#[derive(SystemParam)]
pub(crate) struct EntityPacks<'w> {
    glsl: Option<ResMut<'w, PackRender>>,
    #[cfg(feature = "native_shaders")]
    native: Option<ResMut<'w, NativeRender>>,
}

impl EntityPacks<'_> {
    pub(super) fn entities(&mut self) -> Option<EntityOwner<'_>> {
        #[cfg(feature = "native_shaders")]
        if let Some(owner) = self
            .native
            .as_deref_mut()
            .and_then(NativeRender::entity_owner)
        {
            return Some(owner);
        }
        self.glsl.as_deref_mut()?.entity_owner()
    }
}

impl PackRender {
    fn entity_owner(&mut self) -> Option<EntityOwner<'_>> {
        let Install {
            entities,
            entity_frame,
            frame,
            stage,
            ..
        } = self.install.as_deref_mut()?;
        let passes = match stage {
            Stage::Started {
                pipelines: Building::Ready(_),
                sized: Some(sized),
            } => Some(sized.entity_passes.as_mut_slice()),
            _ => None,
        };
        Some(EntityOwner {
            programs: entities,
            frame: entity_frame,
            camera: frame.camera,
            inputs: &frame.inputs,
            passes,
        })
    }
}
