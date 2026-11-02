use super::registry::Id;
use super::store;

pub fn active() -> bool {
    store().enabled(Id::Fullbright)
}
