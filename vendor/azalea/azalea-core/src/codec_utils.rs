use azalea_buf::SerializableUuid;
use serde::{Serialize, Serializer, ser::SerializeTupleStruct};
use uuid::Uuid;

pub fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    *t == Default::default()
}

pub fn is_true(t: &bool) -> bool {
    *t
}

pub fn flatten_array<S: Serializer, T: Serialize>(x: &Vec<T>, s: S) -> Result<S::Ok, S::Error> {
    if x.len() == 1 {
        x[0].serialize(s)
    } else {
        x.serialize(s)
    }
}

pub fn uuid<'a, S: Serializer>(
    uuid: impl Into<&'a Option<Uuid>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    if let Some(uuid) = uuid.into() {
        let arr: [u32; 4] = uuid.to_int_array();
        let arr: [i32; 4] = [arr[0] as i32, arr[1] as i32, arr[2] as i32, arr[3] as i32];
        IntArray(arr).serialize(serializer)
    } else {
        serializer.serialize_unit()
    }
}

pub struct IntArray<const N: usize>(pub [i32; N]);
pub struct LongArray<const N: usize>(pub [i64; N]);

impl<const N: usize> Serialize for IntArray<N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_tuple_struct("azalea:int_array", N)?;
        for &item in &self.0 {
            seq.serialize_field(&item)?;
        }
        seq.end()
    }
}
impl<const N: usize> Serialize for LongArray<N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_tuple_struct("azalea:long_array", N)?;
        for &item in &self.0 {
            seq.serialize_field(&item)?;
        }
        seq.end()
    }
}
