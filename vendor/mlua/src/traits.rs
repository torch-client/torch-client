use std::os::raw::c_int;
use std::string::String as StdString;
use std::sync::Arc;

use crate::error::{Error, Result};
use crate::multi::MultiValue;
use crate::private::Sealed;
use crate::state::{Lua, RawLua, WeakLua};
use crate::types::MaybeSend;
use crate::util::{check_stack, parse_lookup_path, short_type_name};
use crate::value::Value;

#[cfg(feature = "async")]
use {crate::function::AsyncCallFuture, std::future::Future};

pub trait IntoLua: Sized {
    fn into_lua(self, lua: &Lua) -> Result<Value>;

    #[doc(hidden)]
    #[inline]
    unsafe fn push_into_stack(self, lua: &RawLua) -> Result<()> {
        lua.push_value(&self.into_lua(lua.lua())?)
    }
}

pub trait FromLua: Sized {
    fn from_lua(value: Value, lua: &Lua) -> Result<Self>;

    #[doc(hidden)]
    #[inline]
    fn from_lua_arg(arg: Value, i: usize, to: Option<&str>, lua: &Lua) -> Result<Self> {
        Self::from_lua(arg, lua).map_err(|err| Error::BadArgument {
            to: to.map(|s| s.to_string()),
            pos: i,
            name: None,
            cause: Arc::new(err),
        })
    }

    #[doc(hidden)]
    #[inline]
    unsafe fn from_stack(idx: c_int, lua: &RawLua) -> Result<Self> {
        Self::from_lua(lua.stack_value(idx, None), lua.lua())
    }

    #[doc(hidden)]
    #[inline]
    unsafe fn from_stack_arg(idx: c_int, i: usize, to: Option<&str>, lua: &RawLua) -> Result<Self> {
        Self::from_stack(idx, lua).map_err(|err| Error::BadArgument {
            to: to.map(|s| s.to_string()),
            pos: i,
            name: None,
            cause: Arc::new(err),
        })
    }
}

pub trait IntoLuaMulti: Sized {
    fn into_lua_multi(self, lua: &Lua) -> Result<MultiValue>;

    #[doc(hidden)]
    #[inline]
    unsafe fn push_into_stack_multi(self, lua: &RawLua) -> Result<c_int> {
        let values = self.into_lua_multi(lua.lua())?;
        let len: c_int = values.len().try_into().unwrap();
        unsafe {
            check_stack(lua.state(), len + 1)?;
            for val in &values {
                lua.push_value(val)?;
            }
        }
        Ok(len)
    }
}

pub trait FromLuaMulti: Sized {
    fn from_lua_multi(values: MultiValue, lua: &Lua) -> Result<Self>;

    #[doc(hidden)]
    #[inline]
    fn from_lua_args(args: MultiValue, i: usize, to: Option<&str>, lua: &Lua) -> Result<Self> {
        let _ = (i, to);
        Self::from_lua_multi(args, lua)
    }

    #[doc(hidden)]
    #[inline]
    unsafe fn from_stack_multi(nvals: c_int, lua: &RawLua) -> Result<Self> {
        let mut values = MultiValue::with_capacity(nvals as usize);
        for idx in 0..nvals {
            values.push_back(lua.stack_value(-nvals + idx, None));
        }
        Self::from_lua_multi(values, lua.lua())
    }

    #[doc(hidden)]
    #[inline]
    unsafe fn from_stack_args(nargs: c_int, i: usize, to: Option<&str>, lua: &RawLua) -> Result<Self> {
        let _ = (i, to);
        Self::from_stack_multi(nargs, lua)
    }
}

pub trait ObjectLike: Sealed {
    fn get<V: FromLua>(&self, key: impl IntoLua) -> Result<V>;

    fn set(&self, key: impl IntoLua, value: impl IntoLua) -> Result<()>;

    fn call<R>(&self, args: impl IntoLuaMulti) -> Result<R>
    where
        R: FromLuaMulti;

    #[cfg(feature = "async")]
    #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
    fn call_async<R>(&self, args: impl IntoLuaMulti) -> AsyncCallFuture<R>
    where
        R: FromLuaMulti;

    fn call_method<R>(&self, name: &str, args: impl IntoLuaMulti) -> Result<R>
    where
        R: FromLuaMulti;

    #[cfg(feature = "async")]
    #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
    fn call_async_method<R>(&self, name: &str, args: impl IntoLuaMulti) -> AsyncCallFuture<R>
    where
        R: FromLuaMulti;

    fn call_function<R>(&self, name: &str, args: impl IntoLuaMulti) -> Result<R>
    where
        R: FromLuaMulti;

    #[cfg(feature = "async")]
    #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
    fn call_async_function<R>(&self, name: &str, args: impl IntoLuaMulti) -> AsyncCallFuture<R>
    where
        R: FromLuaMulti;

    fn get_path<V: FromLua>(&self, path: &str) -> Result<V> {
        let mut current = self.to_value();
        for (key, safe_nil) in parse_lookup_path(path)? {
            current = match current {
                Value::Table(table) => table.get::<Value>(key),
                Value::UserData(ud) => ud.get::<Value>(key),
                _ => {
                    let type_name = current.type_name();
                    let err = format!("attempt to index a {type_name} value with key '{key}'");
                    Err(Error::runtime(err))
                }
            }?;
            if safe_nil && (current == Value::Nil || current == Value::NULL) {
                break;
            }
        }

        let lua = self.weak_lua().lock();
        V::from_lua(current, lua.lua())
    }

    fn to_string(&self) -> Result<StdString>;

    fn to_value(&self) -> Value;

    #[doc(hidden)]
    fn weak_lua(&self) -> &WeakLua;
}

pub trait LuaNativeFn<A: FromLuaMulti> {
    type Output: IntoLuaMulti;

    fn call(&self, args: A) -> Self::Output;
}

pub trait LuaNativeFnMut<A: FromLuaMulti> {
    type Output: IntoLuaMulti;

    fn call(&mut self, args: A) -> Self::Output;
}

#[cfg(feature = "async")]
pub trait LuaNativeAsyncFn<A: FromLuaMulti> {
    type Output: IntoLuaMulti;

    fn call(&self, args: A) -> impl Future<Output = Self::Output> + MaybeSend + 'static;
}

macro_rules! impl_lua_native_fn {
    ($($A:ident),*) => {
        impl<FN, $($A,)* R> LuaNativeFn<($($A,)*)> for FN
        where
            FN: Fn($($A,)*) -> R + MaybeSend + 'static,
            ($($A,)*): FromLuaMulti,
            R: IntoLuaMulti,
        {
            type Output = R;

            #[allow(non_snake_case)]
            fn call(&self, args: ($($A,)*)) -> Self::Output {
                let ($($A,)*) = args;
                self($($A,)*)
            }
        }

        impl<FN, $($A,)* R> LuaNativeFnMut<($($A,)*)> for FN
        where
            FN: FnMut($($A,)*) -> R + MaybeSend + 'static,
            ($($A,)*): FromLuaMulti,
            R: IntoLuaMulti,
        {
            type Output = R;

            #[allow(non_snake_case)]
            fn call(&mut self, args: ($($A,)*)) -> Self::Output {
                let ($($A,)*) = args;
                self($($A,)*)
            }
        }

        #[cfg(feature = "async")]
        impl<FN, $($A,)* Fut, R> LuaNativeAsyncFn<($($A,)*)> for FN
        where
            FN: Fn($($A,)*) -> Fut + MaybeSend + 'static,
            ($($A,)*): FromLuaMulti,
            Fut: Future<Output = R> + MaybeSend + 'static,
            R: IntoLuaMulti,
        {
            type Output = R;

            #[allow(non_snake_case)]
            fn call(&self, args: ($($A,)*)) -> impl Future<Output = Self::Output> + MaybeSend + 'static {
                let ($($A,)*) = args;
                self($($A,)*)
            }
        }
    };
}

impl_lua_native_fn!();
impl_lua_native_fn!(A);
impl_lua_native_fn!(A, B);
impl_lua_native_fn!(A, B, C);
impl_lua_native_fn!(A, B, C, D);
impl_lua_native_fn!(A, B, C, D, E);
impl_lua_native_fn!(A, B, C, D, E, F);
impl_lua_native_fn!(A, B, C, D, E, F, G);
impl_lua_native_fn!(A, B, C, D, E, F, G, H);
impl_lua_native_fn!(A, B, C, D, E, F, G, H, I);
impl_lua_native_fn!(A, B, C, D, E, F, G, H, I, J);
impl_lua_native_fn!(A, B, C, D, E, F, G, H, I, J, K);
impl_lua_native_fn!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_lua_native_fn!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_lua_native_fn!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_lua_native_fn!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_lua_native_fn!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);

pub(crate) trait ShortTypeName {
    #[inline(always)]
    fn type_name() -> StdString {
        short_type_name::<Self>()
    }
}

impl<T> ShortTypeName for T {}
