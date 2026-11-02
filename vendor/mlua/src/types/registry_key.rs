use std::hash::{Hash, Hasher};
use std::os::raw::c_int;
use std::sync::Arc;
use std::{fmt, mem, ptr};

use parking_lot::Mutex;

pub struct RegistryKey {
    pub(crate) registry_id: i32,
    pub(crate) unref_list: Arc<Mutex<Option<Vec<c_int>>>>,
}

impl fmt::Debug for RegistryKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RegistryKey({})", self.id())
    }
}

impl Hash for RegistryKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id().hash(state)
    }
}

impl PartialEq for RegistryKey {
    fn eq(&self, other: &RegistryKey) -> bool {
        self.id() == other.id() && Arc::ptr_eq(&self.unref_list, &other.unref_list)
    }
}

impl Eq for RegistryKey {}

impl Drop for RegistryKey {
    fn drop(&mut self) {
        let registry_id = self.id();
        if registry_id > ffi::LUA_REFNIL {
            let mut unref_list = self.unref_list.lock();
            if let Some(list) = unref_list.as_mut() {
                list.push(registry_id);
            }
        }
    }
}

impl RegistryKey {
    pub(crate) const fn new(id: c_int, unref_list: Arc<Mutex<Option<Vec<c_int>>>>) -> Self {
        RegistryKey {
            registry_id: id,
            unref_list,
        }
    }

    #[inline(always)]
    pub fn id(&self) -> c_int {
        self.registry_id
    }

    #[inline(always)]
    pub(crate) fn set_id(&mut self, id: c_int) {
        self.registry_id = id;
    }

    pub(crate) fn take(self) -> i32 {
        let registry_id = self.id();
        unsafe {
            ptr::read(&self.unref_list);
            mem::forget(self);
        }
        registry_id
    }
}

#[cfg(test)]
mod assertions {
    use super::*;

    static_assertions::assert_impl_all!(RegistryKey: Send, Sync);
}
