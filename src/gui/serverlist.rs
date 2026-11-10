#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerEntry {
    pub name: String,
    pub address: String,
    pub resource_packs: PackPolicy,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PackPolicy {
    Enabled,
    Disabled,
    #[default]
    Prompt,
}

impl PackPolicy {
    fn key(self) -> &'static str {
        match self {
            PackPolicy::Enabled => "enabled",
            PackPolicy::Disabled => "disabled",
            PackPolicy::Prompt => "prompt",
        }
    }

    fn from_key(key: &str) -> Option<PackPolicy> {
        [
            PackPolicy::Enabled,
            PackPolicy::Disabled,
            PackPolicy::Prompt,
        ]
        .into_iter()
        .find(|p| p.key() == key)
    }

    pub fn next(self) -> PackPolicy {
        match self {
            PackPolicy::Enabled => PackPolicy::Disabled,
            PackPolicy::Disabled => PackPolicy::Prompt,
            PackPolicy::Prompt => PackPolicy::Enabled,
        }
    }

    pub fn caption(self) -> String {
        crate::gui::tooltip::translate(&format!("manageServer.resourcePack.{}", self.key()), &[])
    }
}

pub fn pack_policy(address: &str) -> PackPolicy {
    load()
        .into_iter()
        .find(|e| e.address == address)
        .map(|e| e.resource_packs)
        .unwrap_or_default()
}

pub fn load() -> Vec<ServerEntry> {
    let Some(text) = crate::platform::storage::SERVERS.load() else {
        return Vec::new();
    };
    parse(&text)
}

pub fn save(entries: &[ServerEntry]) {
    crate::platform::storage::SERVERS.store(&serialize(entries));
}

fn parse(text: &str) -> Vec<ServerEntry> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let Some(array) = json.as_array() else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|v| {
            let address = v.get("address")?.as_str()?.trim().to_string();
            if address.is_empty() {
                return None;
            }
            let name = v
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let resource_packs = v
                .get("resourcePacks")
                .and_then(|p| p.as_str())
                .and_then(PackPolicy::from_key)
                .unwrap_or_default();
            Some(ServerEntry {
                name,
                address,
                resource_packs,
            })
        })
        .collect()
}

fn serialize(entries: &[ServerEntry]) -> String {
    let array: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "name": e.name,
                "address": e.address,
                "resourcePacks": e.resource_packs.key(),
            })
        })
        .collect();
    let mut text = serde_json::to_string_pretty(&serde_json::Value::Array(array))
        .unwrap_or_else(|_| "[]".to_string());
    text.push('\n');
    text
}

pub fn display_name(entry: &ServerEntry) -> &str {
    if entry.name.trim().is_empty() {
        &entry.address
    } else {
        &entry.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let entries = vec![
            ServerEntry {
                name: "Local".into(),
                address: "127.0.0.1:25565".into(),
                resource_packs: PackPolicy::Enabled,
            },
            ServerEntry {
                name: String::new(),
                address: "example.com".into(),
                resource_packs: PackPolicy::Prompt,
            },
        ];
        assert_eq!(parse(&serialize(&entries)), entries);
    }

    #[test]
    fn a_missing_policy_is_prompt() {
        let entries = parse(r#"[{"name": "Old", "address": "example.com"}]"#);
        assert_eq!(entries[0].resource_packs, PackPolicy::Prompt);
    }

    #[test]
    fn malformed_input_reads_as_empty() {
        assert!(parse("").is_empty());
        assert!(parse("{}").is_empty());
        assert!(parse("[{\"name\":\"no address\"}]").is_empty());
        assert!(parse("[{\"address\":\"   \"}]").is_empty());
    }

    #[test]
    fn a_nameless_entry_shows_its_address() {
        let e = ServerEntry {
            name: "  ".into(),
            address: "example.com".into(),
            ..Default::default()
        };
        assert_eq!(display_name(&e), "example.com");
    }
}
