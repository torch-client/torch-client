use std::any::TypeId;
use std::cell::{BorrowError, BorrowMutError, RefCell};
use std::marker::PhantomData;
use std::ops::Deref;
use std::os::raw::{c_char, c_int};
use std::panic::Location;
use std::result::Result as StdResult;
use std::{fmt, mem, ptr};

use crate::chunk::{AsChunk, Chunk};
use crate::debug::Debug;
use crate::error::{Error, Result};
use crate::function::Function;
use crate::memory::MemoryState;
use crate::multi::MultiValue;
use crate::scope::Scope;
use crate::stdlib::StdLib;
use crate::string::String;
use crate::table::Table;
use crate::thread::Thread;
use crate::traits::{FromLua, FromLuaMulti, IntoLua, IntoLuaMulti};
use crate::types::{
    AppDataRef, AppDataRefMut, ArcReentrantMutexGuard, Integer, LuaType, MaybeSend, Number, ReentrantMutex,
    ReentrantMutexGuard, RegistryKey, VmState, XRc, XWeak,
};
use crate::userdata::{AnyUserData, UserData, UserDataProxy, UserDataRegistry, UserDataStorage};
use crate::util::{assert_stack, check_stack, protect_lua_closure, push_string, rawset_field, StackGuard};
use crate::value::{Nil, Value};

#[cfg(not(feature = "luau"))]
use crate::{debug::HookTriggers, types::HookKind};

#[cfg(any(feature = "luau", doc))]
use crate::{buffer::Buffer, chunk::Compiler};

#[cfg(feature = "async")]
use {
    crate::types::LightUserData,
    std::future::{self, Future},
    std::task::Poll,
};

#[cfg(feature = "serde")]
use serde::Serialize;

pub(crate) use extra::ExtraData;
pub use raw::RawLua;
pub(crate) use util::callback_error_ext;

pub struct Lua {
    pub(self) raw: XRc<ReentrantMutex<RawLua>>,
    pub(self) collect_garbage: bool,
}

#[derive(Clone)]
pub struct WeakLua(XWeak<ReentrantMutex<RawLua>>);

pub(crate) struct LuaGuard(ArcReentrantMutexGuard<RawLua>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GCMode {
    Incremental,
    #[cfg(any(feature = "lua55", feature = "lua54"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "lua55", feature = "lua54"))))]
    Generational,
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct LuaOptions {
    pub catch_rust_panics: bool,

    #[cfg(feature = "async")]
    #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
    pub thread_pool_size: usize,
}

impl Default for LuaOptions {
    fn default() -> Self {
        const { LuaOptions::new() }
    }
}

impl LuaOptions {
    pub const fn new() -> Self {
        LuaOptions {
            catch_rust_panics: true,
            #[cfg(feature = "async")]
            thread_pool_size: 0,
        }
    }

    #[must_use]
    pub const fn catch_rust_panics(mut self, enabled: bool) -> Self {
        self.catch_rust_panics = enabled;
        self
    }

    #[cfg(feature = "async")]
    #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
    #[must_use]
    pub const fn thread_pool_size(mut self, size: usize) -> Self {
        self.thread_pool_size = size;
        self
    }
}

impl Drop for Lua {
    fn drop(&mut self) {
        if self.collect_garbage {
            let _ = self.gc_collect();
        }
    }
}

impl Clone for Lua {
    #[inline]
    fn clone(&self) -> Self {
        Lua {
            raw: XRc::clone(&self.raw),
            collect_garbage: false,
        }
    }
}

impl fmt::Debug for Lua {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Lua({:p})", self.lock().state())
    }
}

impl Default for Lua {
    #[inline]
    fn default() -> Self {
        Lua::new()
    }
}

impl Lua {
    pub fn new() -> Lua {
        mlua_expect!(
            Self::new_with(StdLib::ALL_SAFE, LuaOptions::default()),
            "Cannot create a Lua state"
        )
    }

    pub unsafe fn unsafe_new() -> Lua {
        Self::unsafe_new_with(StdLib::ALL, LuaOptions::default())
    }

    pub fn new_with(libs: StdLib, options: LuaOptions) -> Result<Lua> {
        #[cfg(not(feature = "luau"))]
        if libs.contains(StdLib::DEBUG) {
            return Err(Error::SafetyError(
                "The unsafe `debug` module can't be loaded using safe `new_with`".to_string(),
            ));
        }
        #[cfg(feature = "luajit")]
        if libs.contains(StdLib::FFI) {
            return Err(Error::SafetyError(
                "The unsafe `ffi` module can't be loaded using safe `new_with`".to_string(),
            ));
        }

        let lua = unsafe { Self::inner_new(libs, options) };

        #[cfg(not(feature = "luau"))]
        if libs.contains(StdLib::PACKAGE) {
            mlua_expect!(lua.disable_c_modules(), "Error disabling C modules");
        }
        lua.lock().mark_safe();

        Ok(lua)
    }

    pub unsafe fn unsafe_new_with(libs: StdLib, options: LuaOptions) -> Lua {
        let mut _symbols: Vec<*const extern "C-unwind" fn()> =
            vec![ffi::lua_isuserdata as _, ffi::lua_tocfunction as _];

        #[cfg(not(feature = "luau"))]
        _symbols.extend_from_slice(&[
            ffi::lua_atpanic as _,
            ffi::luaL_loadstring as _,
            ffi::luaL_openlibs as _,
        ]);
        #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53", feature = "lua52"))]
        {
            _symbols.push(ffi::lua_getglobal as _);
            _symbols.push(ffi::lua_setglobal as _);
            _symbols.push(ffi::luaL_setfuncs as _);
        }

        Self::inner_new(libs, options)
    }

    unsafe fn inner_new(libs: StdLib, options: LuaOptions) -> Lua {
        let lua = Lua {
            raw: RawLua::new(libs, &options),
            collect_garbage: true,
        };

        #[cfg(feature = "luau")]
        mlua_expect!(lua.configure_luau(), "Error configuring Luau");

        lua
    }

    #[inline]
    pub unsafe fn get_or_init_from_ptr<'a>(state: *mut ffi::lua_State) -> &'a Lua {
        debug_assert!(!state.is_null(), "Lua state is null");
        match ExtraData::get(state) {
            extra if !extra.is_null() => (*extra).lua(),
            _ => {
                RawLua::init_from_ptr(state, false);
                (*ExtraData::get(state)).lua()
            }
        }
    }

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn exec_raw<R: FromLuaMulti>(
        &self,
        args: impl IntoLuaMulti,
        f: impl FnOnce(*mut ffi::lua_State),
    ) -> Result<R> {
        let lua = self.lock();
        let state = lua.state();
        let _sg = StackGuard::new(state);
        let stack_start = ffi::lua_gettop(state);
        let nargs = args.push_into_stack_multi(&lua)?;
        check_stack(state, 3)?;
        protect_lua_closure::<_, ()>(state, nargs, ffi::LUA_MULTRET, f)?;
        let nresults = ffi::lua_gettop(state) - stack_start;
        R::from_stack_multi(nresults, &lua)
    }

    #[doc(hidden)]
    pub fn exec_raw_lua<R>(&self, f: impl FnOnce(&RawLua) -> R) -> R {
        let lua = self.lock();
        f(&lua)
    }

    pub fn load_std_libs(&self, libs: StdLib) -> Result<()> {
        unsafe { self.lock().load_std_libs(libs) }
    }

    pub fn register_module(&self, modname: &str, value: impl IntoLua) -> Result<()> {
        #[cfg(not(feature = "luau"))]
        const LOADED_MODULES_KEY: *const c_char = ffi::LUA_LOADED_TABLE;
        #[cfg(feature = "luau")]
        const LOADED_MODULES_KEY: *const c_char = ffi::LUA_REGISTERED_MODULES_TABLE;

        if cfg!(feature = "luau") && !modname.starts_with('@') {
            return Err(Error::runtime("module name must begin with '@'"));
        }
        #[cfg(feature = "luau")]
        let modname = modname.to_ascii_lowercase();
        unsafe {
            self.exec_raw::<()>(value, |state| {
                ffi::luaL_getsubtable(state, ffi::LUA_REGISTRYINDEX, LOADED_MODULES_KEY);
                ffi::lua_pushlstring(state, modname.as_ptr() as *const c_char, modname.len() as _);
                ffi::lua_pushvalue(state, -3);
                ffi::lua_rawset(state, -3);
            })
        }
    }

    #[cfg(not(feature = "luau"))]
    #[cfg_attr(docsrs, doc(cfg(not(feature = "luau"))))]
    pub fn preload_module(&self, modname: &str, func: Function) -> Result<()> {
        #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53", feature = "lua52"))]
        let preload = unsafe {
            self.exec_raw::<Option<Table>>((), |state| {
                ffi::lua_getfield(state, ffi::LUA_REGISTRYINDEX, ffi::LUA_PRELOAD_TABLE);
            })?
        };
        #[cfg(any(feature = "lua51", feature = "luajit"))]
        let preload = unsafe {
            self.exec_raw::<Option<Table>>((), |state| {
                if ffi::lua_getfield(state, ffi::LUA_REGISTRYINDEX, ffi::LUA_LOADED_TABLE) != ffi::LUA_TNIL {
                    ffi::luaL_getsubtable(state, -1, ffi::LUA_LOADLIBNAME);
                    ffi::luaL_getsubtable(state, -1, cstr!("preload"));
                    ffi::lua_rotate(state, 1, 1);
                }
            })?
        };
        if let Some(preload) = preload {
            preload.raw_set(modname, func)?;
        }
        Ok(())
    }

    #[doc(hidden)]
    #[deprecated(since = "0.11.0", note = "Use `register_module` instead")]
    #[cfg(not(feature = "luau"))]
    #[cfg(not(tarpaulin_include))]
    pub fn load_from_function<T: FromLua>(&self, modname: &str, func: Function) -> Result<T> {
        let loaded = unsafe {
            self.exec_raw::<Table>((), |state| {
                ffi::luaL_getsubtable(state, ffi::LUA_REGISTRYINDEX, ffi::LUA_LOADED_TABLE);
            })?
        };

        let value = match loaded.raw_get(modname)? {
            Value::Nil => {
                let result = match func.call(modname)? {
                    Value::Nil => Value::Boolean(true),
                    res => res,
                };
                loaded.raw_set(modname, &result)?;
                result
            }
            res => res,
        };
        T::from_lua(value, self)
    }

    pub fn unload_module(&self, modname: &str) -> Result<()> {
        self.register_module(modname, Nil)
    }

    #[doc(hidden)]
    #[cfg(not(tarpaulin_include))]
    pub unsafe fn entrypoint<F, A, R>(state: *mut ffi::lua_State, func: F) -> c_int
    where
        F: FnOnce(&Lua, A) -> Result<R>,
        A: FromLuaMulti,
        R: IntoLua,
    {
        let _ = Self::get_or_init_from_ptr(state);

        callback_error_ext(state, ptr::null_mut(), true, move |extra, nargs| {
            let rawlua = (*extra).raw_lua();
            let args = A::from_stack_args(nargs, 1, None, rawlua)?;
            func(rawlua.lua(), args)?.push_into_stack(rawlua)?;
            Ok(1)
        })
    }

    #[doc(hidden)]
    #[cfg(not(tarpaulin_include))]
    pub unsafe fn entrypoint1<F, R>(state: *mut ffi::lua_State, func: F) -> c_int
    where
        F: FnOnce(&Lua) -> Result<R>,
        R: IntoLua,
    {
        Self::entrypoint(state, move |lua, _: ()| func(lua))
    }

    #[doc(hidden)]
    #[cfg(feature = "module")]
    pub fn skip_memory_check(&self, skip: bool) {
        let lua = self.lock();
        unsafe { (*lua.extra.get()).skip_memory_check = skip };
    }

    #[cfg(any(feature = "luau", doc))]
    #[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
    pub fn sandbox(&self, enabled: bool) -> Result<()> {
        let lua = self.lock();
        unsafe {
            if (*lua.extra.get()).sandboxed != enabled {
                let state = lua.main_state();
                check_stack(state, 3)?;
                protect_lua!(state, 0, 0, |state| {
                    if enabled {
                        ffi::luaL_sandbox(state, 1);
                        ffi::luaL_sandboxthread(state);
                    } else {
                        ffi::lua_xpush(lua.ref_thread(), state, ffi::LUA_GLOBALSINDEX);
                        ffi::lua_replace(state, ffi::LUA_GLOBALSINDEX);
                        ffi::luaL_sandbox(state, 0);
                    }
                })?;
                (*lua.extra.get()).sandboxed = enabled;
            }
            Ok(())
        }
    }

    #[cfg(not(feature = "luau"))]
    #[cfg_attr(docsrs, doc(cfg(not(feature = "luau"))))]
    pub fn set_global_hook<F>(&self, triggers: HookTriggers, callback: F) -> Result<()>
    where
        F: Fn(&Lua, &Debug) -> Result<VmState> + MaybeSend + 'static,
    {
        let lua = self.lock();
        unsafe {
            (*lua.extra.get()).hook_triggers = triggers;
            (*lua.extra.get()).hook_callback = Some(XRc::new(callback));
            lua.set_thread_hook(lua.state(), HookKind::Global)
        }
    }

    #[cfg(not(feature = "luau"))]
    #[cfg_attr(docsrs, doc(cfg(not(feature = "luau"))))]
    pub fn set_hook<F>(&self, triggers: HookTriggers, callback: F) -> Result<()>
    where
        F: Fn(&Lua, &Debug) -> Result<VmState> + MaybeSend + 'static,
    {
        let lua = self.lock();
        unsafe { lua.set_thread_hook(lua.state(), HookKind::Thread(triggers, XRc::new(callback))) }
    }

    #[cfg(not(feature = "luau"))]
    #[cfg_attr(docsrs, doc(cfg(not(feature = "luau"))))]
    pub fn remove_global_hook(&self) {
        let lua = self.lock();
        unsafe {
            (*lua.extra.get()).hook_callback = None;
            (*lua.extra.get()).hook_triggers = HookTriggers::default();
        }
    }

    #[cfg(not(feature = "luau"))]
    #[cfg_attr(docsrs, doc(cfg(not(feature = "luau"))))]
    pub fn remove_hook(&self) {
        let lua = self.lock();
        unsafe {
            ffi::lua_sethook(lua.state(), None, 0, 0);
        }
    }

    #[cfg(any(feature = "luau", doc))]
    #[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
    pub fn set_interrupt<F>(&self, callback: F)
    where
        F: Fn(&Lua) -> Result<VmState> + MaybeSend + 'static,
    {
        unsafe extern "C-unwind" fn interrupt_proc(state: *mut ffi::lua_State, gc: c_int) {
            if gc >= 0 {
                return;
            }
            let result = callback_error_ext(state, ptr::null_mut(), false, move |extra, _| {
                let interrupt_cb = (*extra).interrupt_callback.clone();
                let interrupt_cb = mlua_expect!(interrupt_cb, "no interrupt callback set in interrupt_proc");
                if XRc::strong_count(&interrupt_cb) > 2 {
                    return Ok(VmState::Continue);
                }
                interrupt_cb((*extra).lua())
            });
            match result {
                VmState::Continue => {}
                VmState::Yield => {
                    if ffi::lua_isyieldable(state) != 0 {
                        ffi::lua_yield(state, 0);
                    }
                }
            }
        }

        let lua = self.lock();
        unsafe {
            (*lua.extra.get()).interrupt_callback = Some(XRc::new(callback));
            (*ffi::lua_callbacks(lua.main_state())).interrupt = Some(interrupt_proc);
        }
    }

    #[cfg(any(feature = "luau", doc))]
    #[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
    pub fn remove_interrupt(&self) {
        let lua = self.lock();
        unsafe {
            (*lua.extra.get()).interrupt_callback = None;
            (*ffi::lua_callbacks(lua.main_state())).interrupt = None;
        }
    }

    #[cfg(any(feature = "luau", doc))]
    #[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
    pub fn set_thread_creation_callback<F>(&self, callback: F)
    where
        F: Fn(&Lua, Thread) -> Result<()> + MaybeSend + 'static,
    {
        let lua = self.lock();
        unsafe {
            (*lua.extra.get()).thread_creation_callback = Some(XRc::new(callback));
            (*ffi::lua_callbacks(lua.main_state())).userthread = Some(Self::userthread_proc);
        }
    }

    #[cfg(any(feature = "luau", doc))]
    #[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
    pub fn set_thread_collection_callback<F>(&self, callback: F)
    where
        F: Fn(crate::LightUserData) + MaybeSend + 'static,
    {
        let lua = self.lock();
        unsafe {
            (*lua.extra.get()).thread_collection_callback = Some(XRc::new(callback));
            (*ffi::lua_callbacks(lua.main_state())).userthread = Some(Self::userthread_proc);
        }
    }

    #[cfg(feature = "luau")]
    unsafe extern "C-unwind" fn userthread_proc(parent: *mut ffi::lua_State, child: *mut ffi::lua_State) {
        let extra = ExtraData::get(child);
        if !parent.is_null() {
            let callback = match (*extra).thread_creation_callback {
                Some(ref cb) => cb.clone(),
                None => return,
            };
            if XRc::strong_count(&callback) > 2 {
                return;
            }
            ffi::lua_pushthread(child);
            ffi::lua_xmove(child, (*extra).ref_thread, 1);
            let value = Thread((*extra).raw_lua().pop_ref_thread(), child);
            callback_error_ext(parent, extra, false, move |extra, _| {
                callback((*extra).lua(), value)
            })
        } else {
            let callback = match (*extra).thread_collection_callback {
                Some(ref cb) => cb.clone(),
                None => return,
            };

            unsafe extern "C" fn run_callback(
                callback: *const crate::types::ThreadCollectionCallback,
                value: *mut ffi::lua_State,
            ) {
                (*callback)(crate::LightUserData(value as _));
            }

            (*extra).running_gc = true;
            run_callback(&callback, child);
            (*extra).running_gc = false;
        }
    }

    #[cfg(any(feature = "luau", doc))]
    #[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
    pub fn remove_thread_callbacks(&self) {
        let lua = self.lock();
        unsafe {
            let extra = lua.extra.get();
            (*extra).thread_creation_callback = None;
            (*extra).thread_collection_callback = None;
            (*ffi::lua_callbacks(lua.main_state())).userthread = None;
        }
    }

    #[cfg(any(feature = "lua55", feature = "lua54"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "lua55", feature = "lua54"))))]
    pub fn set_warning_function<F>(&self, callback: F)
    where
        F: Fn(&Lua, &str, bool) -> Result<()> + MaybeSend + 'static,
    {
        use std::ffi::CStr;
        use std::os::raw::{c_char, c_void};
        use std::string::String as StdString;

        unsafe extern "C-unwind" fn warn_proc(ud: *mut c_void, msg: *const c_char, tocont: c_int) {
            let extra = ud as *mut ExtraData;
            callback_error_ext((*extra).raw_lua().state(), extra, false, |extra, _| {
                let warn_callback = (*extra).warn_callback.clone();
                let warn_callback = mlua_expect!(warn_callback, "no warning callback set in warn_proc");
                if XRc::strong_count(&warn_callback) > 2 {
                    return Ok(());
                }
                let msg = StdString::from_utf8_lossy(CStr::from_ptr(msg).to_bytes());
                warn_callback((*extra).lua(), &msg, tocont != 0)
            });
        }

        let lua = self.lock();
        unsafe {
            (*lua.extra.get()).warn_callback = Some(XRc::new(callback));
            ffi::lua_setwarnf(lua.state(), Some(warn_proc), lua.extra.get() as *mut c_void);
        }
    }

    #[cfg(any(feature = "lua55", feature = "lua54"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "lua55", feature = "lua54"))))]
    pub fn remove_warning_function(&self) {
        let lua = self.lock();
        unsafe {
            (*lua.extra.get()).warn_callback = None;
            ffi::lua_setwarnf(lua.state(), None, ptr::null_mut());
        }
    }

    #[cfg(any(feature = "lua55", feature = "lua54"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "lua55", feature = "lua54"))))]
    pub fn warning(&self, msg: impl AsRef<str>, incomplete: bool) {
        let msg = msg.as_ref();
        let mut bytes = vec![0; msg.len() + 1];
        bytes[..msg.len()].copy_from_slice(msg.as_bytes());
        let real_len = bytes.iter().position(|&c| c == 0).unwrap();
        bytes.truncate(real_len);
        let lua = self.lock();
        unsafe {
            ffi::lua_warning(lua.state(), bytes.as_ptr() as *const _, incomplete as c_int);
        }
    }

    pub fn inspect_stack<R>(&self, level: usize, f: impl FnOnce(&Debug) -> R) -> Option<R> {
        let lua = self.lock();
        unsafe {
            let mut ar = mem::zeroed::<ffi::lua_Debug>();
            let level = level as c_int;
            #[cfg(not(feature = "luau"))]
            if ffi::lua_getstack(lua.state(), level, &mut ar) == 0 {
                return None;
            }
            #[cfg(feature = "luau")]
            if ffi::lua_getinfo(lua.state(), level, cstr!(""), &mut ar) == 0 {
                return None;
            }

            Some(f(&Debug::new(&lua, level, &mut ar)))
        }
    }

    pub fn traceback(&self, msg: Option<&str>, level: usize) -> Result<String> {
        let lua = self.lock();
        unsafe {
            check_stack(lua.state(), 3)?;
            protect_lua!(lua.state(), 0, 1, |state| {
                let msg = match msg {
                    Some(s) => ffi::lua_pushlstring(state, s.as_ptr() as *const c_char, s.len()),
                    None => ptr::null(),
                };
                ffi::luaL_traceback(state, state, msg, (level + 1) as c_int);
            })?;
            Ok(String(lua.pop_ref()))
        }
    }

    pub fn used_memory(&self) -> usize {
        let lua = self.lock();
        let state = lua.main_state();
        unsafe {
            match MemoryState::get(state) {
                mem_state if !mem_state.is_null() => (*mem_state).used_memory(),
                _ => {
                    let used_kbytes = ffi::lua_gc(state, ffi::LUA_GCCOUNT, 0);
                    let used_kbytes_rem = ffi::lua_gc(state, ffi::LUA_GCCOUNTB, 0);
                    (used_kbytes as usize) * 1024 + (used_kbytes_rem as usize)
                }
            }
        }
    }

    pub fn set_memory_limit(&self, limit: usize) -> Result<usize> {
        let lua = self.lock();
        unsafe {
            match MemoryState::get(lua.state()) {
                mem_state if !mem_state.is_null() => Ok((*mem_state).set_memory_limit(limit)),
                _ => Err(Error::MemoryControlNotAvailable),
            }
        }
    }

    #[cfg(any(
        feature = "lua55",
        feature = "lua54",
        feature = "lua53",
        feature = "lua52",
        feature = "luau"
    ))]
    pub fn gc_is_running(&self) -> bool {
        let lua = self.lock();
        unsafe { ffi::lua_gc(lua.main_state(), ffi::LUA_GCISRUNNING, 0) != 0 }
    }

    pub fn gc_stop(&self) {
        let lua = self.lock();
        unsafe { ffi::lua_gc(lua.main_state(), ffi::LUA_GCSTOP, 0) };
    }

    pub fn gc_restart(&self) {
        let lua = self.lock();
        unsafe { ffi::lua_gc(lua.main_state(), ffi::LUA_GCRESTART, 0) };
    }

    pub fn gc_collect(&self) -> Result<()> {
        let lua = self.lock();
        let state = lua.main_state();
        unsafe {
            check_stack(state, 2)?;
            protect_lua!(state, 0, 0, fn(state) ffi::lua_gc(state, ffi::LUA_GCCOLLECT, 0))
        }
    }

    pub fn gc_step(&self) -> Result<bool> {
        self.gc_step_kbytes(0)
    }

    pub fn gc_step_kbytes(&self, kbytes: c_int) -> Result<bool> {
        let lua = self.lock();
        let state = lua.main_state();
        unsafe {
            check_stack(state, 3)?;
            protect_lua!(state, 0, 0, |state| {
                ffi::lua_gc(state, ffi::LUA_GCSTEP, kbytes) != 0
            })
        }
    }

    pub fn gc_set_pause(&self, pause: c_int) -> c_int {
        let lua = self.lock();
        let state = lua.main_state();
        unsafe {
            #[cfg(feature = "lua55")]
            return ffi::lua_gc(state, ffi::LUA_GCPARAM, ffi::LUA_GCPPAUSE, pause);

            #[cfg(not(any(feature = "lua55", feature = "luau")))]
            return ffi::lua_gc(state, ffi::LUA_GCSETPAUSE, pause);

            #[cfg(feature = "luau")]
            return ffi::lua_gc(state, ffi::LUA_GCSETGOAL, pause);
        }
    }

    pub fn gc_set_step_multiplier(&self, step_multiplier: c_int) -> c_int {
        let lua = self.lock();
        unsafe {
            #[cfg(feature = "lua55")]
            return ffi::lua_gc(
                lua.main_state(),
                ffi::LUA_GCPARAM,
                ffi::LUA_GCPSTEPMUL,
                step_multiplier,
            );

            #[cfg(not(feature = "lua55"))]
            return ffi::lua_gc(lua.main_state(), ffi::LUA_GCSETSTEPMUL, step_multiplier);
        }
    }

    pub fn gc_inc(&self, pause: c_int, step_multiplier: c_int, step_size: c_int) -> GCMode {
        let lua = self.lock();
        let state = lua.main_state();

        #[cfg(any(
            feature = "lua53",
            feature = "lua52",
            feature = "lua51",
            feature = "luajit",
            feature = "luau"
        ))]
        unsafe {
            if pause > 0 {
                #[cfg(not(feature = "luau"))]
                ffi::lua_gc(state, ffi::LUA_GCSETPAUSE, pause);
                #[cfg(feature = "luau")]
                ffi::lua_gc(state, ffi::LUA_GCSETGOAL, pause);
            }

            if step_multiplier > 0 {
                ffi::lua_gc(state, ffi::LUA_GCSETSTEPMUL, step_multiplier);
            }

            #[cfg(feature = "luau")]
            if step_size > 0 {
                ffi::lua_gc(state, ffi::LUA_GCSETSTEPSIZE, step_size);
            }
            #[cfg(not(feature = "luau"))]
            let _ = step_size;

            return GCMode::Incremental;
        }

        #[cfg(feature = "lua55")]
        let prev_mode = unsafe {
            ffi::lua_gc(state, ffi::LUA_GCPARAM, ffi::LUA_GCPPAUSE, pause);
            ffi::lua_gc(state, ffi::LUA_GCPARAM, ffi::LUA_GCPSTEPMUL, step_multiplier);
            ffi::lua_gc(state, ffi::LUA_GCPARAM, ffi::LUA_GCPSTEPSIZE, step_size);
            ffi::lua_gc(state, ffi::LUA_GCINC)
        };
        #[cfg(feature = "lua54")]
        let prev_mode = unsafe { ffi::lua_gc(state, ffi::LUA_GCINC, pause, step_multiplier, step_size) };
        #[cfg(any(feature = "lua55", feature = "lua54"))]
        match prev_mode {
            ffi::LUA_GCINC => GCMode::Incremental,
            ffi::LUA_GCGEN => GCMode::Generational,
            _ => unreachable!(),
        }
    }

    #[cfg(any(feature = "lua55", feature = "lua54"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "lua55", feature = "lua54"))))]
    pub fn gc_gen(&self, minor_multiplier: c_int, major_multiplier: c_int) -> GCMode {
        let lua = self.lock();
        let state = lua.main_state();
        #[cfg(feature = "lua55")]
        let prev_mode = unsafe {
            ffi::lua_gc(state, ffi::LUA_GCPARAM, ffi::LUA_GCPMINORMUL, minor_multiplier);
            ffi::lua_gc(state, ffi::LUA_GCPARAM, ffi::LUA_GCPMINORMAJOR, major_multiplier);
            ffi::lua_gc(state, ffi::LUA_GCGEN)
        };
        #[cfg(not(feature = "lua55"))]
        let prev_mode = unsafe { ffi::lua_gc(state, ffi::LUA_GCGEN, minor_multiplier, major_multiplier) };
        match prev_mode {
            ffi::LUA_GCGEN => GCMode::Generational,
            ffi::LUA_GCINC => GCMode::Incremental,
            _ => unreachable!(),
        }
    }

    #[cfg(any(feature = "luau", doc))]
    #[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
    pub fn set_compiler(&self, compiler: Compiler) {
        let lua = self.lock();
        unsafe { (*lua.extra.get()).compiler = Some(compiler) };
    }

    #[cfg(any(feature = "luau-jit", doc))]
    #[cfg_attr(docsrs, doc(cfg(feature = "luau-jit")))]
    pub fn enable_jit(&self, enable: bool) {
        let lua = self.lock();
        unsafe { (*lua.extra.get()).enable_jit = enable };
    }

    #[cfg(feature = "luau")]
    #[doc(hidden)]
    #[allow(clippy::result_unit_err)]
    pub fn set_fflag(name: &str, enabled: bool) -> StdResult<(), ()> {
        if let Ok(name) = std::ffi::CString::new(name) {
            if unsafe { ffi::luau_setfflag(name.as_ptr(), enabled as c_int) != 0 } {
                return Ok(());
            }
        }
        Err(())
    }

    #[track_caller]
    pub fn load<'a>(&self, chunk: impl AsChunk + 'a) -> Chunk<'a> {
        self.load_with_location(chunk, Location::caller())
    }

    pub(crate) fn load_with_location<'a>(
        &self,
        chunk: impl AsChunk + 'a,
        location: &'static Location<'static>,
    ) -> Chunk<'a> {
        Chunk {
            lua: self.weak(),
            name: chunk
                .name()
                .unwrap_or_else(|| format!("@{}:{}", location.file(), location.line())),
            env: chunk.environment(self),
            mode: chunk.mode(),
            source: chunk.source(),
            #[cfg(feature = "luau")]
            compiler: unsafe { (*self.lock().extra.get()).compiler.clone() },
        }
    }

    #[inline]
    pub fn create_string(&self, s: impl AsRef<[u8]>) -> Result<String> {
        unsafe { self.lock().create_string(s.as_ref()) }
    }

    #[cfg(feature = "lua55")]
    #[cfg_attr(docsrs, doc(cfg(feature = "lua55")))]
    #[inline]
    pub fn create_external_string(&self, s: impl Into<Vec<u8>>) -> Result<String> {
        unsafe { self.lock().create_external_string(s.into()) }
    }

    #[cfg(any(feature = "luau", doc))]
    #[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
    pub fn create_buffer(&self, data: impl AsRef<[u8]>) -> Result<Buffer> {
        let lua = self.lock();
        let data = data.as_ref();
        unsafe {
            let (ptr, buffer) = lua.create_buffer_with_capacity(data.len())?;
            ptr.copy_from_nonoverlapping(data.as_ptr(), data.len());
            Ok(buffer)
        }
    }

    #[cfg(any(feature = "luau", doc))]
    #[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
    pub fn create_buffer_with_capacity(&self, size: usize) -> Result<Buffer> {
        unsafe { Ok(self.lock().create_buffer_with_capacity(size)?.1) }
    }

    #[inline]
    pub fn create_table(&self) -> Result<Table> {
        self.create_table_with_capacity(0, 0)
    }

    pub fn create_table_with_capacity(&self, narr: usize, nrec: usize) -> Result<Table> {
        unsafe { self.lock().create_table_with_capacity(narr, nrec) }
    }

    pub fn create_table_from<K, V>(&self, iter: impl IntoIterator<Item = (K, V)>) -> Result<Table>
    where
        K: IntoLua,
        V: IntoLua,
    {
        unsafe { self.lock().create_table_from(iter) }
    }

    pub fn create_sequence_from<T>(&self, iter: impl IntoIterator<Item = T>) -> Result<Table>
    where
        T: IntoLua,
    {
        unsafe { self.lock().create_sequence_from(iter) }
    }

    pub fn create_function<F, A, R>(&self, func: F) -> Result<Function>
    where
        F: Fn(&Lua, A) -> Result<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
        (self.lock()).create_callback(Box::new(move |rawlua, nargs| unsafe {
            let args = A::from_stack_args(nargs, 1, None, rawlua)?;
            func(rawlua.lua(), args)?.push_into_stack_multi(rawlua)
        }))
    }

    pub fn create_function_mut<F, A, R>(&self, func: F) -> Result<Function>
    where
        F: FnMut(&Lua, A) -> Result<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
        let func = RefCell::new(func);
        self.create_function(move |lua, args| {
            (*func.try_borrow_mut().map_err(|_| Error::RecursiveMutCallback)?)(lua, args)
        })
    }

    pub unsafe fn create_c_function(&self, func: ffi::lua_CFunction) -> Result<Function> {
        let lua = self.lock();
        if cfg!(any(
            feature = "lua55",
            feature = "lua54",
            feature = "lua53",
            feature = "lua52"
        )) {
            ffi::lua_pushcfunction(lua.ref_thread(), func);
            return Ok(Function(lua.pop_ref_thread()));
        }

        let state = lua.state();
        {
            let _sg = StackGuard::new(state);
            check_stack(state, 3)?;

            if lua.unlikely_memory_error() {
                ffi::lua_pushcfunction(state, func);
            } else {
                protect_lua!(state, 0, 1, |state| ffi::lua_pushcfunction(state, func))?;
            }
            Ok(Function(lua.pop_ref()))
        }
    }

    #[cfg(feature = "async")]
    #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
    pub fn create_async_function<F, A, FR, R>(&self, func: F) -> Result<Function>
    where
        F: Fn(Lua, A) -> FR + MaybeSend + 'static,
        A: FromLuaMulti,
        FR: Future<Output = Result<R>> + MaybeSend + 'static,
        R: IntoLuaMulti,
    {
        (self.lock()).create_async_callback(Box::new(move |rawlua, nargs| unsafe {
            let args = match A::from_stack_args(nargs, 1, None, rawlua) {
                Ok(args) => args,
                Err(e) => return Box::pin(future::ready(Err(e))),
            };
            let lua = rawlua.lua();
            let fut = func(lua.clone(), args);
            Box::pin(async move { fut.await?.push_into_stack_multi(lua.raw_lua()) })
        }))
    }

    pub fn create_thread(&self, func: Function) -> Result<Thread> {
        unsafe { self.lock().create_thread(&func) }
    }

    #[inline]
    pub fn create_userdata<T>(&self, data: T) -> Result<AnyUserData>
    where
        T: UserData + MaybeSend + 'static,
    {
        unsafe { self.lock().make_userdata(UserDataStorage::new(data)) }
    }

    #[cfg(feature = "serde")]
    #[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
    #[inline]
    pub fn create_ser_userdata<T>(&self, data: T) -> Result<AnyUserData>
    where
        T: UserData + Serialize + MaybeSend + 'static,
    {
        unsafe { self.lock().make_userdata(UserDataStorage::new_ser(data)) }
    }

    #[inline]
    pub fn create_any_userdata<T>(&self, data: T) -> Result<AnyUserData>
    where
        T: MaybeSend + 'static,
    {
        unsafe { self.lock().make_any_userdata(UserDataStorage::new(data)) }
    }

    #[cfg(feature = "serde")]
    #[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
    #[inline]
    pub fn create_ser_any_userdata<T>(&self, data: T) -> Result<AnyUserData>
    where
        T: Serialize + MaybeSend + 'static,
    {
        unsafe { (self.lock()).make_any_userdata(UserDataStorage::new_ser(data)) }
    }

    pub fn register_userdata_type<T: 'static>(&self, f: impl FnOnce(&mut UserDataRegistry<T>)) -> Result<()> {
        let type_id = TypeId::of::<T>();
        let mut registry = UserDataRegistry::new(self);
        f(&mut registry);

        let lua = self.lock();
        unsafe {
            if let Some(table_id) = (*lua.extra.get()).registered_userdata_t.remove(&type_id) {
                ffi::luaL_unref(lua.state(), ffi::LUA_REGISTRYINDEX, table_id);
            }

            ((*lua.extra.get()).pending_userdata_reg).insert(type_id, registry.into_raw());
        }
        Ok(())
    }

    #[inline]
    pub fn create_proxy<T>(&self) -> Result<AnyUserData>
    where
        T: UserData + 'static,
    {
        let ud = UserDataProxy::<T>(PhantomData);
        unsafe { self.lock().make_userdata(UserDataStorage::new(ud)) }
    }

    #[allow(private_bounds)]
    pub fn type_metatable<T: LuaType>(&self) -> Option<Table> {
        let lua = self.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            assert_stack(state, 2);

            if lua.push_primitive_type::<T>() && ffi::lua_getmetatable(state, -1) != 0 {
                return Some(Table(lua.pop_ref()));
            }
        }
        None
    }

    #[allow(private_bounds)]
    pub fn set_type_metatable<T: LuaType>(&self, metatable: Option<Table>) {
        let lua = self.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            assert_stack(state, 2);

            if lua.push_primitive_type::<T>() {
                match metatable {
                    Some(metatable) => lua.push_ref(&metatable.0),
                    None => ffi::lua_pushnil(state),
                }
                ffi::lua_setmetatable(state, -2);
            }
        }
    }

    pub fn globals(&self) -> Table {
        let lua = self.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            assert_stack(state, 1);
            #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53", feature = "lua52"))]
            ffi::lua_rawgeti(state, ffi::LUA_REGISTRYINDEX, ffi::LUA_RIDX_GLOBALS);
            #[cfg(any(feature = "lua51", feature = "luajit", feature = "luau"))]
            ffi::lua_pushvalue(state, ffi::LUA_GLOBALSINDEX);
            Table(lua.pop_ref())
        }
    }

    pub fn set_globals(&self, globals: Table) -> Result<()> {
        let lua = self.lock();
        let state = lua.state();
        unsafe {
            #[cfg(feature = "luau")]
            if (*lua.extra.get()).sandboxed {
                return Err(Error::runtime("cannot change globals in a sandboxed Lua state"));
            }

            let _sg = StackGuard::new(state);
            check_stack(state, 1)?;

            lua.push_ref(&globals.0);

            #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53", feature = "lua52"))]
            ffi::lua_rawseti(state, ffi::LUA_REGISTRYINDEX, ffi::LUA_RIDX_GLOBALS);
            #[cfg(any(feature = "lua51", feature = "luajit", feature = "luau"))]
            ffi::lua_replace(state, ffi::LUA_GLOBALSINDEX);
        }

        Ok(())
    }

    pub fn current_thread(&self) -> Thread {
        let lua = self.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            assert_stack(state, 1);
            ffi::lua_pushthread(state);
            Thread(lua.pop_ref(), state)
        }
    }

    pub fn scope<'env, R>(
        &self,
        f: impl for<'scope> FnOnce(&'scope Scope<'scope, 'env>) -> Result<R>,
    ) -> Result<R> {
        f(&Scope::new(self.lock_arc()))
    }

    pub fn coerce_string(&self, v: Value) -> Result<Option<String>> {
        Ok(match v {
            Value::String(s) => Some(s),
            v => unsafe {
                let lua = self.lock();
                let state = lua.state();
                let _sg = StackGuard::new(state);
                check_stack(state, 4)?;

                lua.push_value(&v)?;
                let res = if lua.unlikely_memory_error() {
                    ffi::lua_tolstring(state, -1, ptr::null_mut())
                } else {
                    protect_lua!(state, 1, 1, |state| {
                        ffi::lua_tolstring(state, -1, ptr::null_mut())
                    })?
                };
                if !res.is_null() {
                    Some(String(lua.pop_ref()))
                } else {
                    None
                }
            },
        })
    }

    pub fn coerce_integer(&self, v: Value) -> Result<Option<Integer>> {
        Ok(match v {
            Value::Integer(i) => Some(i),
            v => unsafe {
                let lua = self.lock();
                let state = lua.state();
                let _sg = StackGuard::new(state);
                check_stack(state, 2)?;

                lua.push_value(&v)?;
                let mut isint = 0;
                let i = ffi::lua_tointegerx(state, -1, &mut isint);
                if isint == 0 {
                    None
                } else {
                    Some(i)
                }
            },
        })
    }

    pub fn coerce_number(&self, v: Value) -> Result<Option<Number>> {
        Ok(match v {
            Value::Number(n) => Some(n),
            v => unsafe {
                let lua = self.lock();
                let state = lua.state();
                let _sg = StackGuard::new(state);
                check_stack(state, 2)?;

                lua.push_value(&v)?;
                let mut isnum = 0;
                let n = ffi::lua_tonumberx(state, -1, &mut isnum);
                if isnum == 0 {
                    None
                } else {
                    Some(n)
                }
            },
        })
    }

    #[inline]
    pub fn pack(&self, t: impl IntoLua) -> Result<Value> {
        t.into_lua(self)
    }

    #[inline]
    pub fn unpack<T: FromLua>(&self, value: Value) -> Result<T> {
        T::from_lua(value, self)
    }

    #[inline]
    pub fn convert<U: FromLua>(&self, value: impl IntoLua) -> Result<U> {
        U::from_lua(value.into_lua(self)?, self)
    }

    #[inline]
    pub fn pack_multi(&self, t: impl IntoLuaMulti) -> Result<MultiValue> {
        t.into_lua_multi(self)
    }

    #[inline]
    pub fn unpack_multi<T: FromLuaMulti>(&self, value: MultiValue) -> Result<T> {
        T::from_lua_multi(value, self)
    }

    pub fn set_named_registry_value(&self, key: &str, t: impl IntoLua) -> Result<()> {
        let lua = self.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 5)?;

            lua.push(t)?;
            rawset_field(state, ffi::LUA_REGISTRYINDEX, key)
        }
    }

    pub fn named_registry_value<T>(&self, key: &str) -> Result<T>
    where
        T: FromLua,
    {
        let lua = self.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 3)?;

            let protect = !lua.unlikely_memory_error();
            push_string(state, key.as_bytes(), protect)?;
            ffi::lua_rawget(state, ffi::LUA_REGISTRYINDEX);

            T::from_stack(-1, &lua)
        }
    }

    #[inline]
    pub fn unset_named_registry_value(&self, key: &str) -> Result<()> {
        self.set_named_registry_value(key, Nil)
    }

    pub fn create_registry_value(&self, t: impl IntoLua) -> Result<RegistryKey> {
        let lua = self.lock();
        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 4)?;

            lua.push(t)?;

            let unref_list = (*lua.extra.get()).registry_unref_list.clone();

            if ffi::lua_isnil(state, -1) != 0 {
                return Ok(RegistryKey::new(ffi::LUA_REFNIL, unref_list));
            }

            let free_registry_id = unref_list.lock().as_mut().and_then(|x| x.pop());
            if let Some(registry_id) = free_registry_id {
                ffi::lua_rawseti(state, ffi::LUA_REGISTRYINDEX, registry_id as Integer);
                return Ok(RegistryKey::new(registry_id, unref_list));
            }

            let registry_id = if lua.unlikely_memory_error() {
                ffi::luaL_ref(state, ffi::LUA_REGISTRYINDEX)
            } else {
                protect_lua!(state, 1, 0, |state| {
                    ffi::luaL_ref(state, ffi::LUA_REGISTRYINDEX)
                })?
            };
            Ok(RegistryKey::new(registry_id, unref_list))
        }
    }

    pub fn registry_value<T: FromLua>(&self, key: &RegistryKey) -> Result<T> {
        let lua = self.lock();
        if !lua.owns_registry_value(key) {
            return Err(Error::MismatchedRegistryKey);
        }

        let state = lua.state();
        match key.id() {
            ffi::LUA_REFNIL => T::from_lua(Value::Nil, self),
            registry_id => unsafe {
                let _sg = StackGuard::new(state);
                check_stack(state, 1)?;

                ffi::lua_rawgeti(state, ffi::LUA_REGISTRYINDEX, registry_id as Integer);
                T::from_stack(-1, &lua)
            },
        }
    }

    pub fn remove_registry_value(&self, key: RegistryKey) -> Result<()> {
        let lua = self.lock();
        if !lua.owns_registry_value(&key) {
            return Err(Error::MismatchedRegistryKey);
        }

        unsafe { ffi::luaL_unref(lua.state(), ffi::LUA_REGISTRYINDEX, key.take()) };
        Ok(())
    }

    pub fn replace_registry_value(&self, key: &mut RegistryKey, t: impl IntoLua) -> Result<()> {
        let lua = self.lock();
        if !lua.owns_registry_value(key) {
            return Err(Error::MismatchedRegistryKey);
        }

        let t = t.into_lua(self)?;

        let state = lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 2)?;

            match (t, key.id()) {
                (Value::Nil, ffi::LUA_REFNIL) => {
                }
                (Value::Nil, registry_id) => {
                    ffi::luaL_unref(state, ffi::LUA_REGISTRYINDEX, registry_id);
                    key.set_id(ffi::LUA_REFNIL);
                }
                (value, ffi::LUA_REFNIL) => {
                    let new_key = self.create_registry_value(value)?;
                    key.set_id(new_key.take());
                }
                (value, registry_id) => {
                    lua.push_value(&value)?;
                    ffi::lua_rawseti(state, ffi::LUA_REGISTRYINDEX, registry_id as Integer);
                }
            }
        }
        Ok(())
    }

    #[inline]
    pub fn owns_registry_value(&self, key: &RegistryKey) -> bool {
        self.lock().owns_registry_value(key)
    }

    pub fn expire_registry_values(&self) {
        let lua = self.lock();
        let state = lua.state();
        unsafe {
            let mut unref_list = (*lua.extra.get()).registry_unref_list.lock();
            let unref_list = unref_list.replace(Vec::new());
            for id in mlua_expect!(unref_list, "unref list is not set") {
                ffi::luaL_unref(state, ffi::LUA_REGISTRYINDEX, id);
            }
        }
    }

    #[track_caller]
    pub fn set_app_data<T: MaybeSend + 'static>(&self, data: T) -> Option<T> {
        let lua = self.lock();
        let extra = unsafe { &*lua.extra.get() };
        extra.app_data.insert(data)
    }

    pub fn try_set_app_data<T: MaybeSend + 'static>(&self, data: T) -> StdResult<Option<T>, T> {
        let lua = self.lock();
        let extra = unsafe { &*lua.extra.get() };
        extra.app_data.try_insert(data)
    }

    #[track_caller]
    pub fn app_data_ref<T: 'static>(&self) -> Option<AppDataRef<'_, T>> {
        let guard = self.lock_arc();
        let extra = unsafe { &*guard.extra.get() };
        extra.app_data.borrow(Some(guard))
    }

    pub fn try_app_data_ref<T: 'static>(&self) -> StdResult<Option<AppDataRef<'_, T>>, BorrowError> {
        let guard = self.lock_arc();
        let extra = unsafe { &*guard.extra.get() };
        extra.app_data.try_borrow(Some(guard))
    }

    #[track_caller]
    pub fn app_data_mut<T: 'static>(&self) -> Option<AppDataRefMut<'_, T>> {
        let guard = self.lock_arc();
        let extra = unsafe { &*guard.extra.get() };
        extra.app_data.borrow_mut(Some(guard))
    }

    pub fn try_app_data_mut<T: 'static>(&self) -> StdResult<Option<AppDataRefMut<'_, T>>, BorrowMutError> {
        let guard = self.lock_arc();
        let extra = unsafe { &*guard.extra.get() };
        extra.app_data.try_borrow_mut(Some(guard))
    }

    #[track_caller]
    pub fn remove_app_data<T: 'static>(&self) -> Option<T> {
        let lua = self.lock();
        let extra = unsafe { &*lua.extra.get() };
        extra.app_data.remove()
    }

    #[cfg(feature = "async")]
    #[doc(hidden)]
    #[inline(always)]
    pub fn poll_pending() -> LightUserData {
        static ASYNC_POLL_PENDING: u8 = 0;
        LightUserData(&ASYNC_POLL_PENDING as *const u8 as *mut std::os::raw::c_void)
    }

    #[cfg(feature = "async")]
    #[inline(always)]
    pub(crate) fn poll_terminate() -> LightUserData {
        static ASYNC_POLL_TERMINATE: u8 = 0;
        LightUserData(&ASYNC_POLL_TERMINATE as *const u8 as *mut std::os::raw::c_void)
    }

    #[cfg(feature = "async")]
    #[inline(always)]
    pub(crate) fn poll_yield() -> LightUserData {
        static ASYNC_POLL_YIELD: u8 = 0;
        LightUserData(&ASYNC_POLL_YIELD as *const u8 as *mut std::os::raw::c_void)
    }

    #[cfg(feature = "async")]
    #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
    pub async fn yield_with<R: FromLuaMulti>(&self, args: impl IntoLuaMulti) -> Result<R> {
        let mut args = Some(args.into_lua_multi(self)?);
        future::poll_fn(move |_cx| match args.take() {
            Some(args) => unsafe {
                let lua = self.lock();
                lua.push(Self::poll_yield())?;
                if args.len() <= 1 {
                    lua.push(args.front())?;
                } else {
                    lua.push(lua.create_sequence_from(&args)?)?;
                }
                lua.push(args.len())?;
                Poll::Pending
            },
            None => unsafe {
                let lua = self.lock();
                let state = lua.state();
                let top = ffi::lua_gettop(state);
                if top == 0 || ffi::lua_type(state, 1) != ffi::LUA_TUSERDATA {
                    return Poll::Ready(R::from_stack_multi(0, &lua));
                }
                let _sg = StackGuard::with_top(state, 1);
                Poll::Ready(R::from_stack_multi(top - 1, &lua))
            },
        })
        .await
    }

    #[inline(always)]
    pub fn weak(&self) -> WeakLua {
        WeakLua(XRc::downgrade(&self.raw))
    }

    #[cfg(not(feature = "luau"))]
    fn disable_c_modules(&self) -> Result<()> {
        let package: Table = self.globals().get("package")?;

        package.set(
            "loadlib",
            self.create_function(|_, ()| -> Result<()> {
                Err(Error::SafetyError(
                    "package.loadlib is disabled in safe mode".to_string(),
                ))
            })?,
        )?;

        #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53", feature = "lua52"))]
        let searchers: Table = package.get("searchers")?;
        #[cfg(any(feature = "lua51", feature = "luajit"))]
        let searchers: Table = package.get("loaders")?;

        let loader = self.create_function(|_, ()| Ok("\n\tcan't load C modules in safe mode"))?;

        searchers.raw_set(3, loader)?;
        if searchers.raw_len() >= 4 {
            searchers.raw_remove(4)?;
        }

        Ok(())
    }

    #[inline(always)]
    pub(crate) fn lock(&self) -> ReentrantMutexGuard<'_, RawLua> {
        let rawlua = self.raw.lock();
        #[cfg(feature = "luau")]
        if unsafe { (*rawlua.extra.get()).running_gc } {
            panic!("Luau VM is suspended while GC is running");
        }
        rawlua
    }

    #[inline(always)]
    pub(crate) fn lock_arc(&self) -> LuaGuard {
        LuaGuard(self.raw.lock_arc())
    }

    #[cfg(feature = "async")]
    #[inline(always)]
    pub(crate) unsafe fn raw_lua(&self) -> &RawLua {
        &*self.raw.data_ptr()
    }
}

impl WeakLua {
    #[track_caller]
    #[inline(always)]
    pub(crate) fn lock(&self) -> LuaGuard {
        let guard = LuaGuard::new(self.0.upgrade().expect("Lua instance is destroyed"));
        #[cfg(feature = "luau")]
        if unsafe { (*guard.extra.get()).running_gc } {
            panic!("Luau VM is suspended while GC is running");
        }
        guard
    }

    #[inline(always)]
    pub(crate) fn try_lock(&self) -> Option<LuaGuard> {
        Some(LuaGuard::new(self.0.upgrade()?))
    }

    #[track_caller]
    #[inline(always)]
    pub fn upgrade(&self) -> Lua {
        Lua {
            raw: self.0.upgrade().expect("Lua instance is destroyed"),
            collect_garbage: false,
        }
    }

    #[inline(always)]
    pub fn try_upgrade(&self) -> Option<Lua> {
        Some(Lua {
            raw: self.0.upgrade()?,
            collect_garbage: false,
        })
    }
}

impl PartialEq for WeakLua {
    fn eq(&self, other: &Self) -> bool {
        XWeak::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for WeakLua {}

impl LuaGuard {
    #[cfg(feature = "send")]
    pub(crate) fn new(handle: XRc<ReentrantMutex<RawLua>>) -> Self {
        LuaGuard(handle.lock_arc())
    }

    #[cfg(not(feature = "send"))]
    pub(crate) fn new(handle: XRc<ReentrantMutex<RawLua>>) -> Self {
        LuaGuard(handle.into_lock_arc())
    }
}

impl Deref for LuaGuard {
    type Target = RawLua;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub(crate) mod extra;
mod raw;
pub(crate) mod util;

#[cfg(test)]
mod assertions {
    use super::*;

    static_assertions::assert_not_impl_any!(Lua: std::panic::RefUnwindSafe);

    #[cfg(not(feature = "send"))]
    static_assertions::assert_not_impl_any!(Lua: Send);
    #[cfg(feature = "send")]
    static_assertions::assert_impl_all!(Lua: Send, Sync);
}
