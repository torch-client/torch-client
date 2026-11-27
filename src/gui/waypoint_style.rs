use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const ICON_PREFIX: &str = "hud/locator_bar_dot/";
const DEFAULT_NEAR: i32 = 128;
const DEFAULT_FAR: i32 = 332;
const MAX_DISTANCE: i64 = 60_000_000;

pub const DEFAULT: &str = "default";

const BUILT_IN: [(&str, &str); 2] = [
    (
        DEFAULT,
        r#"{"sprites":["minecraft:default_0","minecraft:default_1","minecraft:default_2","minecraft:default_3"]}"#,
    ),
    (
        "bowtie",
        r#"{"near_distance":64,"sprites":["minecraft:bowtie","minecraft:default_0","minecraft:default_1","minecraft:default_2","minecraft:default_3"]}"#,
    ),
];

pub struct WaypointStyle {
    near: i32,
    far: i32,
    sprites: Vec<String>,
}

impl WaypointStyle {
    fn missing() -> Self {
        Self {
            near: 0,
            far: 1,
            sprites: vec![String::new()],
        }
    }

    pub fn sprite(&self, distance: f32) -> &str {
        let last = self.sprites.len() - 1;
        if distance < self.near as f32 {
            &self.sprites[0]
        } else if distance >= self.far as f32 {
            &self.sprites[last]
        } else if self.sprites.len() == 1 {
            &self.sprites[0]
        } else if self.sprites.len() == 3 {
            &self.sprites[1]
        } else {
            let delta = (distance - self.near as f32) / (self.far - self.near) as f32;
            let index = 1 + (delta * (self.sprites.len() as i32 - 2) as f32).floor() as i32;
            &self.sprites[index.clamp(0, last as i32) as usize]
        }
    }
}

pub struct WaypointStyles {
    styles: HashMap<String, WaypointStyle>,
    missing: WaypointStyle,
}

impl WaypointStyles {
    pub fn get(&self, name: &str) -> &WaypointStyle {
        self.styles
            .get(name)
            .or_else(|| self.styles.get(DEFAULT))
            .unwrap_or(&self.missing)
    }
}

pub fn styles() -> Arc<WaypointStyles> {
    static CACHE: Mutex<Option<(u32, Arc<WaypointStyles>)>> = Mutex::new(None);
    let generation = crate::renderer::reload::generation();
    if let Ok(cache) = CACHE.lock()
        && let Some((built_at, styles)) = cache.as_ref()
        && *built_at == generation
    {
        return styles.clone();
    }
    let built = Arc::new(load());
    if let Ok(mut cache) = CACHE.lock() {
        *cache = Some((generation, built.clone()));
    }
    built
}

fn load() -> WaypointStyles {
    let dir = crate::assets_root().join("waypoint_style");
    let mut styles = HashMap::new();
    let mut names: Vec<String> = crate::platform::assets::read_dir(&dir)
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .filter_map(|p| Some(p.file_stem()?.to_str()?.to_owned()))
        .collect();
    for (name, _) in BUILT_IN {
        names.push(name.to_owned());
    }
    names.sort();
    names.dedup();
    for name in names {
        let path = dir.join(format!("{name}.json"));
        let text = match crate::platform::assets::read_to_string(&path) {
            Some(text) => text,
            None => match BUILT_IN.iter().find(|(n, _)| *n == name) {
                Some((_, text)) => (*text).into(),
                None => continue,
            },
        };
        match parse(&text) {
            Ok(style) => {
                styles.insert(name, style);
            }
            Err(e) => crate::log_warn!("waypoint", "waypoint_style/{name}.json: {e}"),
        }
    }
    crate::log_info!("waypoint", "{} waypoint styles", styles.len());
    WaypointStyles {
        styles,
        missing: WaypointStyle::missing(),
    }
}

fn parse(text: &str) -> Result<WaypointStyle, String> {
    let json: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let distance = |field: &str, default: i32| -> Result<i32, String> {
        let Some(value) = json.get(field) else {
            return Ok(default);
        };
        let n = value.as_i64().or_else(|| value.as_f64().map(|f| f as i64));
        match n {
            Some(n) if (0..=MAX_DISTANCE).contains(&n) => Ok(n as i32),
            _ => Err(format!("{field} must be a number from 0 to {MAX_DISTANCE}")),
        }
    };
    let near = distance("near_distance", DEFAULT_NEAR)?;
    let far = distance("far_distance", DEFAULT_FAR)?;
    let list = json
        .get("sprites")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "sprites must be a list".to_owned())?;
    if list.is_empty() {
        return Err("Must have at least one sprite icon".to_owned());
    }
    let mut sprites = Vec::with_capacity(list.len());
    for entry in list {
        let id = entry
            .as_str()
            .ok_or_else(|| "a sprite is not a string".to_owned())?;
        sprites.push(format!("{ICON_PREFIX}{}", identifier_path(id)?));
    }
    if near <= 0 {
        return Err(format!("Near distance ({near}) must be greater than zero"));
    }
    if near >= far {
        return Err(format!(
            "Far distance ({far}) cannot be closer or equal to near distance ({near})"
        ));
    }
    Ok(WaypointStyle { near, far, sprites })
}

fn identifier_path(id: &str) -> Result<&str, String> {
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
    let namespace_ok = !namespace.is_empty()
        && namespace
            .chars()
            .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-' | '.'));
    let path_ok = !path.is_empty()
        && path
            .chars()
            .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-' | '.' | '/'));
    if namespace_ok && path_ok {
        Ok(path)
    } else {
        Err(format!("not a valid identifier: {id}"))
    }
}
