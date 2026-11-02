#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(send), allow(clippy::arc_with_non_send_sync))]
#![allow(clippy::ptr_eq)]
#![allow(unsafe_op_in_unsafe_fn)]

#[macro_use]
mod macros;

mod buffer;
mod chunk;
mod conversion;
mod debug;
mod error;
mod function;
#[cfg(any(feature = "luau", doc))]
mod luau;
mod memory;
mod multi;
mod scope;
mod state;
mod stdlib;
mod string;
mod table;
mod thread;
mod traits;
mod types;
mod userdata;
mod util;
mod value;
mod vector;

pub mod prelude;

pub use bstr::BString;
pub use ffi::{self, lua_CFunction, lua_State};

pub use crate::chunk::{AsChunk, Chunk, ChunkMode};
pub use crate::debug::{Debug, DebugEvent, DebugNames, DebugSource, DebugStack};
pub use crate::error::{Error, ErrorContext, ExternalError, ExternalResult, Result};
pub use crate::function::{Function, FunctionInfo};
pub use crate::multi::{MultiValue, Variadic};
pub use crate::scope::Scope;
pub use crate::state::{GCMode, Lua, LuaOptions, WeakLua};
pub use crate::stdlib::StdLib;
pub use crate::string::{BorrowedBytes, BorrowedStr, String};
pub use crate::table::{Table, TablePairs, TableSequence};
pub use crate::thread::{Thread, ThreadStatus};
pub use crate::traits::{
    FromLua, FromLuaMulti, IntoLua, IntoLuaMulti, LuaNativeFn, LuaNativeFnMut, ObjectLike,
};
pub use crate::types::{
    AppDataRef, AppDataRefMut, Either, Integer, LightUserData, MaybeSend, Number, RegistryKey, VmState,
};
pub use crate::userdata::{
    AnyUserData, MetaMethod, UserData, UserDataFields, UserDataMetatable, UserDataMethods, UserDataRef,
    UserDataRefMut, UserDataRegistry,
};
pub use crate::value::{Nil, Value};

#[cfg(not(feature = "luau"))]
pub use crate::debug::HookTriggers;

#[cfg(any(feature = "luau", doc))]
#[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
pub use crate::{
    buffer::Buffer,
    chunk::{CompileConstant, Compiler},
    function::CoverageInfo,
    luau::{NavigateError, Require, TextRequirer},
    vector::Vector,
};

#[cfg(all(feature = "luau", not(target_arch = "wasm32")))]
pub use crate::luau::HeapDump;

#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub use crate::{thread::AsyncThread, traits::LuaNativeAsyncFn};

#[cfg(feature = "serde")]
#[doc(inline)]
pub use crate::{
    serde::{de::Options as DeserializeOptions, ser::Options as SerializeOptions, LuaSerdeExt},
    value::SerializableValue,
};

#[cfg(feature = "serde")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
pub mod serde;

#[cfg(feature = "mlua_derive")]
#[allow(unused_imports)]
#[macro_use]
extern crate mlua_derive;

#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use mlua_derive::chunk;

#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use mlua_derive::FromLua;

#[cfg(all(feature = "mlua_derive", any(feature = "module", doc)))]
#[cfg_attr(docsrs, doc(cfg(feature = "module")))]
pub use mlua_derive::lua_module;

#[cfg(all(feature = "module", feature = "send"))]
compile_error!("`send` feature is not supported in module mode");

pub(crate) mod private {
    use super::*;

    pub trait Sealed {}

    impl Sealed for Error {}
    impl<T> Sealed for std::result::Result<T, Error> {}
    impl Sealed for Lua {}
    impl Sealed for Table {}
    impl Sealed for AnyUserData {}
}
