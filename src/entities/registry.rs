use std::collections::HashMap;

use azalea_registry::builtin::EntityKind;
use bevy::prelude::Resource;

use super::RenderSpec;

#[derive(Resource)]
pub struct Registry {
    pub specs: Vec<RenderSpec>,
    by_kind: HashMap<EntityKind, Vec<usize>>,
    empty: Vec<usize>,
}

impl Registry {
    pub fn build() -> Registry {
        let mut registry = Registry {
            specs: Vec::new(),
            by_kind: HashMap::new(),
            empty: Vec::new(),
        };
        super::render::register_all(&mut registry);
        registry
    }

    pub fn add(&mut self, kind: EntityKind, spec: RenderSpec) {
        let idx = self.push(spec);
        self.by_kind.entry(kind).or_default().push(idx);
    }

    pub fn add_many(&mut self, kinds: &[EntityKind], spec: RenderSpec) {
        let idx = self.push(spec);
        for kind in kinds {
            self.by_kind.entry(*kind).or_default().push(idx);
        }
    }

    fn push(&mut self, spec: RenderSpec) -> usize {
        debug_assert!(
            !self.specs.iter().any(|s| s.name == spec.name),
            "duplicate entity spec name: {}",
            spec.name
        );
        self.specs.push(spec);
        self.specs.len() - 1
    }

    pub fn specs_for(&self, kind: EntityKind) -> &[usize] {
        self.by_kind.get(&kind).unwrap_or(&self.empty)
    }

    #[allow(dead_code, reason = "read by the entity coverage test")]
    pub fn registered_kinds(&self) -> impl Iterator<Item = EntityKind> + '_ {
        self.by_kind.keys().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_spec_bakes_and_its_texture_exists() {
        use super::super::Geom;

        let registry = Registry::build();
        let mut missing: Vec<String> = Vec::new();
        for spec in &registry.specs {
            let model = match &spec.geom {
                Geom::Model(model) => model,
                Geom::Item(_) | Geom::Block(_) | Geom::Built(_) => continue,
            };
            let layer = (model.layer)();
            let baked = super::super::geom::bake(&layer);
            assert!(
                baked.parts.len() > 1,
                "{} baked to nothing but a root",
                spec.name
            );
            let mut st = crate::entities::EntityState::new(0, EntityKind::Pig);
            st.extras.shared_mut().variant = None;
            let path = (model.texture)(&st);
            if let Some(rest) =
                path.strip_prefix(crate::entities::render::humanoid::armor::STACK_PREFIX)
            {
                if crate::entities::render::humanoid::armor::stack(rest).is_none() {
                    missing.push(format!("{} -> stack {rest} does not compose", spec.name));
                }
                continue;
            }
            let file = crate::assets_root().join(format!("textures/{path}.png"));
            if !file.exists() {
                missing.push(format!("{} -> textures/{path}.png", spec.name));
            }
        }
        assert!(
            missing.is_empty(),
            "missing entity textures:\n{}",
            missing.join("\n")
        );
    }

    #[test]
    fn every_baked_model_carries_the_entity_vertex_layout() {
        use bevy::prelude::Mesh;

        use super::super::Geom;

        let required = [
            Mesh::ATTRIBUTE_POSITION,
            Mesh::ATTRIBUTE_NORMAL,
            Mesh::ATTRIBUTE_UV_0,
            Mesh::ATTRIBUTE_COLOR,
        ];
        let registry = Registry::build();
        for spec in &registry.specs {
            let Geom::Model(model) = &spec.geom else {
                continue;
            };
            let baked = super::super::geom::bake(&(model.layer)());
            for part in &baked.parts {
                let Some(mesh) = &part.mesh else { continue };
                for attribute in &required {
                    assert!(
                        mesh.attribute(attribute.id).is_some(),
                        "{}: part mesh has no {}",
                        spec.name,
                        attribute.name,
                    );
                }
            }
        }
    }

    #[test]
    fn a_built_quad_carries_the_entity_vertex_layout() {
        use bevy::prelude::Mesh;

        let mesh = super::super::quads::billboard_quad(0.25, [0.0, 0.0, 1.0, 1.0]);
        for attribute in [
            Mesh::ATTRIBUTE_POSITION,
            Mesh::ATTRIBUTE_NORMAL,
            Mesh::ATTRIBUTE_UV_0,
            Mesh::ATTRIBUTE_COLOR,
        ] {
            assert!(
                mesh.attribute(attribute.id).is_some(),
                "no {}",
                attribute.name
            );
        }
    }

    const DRAWN_ELSEWHERE: &[EntityKind] = &[
        EntityKind::AreaEffectCloud,
        EntityKind::Interaction,
        EntityKind::Marker,
        EntityKind::Player,
        EntityKind::TextDisplay,
    ];

    #[test]
    #[ignore = "coverage report, not a regression gate"]
    fn every_entity_kind_is_accounted_for() {
        use azalea_registry::Registry as _;

        let registry = Registry::build();
        let mut unregistered: Vec<String> = Vec::new();
        let mut id = 0u32;
        while let Some(kind) = EntityKind::from_u32(id) {
            if registry.specs_for(kind).is_empty() && !DRAWN_ELSEWHERE.contains(&kind) {
                unregistered.push(kind.to_str().to_string());
            }
            id += 1;
        }
        assert!(
            unregistered.is_empty(),
            "{} entity kinds draw nothing and are not on the DRAWN_ELSEWHERE list:\n{}",
            unregistered.len(),
            unregistered.join("\n")
        );
    }
}
