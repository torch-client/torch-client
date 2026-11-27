use bevy::prelude::*;
use bevy::render::Extract;

use super::*;

fn ancestors<'a>(
    parents: &'a Query<&ChildOf>,
    entity: Entity,
) -> impl Iterator<Item = Entity> + 'a {
    std::iter::successors(Some(entity), move |e| {
        parents.get(*e).ok().map(ChildOf::parent)
    })
    .take(8)
}

#[allow(clippy::type_complexity)]
pub(crate) fn extract_entity_draws(
    meshes: Extract<
        Query<(
            Entity,
            &Mesh3d,
            &MeshMaterial3d<crate::renderer::entity_material::EntityMaterial>,
            &GlobalTransform,
            &ViewVisibility,
            &InheritedVisibility,
            Option<&bevy::camera::visibility::RenderLayers>,
            Has<crate::blockentities::BeRigNode>,
            Has<crate::util::particles::ParticleBatch>,
        )>,
    >,
    materials: Extract<Res<Assets<crate::renderer::entity_material::EntityMaterial>>>,
    rigs: Extract<Option<Res<crate::entities::EntityRigs>>>,
    player_rigs: Extract<Option<Res<crate::renderer::player_model::PlayerRigs>>>,
    parents: Extract<Query<&ChildOf>>,
    visibilities: Extract<Query<&Visibility>>,
    world_camera: Extract<Query<&GlobalTransform, With<crate::renderer::systems::WorldCamera>>>,
    mut packs: EntityPacks,
) {
    let Some(EntityOwner {
        programs,
        frame,
        camera,
        ..
    }) = packs.entities()
    else {
        return;
    };
    frame.entity_draws.clear();
    let reach_centre = world_camera
        .single()
        .map_or(camera, GlobalTransform::translation);
    let pass_of = |kind: EntityKind| programs.pass_of(kind);
    let casters = programs.casters;
    let caster_pass = (programs.has(EntityKind::ShadowCaster) && programs.shadow_usable)
        .then_some(EntityKind::ShadowCaster);
    let local_root = player_rigs
        .as_deref()
        .and_then(|r| r.local_root())
        .filter(|_| casters.player || casters.entities);
    let caster_reach = programs.caster_reach;
    let shown_below = |entity: Entity, root: Entity| {
        for node in ancestors(&parents, entity) {
            if node == root {
                return true;
            }
            if visibilities
                .get(node)
                .is_ok_and(|v| *v == Visibility::Hidden)
            {
                return false;
            }
        }
        false
    };
    let under = |entity: Entity, root: Entity| ancestors(&parents, entity).any(|node| node == root);
    let world_kinds = [EntityKind::Entity, EntityKind::Block, EntityKind::Particle];
    if caster_pass.is_none()
        && world_kinds
            .iter()
            .all(|k| pass_of(*k).is_none() && pass_of(k.translucent()).is_none())
    {
        return;
    }
    frame.entity_roots.clear();
    if let Some(rigs) = rigs.as_deref()
        && !programs.entity_ids.is_empty()
    {
        for (root, kind) in rigs.roots() {
            let name = kind.to_str();
            let name = name.strip_prefix("minecraft:").unwrap_or(name);
            if let Some(id) = programs.entity_ids.get(name) {
                frame.entity_roots.insert(root, *id);
            }
        }
    }
    let entity_id = |entity: Entity, roots: &std::collections::HashMap<Entity, i32>| {
        ancestors(&parents, entity)
            .find_map(|node| roots.get(&node).copied())
            .unwrap_or(-1)
    };
    let world = bevy::camera::visibility::RenderLayers::default();
    for (entity, mesh, material, transform, visible, inherited, layers, block, particle) in &meshes
    {
        if layers.is_some_and(|l| !l.intersects(&world)) {
            continue;
        }
        let kind = match (block, particle) {
            (true, _) => EntityKind::Block,
            (_, true) => EntityKind::Particle,
            _ => EntityKind::Entity,
        };
        let local = local_root.filter(|root| kind == EntityKind::Entity && under(entity, *root));
        let casts = caster_pass.is_some()
            && match kind {
                EntityKind::Block => casters.block_entities,
                EntityKind::Particle => false,
                _ => casters.entities || local.is_some(),
            }
            && match local {
                Some(root) => shown_below(entity, root),
                None => inherited.get(),
            }
            && transform.translation().distance(reach_centre) <= caster_reach;
        let Some(material) = materials.get(&material.0) else {
            continue;
        };
        use crate::renderer::entity_material::LightMode;
        if material.params.mode == LightMode::WaterMask as u32 {
            continue;
        }
        if material.params.discards_all() {
            continue;
        }
        let blend = blended(material.alpha_mode);
        let glowing = material.params.mode == LightMode::Emissive as u32;
        let drawn_as = match kind {
            EntityKind::Entity if glowing && programs.has(EntityKind::SpiderEyes) => {
                EntityKind::SpiderEyes
            }
            _ if blend => kind.translucent(),
            _ => kind,
        };
        let pass = pass_of(drawn_as).filter(|_| visible.get());
        if pass.is_none() && !casts {
            continue;
        }
        let light = if glowing {
            Vec2::splat(240.0)
        } else {
            Vec2::new(material.params.light_block.w, material.params.light_sky.w)
        };
        let draw = EntityDraw {
            mesh: mesh.0.id(),
            texture: material.texture.as_ref().map(|t| t.id()),
            model: transform.to_matrix(),
            tint: material.params.tint,
            light,
            blend,
            pass: kind,
            overlay: overlay(&material.params),
            entity_id: if frame.entity_roots.is_empty() {
                -1
            } else {
                entity_id(entity, &frame.entity_roots)
            },
            item_id: 0,
            stage: drawn_as.render_stage(),
            order: 0,
            relative: false,
            flags: if [LightMode::Flat as u32, LightMode::Emissive as u32]
                .contains(&material.params.mode)
            {
                ENTITY_SHADE_IN_ALPHA
            } else {
                0
            },
        };
        if let Some(caster) = caster_pass.filter(|_| casts) {
            frame.entity_draws.push(EntityDraw {
                pass: caster,
                blend: false,
                ..draw
            });
        }
        if let Some(pass) = pass {
            frame.entity_draws.push(EntityDraw { pass, ..draw });
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn extract_standard_draws(
    sky: Extract<
        Query<(
            &crate::renderer::sky::SkyPart,
            &Mesh3d,
            &MeshMaterial3d<crate::renderer::sky::BackdropMaterial>,
            &GlobalTransform,
            &ViewVisibility,
        )>,
    >,
    sky_root: Extract<Query<&GlobalTransform, With<crate::renderer::sky::SkyRoot>>>,
    clouds: Extract<
        Query<
            (
                &Mesh3d,
                &MeshMaterial3d<StandardMaterial>,
                &GlobalTransform,
                &ViewVisibility,
            ),
            With<crate::renderer::clouds::CloudLayer>,
        >,
    >,
    hand: Extract<
        Query<(
            &Mesh3d,
            &MeshMaterial3d<crate::renderer::hand::HandMaterial>,
            &GlobalTransform,
            &ViewVisibility,
        )>,
    >,
    overlays: Extract<
        Query<
            (
                &Mesh3d,
                &MeshMaterial3d<StandardMaterial>,
                &GlobalTransform,
                &ViewVisibility,
                Has<crate::renderer::overlays::BreakOverlayMarker>,
            ),
            Or<(
                With<crate::renderer::overlays::BreakOverlayMarker>,
                With<crate::renderer::overlays::BlockOutlineMarker>,
            )>,
        >,
    >,
    materials: Extract<Res<Assets<StandardMaterial>>>,
    sky_materials: Extract<Res<Assets<crate::renderer::sky::BackdropMaterial>>>,
    hand_materials: Extract<Res<Assets<crate::renderer::hand::HandMaterial>>>,
    mut packs: EntityPacks,
) {
    use crate::renderer::sky::SkyPart;
    use crate::shaderpack::features::render_stage;
    let Some(EntityOwner {
        programs,
        frame,
        inputs,
        ..
    }) = packs.entities()
    else {
        return;
    };
    let has = |kind: EntityKind| programs.has(kind);
    let mut push = |kind: EntityKind,
                    mesh: &Mesh3d,
                    material: &StandardMaterial,
                    model: Mat4,
                    relative: bool,
                    stage: i32,
                    order: u8,
                    second: bool,
                    flags: u32| {
        let (light, item_id) = if kind.is_hand() {
            (inputs.eye_light.as_vec2() * 16.0, inputs.held_items[0])
        } else {
            (Vec2::splat(240.0), 0)
        };
        frame.entity_draws.push(EntityDraw {
            mesh: mesh.0.id(),
            texture: material.base_color_texture.as_ref().map(|t| t.id()),
            model,
            tint: if kind.is_hand() || kind == EntityKind::DamagedBlock {
                Vec4::ONE
            } else {
                Vec4::from_array(material.base_color.to_srgba().to_f32_array())
            },
            light,
            blend: second,
            pass: kind,
            overlay: Vec4::ZERO,
            entity_id: -1,
            item_id,
            stage,
            order,
            relative,
            flags,
        });
    };

    if (has(EntityKind::SkyBasic) || has(EntityKind::SkyTextured))
        && let Ok(root) = sky_root.single()
    {
        let (scale, _, translation) = root.to_scale_rotation_translation();
        let local = Mat4::from_scale(scale.recip()) * Mat4::from_translation(-translation);
        let parts = programs.sky_parts;
        for (part, mesh, material, transform, visible) in &sky {
            let (kind, stage, order, wanted) = match part {
                SkyPart::SkyDisc => (EntityKind::SkyBasic, "SKY", 0, parts.sky),
                SkyPart::SunriseFan => (EntityKind::SkyBasic, "SUNSET", 1, parts.sunrise),
                SkyPart::Sun => (EntityKind::SkyTextured, "SUN", 2, parts.sun),
                SkyPart::Moon => (EntityKind::SkyTextured, "MOON", 3, parts.moon),
                SkyPart::Stars => (EntityKind::SkyBasic, "STARS", 4, parts.stars),
                SkyPart::DarkDisc => (EntityKind::SkyBasic, "VOID", 5, parts.void),
                SkyPart::EndSky => (EntityKind::SkyTextured, "SKY", 0, parts.end),
                SkyPart::EndFlash => (EntityKind::SkyTextured, "SKY", 1, parts.end_flash),
            };
            if !visible.get() || !wanted || !has(kind) {
                continue;
            }
            let Some(material) = sky_materials.get(&material.0).map(|m| &m.base) else {
                continue;
            };
            let second = material.alpha_mode == AlphaMode::Add;
            let flags = sky_part_id(*part) << SKY_PART_SHIFT;
            push(
                kind,
                mesh,
                material,
                local * transform.to_matrix(),
                true,
                render_stage(stage),
                order,
                second,
                flags,
            );
        }
    }

    if has(EntityKind::Clouds) {
        for (mesh, material, transform, visible) in &clouds {
            if !visible.get() {
                continue;
            }
            let Some(material) = materials.get(&material.0) else {
                continue;
            };
            let fast = material.cull_mode.is_none();
            push(
                EntityKind::Clouds,
                mesh,
                material,
                transform.to_matrix(),
                false,
                EntityKind::Clouds.render_stage(),
                0,
                fast,
                0,
            );
        }
    }

    for (mesh, material, transform, visible, cracks) in &overlays {
        let kind = if cracks {
            EntityKind::DamagedBlock
        } else {
            EntityKind::Line
        };
        if !visible.get() || !has(kind) {
            continue;
        }
        let Some(material) = materials.get(&material.0) else {
            continue;
        };
        push(
            kind,
            mesh,
            material,
            transform.to_matrix(),
            false,
            kind.render_stage(),
            0,
            false,
            0,
        );
    }

    if EntityKind::ALL.iter().any(|k| k.is_hand() && has(*k)) {
        for (mesh, material, transform, visible) in &hand {
            if !visible.get() {
                continue;
            }
            let Some(material) = hand_materials.get(&material.0).map(|m| &m.base) else {
                continue;
            };
            let translucent = blended(material.alpha_mode);
            let kind = if translucent && has(EntityKind::HandWater) {
                EntityKind::HandWater
            } else {
                EntityKind::Hand
            };
            if !has(kind) {
                continue;
            }
            push(
                kind,
                mesh,
                material,
                transform.to_matrix(),
                true,
                kind.render_stage(),
                0,
                translucent,
                0,
            );
        }
    }
}

fn overlay(params: &crate::renderer::entity_material::EntityParams) -> Vec4 {
    let strength = (1.0 - params.light1.w).clamp(0.0, 1.0);
    let colour = if params.light0.w < 0.5 {
        Vec3::X
    } else {
        Vec3::ONE
    };
    colour.extend(strength)
}
