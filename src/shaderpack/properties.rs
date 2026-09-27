use std::collections::{HashMap, HashSet};

use super::preprocess::{Defines, preprocess};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Element {
    Option(String),
    Link(String),
    Empty,
    Profile,
    Rest,
}

impl Element {
    fn parse(token: &str) -> Option<Element> {
        Some(match token {
            "" => return None,
            "<empty>" => Element::Empty,
            "<profile>" => Element::Profile,
            "*" => Element::Rest,
            _ => {
                if let Some(inner) = token.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
                    Element::Link(inner.to_owned())
                } else {
                    Element::Option(token.to_owned())
                }
            }
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Profile {
    pub(crate) tokens: Vec<String>,
}

pub(crate) const ROOT_SCREEN: &str = "";

#[derive(Debug, Default)]
pub(crate) struct Properties {
    pub(crate) screens: HashMap<String, Vec<Element>>,
    pub(crate) columns: HashMap<String, u32>,
    sliders: HashSet<String>,
    pub(crate) profiles: Vec<(String, Profile)>,
    pub(crate) other: HashMap<String, String>,
    pub(crate) unknown: Vec<String>,
}

impl Properties {
    pub(crate) fn is_slider(&self, option: &str) -> bool {
        self.sliders.contains(option)
    }

    pub(crate) fn columns(&self, screen: &str) -> Option<u32> {
        self.columns.get(screen).copied()
    }

    pub(crate) fn features(&self, key: &str) -> impl Iterator<Item = &str> {
        self.other
            .get(key)
            .into_iter()
            .flat_map(|list| list.split_whitespace())
    }
}

pub(crate) fn parse(text: &str, defines: &mut Defines) -> Properties {
    let cooked = preprocess(text, defines).unwrap_or_else(|_| text.to_owned());
    let raw: Vec<&str> = text.lines().collect();

    let mut out = Properties::default();

    for (index, line) in cooked.lines().enumerate() {
        let Some((key, value)) = entry(line) else {
            continue;
        };
        let raw_value = || {
            raw.get(index)
                .and_then(|line| entry(line))
                .map_or(value, |(_, v)| v)
        };

        match split_key(key) {
            ("screen", None) => {
                out.screens
                    .insert(ROOT_SCREEN.to_owned(), elements(raw_value()));
            }
            ("screen", Some("columns")) => {
                store_columns(&mut out, ROOT_SCREEN, value, key);
            }
            ("screen", Some(rest)) => match rest.strip_suffix(".columns") {
                Some(id) => store_columns(&mut out, id, value, key),
                None => {
                    out.screens.insert(rest.to_owned(), elements(raw_value()));
                }
            },
            ("sliders", None) => {
                out.sliders
                    .extend(raw_value().split_whitespace().map(str::to_owned));
            }
            ("profile", Some(name)) => {
                let tokens = raw_value().split_whitespace().map(str::to_owned).collect();
                out.profiles.push((name.to_owned(), Profile { tokens }));
            }
            _ => {
                out.other.insert(key.to_owned(), value.to_owned());
            }
        }
    }

    out
}

fn store_columns(out: &mut Properties, screen: &str, value: &str, key: &str) {
    match value.trim().parse() {
        Ok(n) => {
            out.columns.insert(screen.to_owned(), n);
        }
        Err(_) => out.unknown.push(key.to_owned()),
    }
}

fn entry(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
        return None;
    }
    let at = line.find(['=', ':'])?;
    let key = line[..at].trim();
    if key.is_empty() {
        return None;
    }
    Some((key, line[at + 1..].trim()))
}

fn split_key(key: &str) -> (&str, Option<&str>) {
    match key.split_once('.') {
        Some((head, rest)) => (head, Some(rest)),
        None => (key, None),
    }
}

fn elements(value: &str) -> Vec<Element> {
    value
        .split_whitespace()
        .filter_map(Element::parse)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(text: &str) -> Properties {
        parse(text, &mut Defines::new())
    }

    #[test]
    fn a_screen_is_read_as_ordered_elements() {
        let props = parsed("screen=A <empty> [SUB] <profile> * B\n");
        assert_eq!(
            props.screens[ROOT_SCREEN],
            vec![
                Element::Option("A".into()),
                Element::Empty,
                Element::Link("SUB".into()),
                Element::Profile,
                Element::Rest,
                Element::Option("B".into()),
            ]
        );
    }

    #[test]
    fn screen_ids_and_their_column_counts_do_not_collide() {
        let props = parsed("screen.columns=1\nscreen.A.B=X\nscreen.A.B.columns=3\n");
        assert_eq!(props.columns(ROOT_SCREEN), Some(1));
        assert_eq!(props.screens["A.B"], vec![Element::Option("X".into())]);
        assert_eq!(props.columns("A.B"), Some(3));
    }

    #[test]
    fn sliders_and_profiles_are_collected() {
        let props = parsed("sliders=A B\nprofile.LOW=!X Y=2 profile.BASE\n");
        assert!(props.is_slider("A") && props.is_slider("B"));
        assert!(!props.is_slider("C"));
        assert_eq!(props.profiles.len(), 1);
        assert_eq!(props.profiles[0].0, "LOW");
        assert_eq!(props.profiles[0].1.tokens, ["!X", "Y=2", "profile.BASE"]);
    }

    #[test]
    fn the_file_is_preprocessed_before_it_is_parsed() {
        let text = "#if N == 128\nimage.a=small\n#elif N == 256\nimage.a=big\n#endif\n";
        let mut defines = Defines::new();
        defines.define("N", "256");
        assert_eq!(parse(text, &mut defines).other["image.a"], "big");
    }

    #[test]
    fn a_continued_value_is_one_value() {
        let props = parsed("uniform.float.x=a + \\\n    b\n");
        assert_eq!(props.other["uniform.float.x"], "a +     b");
    }

    #[test]
    fn unknown_and_malformed_keys_are_survivable() {
        let props = parsed(
            "#comment\nbeacon.beam.depth=true\nprogram.world0/deferred..enabled=SSAO\n\
             screen.columns=lots\n",
        );
        assert_eq!(props.other["beacon.beam.depth"], "true");
        assert!(props.other.contains_key("program.world0/deferred..enabled"));
        assert_eq!(props.unknown, ["screen.columns"]);
        assert_eq!(props.columns(ROOT_SCREEN), None);
    }

    #[test]
    #[ignore = "needs a shader pack installed"]
    fn a_real_pack_parses() {
        use super::super::{discover, features, source::Source};

        for pack in discover::list() {
            let at = discover::dir().join(&pack.name);
            let Ok(source) = Source::open(&at) else {
                continue;
            };
            let Some(text) = source.read_text("/shaders.properties") else {
                continue;
            };

            let mut defines = features::base_defines();
            let props = parse(&text, &mut defines);

            let placed: usize = props.screens.values().map(Vec::len).sum();
            println!(
                "{}: {} screens holding {placed} elements, {} sliders, {} profiles, \
                 {} other keys, {} unreadable",
                pack.name,
                props.screens.len(),
                props.sliders.len(),
                props.profiles.len(),
                props.other.len(),
                props.unknown.len(),
            );
            for key in &props.unknown {
                println!("    unreadable: {key}");
            }

            assert!(
                props.screens.contains_key(ROOT_SCREEN),
                "{}: no root screen",
                pack.name
            );
            for (screen, elements) in &props.screens {
                for element in elements {
                    if let Element::Link(id) = element {
                        assert!(
                            props.screens.contains_key(id),
                            "{}: screen {screen:?} links to missing {id:?}",
                            pack.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn required_features_are_readable_as_a_list() {
        let props = parsed("iris.features.required=CUSTOM_IMAGES SSBO\n");
        let names: Vec<&str> = props.features("iris.features.required").collect();
        assert_eq!(names, ["CUSTOM_IMAGES", "SSBO"]);
    }
}
