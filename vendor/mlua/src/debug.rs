use std::borrow::Cow;
use std::os::raw::c_int;

use ffi::{lua_Debug, lua_State};

use crate::function::Function;
use crate::state::RawLua;
use crate::util::{assert_stack, linenumber_to_usize, ptr_to_lossy_str, ptr_to_str, StackGuard};

pub struct Debug<'a> {
    state: *mut lua_State,
    lua: &'a RawLua,
    #[cfg_attr(not(feature = "luau"), allow(unused))]
    level: c_int,
    ar: *mut lua_Debug,
}

impl<'a> Debug<'a> {
    pub(crate) fn new(lua: &'a RawLua, level: c_int, ar: *mut lua_Debug) -> Self {
        Debug {
            state: lua.state(),
            lua,
            ar,
            level,
        }
    }

    #[cfg(not(feature = "luau"))]
    #[cfg_attr(docsrs, doc(cfg(not(feature = "luau"))))]
    pub fn event(&self) -> DebugEvent {
        unsafe {
            match (*self.ar).event {
                ffi::LUA_HOOKCALL => DebugEvent::Call,
                ffi::LUA_HOOKRET => DebugEvent::Ret,
                ffi::LUA_HOOKTAILCALL => DebugEvent::TailCall,
                ffi::LUA_HOOKLINE => DebugEvent::Line,
                ffi::LUA_HOOKCOUNT => DebugEvent::Count,
                event => DebugEvent::Unknown(event),
            }
        }
    }

    pub fn function(&self) -> Function {
        unsafe {
            let _sg = StackGuard::new(self.state);
            assert_stack(self.state, 1);

            #[cfg(not(feature = "luau"))]
            mlua_assert!(
                ffi::lua_getinfo(self.state, cstr!("f"), self.ar) != 0,
                "lua_getinfo failed with `f`"
            );
            #[cfg(feature = "luau")]
            mlua_assert!(
                ffi::lua_getinfo(self.state, self.level, cstr!("f"), self.ar) != 0,
                "lua_getinfo failed with `f`"
            );

            ffi::lua_xmove(self.state, self.lua.ref_thread(), 1);
            Function(self.lua.pop_ref_thread())
        }
    }

    pub fn names(&self) -> DebugNames<'_> {
        unsafe {
            #[cfg(not(feature = "luau"))]
            mlua_assert!(
                ffi::lua_getinfo(self.state, cstr!("n"), self.ar) != 0,
                "lua_getinfo failed with `n`"
            );
            #[cfg(feature = "luau")]
            mlua_assert!(
                ffi::lua_getinfo(self.state, self.level, cstr!("n"), self.ar) != 0,
                "lua_getinfo failed with `n`"
            );

            DebugNames {
                name: ptr_to_lossy_str((*self.ar).name),
                #[cfg(not(feature = "luau"))]
                name_what: match ptr_to_str((*self.ar).namewhat) {
                    Some("") => None,
                    val => val,
                },
                #[cfg(feature = "luau")]
                name_what: None,
            }
        }
    }

    pub fn source(&self) -> DebugSource<'_> {
        unsafe {
            #[cfg(not(feature = "luau"))]
            mlua_assert!(
                ffi::lua_getinfo(self.state, cstr!("S"), self.ar) != 0,
                "lua_getinfo failed with `S`"
            );
            #[cfg(feature = "luau")]
            mlua_assert!(
                ffi::lua_getinfo(self.state, self.level, cstr!("s"), self.ar) != 0,
                "lua_getinfo failed with `s`"
            );

            DebugSource {
                source: ptr_to_lossy_str((*self.ar).source),
                #[cfg(not(feature = "luau"))]
                short_src: ptr_to_lossy_str((*self.ar).short_src.as_ptr()),
                #[cfg(feature = "luau")]
                short_src: ptr_to_lossy_str((*self.ar).short_src),
                line_defined: linenumber_to_usize((*self.ar).linedefined),
                #[cfg(not(feature = "luau"))]
                last_line_defined: linenumber_to_usize((*self.ar).lastlinedefined),
                #[cfg(feature = "luau")]
                last_line_defined: None,
                what: ptr_to_str((*self.ar).what).unwrap_or("main"),
            }
        }
    }

    #[doc(hidden)]
    #[deprecated(note = "Use `current_line` instead")]
    pub fn curr_line(&self) -> i32 {
        self.current_line().map(|n| n as i32).unwrap_or(-1)
    }

    pub fn current_line(&self) -> Option<usize> {
        unsafe {
            #[cfg(not(feature = "luau"))]
            mlua_assert!(
                ffi::lua_getinfo(self.state, cstr!("l"), self.ar) != 0,
                "lua_getinfo failed with `l`"
            );
            #[cfg(feature = "luau")]
            mlua_assert!(
                ffi::lua_getinfo(self.state, self.level, cstr!("l"), self.ar) != 0,
                "lua_getinfo failed with `l`"
            );

            linenumber_to_usize((*self.ar).currentline)
        }
    }

    #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53", feature = "lua52"))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(feature = "lua55", feature = "lua54", feature = "lua53", feature = "lua52")))
    )]
    pub fn is_tail_call(&self) -> bool {
        unsafe {
            mlua_assert!(
                ffi::lua_getinfo(self.state, cstr!("t"), self.ar) != 0,
                "lua_getinfo failed with `t`"
            );
            (*self.ar).istailcall != 0
        }
    }

    pub fn stack(&self) -> DebugStack {
        unsafe {
            #[cfg(not(feature = "luau"))]
            mlua_assert!(
                ffi::lua_getinfo(self.state, cstr!("u"), self.ar) != 0,
                "lua_getinfo failed with `u`"
            );
            #[cfg(feature = "luau")]
            mlua_assert!(
                ffi::lua_getinfo(self.state, self.level, cstr!("au"), self.ar) != 0,
                "lua_getinfo failed with `au`"
            );

            #[cfg(not(feature = "luau"))]
            let stack = DebugStack {
                num_ups: (*self.ar).nups as _,
                #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53", feature = "lua52"))]
                num_params: (*self.ar).nparams as _,
                #[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53", feature = "lua52"))]
                is_vararg: (*self.ar).isvararg != 0,
            };
            #[cfg(feature = "luau")]
            let stack = DebugStack {
                num_ups: (*self.ar).nupvals,
                num_params: (*self.ar).nparams,
                is_vararg: (*self.ar).isvararg != 0,
            };
            stack
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugEvent {
    Call,
    Ret,
    TailCall,
    Line,
    Count,
    Unknown(c_int),
}

#[derive(Clone, Debug)]
pub struct DebugNames<'a> {
    pub name: Option<Cow<'a, str>>,
    pub name_what: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub struct DebugSource<'a> {
    pub source: Option<Cow<'a, str>>,
    pub short_src: Option<Cow<'a, str>>,
    pub line_defined: Option<usize>,
    pub last_line_defined: Option<usize>,
    pub what: &'static str,
}

#[derive(Copy, Clone, Debug)]
pub struct DebugStack {
    pub num_ups: u8,
    #[cfg(any(
        feature = "lua55",
        feature = "lua54",
        feature = "lua53",
        feature = "lua52",
        feature = "luau"
    ))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(
            feature = "lua55",
            feature = "lua54",
            feature = "lua53",
            feature = "lua52",
            feature = "luau"
        )))
    )]
    pub num_params: u8,
    #[cfg(any(
        feature = "lua55",
        feature = "lua54",
        feature = "lua53",
        feature = "lua52",
        feature = "luau"
    ))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(
            feature = "lua55",
            feature = "lua54",
            feature = "lua53",
            feature = "lua52",
            feature = "luau"
        )))
    )]
    pub is_vararg: bool,
}

#[cfg(not(feature = "luau"))]
#[cfg_attr(docsrs, doc(cfg(not(feature = "luau"))))]
#[derive(Clone, Copy, Debug, Default)]
pub struct HookTriggers {
    pub on_calls: bool,
    pub on_returns: bool,
    pub every_line: bool,
    pub every_nth_instruction: Option<u32>,
}

#[cfg(not(feature = "luau"))]
impl HookTriggers {
    pub const ON_CALLS: Self = HookTriggers::new().on_calls();

    pub const ON_RETURNS: Self = HookTriggers::new().on_returns();

    pub const EVERY_LINE: Self = HookTriggers::new().every_line();

    pub const fn new() -> Self {
        HookTriggers {
            on_calls: false,
            on_returns: false,
            every_line: false,
            every_nth_instruction: None,
        }
    }

    pub const fn on_calls(mut self) -> Self {
        self.on_calls = true;
        self
    }

    pub const fn on_returns(mut self) -> Self {
        self.on_returns = true;
        self
    }

    pub const fn every_line(mut self) -> Self {
        self.every_line = true;
        self
    }

    pub const fn every_nth_instruction(mut self, n: u32) -> Self {
        self.every_nth_instruction = Some(n);
        self
    }

    pub(crate) const fn mask(&self) -> c_int {
        let mut mask: c_int = 0;
        if self.on_calls {
            mask |= ffi::LUA_MASKCALL
        }
        if self.on_returns {
            mask |= ffi::LUA_MASKRET
        }
        if self.every_line {
            mask |= ffi::LUA_MASKLINE
        }
        if self.every_nth_instruction.is_some() {
            mask |= ffi::LUA_MASKCOUNT
        }
        mask
    }

    pub(crate) const fn count(&self) -> c_int {
        match self.every_nth_instruction {
            Some(n) => n as c_int,
            None => 0,
        }
    }
}

#[cfg(not(feature = "luau"))]
impl std::ops::BitOr for HookTriggers {
    type Output = Self;

    fn bitor(mut self, rhs: Self) -> Self::Output {
        self.on_calls |= rhs.on_calls;
        self.on_returns |= rhs.on_returns;
        self.every_line |= rhs.every_line;
        if self.every_nth_instruction.is_none() && rhs.every_nth_instruction.is_some() {
            self.every_nth_instruction = rhs.every_nth_instruction;
        }
        self
    }
}

#[cfg(not(feature = "luau"))]
impl std::ops::BitOrAssign for HookTriggers {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}
