use std::{
    any::Any,
    fmt::{self, Debug},
    io::{self, Cursor, Write},
};

use azalea_buf::{AzBuf, BufReadError};
use azalea_registry::builtin::DataComponentKind;

use super::{DataComponentTrait, DataComponentUnion, EncodableDataComponent};

pub struct TypedDataComponent {
    kind: DataComponentKind,
    value: DataComponentUnion,
}
impl TypedDataComponent {
    pub fn kind(&self) -> &DataComponentKind {
        &self.kind
    }
    pub fn value(&self) -> &DataComponentUnion {
        &self.value
    }
    pub fn as_dyn(&self) -> &dyn EncodableDataComponent {
        unsafe { self.value.as_kind(self.kind) }
    }
    pub fn get<T: DataComponentTrait>(&self) -> Option<&T> {
        let component = self.as_dyn();
        let component_any = component as &dyn Any;
        component_any.downcast_ref::<T>()
    }
}
impl AzBuf for TypedDataComponent {
    fn azalea_read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        let kind = DataComponentKind::azalea_read(buf)?;
        let value = DataComponentUnion::azalea_read_as(kind, buf)?;
        Ok(Self { kind, value })
    }
    fn azalea_write(&self, buf: &mut impl Write) -> io::Result<()> {
        self.kind.azalea_write(buf)?;
        unsafe { self.value.azalea_write_as(self.kind, buf) }
    }
}
impl Debug for TypedDataComponent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypedDataComponent")
            .field("kind", &self.kind)
            .finish()
    }
}
impl Clone for TypedDataComponent {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind,
            value: unsafe { self.value.clone_as(self.kind) },
        }
    }
}
impl Drop for TypedDataComponent {
    fn drop(&mut self) {
        unsafe { self.value.drop_as(self.kind) };
    }
}
impl PartialEq for TypedDataComponent {
    fn eq(&self, other: &Self) -> bool {
        if self.kind != other.kind {
            return false;
        }
        self.as_dyn().eq(other.as_dyn())
    }
}

#[derive(AzBuf, Clone, Debug, Default, PartialEq)]
pub struct DataComponentExactPredicate {
    pub expected: Vec<TypedDataComponent>,
}

#[derive(AzBuf, Clone, Debug, Default, PartialEq)]
pub struct DataComponentMatchers {
    pub exact: DataComponentExactPredicate,
    pub partial: Vec<PartialComponentPredicate>,
}

#[derive(AzBuf, Clone, Debug, PartialEq)]
pub struct PartialComponentPredicate {
    pub is_predicate_type: bool,
    #[var]
    pub type_id: u32,
    pub value: simdnbt::owned::NbtTag,
}
