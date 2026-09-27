use super::registry::{Id, Kind, module};
use super::value::Value;
use crate::gui::keybinds::Bound;

pub struct Store;

static STORE: Store = Store;

#[inline(always)]
pub fn store() -> &'static Store {
    &STORE
}

pub fn save() {}

impl Store {
    #[inline(always)]
    pub fn enabled(&self, _: Id) -> bool {
        false
    }

    #[inline(always)]
    pub fn enabled_at(&self, _: usize) -> bool {
        false
    }

    #[inline(always)]
    pub fn armed(&self, _: Id) -> bool {
        false
    }

    #[inline(always)]
    pub fn set_enabled(&self, _: Id, _: bool) {}

    #[inline(always)]
    pub fn toggle_at(&self, _: usize) {}

    #[inline(always)]
    pub fn toggle_count(&self, _: usize) -> u32 {
        0
    }

    #[inline(always)]
    pub fn bind_at(&self, _: usize) -> Bound {
        Bound::Unbound
    }

    #[inline(always)]
    fn default(&self, id: Id, n: usize) -> Value {
        Value::default_of(module(id).settings[n].kind)
    }

    #[inline(always)]
    pub fn num(&self, id: Id, n: usize) -> f32 {
        match self.default(id, n) {
            Value::Num(v) => v,
            _ => 0.0,
        }
    }

    #[inline(always)]
    pub fn flag(&self, id: Id, n: usize) -> bool {
        matches!(self.default(id, n), Value::Bool(true))
    }

    #[inline(always)]
    pub fn range(&self, id: Id, n: usize) -> (f32, f32) {
        match self.default(id, n) {
            Value::Range(lo, hi) => (lo, hi),
            _ => (0.0, 0.0),
        }
    }

    #[inline(always)]
    pub fn choice(&self, id: Id, n: usize) -> u8 {
        match self.default(id, n) {
            Value::Choice(c) => c,
            _ => 0,
        }
    }

    pub fn text(&self, id: Id, n: usize) -> String {
        match module(id).settings[n].kind {
            Kind::Text { default, .. } => default.to_string(),
            _ => String::new(),
        }
    }
}
