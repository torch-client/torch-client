use crate::platform::cli::valid_username;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccountKind {
    #[default]
    Offline,
    Microsoft,
}

impl AccountKind {
    fn tag(self) -> &'static str {
        match self {
            AccountKind::Offline => "offline",
            AccountKind::Microsoft => "microsoft",
        }
    }

    fn from_tag(tag: &str) -> AccountKind {
        match tag {
            "microsoft" => AccountKind::Microsoft,
            _ => AccountKind::Offline,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AccountKind::Offline => "Offline Account",
            AccountKind::Microsoft => "Microsoft Account",
        }
    }

    pub fn editable(self) -> bool {
        self == AccountKind::Offline
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Account {
    pub name: String,
    pub kind: AccountKind,
    pub token: Option<String>,
    pub uuid: Option<String>,
    pub session: Option<String>,
    pub session_expires: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Accounts {
    pub entries: Vec<Account>,
    pub active: Option<usize>,
}

impl Accounts {
    pub fn get(&self, index: usize) -> Option<&Account> {
        self.entries.get(index)
    }

    pub fn active(&self) -> Option<&Account> {
        self.entries.get(self.active?)
    }

    pub fn activate(&mut self, index: usize) {
        if index < self.entries.len() {
            self.active = Some(index);
        }
    }

    pub fn add(&mut self, account: Account) {
        self.entries.push(account);
        self.active = Some(self.entries.len() - 1);
    }

    pub fn remove(&mut self, index: usize) {
        if index >= self.entries.len() {
            return;
        }
        self.entries.remove(index);
        self.active = match self.active {
            Some(active) if active == index => None,
            Some(active) if active > index => Some(active - 1),
            other => other,
        };
    }
}

pub fn load() -> Accounts {
    match crate::platform::storage::ACCOUNTS.load() {
        Some(text) => parse(&text),
        None => migrate_profile(),
    }
}

pub fn save(accounts: &Accounts) {
    crate::platform::storage::ACCOUNTS.store(&serialize(accounts));
}

pub fn active_username() -> Option<String> {
    load().active().map(|a| a.name.clone())
}

fn migrate_profile() -> Accounts {
    let Some(text) = crate::platform::storage::PROFILE.load() else {
        return Accounts::default();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Accounts::default();
    };
    let Some(name) = json.get("username").and_then(|n| n.as_str()) else {
        return Accounts::default();
    };
    if !valid_username(name) {
        return Accounts::default();
    }
    Accounts {
        entries: vec![Account {
            name: name.to_string(),
            kind: AccountKind::Offline,
            ..Account::default()
        }],
        active: Some(0),
    }
}

#[cfg(feature = "online_mode")]
pub fn update_tokens(uuid: &str, token: &str, session: &str, session_expires: u64) {
    let mut accounts = load();
    let Some(entry) = accounts
        .entries
        .iter_mut()
        .find(|a| a.kind == AccountKind::Microsoft && a.uuid.as_deref() == Some(uuid))
    else {
        return;
    };
    let unchanged = entry.token.as_deref() == Some(token)
        && entry.session.as_deref() == Some(session)
        && entry.session_expires == Some(session_expires);
    if unchanged {
        return;
    }
    entry.token = Some(token.to_string());
    entry.session = Some(session.to_string());
    entry.session_expires = Some(session_expires);
    save(&accounts);
}

fn parse(text: &str) -> Accounts {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
        return Accounts::default();
    };
    let kept: Vec<(usize, Account)> = json
        .get("accounts")
        .and_then(|a| a.as_array())
        .map(|array| {
            array
                .iter()
                .enumerate()
                .filter_map(|(index, v)| {
                    let name = v.get("name")?.as_str()?.trim().to_string();
                    let kind =
                        AccountKind::from_tag(v.get("type").and_then(|t| t.as_str()).unwrap_or(""));
                    if name.is_empty() || (kind.editable() && !valid_username(&name)) {
                        return None;
                    }
                    let string = |key: &str| {
                        v.get(key)
                            .and_then(|t| t.as_str())
                            .map(str::trim)
                            .filter(|t| !t.is_empty())
                            .map(str::to_string)
                    };
                    let token = string("token");
                    let uuid = string("uuid");
                    if kind == AccountKind::Microsoft && token.is_none() {
                        return None;
                    }
                    let session = string("session");
                    let session_expires = v.get("session_expires").and_then(|e| e.as_u64());
                    let (session, session_expires) = match (session, session_expires) {
                        (Some(session), Some(expires)) => (Some(session), Some(expires)),
                        _ => (None, None),
                    };
                    Some((
                        index,
                        Account {
                            name,
                            kind,
                            token,
                            uuid,
                            session,
                            session_expires,
                        },
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let active = json
        .get("active")
        .and_then(|a| a.as_u64())
        .map(|a| a as usize)
        .and_then(|a| kept.iter().position(|(index, _)| *index == a));
    let entries = kept.into_iter().map(|(_, account)| account).collect();
    Accounts { entries, active }
}

fn serialize(accounts: &Accounts) -> String {
    let array: Vec<serde_json::Value> = accounts
        .entries
        .iter()
        .map(|a| {
            let mut entry = serde_json::json!({
                "name": a.name,
                "type": a.kind.tag(),
            });
            let map = entry.as_object_mut().expect("json! built an object");
            for (key, value) in [
                ("token", &a.token),
                ("uuid", &a.uuid),
                ("session", &a.session),
            ] {
                if let Some(value) = value {
                    map.insert(key.to_string(), serde_json::Value::String(value.clone()));
                }
            }
            if let Some(expires) = a.session_expires {
                map.insert("session_expires".to_string(), expires.into());
            }
            entry
        })
        .collect();
    let json = serde_json::json!({
        "accounts": array,
        "active": accounts.active,
    });
    let mut text = serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string());
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offline(name: &str) -> Account {
        Account {
            name: name.into(),
            kind: AccountKind::Offline,
            ..Account::default()
        }
    }

    fn microsoft(name: &str) -> Account {
        Account {
            name: name.into(),
            kind: AccountKind::Microsoft,
            token: Some("refresh-token".into()),
            uuid: Some("069a79f4-44e9-4726-a5be-fca90e38aaf5".into()),
            session: Some("session-token".into()),
            session_expires: Some(1_800_000_000),
        }
    }

    #[test]
    fn round_trips_through_json() {
        let accounts = Accounts {
            entries: vec![offline("Player"), microsoft("Notch")],
            active: Some(1),
        };
        assert_eq!(parse(&serialize(&accounts)), accounts);
    }

    #[test]
    fn an_offline_account_writes_no_extra_fields() {
        let text = serialize(&Accounts {
            entries: vec![offline("Player")],
            active: Some(0),
        });
        assert!(!text.contains("token"), "{text}");
        assert!(!text.contains("uuid"), "{text}");
    }

    #[test]
    fn half_a_session_reads_as_none() {
        let one = parse(
            "{\"accounts\":[{\"name\":\"Notch\",\"type\":\"microsoft\",\"token\":\"t\",\
             \"session\":\"s\"}],\"active\":0}",
        );
        assert_eq!(one.entries.len(), 1);
        assert_eq!(one.entries[0].session, None);
        assert_eq!(one.entries[0].session_expires, None);

        let other = parse(
            "{\"accounts\":[{\"name\":\"Notch\",\"type\":\"microsoft\",\"token\":\"t\",\
             \"session_expires\":1800000000}],\"active\":0}",
        );
        assert_eq!(other.entries[0].session, None);
        assert_eq!(other.entries[0].session_expires, None);
    }

    #[test]
    fn a_microsoft_account_without_a_token_is_dropped() {
        let parsed = parse(
            "{\"accounts\":[{\"name\":\"Notch\",\"type\":\"microsoft\"},\
             {\"name\":\"Player\",\"type\":\"offline\"}],\"active\":0}",
        );
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].kind, AccountKind::Offline);
        assert_eq!(parsed.active, None);
    }

    #[test]
    fn malformed_input_reads_as_empty() {
        assert_eq!(parse(""), Accounts::default());
        assert_eq!(parse("[]"), Accounts::default());
        assert_eq!(
            parse("{\"accounts\":[{\"type\":\"offline\"}]}"),
            Accounts::default()
        );
    }

    #[test]
    fn an_unusable_offline_name_is_dropped_but_a_microsoft_one_is_not() {
        let parsed = parse(
            "{\"accounts\":[{\"name\":\"no\",\"type\":\"offline\"},\
             {\"name\":\"a name with spaces\",\"type\":\"microsoft\",\"token\":\"t\"}],\
             \"active\":1}",
        );
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].kind, AccountKind::Microsoft);
        assert_eq!(parsed.active, Some(0));
    }

    #[test]
    fn removal_tracks_the_active_index() {
        let mut accounts = Accounts {
            entries: vec![offline("One"), offline("Two"), offline("Three")],
            active: Some(2),
        };
        accounts.remove(0);
        assert_eq!(accounts.active, Some(1));
        assert_eq!(accounts.active().unwrap().name, "Three");
        accounts.remove(1);
        assert_eq!(accounts.active, None);
    }

    #[test]
    fn an_out_of_range_index_is_ignored() {
        let mut accounts = Accounts {
            entries: vec![offline("One")],
            active: None,
        };
        accounts.activate(7);
        assert_eq!(accounts.active, None);
        assert_eq!(parse("{\"accounts\":[],\"active\":3}").active, None);
    }
}
