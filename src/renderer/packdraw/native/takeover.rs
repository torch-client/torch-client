use bevy::render::render_resource::TextureFormat;

use super::super::entities::EntityOwner;
use super::super::install::rejected;
use super::super::pipelines::Building;
use super::super::takeover::{PackVertices, ViewClaim};
use super::install::NativePack;
use super::render::{Install, NativeRender, Sized, WorldSized};

impl NativeRender {
    pub(in crate::renderer::packdraw) fn view_claim(
        &self,
        format: TextureFormat,
    ) -> Option<ViewClaim> {
        let ready = self.ready(format)?;
        let geometry = ready.install.geometry.as_ref()?;
        Some(ViewClaim {
            hand: geometry.entities.draws_hand(),
        })
    }

    pub(in crate::renderer::packdraw) fn entity_owner(&mut self) -> Option<EntityOwner<'_>> {
        let Install {
            geometry: Some(geometry),
            sized,
            pipelines,
            inputs,
            ..
        } = self.install.as_deref_mut()?
        else {
            return None;
        };
        let passes = match (pipelines, sized) {
            (
                Building::Ready(_),
                Some(Sized {
                    world:
                        Some(WorldSized {
                            plans: Some(plans), ..
                        }),
                    ..
                }),
            ) => Some(plans.entity_passes.as_mut_slice()),
            _ => None,
        };
        Some(EntityOwner {
            programs: &geometry.entities,
            frame: &mut geometry.entity_frame,
            camera: geometry.camera,
            inputs,
            passes,
        })
    }
}

impl NativePack {
    pub(in crate::renderer::packdraw) fn vertices(&self) -> Option<PackVertices<'_>> {
        let installed = &*self.installed;
        if rejected(installed.generation) {
            return None;
        }
        let player_shadow = installed.geometry() && installed.built.shadow.is_some();
        Some(PackVertices {
            block_ids: installed.block_ids.as_ref(),
            player_shadow,
        })
    }
}
