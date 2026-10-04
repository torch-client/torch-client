use super::super::images::CustomTextures;
use super::AlphaTest;
use crate::renderer::dimension::Dimension;
use crate::shaderpack::options::{self, Options, Values};
use crate::shaderpack::{include, preprocess, programs, properties};
use crate::util::pack::Source;

pub(super) struct Pack {
    pub(super) source: Source,
    pub(super) declared: Options,
    pub(super) values: Values,
    pub(super) environment: preprocess::Defines,
    pub(super) enable: Vec<(String, String)>,
    pub(super) cache: include::Cache,
    pub(super) images: CustomTextures,
    pub(super) alpha_tests: std::collections::HashMap<String, Option<AlphaTest>>,
    pub(super) folder: String,
    pub(super) custom_images: Vec<crate::shaderpack::customimages::CustomImage>,
    pub(super) buffers: Vec<crate::shaderpack::customimages::BufferObject>,
}

pub(crate) fn program_folder(source: &Source, dimension: Dimension) -> String {
    let files = source.files();
    let exists = |folder: &str| files.iter().any(|f| f.starts_with(&format!("/{folder}/")));
    let has_programs = |folder: &str| {
        files.iter().any(|f| {
            f.strip_prefix(&format!("/{folder}/")).is_some_and(|rest| {
                !rest.contains('/')
                    && [".vsh", ".fsh", ".csh", ".gsh"]
                        .iter()
                        .any(|e| rest.ends_with(e))
            })
        })
    };
    let map: Vec<(String, Vec<String>)> = match source.read_text("/dimension.properties") {
        Some(text) => text
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                let rest = line.strip_prefix("dimension.")?;
                let split = rest.find(|c: char| c == '=' || c == ':' || c.is_whitespace())?;
                let folder = rest[..split].trim().to_owned();
                let ids = rest[split..]
                    .trim_start_matches(|c: char| c == '=' || c == ':' || c.is_whitespace());
                Some((folder, ids.split_whitespace().map(namespaced).collect()))
            })
            .collect(),
        None => [
            ("world0", &["minecraft:overworld", "*"][..]),
            ("world-1", &["minecraft:the_nether"]),
            ("world1", &["minecraft:the_end"]),
        ]
        .into_iter()
        .filter(|(folder, _)| exists(folder))
        .map(|(folder, ids)| {
            (
                folder.to_owned(),
                ids.iter().map(|s| (*s).to_owned()).collect(),
            )
        })
        .collect(),
    };
    let named = |id: &str| {
        map.iter()
            .rev()
            .find(|(_, ids)| ids.iter().any(|i| i == id))
            .map(|(f, _)| f.as_str())
    };
    let id = match dimension {
        Dimension::Overworld => "minecraft:overworld",
        Dimension::Nether => "minecraft:the_nether",
        Dimension::End => "minecraft:the_end",
    };
    match named(id)
        .filter(|folder| has_programs(folder))
        .or_else(|| named("*"))
    {
        Some(folder) if !folder.is_empty() => format!("/{folder}/"),
        _ => "/".to_owned(),
    }
}

fn namespaced(id: &str) -> String {
    if id == "*" || id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

impl Pack {
    pub(super) fn has(&self, name: &str) -> bool {
        let folder = &self.folder;
        let key = format!("{}{name}", &folder[1..]);
        self.source.contains(&format!("{folder}{name}.vsh"))
            && self.source.contains(&format!("{folder}{name}.fsh"))
            && self.enabled(&key)
    }

    pub(super) fn enabled(&self, key: &str) -> bool {
        programs::enabled(
            key,
            self.enable.iter().map(|(p, e)| (p.as_str(), e.as_str())),
            &self.declared,
            &self.values,
        )
    }

    pub(super) fn id_map(
        &self,
        path: &str,
        read: fn(
            &std::collections::HashMap<String, String>,
            &mut Vec<String>,
        ) -> Vec<crate::shaderpack::blockids::Entry>,
        notes: &mut Vec<String>,
    ) -> std::collections::HashMap<String, i32> {
        let mut ids = std::collections::HashMap::new();
        if let Some(text) = self.source.read_text(path) {
            let props = properties::parse_with_options(&text, &self.declared, &self.values);
            for entry in read(&props.other, notes) {
                ids.entry(entry.block).or_insert(i32::from(entry.id));
            }
        }
        ids
    }

    pub(super) fn compute_path(&self, name: &str) -> Option<String> {
        let path = format!("{}{name}.csh", self.folder);
        self.source.contains(&path).then_some(path)
    }

    pub(super) fn compute_array(&self, base: &str) -> Vec<String> {
        let mut names = Vec::new();
        if self.has_compute(base) {
            names.push(base.to_owned());
        }
        for letter in 'a'..='z' {
            let name = format!("{base}_{letter}");
            if self.compute_path(&name).is_none() {
                break;
            }
            if self.has_compute(&name) {
                names.push(name);
            }
        }
        names
    }

    pub(super) fn has_compute(&self, name: &str) -> bool {
        let Some(path) = self.compute_path(name) else {
            return false;
        };
        self.enabled(path.trim_start_matches('/').trim_end_matches(".csh"))
    }

    pub(super) fn stage(&mut self, program: &str, suffix: &str) -> Result<String, String> {
        let path = format!("{}{program}{suffix}", self.folder);
        self.stage_at(&path)
    }

    pub(super) fn stage_at(&mut self, path: &str) -> Result<String, String> {
        let path = path.to_owned();
        let expanded =
            include::expand(&self.source, &path, &mut self.cache).map_err(|e| e.to_string())?;
        let applied = options::apply(&expanded.text, &self.declared, &self.values);
        let mut defines = self.environment.clone();
        preprocess::preprocess(&applied, &mut defines).map_err(|e| {
            match expanded.locate(e.line as u32) {
                Some((file, line)) => format!("{file}:{line}: {}", e.message),
                None => format!("{path}: {e}"),
            }
        })
    }
}
