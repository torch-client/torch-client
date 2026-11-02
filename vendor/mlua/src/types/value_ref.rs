use std::fmt;
use std::os::raw::{c_int, c_void};

use super::XRc;
use crate::state::{RawLua, WeakLua};

#[derive(Clone)]
pub struct ValueRef {
    pub(crate) lua: WeakLua,
    pub(crate) index: c_int,
    pub(crate) index_count: Option<ValueRefIndex>,
}

#[derive(Clone)]
pub(crate) struct ValueRefIndex(pub(crate) XRc<c_int>);

impl From<c_int> for ValueRefIndex {
    #[inline]
    fn from(index: c_int) -> Self {
        ValueRefIndex(XRc::new(index))
    }
}

impl ValueRef {
    #[inline]
    pub(crate) fn new(lua: &RawLua, index: impl Into<ValueRefIndex>) -> Self {
        let index = index.into();
        ValueRef {
            lua: lua.weak().clone(),
            index: *index.0,
            index_count: Some(index),
        }
    }

    #[inline]
    pub(crate) fn to_pointer(&self) -> *const c_void {
        let lua = self.lua.lock();
        unsafe { ffi::lua_topointer(lua.ref_thread(), self.index) }
    }
}

impl fmt::Debug for ValueRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Ref({:p})", self.to_pointer())
    }
}

impl Drop for ValueRef {
    fn drop(&mut self) {
        if let Some(ValueRefIndex(index)) = self.index_count.take() {
            if XRc::into_inner(index).is_some() {
                if let Some(lua) = self.lua.try_lock() {
                    unsafe { lua.drop_ref(self) };
                }
            }
        }
    }
}

impl PartialEq for ValueRef {
    fn eq(&self, other: &Self) -> bool {
        assert!(
            self.lua == other.lua,
            "Lua instance passed Value created from a different main Lua state"
        );
        let lua = self.lua.lock();
        unsafe { ffi::lua_rawequal(lua.ref_thread(), self.index, other.index) == 1 }
    }
}
