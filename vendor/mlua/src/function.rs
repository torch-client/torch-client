use std::cell::RefCell;
use std::os::raw::{c_int, c_void};
use std::{mem, ptr, slice};

use crate::error::{Error, Result};
use crate::state::Lua;
use crate::table::Table;
use crate::traits::{FromLuaMulti, IntoLua, IntoLuaMulti, LuaNativeFn, LuaNativeFnMut};
use crate::types::{Callback, LuaType, MaybeSend, ValueRef};
use crate::util::{
    assert_stack, check_stack, linenumber_to_usize, pop_error, ptr_to_lossy_str, ptr_to_str, StackGuard,
};
use crate::value::Value;

#[cfg(feature = "async")]
use {
    crate::thread::AsyncThread,
    crate::traits::LuaNativeAsyncFn,
    crate::types::AsyncCallback,
    std::future::{self, Future},
    std::pin::{pin, Pin},
    std::task::{Context, Poll},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Function(pub(crate) ValueRef);

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct FunctionInfo {
    pub name: Option<String>,
    pub name_what: Option<&'static str>,
    pub what: &'static str,
    pub source: Option<String>,
    pub short_src: Option<String>,
    pub line_defined: Option<usize>,
    pub last_line_defined: Option<usize>,
    pub num_upvalues: u8,
    #[cfg(any(not(any(feature = "lua51", feature = "luajit")), doc))]
    #[cfg_attr(docsrs, doc(cfg(not(any(feature = "lua51", feature = "luajit")))))]
    pub num_params: u8,
    #[cfg(any(not(any(feature = "lua51", feature = "luajit")), doc))]
    #[cfg_attr(docsrs, doc(cfg(not(any(feature = "lua51", feature = "luajit")))))]
    pub is_vararg: bool,
}

#[cfg(any(feature = "luau", doc))]
#[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageInfo {
    pub function: Option<String>,
    pub line_defined: i32,
    pub depth: i32,
    pub hits: Vec<i32>,
}

impl Function {
    pub fn call<R: FromLuaMulti>(&self, args: impl IntoLuaMulti) -> Result<R> {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 2)?;

            lua.push_error_traceback();
            let stack_start = ffi::lua_gettop(state);
            lua.push_ref(&self.0);
            let nargs = args.push_into_stack_multi(&lua)?;
            let ret = ffi::lua_pcall(state, nargs, ffi::LUA_MULTRET, stack_start);
            if ret != ffi::LUA_OK {
                return Err(pop_error(state, ret));
            }
            let nresults = ffi::lua_gettop(state) - stack_start;
            R::from_stack_multi(nresults, &lua)
        }
    }

    #[cfg(feature = "async")]
    #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
    pub fn call_async<R>(&self, args: impl IntoLuaMulti) -> AsyncCallFuture<R>
    where
        R: FromLuaMulti,
    {
        let lua = self.0.lua.lock();
        AsyncCallFuture(unsafe {
            lua.create_recycled_thread(self).and_then(|th| {
                let mut th = th.into_async(args)?;
                th.set_recyclable(true);
                Ok(th)
            })
        })
    }

    pub fn bind(&self, args: impl IntoLuaMulti) -> Result<Function> {
        unsafe extern "C-unwind" fn args_wrapper_impl(state: *mut ffi::lua_State) -> c_int {
            let nargs = ffi::lua_gettop(state);
            let nbinds = ffi::lua_tointeger(state, ffi::lua_upvalueindex(1)) as c_int;
            ffi::luaL_checkstack(state, nbinds, ptr::null());

            for i in 0..nbinds {
                ffi::lua_pushvalue(state, ffi::lua_upvalueindex(i + 2));
            }
            if nargs > 0 {
                ffi::lua_rotate(state, 1, nbinds);
            }

            nargs + nbinds
        }

        let lua = self.0.lua.lock();
        let state = lua.state();

        let args = args.into_lua_multi(lua.lua())?;
        let nargs = args.len() as c_int;

        if nargs == 0 {
            return Ok(self.clone());
        }

        if nargs + 1 > ffi::LUA_MAX_UPVALUES {
            return Err(Error::BindError);
        }

        let args_wrapper = unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, nargs + 3)?;

            ffi::lua_pushinteger(state, nargs as ffi::lua_Integer);
            for arg in &args {
                lua.push_value(arg)?;
            }
            protect_lua!(state, nargs + 1, 1, fn(state) {
                ffi::lua_pushcclosure(state, args_wrapper_impl, ffi::lua_gettop(state));
            })?;

            Function(lua.pop_ref())
        };

        let lua = lua.lua();
        lua.load(
            r#"
            local func, args_wrapper = ...
            return function(...)
                return func(args_wrapper(...))
            end
            "#,
        )
        .try_cache()
        .set_name("=__mlua_bind")
        .call((self, args_wrapper))
    }

    pub fn environment(&self) -> Option<Table> {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            assert_stack(state, 1);

            lua.push_ref(&self.0);
            if ffi::lua_iscfunction(state, -1) != 0 {
                return None;
            }

            #[cfg(any(feature = "lua51", feature = "luajit", feature = "luau"))]
            ffi::lua_getfenv(state, -1);
            #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53", feature = "lua52"))]
            for i in 1..=255 {
                match ffi::lua_getupvalue(state, -1, i) {
                    s if s.is_null() => break,
                    s if std::ffi::CStr::from_ptr(s as _) == c"_ENV" => break,
                    _ => ffi::lua_pop(state, 1),
                }
            }

            if ffi::lua_type(state, -1) != ffi::LUA_TTABLE {
                return None;
            }
            Some(Table(lua.pop_ref()))
        }
    }

    pub fn set_environment(&self, env: Table) -> Result<bool> {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 2)?;

            lua.push_ref(&self.0);
            if ffi::lua_iscfunction(state, -1) != 0 {
                return Ok(false);
            }

            #[cfg(any(feature = "lua51", feature = "luajit", feature = "luau"))]
            {
                lua.push_ref(&env.0);
                ffi::lua_setfenv(state, -2);
            }
            #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53", feature = "lua52"))]
            for i in 1..=255 {
                match ffi::lua_getupvalue(state, -1, i) {
                    s if s.is_null() => return Ok(false),
                    s if std::ffi::CStr::from_ptr(s as _) == c"_ENV" => {
                        ffi::lua_pop(state, 1);
                        let f_with_env = lua
                            .lua()
                            .load("return _ENV")
                            .set_environment(env)
                            .try_cache()
                            .into_function()?;
                        lua.push_ref(&f_with_env.0);
                        ffi::lua_upvaluejoin(state, -2, i, -1, 1);
                        break;
                    }
                    _ => ffi::lua_pop(state, 1),
                }
            }

            Ok(true)
        }
    }

    pub fn info(&self) -> FunctionInfo {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            assert_stack(state, 1);

            let mut ar: ffi::lua_Debug = mem::zeroed();
            lua.push_ref(&self.0);

            #[cfg(not(feature = "luau"))]
            let res = ffi::lua_getinfo(state, cstr!(">Snu"), &mut ar);
            #[cfg(not(feature = "luau"))]
            mlua_assert!(res != 0, "lua_getinfo failed with `>Snu`");

            #[cfg(feature = "luau")]
            let res = ffi::lua_getinfo(state, -1, cstr!("snau"), &mut ar);
            #[cfg(feature = "luau")]
            mlua_assert!(res != 0, "lua_getinfo failed with `snau`");

            FunctionInfo {
                name: ptr_to_lossy_str(ar.name).map(|s| s.into_owned()),
                #[cfg(not(feature = "luau"))]
                name_what: match ptr_to_str(ar.namewhat) {
                    Some("") => None,
                    val => val,
                },
                #[cfg(feature = "luau")]
                name_what: None,
                what: ptr_to_str(ar.what).unwrap_or("main"),
                source: ptr_to_lossy_str(ar.source).map(|s| s.into_owned()),
                #[cfg(not(feature = "luau"))]
                short_src: ptr_to_lossy_str(ar.short_src.as_ptr()).map(|s| s.into_owned()),
                #[cfg(feature = "luau")]
                short_src: ptr_to_lossy_str(ar.short_src).map(|s| s.into_owned()),
                line_defined: linenumber_to_usize(ar.linedefined),
                #[cfg(not(feature = "luau"))]
                last_line_defined: linenumber_to_usize(ar.lastlinedefined),
                #[cfg(feature = "luau")]
                last_line_defined: None,
                #[cfg(not(feature = "luau"))]
                num_upvalues: ar.nups as _,
                #[cfg(feature = "luau")]
                num_upvalues: ar.nupvals,
                #[cfg(not(any(feature = "lua51", feature = "luajit")))]
                num_params: ar.nparams,
                #[cfg(not(any(feature = "lua51", feature = "luajit")))]
                is_vararg: ar.isvararg != 0,
            }
        }
    }

    #[cfg(not(feature = "luau"))]
    #[cfg_attr(docsrs, doc(cfg(not(feature = "luau"))))]
    pub fn dump(&self, strip: bool) -> Vec<u8> {
        unsafe extern "C-unwind" fn writer(
            _state: *mut ffi::lua_State,
            buf: *const c_void,
            buf_len: usize,
            data_ptr: *mut c_void,
        ) -> c_int {
            if !data_ptr.is_null() && buf_len > 0 {
                let data = &mut *(data_ptr as *mut Vec<u8>);
                let buf = slice::from_raw_parts(buf as *const u8, buf_len);
                data.extend_from_slice(buf);
            }
            0
        }

        let lua = self.0.lua.lock();
        let state = lua.state();
        let mut data: Vec<u8> = Vec::new();
        unsafe {
            let _sg = StackGuard::new(state);
            assert_stack(state, 1);

            lua.push_ref(&self.0);
            let data_ptr = &mut data as *mut Vec<u8> as *mut c_void;
            ffi::lua_dump(state, writer, data_ptr, strip as i32);
            ffi::lua_pop(state, 1);
        }

        data
    }

    #[cfg(any(feature = "luau", doc))]
    #[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
    pub fn coverage<F>(&self, func: F)
    where
        F: FnMut(CoverageInfo),
    {
        use std::ffi::CStr;
        use std::os::raw::c_char;

        unsafe extern "C-unwind" fn callback<F: FnMut(CoverageInfo)>(
            data: *mut c_void,
            function: *const c_char,
            line_defined: c_int,
            depth: c_int,
            hits: *const c_int,
            size: usize,
        ) {
            let function = if !function.is_null() {
                Some(CStr::from_ptr(function).to_string_lossy().to_string())
            } else {
                None
            };
            let rust_callback = &*(data as *const RefCell<F>);
            if let Ok(mut rust_callback) = rust_callback.try_borrow_mut() {
                rust_callback(CoverageInfo {
                    function,
                    line_defined,
                    depth,
                    hits: slice::from_raw_parts(hits, size).to_vec(),
                });
            }
        }

        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            assert_stack(state, 1);

            lua.push_ref(&self.0);
            let func = RefCell::new(func);
            let func_ptr = &func as *const RefCell<F> as *mut c_void;
            ffi::lua_getcoverage(state, -1, func_ptr, callback::<F>);
        }
    }

    #[inline]
    pub fn to_pointer(&self) -> *const c_void {
        self.0.to_pointer()
    }

    #[cfg(any(feature = "luau", doc))]
    #[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
    pub fn deep_clone(&self) -> Result<Self> {
        let lua = self.0.lua.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 2)?;

            lua.push_ref(&self.0);
            if ffi::lua_iscfunction(state, -1) != 0 {
                return Ok(self.clone());
            }

            if lua.unlikely_memory_error() {
                ffi::lua_clonefunction(state, -1);
            } else {
                protect_lua!(state, 1, 1, fn(state) ffi::lua_clonefunction(state, -1))?;
            }
            Ok(Function(lua.pop_ref()))
        }
    }
}

struct WrappedFunction(pub(crate) Callback);

#[cfg(feature = "async")]
struct WrappedAsyncFunction(pub(crate) AsyncCallback);

impl Function {
    #[inline]
    pub fn wrap<F, A, R>(func: F) -> impl IntoLua
    where
        F: LuaNativeFn<A, Output = Result<R>> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
        WrappedFunction(Box::new(move |lua, nargs| unsafe {
            let args = A::from_stack_args(nargs, 1, None, lua)?;
            func.call(args)?.push_into_stack_multi(lua)
        }))
    }

    pub fn wrap_mut<F, A, R>(func: F) -> impl IntoLua
    where
        F: LuaNativeFnMut<A, Output = Result<R>> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
        let func = RefCell::new(func);
        WrappedFunction(Box::new(move |lua, nargs| unsafe {
            let mut func = func.try_borrow_mut().map_err(|_| Error::RecursiveMutCallback)?;
            let args = A::from_stack_args(nargs, 1, None, lua)?;
            func.call(args)?.push_into_stack_multi(lua)
        }))
    }

    #[inline]
    pub fn wrap_raw<F, A>(func: F) -> impl IntoLua
    where
        F: LuaNativeFn<A> + MaybeSend + 'static,
        A: FromLuaMulti,
    {
        WrappedFunction(Box::new(move |lua, nargs| unsafe {
            let args = A::from_stack_args(nargs, 1, None, lua)?;
            func.call(args).push_into_stack_multi(lua)
        }))
    }

    #[inline]
    pub fn wrap_raw_mut<F, A>(func: F) -> impl IntoLua
    where
        F: LuaNativeFnMut<A> + MaybeSend + 'static,
        A: FromLuaMulti,
    {
        let func = RefCell::new(func);
        WrappedFunction(Box::new(move |lua, nargs| unsafe {
            let mut func = func.try_borrow_mut().map_err(|_| Error::RecursiveMutCallback)?;
            let args = A::from_stack_args(nargs, 1, None, lua)?;
            func.call(args).push_into_stack_multi(lua)
        }))
    }

    #[cfg(feature = "async")]
    #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
    pub fn wrap_async<F, A, R>(func: F) -> impl IntoLua
    where
        F: LuaNativeAsyncFn<A, Output = Result<R>> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
        WrappedAsyncFunction(Box::new(move |rawlua, nargs| unsafe {
            let args = match A::from_stack_args(nargs, 1, None, rawlua) {
                Ok(args) => args,
                Err(e) => return Box::pin(future::ready(Err(e))),
            };
            let lua = rawlua.lua();
            let fut = func.call(args);
            Box::pin(async move { fut.await?.push_into_stack_multi(lua.raw_lua()) })
        }))
    }

    #[cfg(feature = "async")]
    #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
    pub fn wrap_raw_async<F, A>(func: F) -> impl IntoLua
    where
        F: LuaNativeAsyncFn<A> + MaybeSend + 'static,
        A: FromLuaMulti,
    {
        WrappedAsyncFunction(Box::new(move |rawlua, nargs| unsafe {
            let args = match A::from_stack_args(nargs, 1, None, rawlua) {
                Ok(args) => args,
                Err(e) => return Box::pin(future::ready(Err(e))),
            };
            let lua = rawlua.lua();
            let fut = func.call(args);
            Box::pin(async move { fut.await.push_into_stack_multi(lua.raw_lua()) })
        }))
    }
}

impl IntoLua for WrappedFunction {
    #[inline]
    fn into_lua(self, lua: &Lua) -> Result<Value> {
        lua.lock().create_callback(self.0).map(Value::Function)
    }
}

#[cfg(feature = "async")]
impl IntoLua for WrappedAsyncFunction {
    #[inline]
    fn into_lua(self, lua: &Lua) -> Result<Value> {
        lua.lock().create_async_callback(self.0).map(Value::Function)
    }
}

impl LuaType for Function {
    const TYPE_ID: c_int = ffi::LUA_TFUNCTION;
}

#[cfg(feature = "async")]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct AsyncCallFuture<R: FromLuaMulti>(Result<AsyncThread<R>>);

#[cfg(feature = "async")]
impl<R: FromLuaMulti> AsyncCallFuture<R> {
    pub(crate) fn error(err: Error) -> Self {
        AsyncCallFuture(Err(err))
    }
}

#[cfg(feature = "async")]
impl<R: FromLuaMulti> Future for AsyncCallFuture<R> {
    type Output = Result<R>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match &mut this.0 {
            Ok(thread) => pin!(thread).poll(cx),
            Err(err) => Poll::Ready(Err(err.clone())),
        }
    }
}

#[cfg(test)]
mod assertions {
    use super::*;

    #[cfg(not(feature = "send"))]
    static_assertions::assert_not_impl_any!(Function: Send);
    #[cfg(feature = "send")]
    static_assertions::assert_impl_all!(Function: Send, Sync);

    #[cfg(all(feature = "async", feature = "send"))]
    static_assertions::assert_impl_all!(AsyncCallFuture<()>: Send);
}
