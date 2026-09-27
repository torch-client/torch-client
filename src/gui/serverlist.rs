use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerEntry {
    pub name: String,
    pub address: String,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn path() -> PathBuf {
    crate::platform::storage::SERVERS.path()
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
            Some(ServerEntry { name, address })
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

pub fn is_temporary(path: &Path) -> bool {
    if cfg!(target_os = "windows") {
        return path.starts_with(std::env::temp_dir());
    }
    path.starts_with("/tmp")
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
            },
            ServerEntry {
                name: String::new(),
                address: "example.com".into(),
            },
        ];
        assert_eq!(parse(&serialize(&entries)), entries);
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
        };
        assert_eq!(display_name(&e), "example.com");
    }
}
