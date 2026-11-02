use std::borrow::{Borrow, Cow};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::os::raw::{c_int, c_void};
use std::string::String as StdString;
use std::{cmp, fmt, slice, str};

use crate::error::{Error, Result};
use crate::state::Lua;
use crate::traits::IntoLua;
use crate::types::{LuaType, ValueRef};
use crate::value::Value;

#[cfg(feature = "serde")]
use {
    serde::ser::{Serialize, Serializer},
    std::result::Result as StdResult,
};

#[derive(Clone)]
pub struct String(pub(crate) ValueRef);

impl String {
    #[inline]
    pub fn to_str(&self) -> Result<BorrowedStr<'_>> {
        BorrowedStr::try_from(self)
    }

    #[inline]
    pub fn to_string_lossy(&self) -> StdString {
        StdString::from_utf8_lossy(&self.as_bytes()).into_owned()
    }

    pub fn display(&self) -> impl fmt::Display + '_ {
        Display(self)
    }

    #[inline]
    pub fn as_bytes(&self) -> BorrowedBytes<'_> {
        BorrowedBytes::from(self)
    }

    pub fn as_bytes_with_nul(&self) -> BorrowedBytes<'_> {
        let BorrowedBytes { buf, borrow, _lua } = BorrowedBytes::from(self);
        let buf = unsafe { slice::from_raw_parts((*buf).as_ptr(), (*buf).len() + 1) };
        BorrowedBytes { buf, borrow, _lua }
    }

    unsafe fn to_slice(&self) -> (&[u8], Lua) {
        let lua = self.0.lua.upgrade();
        let slice = {
            let rawlua = lua.lock();
            let ref_thread = rawlua.ref_thread();

            mlua_debug_assert!(
                ffi::lua_type(ref_thread, self.0.index) == ffi::LUA_TSTRING,
                "string ref is not string type"
            );

            let mut size = 0;
            let data = ffi::lua_tolstring(ref_thread, self.0.index, &mut size);
            slice::from_raw_parts(data as *const u8, size)
        };
        (slice, lua)
    }

    #[inline]
    pub fn to_pointer(&self) -> *const c_void {
        self.0.to_pointer()
    }
}

impl fmt::Debug for String {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let bytes = self.as_bytes();
        if let Ok(s) = str::from_utf8(&bytes) {
            return s.fmt(f);
        }

        write!(f, "b")?;
        <bstr::BStr as fmt::Debug>::fmt(bstr::BStr::new(&bytes), f)
    }
}

impl<T> PartialEq<T> for String
where
    T: AsRef<[u8]> + ?Sized,
{
    fn eq(&self, other: &T) -> bool {
        self.as_bytes() == other.as_ref()
    }
}

impl PartialEq for String {
    fn eq(&self, other: &String) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl Eq for String {}

impl<T> PartialOrd<T> for String
where
    T: AsRef<[u8]> + ?Sized,
{
    fn partial_cmp(&self, other: &T) -> Option<cmp::Ordering> {
        self.as_bytes().partial_cmp(&other.as_ref())
    }
}

impl PartialOrd for String {
    fn partial_cmp(&self, other: &String) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for String {
    fn cmp(&self, other: &String) -> cmp::Ordering {
        self.as_bytes().cmp(&other.as_bytes())
    }
}

impl Hash for String {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state);
    }
}

#[cfg(feature = "serde")]
impl Serialize for String {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.to_str() {
            Ok(s) => serializer.serialize_str(&s),
            Err(_) => serializer.serialize_bytes(&self.as_bytes()),
        }
    }
}

struct Display<'a>(&'a String);

impl fmt::Display for Display<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let bytes = self.0.as_bytes();
        <bstr::BStr as fmt::Display>::fmt(bstr::BStr::new(&bytes), f)
    }
}

pub struct BorrowedStr<'a> {
    pub(crate) buf: &'a str,
    pub(crate) borrow: Cow<'a, String>,
    pub(crate) _lua: Lua,
}

impl Deref for BorrowedStr<'_> {
    type Target = str;

    #[inline(always)]
    fn deref(&self) -> &str {
        self.buf
    }
}

impl Borrow<str> for BorrowedStr<'_> {
    #[inline(always)]
    fn borrow(&self) -> &str {
        self.buf
    }
}

impl AsRef<str> for BorrowedStr<'_> {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        self.buf
    }
}

impl fmt::Display for BorrowedStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.buf.fmt(f)
    }
}

impl fmt::Debug for BorrowedStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.buf.fmt(f)
    }
}

impl<T> PartialEq<T> for BorrowedStr<'_>
where
    T: AsRef<str>,
{
    fn eq(&self, other: &T) -> bool {
        self.buf == other.as_ref()
    }
}

impl Eq for BorrowedStr<'_> {}

impl<T> PartialOrd<T> for BorrowedStr<'_>
where
    T: AsRef<str>,
{
    fn partial_cmp(&self, other: &T) -> Option<cmp::Ordering> {
        self.buf.partial_cmp(other.as_ref())
    }
}

impl Ord for BorrowedStr<'_> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.buf.cmp(other.buf)
    }
}

impl<'a> TryFrom<&'a String> for BorrowedStr<'a> {
    type Error = Error;

    #[inline]
    fn try_from(value: &'a String) -> Result<Self> {
        let BorrowedBytes { buf, borrow, _lua } = BorrowedBytes::from(value);
        let buf = str::from_utf8(buf).map_err(|e| Error::FromLuaConversionError {
            from: "string",
            to: "&str".to_string(),
            message: Some(e.to_string()),
        })?;
        Ok(Self { buf, borrow, _lua })
    }
}

pub struct BorrowedBytes<'a> {
    pub(crate) buf: &'a [u8],
    pub(crate) borrow: Cow<'a, String>,
    pub(crate) _lua: Lua,
}

impl Deref for BorrowedBytes<'_> {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &[u8] {
        self.buf
    }
}

impl Borrow<[u8]> for BorrowedBytes<'_> {
    #[inline(always)]
    fn borrow(&self) -> &[u8] {
        self.buf
    }
}

impl AsRef<[u8]> for BorrowedBytes<'_> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        self.buf
    }
}

impl fmt::Debug for BorrowedBytes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.buf.fmt(f)
    }
}

impl<T> PartialEq<T> for BorrowedBytes<'_>
where
    T: AsRef<[u8]>,
{
    fn eq(&self, other: &T) -> bool {
        self.buf == other.as_ref()
    }
}

impl Eq for BorrowedBytes<'_> {}

impl<T> PartialOrd<T> for BorrowedBytes<'_>
where
    T: AsRef<[u8]>,
{
    fn partial_cmp(&self, other: &T) -> Option<cmp::Ordering> {
        self.buf.partial_cmp(other.as_ref())
    }
}

impl Ord for BorrowedBytes<'_> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.buf.cmp(other.buf)
    }
}

impl<'a> IntoIterator for &'a BorrowedBytes<'_> {
    type Item = &'a u8;
    type IntoIter = slice::Iter<'a, u8>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> From<&'a String> for BorrowedBytes<'a> {
    #[inline]
    fn from(value: &'a String) -> Self {
        let (buf, _lua) = unsafe { value.to_slice() };
        let borrow = Cow::Borrowed(value);
        Self { buf, borrow, _lua }
    }
}

struct WrappedString<T: AsRef<[u8]>>(T);

impl String {
    pub fn wrap(data: impl AsRef<[u8]>) -> impl IntoLua {
        WrappedString(data)
    }
}

impl<T: AsRef<[u8]>> IntoLua for WrappedString<T> {
    fn into_lua(self, lua: &Lua) -> Result<Value> {
        lua.create_string(self.0).map(Value::String)
    }
}

impl LuaType for String {
    const TYPE_ID: c_int = ffi::LUA_TSTRING;
}

#[cfg(test)]
mod assertions {
    use super::*;

    #[cfg(not(feature = "send"))]
    static_assertions::assert_not_impl_any!(String: Send);
    #[cfg(feature = "send")]
    static_assertions::assert_impl_all!(String: Send, Sync);
    #[cfg(feature = "send")]
    static_assertions::assert_impl_all!(BorrowedBytes: Send, Sync);
    #[cfg(feature = "send")]
    static_assertions::assert_impl_all!(BorrowedStr: Send, Sync);
}
