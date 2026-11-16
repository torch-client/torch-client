use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{LazyLock, PoisonError, RwLock};

use crate::platform::storage::FRIENDS as SLOT;

struct Friends {
    names: RwLock<Vec<String>>,
    any: AtomicBool,
}

static FRIENDS: LazyLock<Friends> = LazyLock::new(|| {
    let names: Vec<String> = SLOT
        .load()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    Friends {
        any: AtomicBool::new(!names.is_empty()),
        names: RwLock::new(names),
    }
});

pub fn load() {
    LazyLock::force(&FRIENDS);
}

pub fn valid_name(name: &str) -> bool {
    (1..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

static ATTACK: AtomicBool = AtomicBool::new(false);

pub fn set_attack(on: bool) {
    ATTACK.store(on, Relaxed);
}

pub fn is_friend(name: &str) -> bool {
    let f = &*FRIENDS;
    f.any.load(Relaxed)
        && f.names
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|n| n.eq_ignore_ascii_case(name))
}

pub fn guarding() -> bool {
    !ATTACK.load(Relaxed) && FRIENDS.any.load(Relaxed)
}

pub fn protected(name: &str) -> bool {
    guarding() && is_friend(name)
}

pub fn list() -> Vec<String> {
    FRIENDS
        .names
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

pub fn add(name: &str) -> bool {
    edit(|names| {
        if names.iter().any(|n| n.eq_ignore_ascii_case(name)) {
            return false;
        }
        names.push(name.to_owned());
        true
    })
}

pub fn remove(name: &str) -> Option<String> {
    let mut gone = None;
    edit(|names| {
        let at = names.iter().position(|n| n.eq_ignore_ascii_case(name));
        gone = at.map(|i| names.remove(i));
        gone.is_some()
    });
    gone
}

fn edit(f: impl FnOnce(&mut Vec<String>) -> bool) -> bool {
    let friends = &*FRIENDS;
    let mut names = friends
        .names
        .write()
        .unwrap_or_else(PoisonError::into_inner);
    if !f(&mut names) {
        return false;
    }
    friends.any.store(!names.is_empty(), Relaxed);
    let text = serde_json::to_string(&*names).unwrap_or_default();
    drop(names);
    SLOT.store(&text);
    true
}
