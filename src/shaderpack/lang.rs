use std::collections::HashMap;

use super::source::Source;

#[derive(Debug, Default)]
pub(crate) struct Lang {
    entries: HashMap<String, String>,
}

impl Lang {
    pub(crate) fn load(source: &Source) -> Lang {
        let Some(text) = source.read_text("/lang/en_US.lang") else {
            return Lang::default();
        };
        Lang::parse(&text)
    }

    pub(crate) fn parse(text: &str) -> Lang {
        let mut entries = HashMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            entries.insert(key.trim().to_owned(), strip_codes(value.trim()));
        }
        Lang { entries }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    pub(crate) fn option(&self, name: &str) -> Option<&str> {
        self.get(&format!("option.{name}"))
    }

    pub(crate) fn comment(&self, name: &str) -> Option<&str> {
        self.get(&format!("option.{name}.comment"))
    }

    pub(crate) fn value(&self, name: &str, value: &str) -> Option<&str> {
        self.get(&format!("value.{name}.{value}"))
    }

    pub(crate) fn screen(&self, id: &str) -> Option<&str> {
        self.get(&format!("screen.{id}"))
    }

    pub(crate) fn profile(&self, name: &str) -> Option<&str> {
        self.get(&format!("profile.{name}"))
    }
}

fn strip_codes(text: &str) -> String {
    if !text.contains('§') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

pub(crate) fn humanise(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for word in name.split('_') {
        if !out.is_empty() {
            out.push(' ');
        }
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.extend(chars.flat_map(char::to_lowercase));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
#shaders/lang/en_US.lang

profile.HIGH=High (Default)
option.SOLAS_BY_SEPTONIOUS=§6Solas Shader
option.VC_SCALE.comment=Lower values make clouds smaller.
screen.ATMOSPHERICS=Atmospherics
value.DISTANT_FADE_STYLE.0=Spherical
value.VC_AMOUNT.14.25=Middling
not an entry
";

    #[test]
    fn every_kind_of_key_is_readable() {
        let lang = Lang::parse(SAMPLE);
        assert_eq!(lang.profile("HIGH"), Some("High (Default)"));
        assert_eq!(lang.screen("ATMOSPHERICS"), Some("Atmospherics"));
        assert_eq!(lang.value("DISTANT_FADE_STYLE", "0"), Some("Spherical"));
        assert_eq!(
            lang.comment("VC_SCALE"),
            Some("Lower values make clouds smaller.")
        );
        assert_eq!(lang.option("MISSING"), None);
    }

    #[test]
    fn a_value_containing_a_dot_still_resolves() {
        assert_eq!(
            Lang::parse(SAMPLE).value("VC_AMOUNT", "14.25"),
            Some("Middling")
        );
    }

    #[test]
    fn formatting_codes_are_removed() {
        assert_eq!(
            Lang::parse(SAMPLE).option("SOLAS_BY_SEPTONIOUS"),
            Some("Solas Shader")
        );
        assert_eq!(strip_codes("§aA§lB§"), "AB");
        assert_eq!(strip_codes("plain"), "plain");
    }

    #[test]
    #[ignore = "needs a shader pack installed"]
    fn a_real_pack_names_its_own_options() {
        use super::super::{discover, features, options, properties, source::Source};

        for pack in discover::list() {
            let at = discover::dir().join(&pack.name);
            let Ok(source) = Source::open(&at) else {
                continue;
            };
            let lang = Lang::load(&source);
            if lang.is_empty() {
                println!("{}: ships no lang file", pack.name);
                continue;
            }

            let texts: Vec<String> = source
                .files()
                .into_iter()
                .filter(|f| {
                    [".glsl", ".vsh", ".fsh", ".gsh", ".csh"]
                        .iter()
                        .any(|e| f.ends_with(e))
                })
                .filter_map(|f| source.read_text(&f))
                .collect();
            let found = options::discover(texts.iter().map(String::as_str));

            let named = found
                .iter()
                .filter(|o| lang.option(&o.name).is_some())
                .count();
            let described = found
                .iter()
                .filter(|o| lang.comment(&o.name).is_some())
                .count();

            let props = properties::parse(
                &source.read_text("/shaders.properties").unwrap_or_default(),
                &mut features::base_defines(),
            );
            let screens = props.screens.keys().filter(|id| !id.is_empty());
            let titled = screens
                .clone()
                .filter(|id| lang.screen(id).is_some())
                .count();

            println!(
                "{}: {named}/{} options named, {described} described, \
                 {titled}/{} screens titled",
                pack.name,
                found.len(),
                screens.count(),
            );

            assert!(
                named * 2 > found.len(),
                "{}: only {named} of {} options are named",
                pack.name,
                found.len()
            );
        }
    }

    #[test]
    fn ids_are_only_humanised_when_the_pack_names_nothing() {
        assert_eq!(humanise("VOLUMETRIC_CLOUDS"), "Volumetric Clouds");
        assert_eq!(humanise("PBR"), "Pbr");
    }
}
