use std::collections::HashMap;

use crate::util::pack::Source;

const MAX_SIDE: u32 = 8192;

const MAX_BYTES: usize = 256 << 20;

pub(crate) struct CustomTexture {
    pub(crate) path: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
    pub(crate) blur: bool,
    pub(crate) clamp: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Setup,
    Begin,
    Shadowcomp,
    Prepare,
    Gbuffers,
    Deferred,
    Composite,
}

impl Stage {
    fn parse(word: &str) -> Option<Stage> {
        match word {
            "setup" => Some(Stage::Setup),
            "begin" => Some(Stage::Begin),
            "shadowcomp" => Some(Stage::Shadowcomp),
            "prepare" => Some(Stage::Prepare),
            "gbuffers" => Some(Stage::Gbuffers),
            "deferred" => Some(Stage::Deferred),
            "composite" => Some(Stage::Composite),
            _ => None,
        }
    }

    fn of_program(name: &str) -> Option<Stage> {
        let stages = [
            ("setup", Stage::Setup),
            ("begin", Stage::Begin),
            ("shadowcomp", Stage::Shadowcomp),
            ("prepare", Stage::Prepare),
            ("gbuffers_", Stage::Gbuffers),
            ("shadow", Stage::Gbuffers),
            ("deferred", Stage::Deferred),
            ("composite", Stage::Composite),
            ("final", Stage::Composite),
        ];
        stages
            .iter()
            .find(|(prefix, _)| name.starts_with(prefix))
            .map(|(_, stage)| *stage)
    }
}

#[derive(Default)]
pub(crate) struct CustomTextures {
    pub(crate) textures: Vec<CustomTexture>,
    noise: Option<u8>,
    overrides: Vec<(Stage, String, u8)>,
}

impl CustomTextures {
    pub(crate) fn read(
        properties: &HashMap<String, String>,
        source: &Source,
        notes: &mut Vec<String>,
    ) -> CustomTextures {
        let mut out = CustomTextures::default();
        let mut by_path: HashMap<String, u8> = HashMap::new();
        let mut bytes = 0usize;

        let mut keys: Vec<(&str, &str)> = properties
            .iter()
            .filter_map(|(key, value)| Some((key.strip_prefix("texture.")?, value.trim())))
            .collect();
        keys.sort_unstable();

        for (key, value) in keys {
            let target = if key == "noise" {
                None
            } else {
                let Some((stage, sampler)) = key.split_once('.') else {
                    notes.push(format!(
                        "texture.{key}: not a texture key this client reads"
                    ));
                    continue;
                };
                let Some(stage) = Stage::parse(stage) else {
                    notes.push(format!("texture.{key}: {stage} is not a stage Iris reads"));
                    continue;
                };
                Some((stage, sampler.to_owned()))
            };
            let path = match pack_path(value) {
                Ok(path) => path,
                Err(why) => {
                    notes.push(format!("texture.{key}={value}: {why}"));
                    continue;
                }
            };
            let index = match by_path.get(&path) {
                Some(index) => *index,
                None => {
                    if out.textures.len() > usize::from(u8::MAX) {
                        notes.push(format!("texture.{key}: more than 256 custom textures"));
                        continue;
                    }
                    if bytes >= MAX_BYTES {
                        notes.push(format!(
                            "texture.{key}={value}: the pack's textures pass {} MiB",
                            MAX_BYTES >> 20
                        ));
                        continue;
                    }
                    let texture = match decode(source, &path, MAX_BYTES - bytes) {
                        Ok(texture) => texture,
                        Err(why) => {
                            notes.push(format!("texture.{key}={value}: {why}"));
                            continue;
                        }
                    };
                    bytes += texture.rgba.len();
                    let index = out.textures.len() as u8;
                    out.textures.push(texture);
                    by_path.insert(path, index);
                    index
                }
            };
            match target {
                None => out.noise = Some(index),
                Some((stage, sampler)) => out.overrides.push((stage, sampler, index)),
            }
        }
        out
    }

    pub(crate) fn lookup(&self, program: &str, sampler: &str) -> Option<u8> {
        let stage = Stage::of_program(program);
        self.overrides
            .iter()
            .find(|(s, name, _)| Some(*s) == stage && name == sampler)
            .map(|(_, _, index)| *index)
            .or(if sampler == "noisetex" {
                self.noise
            } else {
                None
            })
    }
}

fn pack_path(value: &str) -> Result<String, &'static str> {
    if value.split_whitespace().nth(1).is_some() {
        return Err("raw binary textures are not supported yet");
    }
    if value.contains(':') {
        return Err("resource-location textures are not supported yet");
    }
    let path = value.trim_start_matches('/');
    if path.split(['/', '\\']).any(|part| part == "..") {
        return Err("the path leaves the pack");
    }
    if !path.to_ascii_lowercase().ends_with(".png") {
        return Err("only PNG textures are read");
    }
    Ok(format!("/{path}"))
}

fn decode(source: &Source, path: &str, budget: usize) -> Result<CustomTexture, String> {
    let file = source.read(path).ok_or("the pack has no such file")?;
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(&file), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(budget as u64);
    reader.limits(limits);
    let rgba = reader.decode().map_err(|e| e.to_string())?.into_rgba8();
    let (blur, clamp) = source
        .read_text(&format!("{path}.mcmeta"))
        .map_or((false, false), |text| mcmeta(&text));
    Ok(CustomTexture {
        path: path.to_owned(),
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
        blur,
        clamp,
    })
}

fn mcmeta(text: &str) -> (bool, bool) {
    let json: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
    let flag = |name: &str| json["texture"][name].as_bool().unwrap_or(false);
    (flag("blur"), flag("clamp"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_become_pack_paths() {
        assert_eq!(pack_path("tex/noise.png"), Ok("/tex/noise.png".into()));
        assert_eq!(pack_path("/tex/noise.png"), Ok("/tex/noise.png".into()));
        assert!(pack_path("../x.png").is_err());
        assert!(pack_path("minecraft:textures/x.png").is_err());
        assert!(pack_path("tex/x.dat TEXTURE_3D RGBA8 32 32 32 RGBA UNSIGNED_BYTE").is_err());
        assert!(pack_path("tex/x.jpg").is_err());
    }

    #[test]
    fn stage_keys_apply_to_their_programs() {
        let textures = CustomTextures {
            textures: Vec::new(),
            noise: Some(0),
            overrides: vec![(Stage::Deferred, "depthtex2".into(), 1)],
        };
        assert_eq!(textures.lookup("deferred1", "depthtex2"), Some(1));
        assert_eq!(textures.lookup("composite", "depthtex2"), None);
        assert_eq!(textures.lookup("final", "noisetex"), Some(0));
        assert_eq!(textures.lookup("gbuffers_terrain", "colortex0"), None);
        assert_eq!(Stage::of_program("shadowcomp_a"), Some(Stage::Shadowcomp));
        assert_eq!(Stage::of_program("shadow_cutout"), Some(Stage::Gbuffers));
        assert_eq!(Stage::of_program("final_a"), Some(Stage::Composite));
        assert_eq!(Stage::of_program("prepare1"), Some(Stage::Prepare));
    }

    #[test]
    fn mcmeta_flags_default_off() {
        assert_eq!(
            mcmeta(r#"{"texture":{"blur":true,"clamp":false}}"#),
            (true, false)
        );
        assert_eq!(mcmeta("{}"), (false, false));
        assert_eq!(mcmeta("not json"), (false, false));
    }

    #[test]
    fn solas_textures_load() {
        let dir = crate::shaderpack::discover::dir().join("Solas Shader V3.7b");
        let Ok(source) = Source::open(&dir, crate::shaderpack::ROOT) else {
            return;
        };
        let text = source
            .read_text("/shaders.properties")
            .expect("Solas has properties");
        use crate::shaderpack::options;
        let files: Vec<String> = crate::shaderpack::include::option_texts(&source);
        let declared = options::discover(files.iter().map(String::as_str));
        let props = crate::shaderpack::properties::parse_with_options(
            &text,
            &declared,
            &options::Values::default(),
        );
        let mut notes = Vec::new();
        let textures = CustomTextures::read(&props.other, &source, &mut notes);
        let noise = textures
            .lookup("deferred1", "noisetex")
            .expect("texture.noise");
        let noise = &textures.textures[usize::from(noise)];
        assert_eq!(
            (noise.width, noise.height, noise.blur, noise.clamp),
            (512, 512, true, false)
        );
        assert!(
            textures.lookup("deferred1", "depthtex2").is_some(),
            "{notes:?}"
        );
    }
}
