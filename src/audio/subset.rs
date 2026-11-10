use serde_json::{Map, Value, json};

pub(crate) const MATERIALS: [&str; 10] = [
    "stone", "wood", "grass", "gravel", "sand", "wool", "glass", "metal", "snow", "ladder",
];

pub(crate) const ACTIONS: [&str; 5] = ["break", "place", "step", "hit", "fall"];

const PREFIXES: &[&str] = &[
    "ui.button.click",
    "entity.player.hurt",
    "entity.player.death",
    "entity.player.levelup",
    "entity.player.attack.",
    "entity.player.burp",
    "entity.player.big_fall",
    "entity.player.small_fall",
    "entity.generic.hurt",
    "entity.generic.eat",
    "entity.generic.drink",
    "entity.generic.explode",
    "entity.generic.splash",
    "entity.generic.swim",
    "entity.generic.burn",
    "entity.generic.extinguish_fire",
    "entity.item.pickup",
    "entity.experience_orb.pickup",
    "entity.arrow.shoot",
    "entity.arrow.hit",
    "item.bow.",
    "block.wooden_door.",
    "block.iron_door.",
    "block.chest.",
    "block.lever.",
    "block.stone_button.",
    "block.wooden_button.",
    "block.lava.ambient",
    "block.water.ambient",
];

const MOBS: [&str; 8] = [
    "zombie", "skeleton", "creeper", "spider", "cow", "pig", "sheep", "chicken",
];
const MOB_SOUNDS: [&str; 5] = ["ambient", "hurt", "death", "step", "primed"];

const FALLBACKS: &[(&str, &str)] = &[
    ("muddy", "gravel"),
    ("mud_bricks", "stone"),
    ("wool", "wool"),
    ("moss_carpet", "wool"),
    ("cobweb", "wool"),
    ("snow", "snow"),
    ("sand", "sand"),
    ("soul_soil", "sand"),
    ("gravel", "gravel"),
    ("mud", "gravel"),
    ("rooted_dirt", "gravel"),
    ("glass", "glass"),
    ("amethyst", "glass"),
    ("wood", "wood"),
    ("bamboo", "wood"),
    ("stem", "wood"),
    ("scaffolding", "wood"),
    ("shelf", "wood"),
    ("hanging_sign", "wood"),
    ("creaking_heart", "wood"),
    ("copper", "metal"),
    ("chain", "metal"),
    ("anvil", "metal"),
    ("iron", "metal"),
    ("lantern", "metal"),
    ("netherite", "metal"),
    ("heavy_core", "metal"),
    ("spawner", "metal"),
    ("vault", "metal"),
    ("grass", "grass"),
    ("leaves", "grass"),
    ("leaf", "grass"),
    ("azalea", "grass"),
    ("sapling", "grass"),
    ("moss", "grass"),
    ("vine", "grass"),
    ("roots", "grass"),
    ("fungus", "grass"),
    ("nylium", "grass"),
    ("wart", "grass"),
    ("crop", "grass"),
    ("berry", "grass"),
    ("dripleaf", "grass"),
    ("blossom", "grass"),
    ("petals", "grass"),
    ("lily_pad", "grass"),
    ("sprouts", "grass"),
    ("cactus_flower", "grass"),
    ("sculk", "grass"),
    ("slime", "grass"),
    ("honey", "grass"),
    ("frogspawn", "grass"),
    ("sponge", "grass"),
    ("shroomlight", "grass"),
    ("dried_ghast", "grass"),
];

pub(crate) fn wanted(event: &str) -> bool {
    if PREFIXES.iter().any(|prefix| event.starts_with(prefix)) {
        return true;
    }
    if let Some((group, action)) = block_sound(event) {
        return MATERIALS.contains(&group) && ACTIONS.contains(&action);
    }
    let Some((mob, sound)) = event
        .strip_prefix("entity.")
        .and_then(|rest| rest.split_once('.'))
    else {
        return false;
    };
    MOBS.contains(&mob) && MOB_SOUNDS.iter().any(|kept| sound.starts_with(kept))
}

fn block_sound(event: &str) -> Option<(&str, &str)> {
    let (group, action) = event.strip_prefix("block.")?.rsplit_once('.')?;
    (!group.contains('.')).then_some((group, action))
}

pub(crate) fn fallback(event: &str) -> Option<String> {
    let (group, action) = block_sound(event)?;
    if !ACTIONS.contains(&action) || MATERIALS.contains(&group) {
        return None;
    }
    let material = FALLBACKS
        .iter()
        .find(|(needle, _)| group.contains(needle))
        .map_or("stone", |(_, material)| material);
    Some(format!("block.{material}.{action}"))
}

fn referenced(body: &Value) -> impl Iterator<Item = &str> {
    body["sounds"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|sound| {
            (sound["type"].as_str() == Some("event"))
                .then(|| sound["name"].as_str())
                .flatten()
                .map(|name| name.strip_prefix("minecraft:").unwrap_or(name))
        })
}

pub(crate) fn trim(text: &str) -> Result<String, String> {
    let json: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let all = json.as_object().ok_or("sounds.json is not an object")?;

    let mut kept = Map::new();
    let mut queue: Vec<&str> = all
        .keys()
        .map(String::as_str)
        .filter(|name| wanted(name))
        .collect();
    while let Some(name) = queue.pop() {
        if kept.contains_key(name) {
            continue;
        }
        let Some(body) = all.get(name) else {
            continue;
        };
        queue.extend(referenced(body));
        kept.insert(name.to_owned(), body.clone());
    }

    let mut redirects = Vec::new();
    for (name, body) in all {
        if kept.contains_key(name) {
            continue;
        }
        let Some(target) = fallback(name).filter(|target| kept.contains_key(target)) else {
            continue;
        };
        let mut redirect = Map::new();
        redirect.insert(
            "sounds".to_owned(),
            json!([{ "name": target, "type": "event" }]),
        );
        if let Some(subtitle) = body.get("subtitle") {
            redirect.insert("subtitle".to_owned(), subtitle.clone());
        }
        redirects.push((name.clone(), Value::Object(redirect)));
    }
    let redirected = redirects.len();
    kept.extend(redirects);
    crate::log_info!(
        "audio",
        "sound subset: {} of {} events, {redirected} of them redirected",
        kept.len(),
        all.len()
    );
    Ok(Value::Object(kept).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kept_materials_and_their_actions_are_wanted() {
        assert!(wanted("block.stone.hit"));
        assert!(wanted("block.wool.fall"));
        assert!(!wanted("block.deepslate.hit"));
        assert!(!wanted("block.stone.idle"));
        assert!(wanted("entity.player.attack.nodamage"));
        assert!(wanted("entity.zombie.ambient"));
        assert!(!wanted("entity.zombie.attack_iron_door"));
        assert!(!wanted("music.game"));
    }

    #[test]
    fn a_dropped_block_group_mines_like_its_nearest_material() {
        assert_eq!(
            fallback("block.deepslate.hit").as_deref(),
            Some("block.stone.hit")
        );
        assert_eq!(
            fallback("block.wet_grass.step").as_deref(),
            Some("block.grass.step")
        );
        assert_eq!(
            fallback("block.muddy_mangrove_roots.break").as_deref(),
            Some("block.gravel.break")
        );
        assert_eq!(
            fallback("block.mud_bricks.place").as_deref(),
            Some("block.stone.place")
        );
        assert_eq!(
            fallback("block.copper_bulb.hit").as_deref(),
            Some("block.metal.hit")
        );
        assert_eq!(
            fallback("block.netherrack.fall").as_deref(),
            Some("block.stone.fall")
        );
        assert_eq!(fallback("block.stone.hit"), None);
        assert_eq!(fallback("block.anvil.land"), None);
        assert_eq!(fallback("block.note_block.basedrum.hit"), None);
    }

    #[test]
    fn trimming_keeps_references_and_redirects_with_the_subtitle() {
        let text = r#"{
            "block.stone.hit": {"sounds": ["step/stone1"], "subtitle": "subtitles.block.generic.hit"},
            "block.deepslate.hit": {"sounds": ["step/deepslate1"], "subtitle": "subtitles.block.generic.hit"},
            "entity.player.hurt": {"sounds": [{"name": "inner.event", "type": "event"}]},
            "inner.event": {"sounds": ["damage/hit1"]},
            "music.game": {"sounds": ["music/game/a"]}
        }"#;
        let trimmed: Value = serde_json::from_str(&trim(text).unwrap()).unwrap();
        let trimmed = trimmed.as_object().unwrap();
        assert!(trimmed.contains_key("inner.event"));
        assert!(!trimmed.contains_key("music.game"));
        let redirect = &trimmed["block.deepslate.hit"];
        assert_eq!(redirect["sounds"][0]["name"], "block.stone.hit");
        assert_eq!(redirect["sounds"][0]["type"], "event");
        assert_eq!(redirect["subtitle"], "subtitles.block.generic.hit");

        let defs = crate::audio::defs::Defs::parse(&trim(text).unwrap()).unwrap();
        assert_eq!(
            defs.object_keys(),
            ["sounds/damage/hit1.ogg", "sounds/step/stone1.ogg"]
        );
    }
}
