use std::{
    fmt::{self, Display},
    hash::{Hash, Hasher},
    io::{self, Cursor},
};

use azalea_buf::{AzBuf, AzBufVar, BufReadError};
use derive_more::{Deref, DerefMut};

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "bevy_ecs", derive(bevy_ecs::component::Component))]
#[derive(Clone, Copy, Debug, Default, Deref, DerefMut, Eq, PartialEq)]
pub struct MinecraftEntityId(pub i32);

impl Hash for MinecraftEntityId {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        hasher.write_i32(self.0);
    }
}
impl nohash_hasher::IsEnabled for MinecraftEntityId {}

impl AzBuf for MinecraftEntityId {
    fn azalea_read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        i32::azalea_read(buf).map(MinecraftEntityId)
    }
    fn azalea_write(&self, buf: &mut impl io::Write) -> io::Result<()> {
        i32::azalea_write(&self.0, buf)
    }
}
impl AzBufVar for MinecraftEntityId {
    fn azalea_read_var(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        i32::azalea_read_var(buf).map(MinecraftEntityId)
    }
    fn azalea_write_var(&self, buf: &mut impl io::Write) -> io::Result<()> {
        i32::azalea_write_var(&self.0, buf)
    }
}
impl Display for MinecraftEntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "eid({})", self.0)
    }
}
impl From<i32> for MinecraftEntityId {
    fn from(id: i32) -> Self {
        Self(id)
    }
}
impl From<u32> for MinecraftEntityId {
    fn from(id: u32) -> Self {
        Self(id as i32)
    }
}
