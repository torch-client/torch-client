use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

use crate::util::datapack;

pub(crate) struct MobVariant {
    pub(crate) asset_id: String,
    pub(crate) baby_asset_id: Option<String>,
    pub(crate) model: Option<String>,
}

impl MobVariant {
    pub(crate) fn texture(&self, is_baby: bool) -> &str {
        match (is_baby, &self.baby_asset_id) {
            (true, Some(path)) => path,
            _ => &self.asset_id,
        }
    }

    pub(crate) fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }
}

pub(crate) struct WolfAssets {
    wild: String,
    tame: String,
    angry: String,
}

pub(crate) struct WolfVariant {
    assets: WolfAssets,
    baby_assets: Option<WolfAssets>,
}

impl WolfVariant {
    pub(crate) fn texture(&self, tame: bool, angry: bool, is_baby: bool) -> &str {
        let set = match (is_baby, &self.baby_assets) {
            (true, Some(baby)) => baby,
            _ => &self.assets,
        };
        if tame {
            &set.tame
        } else if angry {
            &set.angry
        } else {
            &set.wild
        }
    }
}

struct Variants {
    cat: HashMap<String, MobVariant>,
    cow: HashMap<String, MobVariant>,
    pig: HashMap<String, MobVariant>,
    chicken: HashMap<String, MobVariant>,
    frog: HashMap<String, MobVariant>,
    wolf: HashMap<String, WolfVariant>,
}

static VARIANTS: OnceLock<Variants> = OnceLock::new();

fn variants() -> &'static Variants {
    VARIANTS.get_or_init(|| {
        let loaded = Variants {
            cat: mob_registry("cat_variant"),
            cow: mob_registry("cow_variant"),
            pig: mob_registry("pig_variant"),
            chicken: mob_registry("chicken_variant"),
            frog: mob_registry("frog_variant"),
            wolf: wolf_registry(),
        };
        crate::log_info!(
            "variants",
            "cat {}, wolf {}, cow {}, pig {}, chicken {}, frog {}",
            loaded.cat.len(),
            loaded.wolf.len(),
            loaded.cow.len(),
            loaded.pig.len(),
            loaded.chicken.len(),
            loaded.frog.len()
        );
        loaded
    })
}

fn mob_registry(registry: &str) -> HashMap<String, MobVariant> {
    datapack::entries(registry)
        .into_iter()
        .filter_map(|(id, json)| {
            let asset_id = datapack::bare_id(json.get("asset_id")?)?.to_owned();
            let baby_asset_id = json
                .get("baby_asset_id")
                .and_then(datapack::bare_id)
                .map(str::to_owned);
            let model = json.get("model").and_then(Value::as_str).map(str::to_owned);
            Some((
                id,
                MobVariant {
                    asset_id,
                    baby_asset_id,
                    model,
                },
            ))
        })
        .collect()
}

fn wolf_registry() -> HashMap<String, WolfVariant> {
    datapack::entries("wolf_variant")
        .into_iter()
        .filter_map(|(id, json)| {
            let assets = wolf_assets(json.get("assets")?)?;
            let baby_assets = json.get("baby_assets").and_then(wolf_assets);
            Some((
                id,
                WolfVariant {
                    assets,
                    baby_assets,
                },
            ))
        })
        .collect()
}

fn wolf_assets(value: &Value) -> Option<WolfAssets> {
    Some(WolfAssets {
        wild: datapack::bare_id(value.get("wild")?)?.to_owned(),
        tame: datapack::bare_id(value.get("tame")?)?.to_owned(),
        angry: datapack::bare_id(value.get("angry")?)?.to_owned(),
    })
}

fn key(id: &str) -> &str {
    id.rsplit(':').next().unwrap_or(id)
}

pub(crate) fn cat(id: &str) -> Option<&'static MobVariant> {
    variants().cat.get(key(id))
}

pub(crate) fn cow(id: &str) -> Option<&'static MobVariant> {
    variants().cow.get(key(id))
}

pub(crate) fn pig(id: &str) -> Option<&'static MobVariant> {
    variants().pig.get(key(id))
}

pub(crate) fn chicken(id: &str) -> Option<&'static MobVariant> {
    variants().chicken.get(key(id))
}

pub(crate) fn frog(id: &str) -> Option<&'static MobVariant> {
    variants().frog.get(key(id))
}

pub(crate) fn wolf(id: &str) -> Option<&'static WolfVariant> {
    variants().wolf.get(key(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mob(asset: &str, baby: Option<&str>, model: Option<&str>) -> MobVariant {
        MobVariant {
            asset_id: asset.to_owned(),
            baby_asset_id: baby.map(str::to_owned),
            model: model.map(str::to_owned),
        }
    }

    #[test]
    fn a_baby_without_its_own_asset_wears_the_adult_texture() {
        let frog = mob("entity/frog/frog_warm", None, None);
        assert_eq!(frog.texture(false), "entity/frog/frog_warm");
        assert_eq!(frog.texture(true), "entity/frog/frog_warm");

        let cow = mob(
            "entity/cow/cow_cold",
            Some("entity/cow/cow_cold_baby"),
            Some("cold"),
        );
        assert_eq!(cow.texture(true), "entity/cow/cow_cold_baby");
        assert_eq!(cow.model(), Some("cold"));
    }

    #[test]
    fn a_wolf_picks_tame_over_angry() {
        let set = |suffix: &str| WolfAssets {
            wild: format!("wolf{suffix}"),
            tame: format!("wolf_tame{suffix}"),
            angry: format!("wolf_angry{suffix}"),
        };
        let pale = WolfVariant {
            assets: set(""),
            baby_assets: Some(set("_baby")),
        };
        assert_eq!(pale.texture(false, false, false), "wolf");
        assert_eq!(pale.texture(false, true, false), "wolf_angry");
        assert_eq!(pale.texture(true, true, false), "wolf_tame");
        assert_eq!(pale.texture(true, false, true), "wolf_tame_baby");
    }

    #[test]
    fn a_namespaced_id_and_a_bare_one_are_the_same_key() {
        assert_eq!(key("minecraft:black"), "black");
        assert_eq!(key("black"), "black");
    }
}
