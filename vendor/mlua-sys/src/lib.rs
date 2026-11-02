#![allow(non_camel_case_types, non_snake_case)]
#![allow(clippy::missing_safety_doc)]
#![allow(unsafe_op_in_unsafe_fn)]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::os::raw::c_int;

#[cfg(feature = "lua55")]
pub use lua55::*;

#[cfg(feature = "lua54")]
pub use lua54::*;

#[cfg(feature = "lua53")]
pub use lua53::*;

#[cfg(feature = "lua52")]
pub use lua52::*;

#[cfg(any(feature = "lua51", feature = "luajit"))]
pub use lua51::*;

#[cfg(any(feature = "luau", doc))]
pub use luau::*;

#[cfg(any(feature = "lua55", feature = "lua54", feature = "lua53", feature = "lua52"))]
#[doc(hidden)]
pub const LUA_MAX_UPVALUES: c_int = 255;

#[cfg(any(feature = "lua51", feature = "luajit"))]
#[doc(hidden)]
pub const LUA_MAX_UPVALUES: c_int = 60;

#[cfg(feature = "luau")]
#[doc(hidden)]
pub const LUA_MAX_UPVALUES: c_int = 200;

#[doc(hidden)]
pub const LUA_TRACEBACK_STACK: c_int = 11;

#[doc(hidden)]
#[rustfmt::skip]
pub const SYS_MIN_ALIGN: usize = if cfg!(any(
    all(target_arch = "riscv32", any(target_os = "espidf", target_os = "zkvm")),
    all(target_arch = "xtensa", target_os = "espidf"),
)) {
    4
} else if cfg!(any(
    target_arch = "x86",
    target_arch = "arm",
    target_arch = "m68k",
    target_arch = "csky",
    target_arch = "loongarch32",
    target_arch = "mips",
    target_arch = "mips32r6",
    target_arch = "powerpc",
    target_arch = "powerpc64",
    target_arch = "sparc",
    target_arch = "wasm32",
    target_arch = "hexagon",
    target_arch = "riscv32",
    target_arch = "xtensa",
)) {
    8
} else if cfg!(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "loongarch64",
    target_arch = "mips64",
    target_arch = "mips64r6",
    target_arch = "s390x",
    target_arch = "sparc64",
    target_arch = "riscv64",
    target_arch = "wasm64",
)) {
    16
} else {
    panic!("no value for SYS_MIN_ALIGN")
};

#[macro_use]
mod macros;

#[cfg(feature = "lua55")]
#[cfg_attr(docsrs, doc(cfg(feature = "lua55")))]
pub mod lua55;

#[cfg(feature = "lua54")]
#[cfg_attr(docsrs, doc(cfg(feature = "lua54")))]
pub mod lua54;

#[cfg(feature = "lua53")]
#[cfg_attr(docsrs, doc(cfg(feature = "lua53")))]
pub mod lua53;

#[cfg(feature = "lua52")]
#[cfg_attr(docsrs, doc(cfg(feature = "lua52")))]
pub mod lua52;

#[cfg(any(feature = "lua51", feature = "luajit"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "lua51", feature = "luajit"))))]
pub mod lua51;

#[cfg(any(feature = "luau", doc))]
#[cfg_attr(docsrs, doc(cfg(feature = "luau")))]
pub mod luau;

#[cfg(target_arch = "wasm32")]
mod wasm;
