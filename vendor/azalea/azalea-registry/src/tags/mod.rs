use std::{collections::HashSet, ops::Deref};

use crate::Registry;

pub mod blocks;
pub mod entities;
pub mod fluids;
pub mod items;

#[derive(Clone, Debug)]
pub struct RegistryTag<R: Registry + 'static> {
    entries: Vec<R>,
}
impl<R: Registry + 'static> RegistryTag<R> {
    pub(crate) fn new(entries: Vec<R>) -> Self {
        debug_assert!(entries.is_sorted());

        Self { entries }
    }
}
impl<R: Registry + 'static> RegistryTag<R> {
    pub fn contains(&self, value: &R) -> bool {
        self.find(value).is_some()
    }

    pub fn remove(&mut self, value: &R) -> Option<R> {
        self.find(value).map(|index| self.entries.remove(index))
    }

    fn find(&self, value: &R) -> Option<usize> {
        if self.entries.len() > 64 {
            self.binary_search_find(value)
        } else {
            self.linear_search_find(value)
        }
    }
    fn linear_search_find(&self, value: &R) -> Option<usize> {
        self.entries.iter().position(|e| e == value)
    }
    fn binary_search_find(&self, value: &R) -> Option<usize> {
        self.entries.binary_search(value).ok()
    }

    pub fn into_hashset(&self) -> HashSet<R> {
        self.clone().into_iter().collect()
    }
}
impl<R: Registry> IntoIterator for RegistryTag<R> {
    type Item = R;

    type IntoIter = std::vec::IntoIter<R>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<R: Registry> From<RegistryTag<R>> for HashSet<R> {
    fn from(tag: RegistryTag<R>) -> Self {
        tag.into_hashset()
    }
}

impl<R: Registry> Deref for RegistryTag<R> {
    type Target = [R];

    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

impl<R: Registry> FromIterator<R> for RegistryTag<R> {
    fn from_iter<T: IntoIterator<Item = R>>(iter: T) -> Self {
        let mut entries = iter.into_iter().collect::<Vec<_>>();
        entries.sort();
        Self::new(entries)
    }
}
