use std::any::TypeId;
use std::ffi::CStr;
use std::fmt;
use std::hash::Hash;
use std::os::raw::{c_char, c_void};
use std::string::String as StdString;

use crate::error::{Error, Result};
use crate::function::Function;
use crate::state::Lua;
use crate::string::String;
use crate::table::{Table, TablePairs};
use crate::traits::{FromLua, FromLuaMulti, IntoLua, IntoLuaMulti};
use crate::types::{MaybeSend, ValueRef};
use crate::util::{check_stack, get_userdata, push_string, short_type_name, take_userdata, StackGuard};
use crate::value::Value;

#[cfg(feature = "async")]
use std::future::Future;

#[cfg(feature = "serde")]
use {
    serde::ser::{self, Serialize, Serializer},
    std::result::Result as StdResult,
};

pub(crate) use cell::UserDataStorage;
pub use r#ref::{UserDataRef, UserDataRefMut};
pub use registry::UserDataRegistry;
pub(crate) use registry::{RawUserDataRegistry, UserDataProxy};
pub(crate) use util::{
    borrow_userdata_scoped, borrow_userdata_scoped_mut, collect_userdata, init_userdata_metatable,
    TypeIdHints,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MetaMethod {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Unm,
    #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53", feature = "luau"))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(feature = "lua55", feature = "lua54", feature = "lua53", feature = "luau")))
    )]
    IDiv,
    #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53"))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(feature = "lua55", feature = "lua54", feature = "lua53")))
    )]
    BAnd,
    #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53"))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(feature = "lua55", feature = "lua54", feature = "lua53")))
    )]
    BOr,
    #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53"))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(feature = "lua55", feature = "lua54", feature = "lua53")))
    )]
    BXor,
    #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53"))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(feature = "lua55", feature = "lua54", feature = "lua53")))
    )]
    BNot,
    #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53"))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(feature = "lua55", feature = "lua54", feature = "lua53")))
    )]
    Shl,
    #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53"))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(feature = "lua55", feature = "lua54", feature = "lua53")))
    )]
    Shr,
    Concat,
    Len,
    Eq,
    Lt,
    Le,
    Index,
    NewIndex,
    Call,
    ToString,
    #[cfg(any(
        feature = "lua55",
        feature = "lua54",
        feature = "lua53",
        feature = "lua52",
        feature = "luajit52"
    ))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(
            feature = "lua55",
            feature = "lua54",
            feature = "lua53",
            feature = "lua52",
            feature = "luajit52"
        )))
    )]
    Pairs,
    #[cfg(any(feature = "lua52", feature = "luajit52", doc))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "lua52", feature = "luajit52"))))]
    IPairs,
    #[cfg(any(feature = "luau", doc))]
    #[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
    Iter,
    #[cfg(any(feature = "lua55", feature = "lua54"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "lua55", feature = "lua54"))))]
    Close,
    #[doc(hidden)]
    Type,
}

impl PartialEq<MetaMethod> for &str {
    fn eq(&self, other: &MetaMethod) -> bool {
        *self == other.name()
    }
}

impl PartialEq<MetaMethod> for StdString {
    fn eq(&self, other: &MetaMethod) -> bool {
        self == other.name()
    }
}

impl fmt::Display for MetaMethod {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "{}", self.name())
    }
}

impl MetaMethod {
    pub const fn name(self) -> &'static str {
        match self {
            MetaMethod::Add => "__add",
            MetaMethod::Sub => "__sub",
            MetaMethod::Mul => "__mul",
            MetaMethod::Div => "__div",
            MetaMethod::Mod => "__mod",
            MetaMethod::Pow => "__pow",
            MetaMethod::Unm => "__unm",

            #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53", feature = "luau"))]
            MetaMethod::IDiv => "__idiv",
            #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53"))]
            MetaMethod::BAnd => "__band",
            #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53"))]
            MetaMethod::BOr => "__bor",
            #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53"))]
            MetaMethod::BXor => "__bxor",
            #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53"))]
            MetaMethod::BNot => "__bnot",
            #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53"))]
            MetaMethod::Shl => "__shl",
            #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53"))]
            MetaMethod::Shr => "__shr",

            MetaMethod::Concat => "__concat",
            MetaMethod::Len => "__len",
            MetaMethod::Eq => "__eq",
            MetaMethod::Lt => "__lt",
            MetaMethod::Le => "__le",
            MetaMethod::Index => "__index",
            MetaMethod::NewIndex => "__newindex",
            MetaMethod::Call => "__call",
            MetaMethod::ToString => "__tostring",

            #[cfg(any(
                feature = "lua55",
                feature = "lua54",
                feature = "lua53",
                feature = "lua52",
                feature = "luajit52"
            ))]
            MetaMethod::Pairs => "__pairs",
            #[cfg(any(feature = "lua52", feature = "luajit52"))]
            MetaMethod::IPairs => "__ipairs",
            #[cfg(feature = "luau")]
            MetaMethod::Iter => "__iter",

            #[cfg(any(feature = "lua55", feature = "lua54"))]
            MetaMethod::Close => "__close",

            #[rustfmt::skip]
            MetaMethod::Type => if cfg!(feature = "luau") { "__type" } else { "__name" },
        }
    }

    pub(crate) const fn as_cstr(self) -> &'static CStr {
        match self {
            #[rustfmt::skip]
            MetaMethod::Type => if cfg!(feature = "luau") { c"__type" } else { c"__name" },
            _ => unreachable!(),
        }
    }

    pub(crate) fn validate(name: &str) -> Result<&str> {
        match name {
            "__gc" => Err(Error::MetaMethodRestricted(name.to_string())),
            "__metatable" => Err(Error::MetaMethodRestricted(name.to_string())),
            _ if name.starts_with("__mlua") => Err(Error::MetaMethodRestricted(name.to_string())),
            name => Ok(name),
        }
    }
}

impl AsRef<str> for MetaMethod {
    fn as_ref(&self) -> &str {
        self.name()
    }
}

impl From<MetaMethod> for StdString {
    #[inline]
    fn from(method: MetaMethod) -> Self {
        method.name().to_owned()
    }
}

pub trait UserDataMethods<T> {
    fn add_method<M, A, R>(&mut self, name: impl Into<StdString>, method: M)
    where
        M: Fn(&Lua, &T, A) -> Result<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti;

    fn add_method_mut<M, A, R>(&mut self, name: impl Into<StdString>, method: M)
    where
        M: FnMut(&Lua, &mut T, A) -> Result<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti;

    #[doc(hidden)]
    fn add_method_once<M, A, R>(&mut self, name: impl Into<StdString>, method: M)
    where
        T: 'static,
        M: Fn(&Lua, T, A) -> Result<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
        let name = name.into();
        let method_name = format!("{}.{name}", short_type_name::<T>());
        self.add_function(name, move |lua, (ud, args): (AnyUserData, A)| {
            let this = (ud.take()).map_err(|err| Error::bad_self_argument(&method_name, err))?;
            method(lua, this, args)
        });
    }

    #[cfg(feature = "async")]
    #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
    fn add_async_method<M, A, MR, R>(&mut self, name: impl Into<StdString>, method: M)
    where
        T: 'static,
        M: Fn(Lua, UserDataRef<T>, A) -> MR + MaybeSend + 'static,
        A: FromLuaMulti,
        MR: Future<Output = Result<R>> + MaybeSend + 'static,
        R: IntoLuaMulti;

    #[cfg(feature = "async")]
    #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
    fn add_async_method_mut<M, A, MR, R>(&mut self, name: impl Into<StdString>, method: M)
    where
        T: 'static,
        M: Fn(Lua, UserDataRefMut<T>, A) -> MR + MaybeSend + 'static,
        A: FromLuaMulti,
        MR: Future<Output = Result<R>> + MaybeSend + 'static,
        R: IntoLuaMulti;

    #[cfg(feature = "async")]
    #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
    #[doc(hidden)]
    fn add_async_method_once<M, A, MR, R>(&mut self, name: impl Into<StdString>, method: M)
    where
        T: 'static,
        M: Fn(Lua, T, A) -> MR + MaybeSend + 'static,
        A: FromLuaMulti,
        MR: Future<Output = Result<R>> + MaybeSend + 'static,
        R: IntoLuaMulti,
    {
        let name = name.into();
        let method_name = format!("{}.{name}", short_type_name::<T>());
        self.add_async_function(name, move |lua, (ud, args): (AnyUserData, A)| {
            match (ud.take()).map_err(|err| Error::bad_self_argument(&method_name, err)) {
                Ok(this) => either::Either::Left(method(lua, this, args)),
                Err(err) => either::Either::Right(async move { Err(err) }),
            }
        });
    }

    fn add_function<F, A, R>(&mut self, name: impl Into<StdString>, function: F)
    where
        F: Fn(&Lua, A) -> Result<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti;

    fn add_function_mut<F, A, R>(&mut self, name: impl Into<StdString>, function: F)
    where
        F: FnMut(&Lua, A) -> Result<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti;

    #[cfg(feature = "async")]
    #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
    fn add_async_function<F, A, FR, R>(&mut self, name: impl Into<StdString>, function: F)
    where
        F: Fn(Lua, A) -> FR + MaybeSend + 'static,
        A: FromLuaMulti,
        FR: Future<Output = Result<R>> + MaybeSend + 'static,
        R: IntoLuaMulti;

    fn add_meta_method<M, A, R>(&mut self, name: impl Into<StdString>, method: M)
    where
        M: Fn(&Lua, &T, A) -> Result<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti;

    fn add_meta_method_mut<M, A, R>(&mut self, name: impl Into<StdString>, method: M)
    where
        M: FnMut(&Lua, &mut T, A) -> Result<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti;

    #[cfg(all(feature = "async", not(any(feature = "lua51", feature = "luau"))))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(feature = "async", not(any(feature = "lua51", feature = "luau")))))
    )]
    fn add_async_meta_method<M, A, MR, R>(&mut self, name: impl Into<StdString>, method: M)
    where
        T: 'static,
        M: Fn(Lua, UserDataRef<T>, A) -> MR + MaybeSend + 'static,
        A: FromLuaMulti,
        MR: Future<Output = Result<R>> + MaybeSend + 'static,
        R: IntoLuaMulti;

    #[cfg(all(feature = "async", not(any(feature = "lua51", feature = "luau"))))]
    #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
    fn add_async_meta_method_mut<M, A, MR, R>(&mut self, name: impl Into<StdString>, method: M)
    where
        T: 'static,
        M: Fn(Lua, UserDataRefMut<T>, A) -> MR + MaybeSend + 'static,
        A: FromLuaMulti,
        MR: Future<Output = Result<R>> + MaybeSend + 'static,
        R: IntoLuaMulti;

    fn add_meta_function<F, A, R>(&mut self, name: impl Into<StdString>, function: F)
    where
        F: Fn(&Lua, A) -> Result<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti;

    fn add_meta_function_mut<F, A, R>(&mut self, name: impl Into<StdString>, function: F)
    where
        F: FnMut(&Lua, A) -> Result<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti;

    #[cfg(all(feature = "async", not(any(feature = "lua51", feature = "luau"))))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(feature = "async", not(any(feature = "lua51", feature = "luau")))))
    )]
    fn add_async_meta_function<F, A, FR, R>(&mut self, name: impl Into<StdString>, function: F)
    where
        F: Fn(Lua, A) -> FR + MaybeSend + 'static,
        A: FromLuaMulti,
        FR: Future<Output = Result<R>> + MaybeSend + 'static,
        R: IntoLuaMulti;
}

pub trait UserDataFields<T> {
    fn add_field<V>(&mut self, name: impl Into<StdString>, value: V)
    where
        V: IntoLua + 'static;

    fn add_field_method_get<M, R>(&mut self, name: impl Into<StdString>, method: M)
    where
        M: Fn(&Lua, &T) -> Result<R> + MaybeSend + 'static,
        R: IntoLua;

    fn add_field_method_set<M, A>(&mut self, name: impl Into<StdString>, method: M)
    where
        M: FnMut(&Lua, &mut T, A) -> Result<()> + MaybeSend + 'static,
        A: FromLua;

    fn add_field_function_get<F, R>(&mut self, name: impl Into<StdString>, function: F)
    where
        F: Fn(&Lua, AnyUserData) -> Result<R> + MaybeSend + 'static,
        R: IntoLua;

    fn add_field_function_set<F, A>(&mut self, name: impl Into<StdString>, function: F)
    where
        F: FnMut(&Lua, AnyUserData, A) -> Result<()> + MaybeSend + 'static,
        A: FromLua;

    fn add_meta_field<V>(&mut self, name: impl Into<StdString>, value: V)
    where
        V: IntoLua + 'static;

    fn add_meta_field_with<F, R>(&mut self, name: impl Into<StdString>, f: F)
    where
        F: FnOnce(&Lua) -> Result<R> + 'static,
        R: IntoLua;
}

pub trait UserData: Sized {
    #[allow(unused_variables)]
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {}

    #[allow(unused_variables)]
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {}

    fn register(registry: &mut UserDataRegistry<Self>) {
        Self::add_fields(registry);
        Self::add_methods(registry);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnyUserData(pub(crate) ValueRef);

impl AnyUserData {
    #[inline]
    pub fn is<T: 'static>(&self) -> bool {
        let type_id = self.type_id();
        matches!(type_id, Some(type_id) if type_id == TypeId::of::<T>())
    }

    #[inline]
    pub fn is_proxy<T: 'static>(&self) -> bool {
        self.is::<UserDataProxy<T>>()
    }

    #[inline]
    pub fn borrow<T: 'static>(&self) -> Result<UserDataRef<T>> {
        let lua = self.0.lua.lock();
        unsafe { UserDataRef::borrow_from_stack(&lua, lua.ref_thread(), self.0.index) }
    }

    pub fn borrow_scoped<T: 'static, R>(&self, f: impl FnOnce(&T) -> R) -> Result<R> {
        let lua = self.0.lua.lock();
        let type_id = lua.get_userdata_ref_type_id(&self.0)?;
        let type_hints = TypeIdHints::new::<T>();
        unsafe { borrow_userdata_scoped(lua.ref_thread(), self.0.index, type_id, type_hints, f) }
    }

    #[inline]
    pub fn borrow_mut<T: 'static>(&self) -> Result<UserDataRefMut<T>> {
        let lua = self.0.lua.lock();
        unsafe { UserDataRefMut::borrow_from_stack(&lua, lua.ref_thread(), self.0.index) }
    }

    pub fn borrow_mut_scoped<T: 'static, R>(&self, f: impl FnOnce(&mut T) -> R) -> Result<R> {
        let lua = self.0.lua.lock();
        let type_id = lua.get_userdata_ref_type_id(&self.0)?;
        let type_hints = TypeIdHints::new::<T>();
        unsafe { borrow_userdata_scoped_mut(lua.ref_thread(), self.0.index, type_id, type_hints, f) }
    }

    pub fn take<T: 'static>(&self) -> Result<T> {
        let lua = self.0.lua.lock();
        match lua.get_userdata_ref_type_id(&self.0)? {
            Some(type_id) if type_id == TypeId::of::<T>() => unsafe {
                let ref_thread = lua.ref_thread();
                if (*get_userdata::<UserDataStorage<T>>(ref_thread, self.0.index)).has_exclusive_access() {
                    take_userdata::<UserDataStorage<T>>(ref_thread, self.0.index).into_inner()
                } else {
                    Err(Error::UserDataBorrowMutError)
                }
            },
            _ => Err(Error::UserDataTypeMismatch),
        }
    }

    pub fn destroy(&self) -> Result<()> {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 3)?;

            lua.push_userdata_ref(&self.0)?;
            protect_lua!(state, 1, 1, fn(state) {
                if ffi::luaL_callmeta(state, -1, cstr!("__gc")) == 0 {
                    ffi::lua_pushboolean(state, 0);
                }
            })?;
            if ffi::lua_isboolean(state, -1) != 0 && ffi::lua_toboolean(state, -1) != 0 {
                return Ok(());
            }
            Err(Error::UserDataBorrowMutError)
        }
    }

    #[inline]
    pub fn set_user_value(&self, v: impl IntoLua) -> Result<()> {
        self.set_nth_user_value(1, v)
    }

    #[inline]
    pub fn user_value<V: FromLua>(&self) -> Result<V> {
        self.nth_user_value(1)
    }

    pub fn set_nth_user_value(&self, n: usize, v: impl IntoLua) -> Result<()> {
        if n < 1 || n > u16::MAX as usize {
            return Err(Error::runtime("user value index out of bounds"));
        }

        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 5)?;

            lua.push_userdata_ref(&self.0)?;
            lua.push(v)?;

            protect_lua!(state, 2, 0, |state| {
                if ffi::lua_getuservalue(state, -2) != ffi::LUA_TTABLE {
                    ffi::lua_pop(state, 1);
                    ffi::lua_newtable(state);
                    ffi::lua_pushvalue(state, -1);
                    ffi::lua_setuservalue(state, -4);
                }
                ffi::lua_pushvalue(state, -2);
                ffi::lua_rawseti(state, -2, n as ffi::lua_Integer);
            })?;

            Ok(())
        }
    }

    pub fn nth_user_value<V: FromLua>(&self, n: usize) -> Result<V> {
        if n < 1 || n > u16::MAX as usize {
            return Err(Error::runtime("user value index out of bounds"));
        }

        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 4)?;

            lua.push_userdata_ref(&self.0)?;

            if ffi::lua_getuservalue(state, -1) != ffi::LUA_TTABLE {
                return V::from_lua(Value::Nil, lua.lua());
            }
            ffi::lua_rawgeti(state, -1, n as ffi::lua_Integer);

            V::from_lua(lua.pop_value(), lua.lua())
        }
    }

    pub fn set_named_user_value(&self, name: &str, v: impl IntoLua) -> Result<()> {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 5)?;

            lua.push_userdata_ref(&self.0)?;
            lua.push(v)?;

            protect_lua!(state, 2, 0, |state| {
                if ffi::lua_getuservalue(state, -2) != ffi::LUA_TTABLE {
                    ffi::lua_pop(state, 1);
                    ffi::lua_newtable(state);
                    ffi::lua_pushvalue(state, -1);
                    ffi::lua_setuservalue(state, -4);
                }
                ffi::lua_pushlstring(state, name.as_ptr() as *const c_char, name.len());
                ffi::lua_pushvalue(state, -3);
                ffi::lua_rawset(state, -3);
            })?;

            Ok(())
        }
    }

    pub fn named_user_value<V: FromLua>(&self, name: &str) -> Result<V> {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 4)?;

            lua.push_userdata_ref(&self.0)?;

            if ffi::lua_getuservalue(state, -1) != ffi::LUA_TTABLE {
                return V::from_lua(Value::Nil, lua.lua());
            }
            push_string(state, name.as_bytes(), !lua.unlikely_memory_error())?;
            ffi::lua_rawget(state, -2);

            V::from_stack(-1, &lua)
        }
    }

    #[inline]
    pub fn metatable(&self) -> Result<UserDataMetatable> {
        self.raw_metatable().map(UserDataMetatable)
    }

    fn raw_metatable(&self) -> Result<Table> {
        let lua = self.0.lua.lock();
        let ref_thread = lua.ref_thread();
        unsafe {
            let _type_id = lua.get_userdata_ref_type_id(&self.0)?;

            ffi::lua_getmetatable(ref_thread, self.0.index);
            Ok(Table(lua.pop_ref_thread()))
        }
    }

    #[inline]
    pub fn to_pointer(&self) -> *const c_void {
        self.0.to_pointer()
    }

    #[inline]
    pub fn type_id(&self) -> Option<TypeId> {
        let lua = self.0.lua.lock();
        lua.get_userdata_ref_type_id(&self.0).ok().flatten()
    }

    pub fn type_name(&self) -> Result<Option<StdString>> {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 3)?;

            lua.push_userdata_ref(&self.0)?;
            let protect = !lua.unlikely_memory_error();
            let name_type = if protect {
                protect_lua!(state, 1, 1, |state| {
                    ffi::luaL_getmetafield(state, -1, MetaMethod::Type.as_cstr().as_ptr())
                })?
            } else {
                ffi::luaL_getmetafield(state, -1, MetaMethod::Type.as_cstr().as_ptr())
            };
            match name_type {
                ffi::LUA_TSTRING => Ok(Some(String(lua.pop_ref()).to_str()?.to_owned())),
                _ => Ok(None),
            }
        }
    }

    pub(crate) fn equals(&self, other: &Self) -> Result<bool> {
        if self == other {
            return Ok(true);
        }

        let mt = self.raw_metatable()?;
        if mt != other.raw_metatable()? {
            return Ok(false);
        }

        if mt.contains_key("__eq")? {
            return mt.get::<Function>("__eq")?.call((self, other));
        }

        Ok(false)
    }

    #[cfg(feature = "serde")]
    pub(crate) fn is_serializable(&self) -> bool {
        let lua = self.0.lua.lock();
        let is_serializable = || unsafe {
            let _ = lua.get_userdata_ref_type_id(&self.0)?;
            let ud = &*get_userdata::<UserDataStorage<()>>(lua.ref_thread(), self.0.index);
            Ok::<_, Error>((*ud).is_serializable())
        };
        is_serializable().unwrap_or(false)
    }
}

#[derive(Clone, Debug)]
pub struct UserDataMetatable(pub(crate) Table);

impl UserDataMetatable {
    pub fn get<V: FromLua>(&self, key: impl AsRef<str>) -> Result<V> {
        self.0.raw_get(MetaMethod::validate(key.as_ref())?)
    }

    pub fn set(&self, key: impl AsRef<str>, value: impl IntoLua) -> Result<()> {
        let key = MetaMethod::validate(key.as_ref())?;
        if key == MetaMethod::Index || key == MetaMethod::NewIndex {
            return Err(Error::MetaMethodRestricted(key.to_string()));
        }
        self.0.raw_set(key, value)
    }

    pub fn contains(&self, key: impl AsRef<str>) -> Result<bool> {
        self.0.contains_key(MetaMethod::validate(key.as_ref())?)
    }

    pub fn pairs<V: FromLua>(&self) -> UserDataMetatablePairs<'_, V> {
        UserDataMetatablePairs(self.0.pairs())
    }
}

pub struct UserDataMetatablePairs<'a, V>(TablePairs<'a, StdString, V>);

impl<V> Iterator for UserDataMetatablePairs<'_, V>
where
    V: FromLua,
{
    type Item = Result<(StdString, V)>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.0.next()? {
                Ok((key, value)) => {
                    if MetaMethod::validate(&key).is_ok() {
                        break Some(Ok((key, value)));
                    }
                }
                Err(e) => break Some(Err(e)),
            }
        }
    }
}

#[cfg(feature = "serde")]
impl Serialize for AnyUserData {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let lua = self.0.lua.lock();
        unsafe {
            let _ = lua
                .get_userdata_ref_type_id(&self.0)
                .map_err(ser::Error::custom)?;
            let ud = &*get_userdata::<UserDataStorage<()>>(lua.ref_thread(), self.0.index);
            ud.serialize(serializer)
        }
    }
}

struct WrappedUserdata<F: FnOnce(&Lua) -> Result<AnyUserData>>(F);

impl AnyUserData {
    pub fn wrap<T: MaybeSend + 'static>(data: T) -> impl IntoLua {
        WrappedUserdata(move |lua| lua.create_any_userdata(data))
    }

    #[cfg(feature = "serde")]
    #[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
    pub fn wrap_ser<T: Serialize + MaybeSend + 'static>(data: T) -> impl IntoLua {
        WrappedUserdata(move |lua| lua.create_ser_any_userdata(data))
    }
}

impl<F> IntoLua for WrappedUserdata<F>
where
    F: for<'l> FnOnce(&'l Lua) -> Result<AnyUserData>,
{
    fn into_lua(self, lua: &Lua) -> Result<Value> {
        (self.0)(lua).map(Value::UserData)
    }
}

mod cell;
mod lock;
mod object;
mod r#ref;
mod registry;
mod util;

#[cfg(test)]
mod assertions {
    use super::*;

    #[cfg(not(feature = "send"))]
    static_assertions::assert_not_impl_any!(AnyUserData: Send);
    #[cfg(feature = "send")]
    static_assertions::assert_impl_all!(AnyUserData: Send, Sync);
}
