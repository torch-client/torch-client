use std::{
    borrow::{Borrow, Cow},
    fmt, mem,
    ops::Deref,
    simd::prelude::*,
};

use simd_cesu8::mutf8;

#[derive(Eq, PartialEq)]
pub struct Mutf8Str {
    pub(crate) slice: [u8],
}
#[derive(Eq, PartialEq, Clone, Default)]
pub struct Mutf8String {
    pub(crate) vec: Vec<u8>,
}

#[inline]
fn is_plain_ascii(slice: &[u8]) -> bool {
    let mut is_plain_ascii = true;
    let (chunks_32_exact, mut remainder) = slice.as_chunks::<32>();
    if remainder.len() > 16 {
        let chunk;
        (chunk, remainder) = remainder.split_first_chunk::<16>().unwrap();
        let mask = u8x16::splat(0b10000000);
        let zero = u8x16::splat(0);
        let simd = u8x16::from_array(*chunk);
        let masked = simd & mask;
        if masked != zero {
            is_plain_ascii = false;
        }
    }
    if remainder.len() > 8 {
        let chunk;
        (chunk, remainder) = remainder.split_first_chunk::<8>().unwrap();
        let mask = u8x8::splat(0b10000000);
        let zero = u8x8::splat(0);
        let simd = u8x8::from_array(*chunk);
        let masked = simd & mask;
        if masked != zero {
            is_plain_ascii = false;
        }
    }
    if remainder.len() > 4 {
        let chunk;
        (chunk, remainder) = remainder.split_first_chunk::<4>().unwrap();
        let mask = u8x4::splat(0b10000000);
        let zero = u8x4::splat(0);
        let simd = u8x4::from_array(*chunk);
        let masked = simd & mask;
        if masked != zero {
            is_plain_ascii = false;
        }
    }
    for &byte in remainder {
        if byte & 0b10000000 != 0 {
            is_plain_ascii = false;
        }
    }

    for &chunk in chunks_32_exact {
        let mask = u8x32::splat(0b10000000);
        let zero = u8x32::splat(0);
        let simd = u8x32::from_array(chunk);
        let masked = simd & mask;
        if masked != zero {
            is_plain_ascii = false;
        }
    }

    is_plain_ascii
}

impl Mutf8Str {
    #[allow(clippy::should_implement_trait)]
    #[inline]
    pub fn from_str(s: &str) -> Cow<'_, Mutf8Str> {
        match mutf8::encode(s) {
            Cow::Borrowed(slice) => Cow::Borrowed(Mutf8Str::from_slice(slice)),
            Cow::Owned(vec) => Cow::Owned(Mutf8String { vec }),
        }
    }

    #[inline]
    pub fn to_str(&self) -> Cow<'_, str> {
        if is_plain_ascii(&self.slice) {
            unsafe { Cow::Borrowed(std::str::from_utf8_unchecked(&self.slice)) }
        } else {
            mutf8::decode(&self.slice).unwrap_or_default()
        }
    }

    #[inline]
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        mutf8::decode_lossy(&self.slice)
    }

    #[inline]
    pub fn from_slice(slice: &[u8]) -> &Mutf8Str {
        unsafe { mem::transmute::<&[u8], &Mutf8Str>(slice) }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.slice.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slice.is_empty()
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.slice
    }
}

impl fmt::Display for Mutf8Str {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_str())
    }
}

impl fmt::Debug for Mutf8Str {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("m")?;
        fmt::Debug::fmt(&self.to_str(), f)
    }
}

impl fmt::Debug for Mutf8String {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("m")?;
        fmt::Debug::fmt(&self.to_str(), f)
    }
}

impl ToOwned for Mutf8Str {
    type Owned = Mutf8String;

    #[inline]
    fn to_owned(&self) -> Self::Owned {
        Mutf8String {
            vec: self.slice.to_vec(),
        }
    }
}
impl Borrow<Mutf8Str> for Mutf8String {
    #[inline]
    fn borrow(&self) -> &Mutf8Str {
        self.as_str()
    }
}

impl Mutf8String {
    pub fn new() -> Self {
        Self { vec: Vec::new() }
    }

    #[inline]
    pub fn as_str(&self) -> &Mutf8Str {
        Mutf8Str::from_slice(self.vec.as_slice())
    }

    #[inline]
    pub fn into_string(self) -> String {
        if is_plain_ascii(&self.vec) {
            unsafe { String::from_utf8_unchecked(self.vec) }
        } else {
            mutf8::decode(&self.vec).unwrap_or_default().to_string()
        }
    }

    #[inline]
    pub fn try_into_string(self) -> Result<String, simd_cesu8::DecodingError> {
        if is_plain_ascii(&self.vec) {
            Ok(unsafe { String::from_utf8_unchecked(self.vec) })
        } else {
            mutf8::decode(&self.vec).map(|cow| cow.into_owned())
        }
    }

    #[inline]
    pub fn from_string(s: String) -> Mutf8String {
        Self::from_vec(mutf8::encode(&s).into_owned())
    }

    #[inline]
    pub fn from_vec(vec: Vec<u8>) -> Mutf8String {
        Self { vec }
    }
}
impl Deref for Mutf8String {
    type Target = Mutf8Str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl From<String> for Mutf8String {
    #[inline]
    fn from(s: String) -> Self {
        Self::from_string(s)
    }
}
impl From<&str> for Mutf8String {
    #[inline]
    fn from(s: &str) -> Self {
        Self::from_string(s.to_owned())
    }
}

impl Default for &Mutf8Str {
    #[inline]
    fn default() -> Self {
        Mutf8Str::from_slice(&[])
    }
}

impl From<&Mutf8Str> for Mutf8String {
    #[inline]
    fn from(s: &Mutf8Str) -> Self {
        s.to_owned()
    }
}

impl From<&Mutf8Str> for String {
    #[inline]
    fn from(s: &Mutf8Str) -> Self {
        s.to_str().into_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::mutf8::Mutf8Str;

    #[test]
    fn same_as_utf8() {
        let str = "Hello, world!";
        assert_eq!(
            Mutf8Str::from_str(str),
            Cow::Borrowed(Mutf8Str::from_slice(str.as_bytes()))
        );
        assert_eq!(Mutf8Str::from_str(str).to_str(), Cow::Borrowed(str));
    }

    #[test]
    fn surrogate_pairs() {
        let str = "\u{10401}";
        let mutf8_data = &[0xED, 0xA0, 0x81, 0xED, 0xB0, 0x81];
        assert_eq!(
            Mutf8Str::from_slice(mutf8_data).to_str(),
            Cow::Borrowed(str)
        );
    }

    #[test]
    fn null_bytes() {
        let str = "\0";
        let mutf8_data = vec![0xC0, 0x80];
        assert_eq!(
            Mutf8Str::from_slice(&mutf8_data).to_str(),
            Cow::Borrowed(str)
        );
    }
}
