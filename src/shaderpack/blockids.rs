use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) id: u16,
    pub(crate) block: String,
    pub(crate) properties: Vec<(String, Vec<String>)>,
}

pub(crate) fn entries(properties: &HashMap<String, String>, notes: &mut Vec<String>) -> Vec<Entry> {
    entries_under(properties, "block.", notes)
}

pub(crate) fn item_entries(
    properties: &HashMap<String, String>,
    notes: &mut Vec<String>,
) -> Vec<Entry> {
    entries_under(properties, "item.", notes)
}

pub(crate) fn entity_entries(
    properties: &HashMap<String, String>,
    notes: &mut Vec<String>,
) -> Vec<Entry> {
    entries_under(properties, "entity.", notes)
}

fn entries_under(
    properties: &HashMap<String, String>,
    prefix: &str,
    notes: &mut Vec<String>,
) -> Vec<Entry> {
    let mut keys: Vec<(u16, &str)> = Vec::new();
    for (key, value) in properties {
        let Some(id) = key.strip_prefix(prefix) else {
            continue;
        };
        match id.parse::<u16>() {
            Ok(id) if id != u16::MAX => keys.push((id, value)),
            _ => notes.push(format!("{key}: not an id below {}", u16::MAX)),
        }
    }
    keys.sort_unstable_by_key(|(id, _)| *id);

    let mut out = Vec::new();
    for (id, value) in keys {
        for word in value.split_whitespace() {
            if let Some(entry) = parse_entry(id, word) {
                out.push(entry);
            }
        }
    }
    out
}

fn parse_entry(id: u16, word: &str) -> Option<Entry> {
    let mut parts = word.split(':');
    let first = parts.next()?;
    let rest: Vec<&str> = parts.collect();
    let (block, properties) = match rest.first() {
        Some(second) if !second.contains('=') => {
            if first != "minecraft" {
                return None;
            }
            (*second, &rest[1..])
        }
        _ => (first, &rest[..]),
    };
    if block.is_empty() || block.contains('=') {
        return None;
    }
    let properties = properties
        .iter()
        .map(|p| {
            let (key, values) = p.split_once('=')?;
            let values: Vec<String> = values
                .split(',')
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
                .collect();
            (!key.is_empty() && !values.is_empty()).then(|| (key.to_owned(), values))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Entry {
        id,
        block: block.to_owned(),
        properties,
    })
}

impl Entry {
    pub(crate) fn matches(&self, block: &str, properties: &HashMap<&str, &str>) -> bool {
        self.block == block
            && self.properties.iter().all(|(key, values)| {
                properties
                    .get(key.as_str())
                    .is_some_and(|v| values.iter().any(|w| w == v))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn entries_are_read_in_id_order() {
        let mut notes = Vec::new();
        let found = entries(
            &props(&[
                ("block.10304", "short_grass minecraft:fern byg:beach_grass"),
                ("block.10001", "water"),
                ("block.10020", "wheat:age=6,7 minecraft:oak_log:axis=y"),
                ("block.huge", "stone"),
            ]),
            &mut notes,
        );
        let names: Vec<(u16, &str)> = found.iter().map(|e| (e.id, e.block.as_str())).collect();
        assert_eq!(
            names,
            [
                (10001, "water"),
                (10020, "wheat"),
                (10020, "oak_log"),
                (10304, "short_grass"),
                (10304, "fern")
            ]
        );
        assert_eq!(
            found[1].properties,
            [("age".to_string(), vec!["6".to_string(), "7".to_string()])]
        );
        assert_eq!(notes.len(), 1, "{notes:?}");
    }

    #[test]
    fn entries_match_their_states() {
        let entry = parse_entry(1, "wheat:age=6,7").expect("valid");
        let state = |age: &'static str| HashMap::from([("age", age)]);
        assert!(entry.matches("wheat", &state("7")));
        assert!(!entry.matches("wheat", &state("5")));
        assert!(!entry.matches("carrots", &state("7")));
        assert!(
            parse_entry(1, "stone")
                .expect("valid")
                .matches("stone", &HashMap::new())
        );
        assert_eq!(parse_entry(1, "wheat:age"), None);
    }
}
