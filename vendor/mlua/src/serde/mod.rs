use std::os::raw::c_void;

use serde::de::DeserializeOwned;
use serde::ser::Serialize;

use crate::error::Result;
use crate::private::Sealed;
use crate::state::Lua;
use crate::table::Table;
use crate::util::check_stack;
use crate::value::Value;

#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
pub trait LuaSerdeExt: Sealed {
    fn null(&self) -> Value;

    fn array_metatable(&self) -> Table;

    fn to_value<T: Serialize + ?Sized>(&self, t: &T) -> Result<Value>;

    fn to_value_with<T>(&self, t: &T, options: ser::Options) -> Result<Value>
    where
        T: Serialize + ?Sized;

    #[allow(clippy::wrong_self_convention)]
    fn from_value<T: DeserializeOwned>(&self, value: Value) -> Result<T>;

    #[allow(clippy::wrong_self_convention)]
    fn from_value_with<T: DeserializeOwned>(&self, value: Value, options: de::Options) -> Result<T>;
}

impl LuaSerdeExt for Lua {
    fn null(&self) -> Value {
        Value::NULL
    }

    fn array_metatable(&self) -> Table {
        let lua = self.lock();
        unsafe {
            push_array_metatable(lua.ref_thread());
            Table(lua.pop_ref_thread())
        }
    }

    fn to_value<T>(&self, t: &T) -> Result<Value>
    where
        T: Serialize + ?Sized,
    {
        t.serialize(ser::Serializer::new(self))
    }

    fn to_value_with<T>(&self, t: &T, options: ser::Options) -> Result<Value>
    where
        T: Serialize + ?Sized,
    {
        t.serialize(ser::Serializer::new_with_options(self, options))
    }

    fn from_value<T>(&self, value: Value) -> Result<T>
    where
        T: DeserializeOwned,
    {
        T::deserialize(de::Deserializer::new(value))
    }

    fn from_value_with<T>(&self, value: Value, options: de::Options) -> Result<T>
    where
        T: DeserializeOwned,
    {
        T::deserialize(de::Deserializer::new_with_options(value, options))
    }
}

pub(crate) unsafe fn init_metatables(state: *mut ffi::lua_State) -> Result<()> {
    check_stack(state, 2)?;
    protect_lua!(state, 0, 0, fn(state) {
        ffi::lua_createtable(state, 0, 1);

        ffi::lua_pushstring(state, cstr!("__metatable"));
        ffi::lua_pushboolean(state, 0);
        ffi::lua_rawset(state, -3);

        let array_metatable_key = &ARRAY_METATABLE_REGISTRY_KEY as *const u8 as *const c_void;
        ffi::lua_rawsetp(state, ffi::LUA_REGISTRYINDEX, array_metatable_key);
    })
}

pub(crate) unsafe fn push_array_metatable(state: *mut ffi::lua_State) {
    let array_metatable_key = &ARRAY_METATABLE_REGISTRY_KEY as *const u8 as *const c_void;
    ffi::lua_rawgetp(state, ffi::LUA_REGISTRYINDEX, array_metatable_key);
}

static ARRAY_METATABLE_REGISTRY_KEY: u8 = 0;

pub mod de;
pub mod ser;

#[doc(inline)]
pub use de::Deserializer;
#[doc(inline)]
pub use ser::Serializer;
