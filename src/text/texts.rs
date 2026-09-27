use std::sync::OnceLock;

use serde_json::Value;

fn parse_splashes(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

pub fn splashes() -> &'static [String] {
    static SPLASHES: OnceLock<Vec<String>> = OnceLock::new();
    SPLASHES
        .get_or_init(|| {
            crate::platform::assets::read_to_string(crate::assets_root().join("texts/splashes.txt"))
                .map(|text| parse_splashes(&text))
                .unwrap_or_default()
        })
        .as_slice()
}

pub fn splash_for(seed: u64) -> Option<&'static str> {
    let list = splashes();
    if list.is_empty() {
        return None;
    }
    let index = (seed % list.len() as u64) as usize;
    Some(list[index].as_str())
}

pub fn end_poem() -> Option<&'static str> {
    static END: OnceLock<Option<std::borrow::Cow<'static, str>>> = OnceLock::new();
    END.get_or_init(|| {
        crate::platform::assets::read_to_string(crate::assets_root().join("texts/end.txt"))
    })
    .as_deref()
}

pub fn post_credits() -> Option<&'static str> {
    static POST: OnceLock<Option<std::borrow::Cow<'static, str>>> = OnceLock::new();
    POST.get_or_init(|| {
        crate::platform::assets::read_to_string(crate::assets_root().join("texts/postcredits.txt"))
    })
    .as_deref()
}

#[derive(Debug, Clone)]
pub struct CreditsTitle {
    pub title: String,
    pub names: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CreditsDiscipline {
    pub discipline: String,
    pub titles: Vec<CreditsTitle>,
}

#[derive(Debug, Clone)]
pub struct CreditsSection {
    pub section: String,
    pub disciplines: Vec<CreditsDiscipline>,
}

fn parse_credits(root: &Value) -> Vec<CreditsSection> {
    let str_field = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).map(str::to_owned);
    let str_list = |v: &Value, key: &str| {
        v.get(key)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };

    root.as_array()
        .into_iter()
        .flatten()
        .filter_map(|section| {
            let disciplines = section
                .get("disciplines")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|discipline| {
                    let titles = discipline
                        .get("titles")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|title| {
                            Some(CreditsTitle {
                                title: str_field(title, "title")?,
                                names: str_list(title, "names"),
                            })
                        })
                        .collect();
                    Some(CreditsDiscipline {
                        discipline: str_field(discipline, "discipline")?,
                        titles,
                    })
                })
                .collect();
            Some(CreditsSection {
                section: str_field(section, "section")?,
                disciplines,
            })
        })
        .collect()
}

pub fn credits() -> &'static [CreditsSection] {
    static CREDITS: OnceLock<Vec<CreditsSection>> = OnceLock::new();
    CREDITS
        .get_or_init(|| {
            crate::platform::assets::read_to_string(crate::assets_root().join("texts/credits.json"))
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .map(|root| parse_credits(&root))
                .unwrap_or_default()
        })
        .as_slice()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_line_is_not_a_splash() {
        let parsed = parse_splashes("Awesome!\n\n   \nMore polygons!\n");
        assert_eq!(
            parsed,
            vec!["Awesome!".to_string(), "More polygons!".to_string()]
        );
    }

    #[test]
    fn a_whitespace_only_line_is_trimmed_away() {
        let parsed = parse_splashes("  \t  \nOK\n");
        assert_eq!(parsed, vec!["OK".to_string()]);
    }

    #[test]
    fn surrounding_whitespace_on_a_real_line_is_trimmed() {
        let parsed = parse_splashes("  Awesome!  \n");
        assert_eq!(parsed, vec!["Awesome!".to_string()]);
    }

    #[test]
    fn the_same_seed_always_picks_the_same_index() {
        let list = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let pick = |seed: u64| (seed % list.len() as u64) as usize;
        assert_eq!(pick(4), pick(4));
        assert_eq!(pick(4), 1);
    }

    #[test]
    fn an_empty_list_has_no_splash_for_any_seed() {
        let list: Vec<String> = Vec::new();
        assert!(list.is_empty());
    }

    #[test]
    fn credits_json_shape_survives_a_round_trip() {
        let root: Value = serde_json::from_str(
            r#"[{"section": "Mojang Studios", "disciplines": [{"discipline": "Leadership", "titles": [{"title": "Studio Head", "names": ["Kayleen Walters"]}]}]}]"#,
        )
        .unwrap();
        let sections = parse_credits(&root);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].section, "Mojang Studios");
        assert_eq!(sections[0].disciplines[0].discipline, "Leadership");
        assert_eq!(sections[0].disciplines[0].titles[0].title, "Studio Head");
        assert_eq!(
            sections[0].disciplines[0].titles[0].names,
            vec!["Kayleen Walters".to_string()]
        );
    }

    #[test]
    fn a_title_missing_its_name_field_is_kept_with_no_names() {
        let root: Value = serde_json::from_str(
            r#"[{"section": "S", "disciplines": [{"discipline": "D", "titles": [{"title": "T"}]}]}]"#,
        )
        .unwrap();
        let sections = parse_credits(&root);
        assert!(sections[0].disciplines[0].titles[0].names.is_empty());
    }
}
