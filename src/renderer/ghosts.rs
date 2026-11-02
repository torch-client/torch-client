use std::collections::HashMap;

use bevy::prelude::*;

use super::entity_material::{ALPHA_CUTOUT, EntityMaterial, LightMode, Lit};
use super::origin::RenderOrigin;
use super::systems::BlockAtlasHandle;
use crate::modules::script::ghost::{Change, Key, take_changes};

const WRONG_TINT: [f32; 4] = [1.0, 0.15, 0.15, 0.35];

#[derive(Resource, Default)]
pub(super) struct Ghosts {
    shown: HashMap<Key, Vec<Entity>>,
    ghost: HashMap<u8, Handle<EntityMaterial>>,
    wrong: Option<Handle<EntityMaterial>>,
}

#[derive(Component)]
pub(super) struct GhostSection([i32; 3]);

fn material(texture: Option<Handle<Image>>, tint: [f32; 4], cutoff: f32) -> EntityMaterial {
    EntityMaterial::new(
        texture,
        LightMode::Flat,
        AlphaMode::Blend,
        cutoff,
        Lit {
            tint,
            ..Lit::full_bright()
        },
        0,
    )
}

pub(super) fn update_ghosts(
    mut commands: Commands,
    mut ghosts: ResMut<Ghosts>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<EntityMaterial>>,
    atlas: Option<Res<BlockAtlasHandle>>,
    origin: Res<RenderOrigin>,
    mut shown_origin: Local<RenderOrigin>,
    mut sections: Query<(&GhostSection, &mut Transform)>,
) {
    let Some(atlas) = atlas else {
        return;
    };
    let place = |sec: [i32; 3]| {
        Transform::from_translation(origin.block_to_render(IVec3::from_array(sec) * 16))
    };
    for change in take_changes() {
        match change {
            Change::Clear => {
                for (_, es) in ghosts.shown.drain() {
                    es.into_iter().for_each(|e| commands.entity(e).despawn());
                }
            }
            Change::Remove(key) => {
                if let Some(es) = ghosts.shown.remove(&key) {
                    es.into_iter().for_each(|e| commands.entity(e).despawn());
                }
            }
            Change::Set {
                key,
                ghost,
                wrong,
                alpha,
            } => {
                if let Some(es) = ghosts.shown.remove(&key) {
                    es.into_iter().for_each(|e| commands.entity(e).despawn());
                }
                let mut es = Vec::new();
                if let Some(mesh) = ghost {
                    let m = ghosts
                        .ghost
                        .entry(alpha)
                        .or_insert_with(|| {
                            let a = f32::from(alpha) / 255.0;
                            materials.add(material(
                                Some(atlas.0.clone()),
                                [1.0, 1.0, 1.0, a],
                                ALPHA_CUTOUT,
                            ))
                        })
                        .clone();
                    es.push(spawn(
                        &mut commands,
                        meshes.add(mesh),
                        m,
                        key.1,
                        place(key.1),
                    ));
                }
                if let Some(mesh) = wrong {
                    let m = ghosts
                        .wrong
                        .get_or_insert_with(|| materials.add(material(None, WRONG_TINT, 0.0)))
                        .clone();
                    es.push(spawn(
                        &mut commands,
                        meshes.add(mesh),
                        m,
                        key.1,
                        place(key.1),
                    ));
                }
                if !es.is_empty() {
                    ghosts.shown.insert(key, es);
                }
            }
        }
    }
    if *shown_origin != *origin {
        *shown_origin = *origin;
        for (sec, mut t) in &mut sections {
            *t = place(sec.0);
        }
    }
}

fn spawn(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    m: Handle<EntityMaterial>,
    sec: [i32; 3],
    at: Transform,
) -> Entity {
    #[allow(unused_mut, reason = "the shadow opt-out is builtin_shaders only")]
    let mut e = commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(m),
        at,
        Visibility::Visible,
        GhostSection(sec),
    ));
    #[cfg(feature = "builtin_shaders")]
    e.insert(bevy::light::NotShadowCaster);
    e.id()
}
