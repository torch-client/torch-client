use std::{mem, slice};

use crate::{
    Mutf8Str,
    error::UnexpectedEofError,
    fastvec::FastVec,
    raw_list::RawList,
    reader::Reader,
    swap_endianness::{SwappableNumber, swap_endianness_as_u8},
};

pub const END_ID: u8 = 0;
pub const BYTE_ID: u8 = 1;
pub const SHORT_ID: u8 = 2;
pub const INT_ID: u8 = 3;
pub const LONG_ID: u8 = 4;
pub const FLOAT_ID: u8 = 5;
pub const DOUBLE_ID: u8 = 6;
pub const BYTE_ARRAY_ID: u8 = 7;
pub const STRING_ID: u8 = 8;
pub const LIST_ID: u8 = 9;
pub const COMPOUND_ID: u8 = 10;
pub const INT_ARRAY_ID: u8 = 11;
pub const LONG_ARRAY_ID: u8 = 12;

pub const MAX_DEPTH: usize = 512;

#[inline(always)]
pub fn read_with_u16_length<'a>(
    data: &mut Reader<'a>,
    width: usize,
) -> Result<&'a [u8], UnexpectedEofError> {
    let length = data.read_u16()?;
    let length_in_bytes: usize = length as usize * width;
    data.read_slice(length_in_bytes)
}

#[inline(never)]
pub fn read_with_u32_length<'a>(
    data: &mut Reader<'a>,
    width: usize,
) -> Result<&'a [u8], UnexpectedEofError> {
    let length = data.read_u32()?;
    let length_in_bytes = (length as usize)
        .checked_mul(width)
        .ok_or(UnexpectedEofError)?;
    data.read_slice(length_in_bytes)
}

pub fn read_string<'a>(data: &mut Reader<'a>) -> Result<&'a Mutf8Str, UnexpectedEofError> {
    let data = read_with_u16_length(data, 1)?;
    Ok(Mutf8Str::from_slice(data))
}

pub fn read_u8_array<'a>(data: &mut Reader<'a>) -> Result<&'a [u8], UnexpectedEofError> {
    read_with_u32_length(data, 1)
}
pub fn read_i8_array<'a>(data: &mut Reader<'a>) -> Result<&'a [i8], UnexpectedEofError> {
    Ok(slice_u8_into_i8(read_u8_array(data)?))
}

pub fn read_int_array<'a>(data: &mut Reader<'a>) -> Result<RawList<'a, i32>, UnexpectedEofError> {
    let array_bytes = read_with_u32_length(data, 4)?;
    Ok(RawList::new(array_bytes))
}

pub fn read_long_array<'a>(data: &mut Reader<'a>) -> Result<RawList<'a, i64>, UnexpectedEofError> {
    let array_bytes = read_with_u32_length(data, 8)?;
    Ok(RawList::new(array_bytes))
}

fn slice_u8_into_i8(s: &[u8]) -> &[i8] {
    unsafe { slice::from_raw_parts(s.as_ptr() as *const i8, s.len()) }
}

pub fn slice_i8_into_u8(s: &[i8]) -> &[u8] {
    unsafe { slice::from_raw_parts(s.as_ptr() as *const u8, s.len()) }
}

#[inline(always)]
pub fn write_with_u32_length(data: &mut FastVec<u8>, width: usize, value: &[u8]) {
    let length = value.len() / width;
    data.reserve(4 + value.len());
    unsafe {
        data.extend_from_slice_unchecked(&(length as u32).to_be_bytes());
        data.extend_from_slice_unchecked(value);
    }
}

pub fn write_u32(data: &mut FastVec<u8>, value: u32) {
    data.extend_from_slice(&value.to_be_bytes());
}
pub fn write_string(data: &mut FastVec<u8>, value: &Mutf8Str) {
    data.reserve(2 + value.len());
    unsafe {
        write_string_unchecked(data, value);
    }
}
#[inline]
pub unsafe fn write_string_unchecked(data: &mut FastVec<u8>, value: &Mutf8Str) {
    let bytes = value.as_bytes();
    let bytes = if bytes.len() <= u16::MAX as usize {
        bytes
    } else {
        truncate_mutf8_to_u16_len(bytes)
    };
    unsafe {
        data.extend_from_slice_unchecked(&(bytes.len() as u16).to_be_bytes());
        data.extend_from_slice_unchecked(bytes);
    }
}

#[cold]
#[inline(never)]
fn truncate_mutf8_to_u16_len(bytes: &[u8]) -> &[u8] {
    let max = u16::MAX as usize;
    let mut end = max;
    while end > max - 2 && bytes[end] & 0b1100_0000 == 0b1000_0000 {
        end -= 1;
    }
    if bytes[end] & 0b1100_0000 == 0b1000_0000 {
        end = max;
    }
    &bytes[..end]
}

#[inline]
pub fn slice_into_u8_native_endian<T>(s: &[T]) -> &[u8] {
    unsafe { slice::from_raw_parts(s.as_ptr() as *const u8, mem::size_of_val(s)) }
}

#[inline]
pub fn slice_into_u8_big_endian<T: SwappableNumber>(s: &[T]) -> Vec<u8> {
    swap_endianness_as_u8::<T>(slice_into_u8_native_endian(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_endian = "little")]
    #[test]
    fn test_slice_into_u8_native_endian() {
        assert_eq!(slice_into_u8_native_endian(&[1u16, 2u16]), [1, 0, 2, 0]);
        assert_eq!(
            slice_into_u8_native_endian(&[1u32, 2u32]),
            [1, 0, 0, 0, 2, 0, 0, 0]
        );
    }

    #[test]
    fn truncate_mutf8_to_u16_len_boundaries() {
        let ascii = vec![b'a'; 70_000];
        assert_eq!(truncate_mutf8_to_u16_len(&ascii).len(), 65_535);

        let mut two = vec![b'a'; 65_534];
        two.extend_from_slice(&[0xc3, 0xa9, b'a']);
        assert_eq!(truncate_mutf8_to_u16_len(&two).len(), 65_534);

        let mut fits = vec![b'a'; 65_533];
        fits.extend_from_slice(&[0xc3, 0xa9, b'a']);
        assert_eq!(truncate_mutf8_to_u16_len(&fits).len(), 65_535);

        let junk = vec![0x80; 70_000];
        assert_eq!(truncate_mutf8_to_u16_len(&junk).len(), 65_535);
    }

    #[test]
    fn test_slice_into_u8_big_endian() {
        assert_eq!(slice_into_u8_big_endian(&[1u16, 2u16]), [0, 1, 0, 2]);
        assert_eq!(
            slice_into_u8_big_endian(&[1u32, 2u32]),
            [0, 0, 0, 1, 0, 0, 0, 2]
        );
    }
}
