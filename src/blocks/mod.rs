use crate::platform::time::Instant;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

use parking_lot::RwLock;

use crate::items::model::Assets;

pub mod ambient;
pub mod bake;
pub mod rand;
pub mod rotation;
pub mod state;

#[cfg(test)]
pub(crate) mod tests;

pub use bake::{BakedBlock, BakedModel, BakedQuad};
use state::{ModelRef, Rot, StateDef};

type ModelKey = (String, Rot);

pub struct Blocks {
    assets: Assets,
    defs: HashMap<String, StateDef>,
    models: RwLock<HashMap<ModelKey, Arc<BakedModel>>>,
    baked: RwLock<HashMap<u16, Arc<BakedBlock>>>,
    atlas_rows: u32,
    missing_defs: AtomicU32,
    missing_textures: AtomicU32,
}

static REGISTRY: OnceLock<Blocks> = OnceLock::new();

pub fn registry() -> &'static Blocks {
    REGISTRY.get_or_init(Blocks::load)
}

pub fn cache_bytes() -> (u64, usize, usize) {
    REGISTRY.get().map(Blocks::cache_bytes).unwrap_or((0, 0, 0))
}

impl Blocks {
    fn load() -> Blocks {
        let started = Instant::now();
        let root = crate::assets_root();
        let assets = Assets::new(&root);
        let mut defs = HashMap::new();
        let dir = root.join("blockstates");
        let blockstates = crate::platform::assets::read_dir(&dir);
        if blockstates.is_empty() {
            eprintln!("[BlockModels] cannot read {}", dir.display());
        }
        for path in blockstates {
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some(json) = assets.json(&format!("blockstates/{}.json", name)) else {
                eprintln!("[BlockModels] {}.json did not parse", name);
                continue;
            };
            match StateDef::parse(&json) {
                Some(def) => {
                    defs.insert(name.to_string(), def);
                }
                None => eprintln!("[BlockModels] {}.json has no variants/multipart", name),
            }
        }
        let atlas_rows = crate::ATLAS_ROWS
            .get()
            .copied()
            .unwrap_or(crate::renderer::TILE_PX);
        println!(
            "[BlockModels] {} blockstates parsed in {:.1} ms",
            defs.len(),
            started.elapsed().as_secs_f32() * 1000.0
        );
        Blocks {
            assets,
            defs,
            models: RwLock::new(HashMap::new()),
            baked: RwLock::new(HashMap::new()),
            atlas_rows,
            missing_defs: AtomicU32::new(0),
            missing_textures: AtomicU32::new(0),
        }
    }

    pub fn cache_bytes(&self) -> (u64, usize, usize) {
        let models = self.models.read();
        let quad = std::mem::size_of::<bake::BakedQuad>() as u64;
        let bytes = models.values().map(|m| m.quads.len() as u64 * quad).sum();
        (bytes, self.baked.read().len(), models.len())
    }

    pub fn baked(&self, state_id: u16, name: &str, props: &HashMap<&str, &str>) -> Arc<BakedBlock> {
        if let Some(hit) = self.baked.read().get(&state_id) {
            return hit.clone();
        }
        let built = Arc::new(self.build(name, props));
        self.baked.write().insert(state_id, built.clone());
        built
    }

    fn build(&self, name: &str, props: &HashMap<&str, &str>) -> BakedBlock {
        let Some(def) = self.defs.get(name) else {
            self.missing_defs.fetch_add(1, Ordering::Relaxed);
            return BakedBlock::default();
        };
        let mut out = BakedBlock::default();
        for slot in def.select(props) {
            let part = bake::bake_slot(slot, |r| self.model(r));
            out.is_solid |= part.choices.iter().all(|m| m.occludes);
            out.full_cube |= part.choices.iter().all(|m| m.full_cube);
            out.ambient_occlusion &= part.choices.iter().all(|m| m.ambient_occlusion);
            out.randomized |= part.choices.len() > 1;
            out.parts.push(part);
        }
        out
    }

    fn model(&self, reference: &ModelRef) -> Arc<BakedModel> {
        let key = (reference.model.clone(), reference.rot);
        if let Some(hit) = self.models.read().get(&key) {
            return hit.clone();
        }
        let built = Arc::new(bake::bake_model(
            &self.assets,
            &reference.model,
            reference.rot,
            self.atlas_rows,
        ));
        if built.missing_textures > 0 {
            self.missing_textures
                .fetch_add(built.missing_textures, Ordering::Relaxed);
        }
        self.models.write().insert(key, built.clone());
        built
    }

    #[cfg(test)]
    pub fn definition_count(&self) -> usize {
        self.defs.len()
    }

    #[cfg(test)]
    pub fn definition_for_test(&self, name: &str) -> Option<&StateDef> {
        self.defs.get(name)
    }

    #[cfg(test)]
    pub fn misses(&self) -> (u32, u32) {
        (
            self.missing_defs.load(Ordering::Relaxed),
            self.missing_textures.load(Ordering::Relaxed),
        )
    }
}
