use std::collections::HashSet;
use std::fmt;
use std::marker::PhantomData;
use std::os::raw::c_void;
use std::string::String as StdString;

use crate::error::{Error, Result};
use crate::function::Function;
use crate::state::{LuaGuard, RawLua, WeakLua};
use crate::traits::{FromLua, FromLuaMulti, IntoLua, IntoLuaMulti, ObjectLike};
use crate::types::{Integer, ValueRef};
use crate::util::{assert_stack, check_stack, get_metatable_ptr, StackGuard};
use crate::value::{Nil, Value};

#[cfg(feature = "async")]
use crate::function::AsyncCallFuture;

#[cfg(feature = "serde")]
use {
    rustc_hash::FxHashSet,
    serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer},
    std::{cell::RefCell, rc::Rc, result::Result as StdResult},
};

#[derive(Clone, PartialEq)]
pub struct Table(pub(crate) ValueRef);

impl Table {
    pub fn set(&self, key: impl IntoLua, value: impl IntoLua) -> Result<()> {
        if !self.has_metatable() {
            return self.raw_set(key, value);
        }

        self.set_protected(key, value)
    }

    pub(crate) fn set_protected(&self, key: impl IntoLua, value: impl IntoLua) -> Result<()> {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 5)?;

            lua.push_ref(&self.0);
            key.push_into_stack(&lua)?;
            value.push_into_stack(&lua)?;
            protect_lua!(state, 3, 0, fn(state) ffi::lua_settable(state, -3))
        }
    }

    pub fn get<V: FromLua>(&self, key: impl IntoLua) -> Result<V> {
        if !self.has_metatable() {
            return self.raw_get(key);
        }

        self.get_protected(key)
    }

    pub(crate) fn get_protected<V: FromLua>(&self, key: impl IntoLua) -> Result<V> {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 4)?;

            lua.push_ref(&self.0);
            key.push_into_stack(&lua)?;
            protect_lua!(state, 2, 1, fn(state) ffi::lua_gettable(state, -2))?;

            V::from_stack(-1, &lua)
        }
    }

    pub fn contains_key(&self, key: impl IntoLua) -> Result<bool> {
        Ok(self.get::<Value>(key)? != Value::Nil)
    }

    pub fn push(&self, value: impl IntoLua) -> Result<()> {
        if !self.has_metatable() {
            return self.raw_push(value);
        }

        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 4)?;

            lua.push_ref(&self.0);
            value.push_into_stack(&lua)?;
            protect_lua!(state, 2, 0, fn(state) {
                let len = ffi::luaL_len(state, -2) as Integer;
                ffi::lua_seti(state, -2, len + 1);
            })?
        }
        Ok(())
    }

    pub fn pop<V: FromLua>(&self) -> Result<V> {
        if !self.has_metatable() {
            return self.raw_pop();
        }

        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 4)?;

            lua.push_ref(&self.0);
            protect_lua!(state, 1, 1, fn(state) {
                let len = ffi::luaL_len(state, -1) as Integer;
                ffi::lua_geti(state, -1, len);
                ffi::lua_pushnil(state);
                ffi::lua_seti(state, -3, len);
            })?;
            V::from_stack(-1, &lua)
        }
    }

    pub fn equals(&self, other: &Self) -> Result<bool> {
        if self == other {
            return Ok(true);
        }

        if let Some(mt) = self.metatable() {
            if mt.contains_key("__eq")? {
                return mt.get::<Function>("__eq")?.call((self, other));
            }
        }
        if let Some(mt) = other.metatable() {
            if mt.contains_key("__eq")? {
                return mt.get::<Function>("__eq")?.call((self, other));
            }
        }

        Ok(false)
    }

    pub fn raw_set(&self, key: impl IntoLua, value: impl IntoLua) -> Result<()> {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            #[cfg(feature = "luau")]
            self.check_readonly_write(&lua)?;

            let _sg = StackGuard::new(state);
            check_stack(state, 5)?;

            lua.push_ref(&self.0);
            key.push_into_stack(&lua)?;
            value.push_into_stack(&lua)?;

            if lua.unlikely_memory_error() {
                ffi::lua_rawset(state, -3);
                ffi::lua_pop(state, 1);
                Ok(())
            } else {
                protect_lua!(state, 3, 0, fn(state) ffi::lua_rawset(state, -3))
            }
        }
    }

    pub fn raw_get<V: FromLua>(&self, key: impl IntoLua) -> Result<V> {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 3)?;

            lua.push_ref(&self.0);
            key.push_into_stack(&lua)?;
            ffi::lua_rawget(state, -2);

            V::from_stack(-1, &lua)
        }
    }

    pub fn raw_insert(&self, idx: Integer, value: impl IntoLua) -> Result<()> {
        let size = self.raw_len() as Integer;
        if idx < 1 || idx > size + 1 {
            return Err(Error::runtime("index out of bounds"));
        }

        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 5)?;

            lua.push_ref(&self.0);
            value.push_into_stack(&lua)?;
            protect_lua!(state, 2, 0, |state| {
                for i in (idx..=size).rev() {
                    ffi::lua_rawgeti(state, -2, i);
                    ffi::lua_rawseti(state, -3, i + 1);
                }
                ffi::lua_rawseti(state, -2, idx)
            })
        }
    }

    pub fn raw_push(&self, value: impl IntoLua) -> Result<()> {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            #[cfg(feature = "luau")]
            self.check_readonly_write(&lua)?;

            let _sg = StackGuard::new(state);
            check_stack(state, 4)?;

            lua.push_ref(&self.0);
            value.push_into_stack(&lua)?;

            unsafe fn callback(state: *mut ffi::lua_State) {
                let len = ffi::lua_rawlen(state, -2) as Integer;
                ffi::lua_rawseti(state, -2, len + 1);
            }

            if lua.unlikely_memory_error() {
                callback(state);
            } else {
                protect_lua!(state, 2, 0, fn(state) callback(state))?;
            }
        }
        Ok(())
    }

    pub fn raw_pop<V: FromLua>(&self) -> Result<V> {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            #[cfg(feature = "luau")]
            self.check_readonly_write(&lua)?;

            let _sg = StackGuard::new(state);
            check_stack(state, 3)?;

            lua.push_ref(&self.0);
            let len = ffi::lua_rawlen(state, -1) as Integer;
            ffi::lua_rawgeti(state, -1, len);
            ffi::lua_pushnil(state);
            ffi::lua_rawseti(state, -3, len);

            V::from_stack(-1, &lua)
        }
    }

    pub fn raw_remove(&self, key: impl IntoLua) -> Result<()> {
        let lua = self.0.lua.lock();
        let state = lua.state();
        let key = key.into_lua(lua.lua())?;
        match key {
            Value::Integer(idx) => {
                let size = self.raw_len() as Integer;
                if idx < 1 || idx > size {
                    return Err(Error::runtime("index out of bounds"));
                }
                unsafe {
                    let _sg = StackGuard::new(state);
                    check_stack(state, 4)?;

                    lua.push_ref(&self.0);
                    protect_lua!(state, 1, 0, |state| {
                        for i in idx..size {
                            ffi::lua_rawgeti(state, -1, i + 1);
                            ffi::lua_rawseti(state, -2, i);
                        }
                        ffi::lua_pushnil(state);
                        ffi::lua_rawseti(state, -2, size);
                    })
                }
            }
            _ => self.raw_set(key, Nil),
        }
    }

    pub fn clear(&self) -> Result<()> {
        let lua = self.0.lua.lock();
        unsafe {
            #[cfg(feature = "luau")]
            {
                self.check_readonly_write(&lua)?;
                ffi::lua_cleartable(lua.ref_thread(), self.0.index);
            }

            #[cfg(not(feature = "luau"))]
            {
                let state = lua.state();
                check_stack(state, 4)?;

                lua.push_ref(&self.0);

                ffi::lua_pushnil(state);
                while ffi::lua_next(state, -2) != 0 {
                    ffi::lua_pop(state, 1);
                    ffi::lua_pushvalue(state, -1);
                    ffi::lua_pushnil(state);
                    ffi::lua_rawset(state, -4);
                }
            }
        }

        Ok(())
    }

    pub fn len(&self) -> Result<Integer> {
        if !self.has_metatable() {
            return Ok(self.raw_len() as Integer);
        }

        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 4)?;

            lua.push_ref(&self.0);
            protect_lua!(state, 1, 0, |state| ffi::luaL_len(state, -1))
        }
    }

    pub fn raw_len(&self) -> usize {
        let lua = self.0.lua.lock();
        unsafe { ffi::lua_rawlen(lua.ref_thread(), self.0.index) }
    }

    pub fn is_empty(&self) -> bool {
        let lua = self.0.lua.lock();
        let ref_thread = lua.ref_thread();
        unsafe {
            ffi::lua_pushnil(ref_thread);
            if ffi::lua_next(ref_thread, self.0.index) == 0 {
                return true;
            }
            ffi::lua_pop(ref_thread, 2);
        }
        false
    }

    pub fn metatable(&self) -> Option<Table> {
        let lua = self.0.lua.lock();
        let ref_thread = lua.ref_thread();
        unsafe {
            if ffi::lua_getmetatable(ref_thread, self.0.index) == 0 {
                None
            } else {
                Some(Table(lua.pop_ref_thread()))
            }
        }
    }

    pub fn set_metatable(&self, metatable: Option<Table>) -> Result<()> {
        #[cfg(feature = "luau")]
        if self.is_readonly() {
            return Err(Error::runtime("attempt to modify a readonly table"));
        }

        let lua = self.0.lua.lock();
        let ref_thread = lua.ref_thread();
        unsafe {
            if let Some(metatable) = &metatable {
                ffi::lua_pushvalue(ref_thread, metatable.0.index);
            } else {
                ffi::lua_pushnil(ref_thread);
            }
            ffi::lua_setmetatable(ref_thread, self.0.index);
        }
        Ok(())
    }

    #[doc(hidden)]
    #[inline]
    pub fn has_metatable(&self) -> bool {
        let lua = self.0.lua.lock();
        unsafe { !get_metatable_ptr(lua.ref_thread(), self.0.index).is_null() }
    }

    #[cfg(any(feature = "luau", doc))]
    #[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
    pub fn set_readonly(&self, enabled: bool) {
        let lua = self.0.lua.lock();
        let ref_thread = lua.ref_thread();
        unsafe {
            ffi::lua_setreadonly(ref_thread, self.0.index, enabled as _);
            if !enabled {
                ffi::lua_setsafeenv(ref_thread, self.0.index, 0);
            }
        }
    }

    #[cfg(any(feature = "luau", doc))]
    #[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
    pub fn is_readonly(&self) -> bool {
        let lua = self.0.lua.lock();
        let ref_thread = lua.ref_thread();
        unsafe { ffi::lua_getreadonly(ref_thread, self.0.index) != 0 }
    }

    #[cfg(any(feature = "luau", doc))]
    #[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
    pub fn set_safeenv(&self, enabled: bool) {
        let lua = self.0.lua.lock();
        unsafe { ffi::lua_setsafeenv(lua.ref_thread(), self.0.index, enabled as _) };
    }

    #[inline]
    pub fn to_pointer(&self) -> *const c_void {
        self.0.to_pointer()
    }

    pub fn pairs<K: FromLua, V: FromLua>(&self) -> TablePairs<'_, K, V> {
        TablePairs {
            guard: self.0.lua.lock(),
            table: self,
            key: Some(Nil),
            _phantom: PhantomData,
        }
    }

    pub fn for_each<K, V>(&self, mut f: impl FnMut(K, V) -> Result<()>) -> Result<()>
    where
        K: FromLua,
        V: FromLua,
    {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 5)?;

            lua.push_ref(&self.0);
            ffi::lua_pushnil(state);
            while ffi::lua_next(state, -2) != 0 {
                let k = K::from_stack(-2, &lua)?;
                let v = V::from_stack(-1, &lua)?;
                f(k, v)?;
                ffi::lua_pop(state, 1);
            }
        }
        Ok(())
    }

    pub fn sequence_values<V: FromLua>(&self) -> TableSequence<'_, V> {
        TableSequence {
            guard: self.0.lua.lock(),
            table: self,
            index: 1,
            len: None,
            _phantom: PhantomData,
        }
    }

    #[doc(hidden)]
    pub fn for_each_value<V: FromLua>(&self, f: impl FnMut(V) -> Result<()>) -> Result<()> {
        self.for_each_value_by_len(None, f)
    }

    fn for_each_value_by_len<V: FromLua>(
        &self,
        len: impl Into<Option<usize>>,
        mut f: impl FnMut(V) -> Result<()>,
    ) -> Result<()> {
        let len = len.into();
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 4)?;

            lua.push_ref(&self.0);
            for i in 1.. {
                if len.map(|len| i > len).unwrap_or(false) {
                    break;
                }
                let t = ffi::lua_rawgeti(state, -1, i as _);
                if len.is_none() && t == ffi::LUA_TNIL {
                    break;
                }
                f(V::from_stack(-1, &lua)?)?;
                ffi::lua_pop(state, 1);
            }
        }
        Ok(())
    }

    #[doc(hidden)]
    pub fn raw_seti(&self, idx: usize, value: impl IntoLua) -> Result<()> {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            #[cfg(feature = "luau")]
            self.check_readonly_write(&lua)?;

            let _sg = StackGuard::new(state);
            check_stack(state, 5)?;

            lua.push_ref(&self.0);
            value.push_into_stack(&lua)?;

            let idx = idx.try_into().unwrap();
            if lua.unlikely_memory_error() {
                ffi::lua_rawseti(state, -2, idx);
            } else {
                protect_lua!(state, 2, 0, |state| ffi::lua_rawseti(state, -2, idx))?;
            }
        }
        Ok(())
    }

    #[cfg(feature = "serde")]
    fn has_array_metatable(&self) -> bool {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            assert_stack(state, 3);

            lua.push_ref(&self.0);
            if ffi::lua_getmetatable(state, -1) == 0 {
                return false;
            }
            crate::serde::push_array_metatable(state);
            ffi::lua_rawequal(state, -1, -2) != 0
        }
    }

    #[cfg(feature = "serde")]
    fn find_array_len(&self) -> Option<(usize, usize)> {
        let lua = self.0.lua.lock();
        let ref_thread = lua.ref_thread();
        unsafe {
            let _sg = StackGuard::new(ref_thread);

            let (mut count, mut max_index) = (0, 0);
            ffi::lua_pushnil(ref_thread);
            while ffi::lua_next(ref_thread, self.0.index) != 0 {
                if ffi::lua_type(ref_thread, -2) != ffi::LUA_TNUMBER {
                    return None;
                }

                let k = ffi::lua_tonumber(ref_thread, -2);
                if k.trunc() != k || k < 1.0 {
                    return None;
                }
                max_index = std::cmp::max(max_index, k as usize);
                count += 1;
                ffi::lua_pop(ref_thread, 1);
            }
            Some((count, max_index))
        }
    }

    #[cfg(feature = "serde")]
    pub(crate) fn encode_as_array(&self, options: crate::serde::de::Options) -> Option<usize> {
        if options.detect_mixed_tables {
            if let Some((len, max_idx)) = self.find_array_len() {
                if len < 10 || len * 2 >= max_idx {
                    return Some(max_idx);
                }
            }
        } else {
            let len = self.raw_len();
            if len > 0 || self.has_array_metatable() {
                return Some(len);
            }
            if options.encode_empty_tables_as_array && self.is_empty() {
                return Some(0);
            }
        }
        None
    }

    #[cfg(feature = "luau")]
    #[inline(always)]
    fn check_readonly_write(&self, lua: &RawLua) -> Result<()> {
        if unsafe { ffi::lua_getreadonly(lua.ref_thread(), self.0.index) != 0 } {
            return Err(Error::runtime("attempt to modify a readonly table"));
        }
        Ok(())
    }

    pub(crate) fn fmt_pretty(
        &self,
        fmt: &mut fmt::Formatter,
        ident: usize,
        visited: &mut HashSet<*const c_void>,
    ) -> fmt::Result {
        visited.insert(self.to_pointer());

        let mut pairs = self.pairs::<Value, Value>().flatten().collect::<Vec<_>>();
        pairs.sort_by(|(a, _), (b, _)| a.sort_cmp(b));
        let is_sequence = (pairs.iter().enumerate())
            .all(|(i, (k, _))| matches!(k, Value::Integer(n) if *n == (i + 1) as Integer));
        if pairs.is_empty() {
            return write!(fmt, "{{}}");
        }
        writeln!(fmt, "{{")?;
        if is_sequence {
            for (_, value) in pairs {
                write!(fmt, "{}", " ".repeat(ident + 2))?;
                value.fmt_pretty(fmt, true, ident + 2, visited)?;
                writeln!(fmt, ",")?;
            }
        } else {
            fn is_simple_key(key: &[u8]) -> bool {
                key.iter().take(1).all(|c| c.is_ascii_alphabetic() || *c == b'_')
                    && key.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
            }

            for (key, value) in pairs {
                match key {
                    Value::String(key) if is_simple_key(&key.as_bytes()) => {
                        write!(fmt, "{}{}", " ".repeat(ident + 2), key.display())?;
                        write!(fmt, " = ")?;
                    }
                    _ => {
                        write!(fmt, "{}[", " ".repeat(ident + 2))?;
                        key.fmt_pretty(fmt, false, ident + 2, visited)?;
                        write!(fmt, "] = ")?;
                    }
                }
                value.fmt_pretty(fmt, true, ident + 2, visited)?;
                writeln!(fmt, ",")?;
            }
        }
        write!(fmt, "{}}}", " ".repeat(ident))
    }
}

impl fmt::Debug for Table {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if fmt.alternate() {
            return self.fmt_pretty(fmt, 0, &mut HashSet::new());
        }
        fmt.debug_tuple("Table").field(&self.0).finish()
    }
}

impl<T> PartialEq<[T]> for Table
where
    T: IntoLua + Clone,
{
    fn eq(&self, other: &[T]) -> bool {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            assert_stack(state, 4);

            lua.push_ref(&self.0);

            let len = ffi::lua_rawlen(state, -1);
            for i in 0..len {
                ffi::lua_rawgeti(state, -1, (i + 1) as _);
                let val = lua.pop_value();
                if val == Nil {
                    return i == other.len();
                }
                match other.get(i).map(|v| v.clone().into_lua(lua.lua())) {
                    Some(Ok(other_val)) if val == other_val => continue,
                    _ => return false,
                }
            }
        }
        true
    }
}

impl<T> PartialEq<&[T]> for Table
where
    T: IntoLua + Clone,
{
    #[inline]
    fn eq(&self, other: &&[T]) -> bool {
        self == *other
    }
}

impl<T, const N: usize> PartialEq<[T; N]> for Table
where
    T: IntoLua + Clone,
{
    #[inline]
    fn eq(&self, other: &[T; N]) -> bool {
        self == &other[..]
    }
}

impl ObjectLike for Table {
    #[inline]
    fn get<V: FromLua>(&self, key: impl IntoLua) -> Result<V> {
        self.get(key)
    }

    #[inline]
    fn set(&self, key: impl IntoLua, value: impl IntoLua) -> Result<()> {
        self.set(key, value)
    }

    #[inline]
    fn call<R>(&self, args: impl IntoLuaMulti) -> Result<R>
    where
        R: FromLuaMulti,
    {
        Function(self.0.clone()).call(args)
    }

    #[cfg(feature = "async")]
    #[inline]
    fn call_async<R>(&self, args: impl IntoLuaMulti) -> AsyncCallFuture<R>
    where
        R: FromLuaMulti,
    {
        Function(self.0.clone()).call_async(args)
    }

    #[inline]
    fn call_method<R>(&self, name: &str, args: impl IntoLuaMulti) -> Result<R>
    where
        R: FromLuaMulti,
    {
        self.call_function(name, (self, args))
    }

    #[cfg(feature = "async")]
    fn call_async_method<R>(&self, name: &str, args: impl IntoLuaMulti) -> AsyncCallFuture<R>
    where
        R: FromLuaMulti,
    {
        self.call_async_function(name, (self, args))
    }

    #[inline]
    fn call_function<R: FromLuaMulti>(&self, name: &str, args: impl IntoLuaMulti) -> Result<R> {
        match self.get(name)? {
            Value::Function(func) => func.call(args),
            val => {
                let msg = format!("attempt to call a {} value (function '{name}')", val.type_name());
                Err(Error::runtime(msg))
            }
        }
    }

    #[cfg(feature = "async")]
    #[inline]
    fn call_async_function<R>(&self, name: &str, args: impl IntoLuaMulti) -> AsyncCallFuture<R>
    where
        R: FromLuaMulti,
    {
        match self.get(name) {
            Ok(Value::Function(func)) => func.call_async(args),
            Ok(val) => {
                let msg = format!("attempt to call a {} value (function '{name}')", val.type_name());
                AsyncCallFuture::error(Error::RuntimeError(msg))
            }
            Err(err) => AsyncCallFuture::error(err),
        }
    }

    #[inline]
    fn to_string(&self) -> Result<StdString> {
        Value::Table(Table(self.0.clone())).to_string()
    }

    #[inline]
    fn to_value(&self) -> Value {
        Value::Table(self.clone())
    }

    #[inline]
    fn weak_lua(&self) -> &WeakLua {
        &self.0.lua
    }
}

#[cfg(feature = "serde")]
pub(crate) struct SerializableTable<'a> {
    table: &'a Table,
    options: crate::serde::de::Options,
    visited: Rc<RefCell<FxHashSet<*const c_void>>>,
}

#[cfg(feature = "serde")]
impl Serialize for Table {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> StdResult<S::Ok, S::Error> {
        SerializableTable::new(self, Default::default(), Default::default()).serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'a> SerializableTable<'a> {
    #[inline]
    pub(crate) fn new(
        table: &'a Table,
        options: crate::serde::de::Options,
        visited: Rc<RefCell<FxHashSet<*const c_void>>>,
    ) -> Self {
        Self {
            table,
            options,
            visited,
        }
    }
}

impl<V> TableSequence<'_, V> {
    #[cfg(feature = "serde")]
    pub(crate) fn with_len(mut self, len: usize) -> Self {
        self.len = Some(len);
        self
    }
}

#[cfg(feature = "serde")]
impl Serialize for SerializableTable<'_> {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use crate::serde::de::{check_value_for_skip, MapPairs, RecursionGuard};
        use crate::value::SerializableValue;

        let convert_result = |res: Result<()>, serialize_err: Option<S::Error>| match res {
            Ok(v) => Ok(v),
            Err(Error::SerializeError(_)) if serialize_err.is_some() => Err(serialize_err.unwrap()),
            Err(Error::SerializeError(msg)) => Err(serde::ser::Error::custom(msg)),
            Err(err) => Err(serde::ser::Error::custom(err.to_string())),
        };

        let options = self.options;
        let visited = &self.visited;
        let _guard = RecursionGuard::new(self.table, visited);

        if let Some(len) = self.table.encode_as_array(self.options) {
            let mut seq = serializer.serialize_seq(Some(len))?;
            let mut serialize_err = None;
            let res = self.table.for_each_value_by_len::<Value>(len, |value| {
                let skip = check_value_for_skip(&value, self.options, visited)
                    .map_err(|err| Error::SerializeError(err.to_string()))?;
                if skip {
                    return Ok(());
                }
                seq.serialize_element(&SerializableValue::new(&value, options, Some(visited)))
                    .map_err(|err| {
                        serialize_err = Some(err);
                        Error::SerializeError(StdString::new())
                    })
            });
            convert_result(res, serialize_err)?;
            return seq.end();
        }

        let mut map = serializer.serialize_map(None)?;
        let mut serialize_err = None;
        let mut process_pair = |key, value| {
            let skip_key = check_value_for_skip(&key, self.options, visited)
                .map_err(|err| Error::SerializeError(err.to_string()))?;
            let skip_value = check_value_for_skip(&value, self.options, visited)
                .map_err(|err| Error::SerializeError(err.to_string()))?;
            if skip_key || skip_value {
                return Ok(());
            }
            map.serialize_entry(
                &SerializableValue::new(&key, options, Some(visited)),
                &SerializableValue::new(&value, options, Some(visited)),
            )
            .map_err(|err| {
                serialize_err = Some(err);
                Error::SerializeError(StdString::new())
            })
        };

        let res = if !self.options.sort_keys {
            self.table.for_each(process_pair)
        } else {
            MapPairs::new(self.table, self.options.sort_keys)
                .map_err(serde::ser::Error::custom)?
                .try_for_each(|kv| {
                    let (key, value) = kv?;
                    process_pair(key, value)
                })
        };
        convert_result(res, serialize_err)?;
        map.end()
    }
}

pub struct TablePairs<'a, K, V> {
    guard: LuaGuard,
    table: &'a Table,
    key: Option<Value>,
    _phantom: PhantomData<(K, V)>,
}

impl<K, V> Iterator for TablePairs<'_, K, V>
where
    K: FromLua,
    V: FromLua,
{
    type Item = Result<(K, V)>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(prev_key) = self.key.take() {
            let lua: &RawLua = &self.guard;
            let state = lua.state();

            let res = (|| unsafe {
                let _sg = StackGuard::new(state);
                check_stack(state, 5)?;

                lua.push_ref(&self.table.0);
                lua.push_value(&prev_key)?;

                if ffi::lua_next(state, -2) != 0 {
                    let key = lua.stack_value(-2, None);
                    Ok(Some((
                        key.clone(),
                        K::from_lua(key, lua.lua())?,
                        V::from_stack(-1, lua)?,
                    )))
                } else {
                    Ok(None)
                }
            })();

            match res {
                Ok(Some((key, ret_key, value))) => {
                    self.key = Some(key);
                    Some(Ok((ret_key, value)))
                }
                Ok(None) => None,
                Err(e) => Some(Err(e)),
            }
        } else {
            None
        }
    }
}

pub struct TableSequence<'a, V> {
    guard: LuaGuard,
    table: &'a Table,
    index: Integer,
    len: Option<usize>,
    _phantom: PhantomData<V>,
}

impl<V: FromLua> Iterator for TableSequence<'_, V> {
    type Item = Result<V>;

    fn next(&mut self) -> Option<Self::Item> {
        let lua: &RawLua = &self.guard;
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            if let Err(err) = check_stack(state, 1) {
                return Some(Err(err));
            }

            lua.push_ref(&self.table.0);
            match ffi::lua_rawgeti(state, -1, self.index) {
                ffi::LUA_TNIL if self.index as usize > self.len.unwrap_or(0) => None,
                _ => {
                    self.index += 1;
                    Some(V::from_stack(-1, lua))
                }
            }
        }
    }
}

#[cfg(test)]
mod assertions {
    use super::*;

    #[cfg(not(feature = "send"))]
    static_assertions::assert_not_impl_any!(Table: Send);
    #[cfg(feature = "send")]
    static_assertions::assert_impl_all!(Table: Send, Sync);
}
