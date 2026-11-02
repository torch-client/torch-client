use std::os::raw::{c_char, c_int, c_void};

use super::lua::lua_State;

pub const LUA_REGISTERED_MODULES_TABLE: *const c_char = cstr!("_REGISTEREDMODULES");

#[repr(C)]
pub enum luarequire_NavigateResult {
    Success,
    Ambiguous,
    NotFound,
}

#[repr(C)]
pub enum luarequire_WriteResult {
    Success,
    BufferTooSmall,
    Failure,
}

#[repr(C)]
pub enum luarequire_ConfigStatus {
    Absent,
    Ambiguous,
    PresentJson,
    PresentLuau,
}

#[repr(C)]
pub struct luarequire_Configuration {
    pub is_require_allowed: unsafe extern "C-unwind" fn(
        L: *mut lua_State,
        ctx: *mut c_void,
        requirer_chunkname: *const c_char,
    ) -> bool,

    pub reset: unsafe extern "C-unwind" fn(
        L: *mut lua_State,
        ctx: *mut c_void,
        requirer_chunkname: *const c_char,
    ) -> luarequire_NavigateResult,

    pub jump_to_alias: unsafe extern "C-unwind" fn(
        L: *mut lua_State,
        ctx: *mut c_void,
        path: *const c_char,
    ) -> luarequire_NavigateResult,

    pub to_alias_override: Option<
        unsafe extern "C-unwind" fn(
            L: *mut lua_State,
            ctx: *mut c_void,
            alias_unprefixed: *const c_char,
        ) -> luarequire_NavigateResult,
    >,

    pub to_alias_fallback: Option<
        unsafe extern "C-unwind" fn(
            L: *mut lua_State,
            ctx: *mut c_void,
            alias_unprefixed: *const c_char,
        ) -> luarequire_NavigateResult,
    >,

    pub to_parent:
        unsafe extern "C-unwind" fn(L: *mut lua_State, ctx: *mut c_void) -> luarequire_NavigateResult,
    pub to_child: unsafe extern "C-unwind" fn(
        L: *mut lua_State,
        ctx: *mut c_void,
        name: *const c_char,
    ) -> luarequire_NavigateResult,

    pub is_module_present: unsafe extern "C-unwind" fn(L: *mut lua_State, ctx: *mut c_void) -> bool,

    pub get_chunkname: unsafe extern "C-unwind" fn(
        L: *mut lua_State,
        ctx: *mut c_void,
        buffer: *mut c_char,
        buffer_size: usize,
        size_out: *mut usize,
    ) -> luarequire_WriteResult,

    pub get_loadname: unsafe extern "C-unwind" fn(
        L: *mut lua_State,
        ctx: *mut c_void,
        buffer: *mut c_char,
        buffer_size: usize,
        size_out: *mut usize,
    ) -> luarequire_WriteResult,

    pub get_cache_key: unsafe extern "C-unwind" fn(
        L: *mut lua_State,
        ctx: *mut c_void,
        buffer: *mut c_char,
        buffer_size: usize,
        size_out: *mut usize,
    ) -> luarequire_WriteResult,

    pub get_config_status:
        unsafe extern "C-unwind" fn(L: *mut lua_State, ctx: *mut c_void) -> luarequire_ConfigStatus,

    pub get_alias: Option<
        unsafe extern "C-unwind" fn(
            L: *mut lua_State,
            ctx: *mut c_void,
            alias: *const c_char,
            buffer: *mut c_char,
            buffer_size: usize,
            size_out: *mut usize,
        ) -> luarequire_WriteResult,
    >,

    pub get_config: Option<
        unsafe extern "C-unwind" fn(
            L: *mut lua_State,
            ctx: *mut c_void,
            buffer: *mut c_char,
            buffer_size: usize,
            size_out: *mut usize,
        ) -> luarequire_WriteResult,
    >,

    pub get_luau_config_timeout:
        Option<unsafe extern "C-unwind" fn(L: *mut lua_State, ctx: *mut c_void) -> c_int>,

    pub load: unsafe extern "C-unwind" fn(
        L: *mut lua_State,
        ctx: *mut c_void,
        path: *const c_char,
        chunkname: *const c_char,
        loadname: *const c_char,
    ) -> c_int,
}

pub type luarequire_Configuration_init = unsafe extern "C-unwind" fn(config: *mut luarequire_Configuration);

unsafe extern "C-unwind" {
    pub fn luarequire_pushrequire(
        L: *mut lua_State,
        config_init: luarequire_Configuration_init,
        ctx: *mut c_void,
    ) -> c_int;

    pub fn luaopen_require(L: *mut lua_State, config_init: luarequire_Configuration_init, ctx: *mut c_void);

    pub fn luarequire_pushproxyrequire(
        L: *mut lua_State,
        config_init: luarequire_Configuration_init,
        ctx: *mut c_void,
    ) -> c_int;

    pub fn luarequire_registermodule(L: *mut lua_State) -> c_int;

    pub fn luarequire_clearcacheentry(L: *mut lua_State) -> c_int;

    pub fn luarequire_clearcache(L: *mut lua_State) -> c_int;
}
