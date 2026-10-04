use std::{
    any::Any,
    borrow::Cow,
    fmt::{self, Debug},
    io::{self, Cursor, Write},
    sync::LazyLock,
};

use azalea_buf::{AzBuf, AzBufVar, BufReadError};
use azalea_core::codec_utils::is_default;
use azalea_registry::builtin::{DataComponentKind, ItemKind};
use indexmap::IndexMap;
use serde::{Serialize, ser::SerializeMap};

use crate::{
    components::{self, DataComponentUnion},
    default_components::get_default_component,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub enum ItemStack {
    #[default]
    Empty,
    Present(ItemStackData),
}

impl ItemStack {
    pub fn new(item: ItemKind, count: i32) -> Self {
        let mut i = ItemStack::Present(ItemStackData::new(item, count));
        i.update_empty();
        i
    }

    pub fn is_empty(&self) -> bool {
        match self {
            ItemStack::Empty => true,
            ItemStack::Present(item) => item.is_empty(),
        }
    }
    pub fn is_present(&self) -> bool {
        !self.is_empty()
    }

    pub fn count(&self) -> i32 {
        match self {
            ItemStack::Empty => 0,
            ItemStack::Present(i) => i.count,
        }
    }

    pub fn split(&mut self, count: u32) -> ItemStack {
        match self {
            ItemStack::Empty => ItemStack::Empty,
            ItemStack::Present(i) => {
                let returning = i.split(count);
                if i.is_empty() {
                    *self = ItemStack::Empty;
                }
                ItemStack::Present(returning)
            }
        }
    }

    pub fn kind(&self) -> ItemKind {
        match self {
            ItemStack::Empty => ItemKind::Air,
            ItemStack::Present(i) => i.kind,
        }
    }

    pub fn update_empty(&mut self) {
        if let ItemStack::Present(i) = self
            && i.is_empty()
        {
            *self = ItemStack::Empty;
        }
    }

    pub fn as_present(&self) -> Option<&ItemStackData> {
        match self {
            ItemStack::Empty => None,
            ItemStack::Present(i) => Some(i),
        }
    }

    pub fn as_present_mut(&mut self) -> Option<&mut ItemStackData> {
        match self {
            ItemStack::Empty => None,
            ItemStack::Present(i) => Some(i),
        }
    }

    pub fn component_patch(&self) -> &DataComponentPatch {
        self.as_present()
            .map_or_else(|| &*EMPTY_DATA_COMPONENT_PATCH, |i| &i.component_patch)
    }

    pub fn get_component<'a, T: components::DataComponentTrait>(&'a self) -> Option<Cow<'a, T>> {
        self.as_present().and_then(|i| i.get_component::<T>())
    }

    pub fn with_component<
        T: components::EncodableDataComponent + components::DataComponentTrait,
    >(
        mut self,
        component: impl Into<Option<T>>,
    ) -> Self {
        if let ItemStack::Present(i) = &mut self {
            let component: Option<T> = component.into();
            let component: Option<DataComponentUnion> = component.map(|c| c.into());
            unsafe {
                i.component_patch
                    .unchecked_insert_component(T::KIND, component)
            };
        }
        self
    }
}
impl Serialize for ItemStack {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ItemStack::Empty => serializer.serialize_unit(),
            ItemStack::Present(i) => i.serialize(serializer),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ItemStackData {
    #[serde(rename = "id")]
    pub kind: ItemKind,
    pub count: i32,
    #[serde(rename = "components", skip_serializing_if = "is_default")]
    pub component_patch: DataComponentPatch,
}

impl ItemStackData {
    pub fn new(item: ItemKind, count: i32) -> Self {
        ItemStackData {
            count,
            kind: item,
            component_patch: Default::default(),
        }
    }

    pub fn split(&mut self, count: u32) -> ItemStackData {
        let returning_count = i32::min(count as i32, self.count);
        let mut returning = self.clone();
        returning.count = returning_count;
        self.count -= returning_count;
        returning
    }

    pub fn is_empty(&self) -> bool {
        self.count <= 0 || self.kind == ItemKind::Air
    }

    pub fn is_same_item_and_components(&self, other: &ItemStackData) -> bool {
        self.kind == other.kind && self.component_patch == other.component_patch
    }

    pub fn get_component<'a, T: components::DataComponentTrait>(&'a self) -> Option<Cow<'a, T>> {
        if let Some(c) = self.component_patch.get::<T>() {
            Some(Cow::Borrowed(c))
        } else {
            get_default_component::<T>(self.kind).map(|c| Cow::Owned(c))
        }
    }
}

impl AzBuf for ItemStack {
    fn azalea_read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        let count = i32::azalea_read_var(buf)?;
        if count <= 0 {
            Ok(ItemStack::Empty)
        } else {
            let kind = ItemKind::azalea_read(buf)?;
            let component_patch = match DataComponentPatch::azalea_read(buf) {
                Ok(patch) => patch,
                Err(e) => {
                    tracing::warn!(
                        "dropping unparseable item component patch for {kind:?} (keeping count/kind): {e}"
                    );
                    DataComponentPatch::default()
                }
            };
            Ok(ItemStack::from(ItemStackData {
                count,
                kind,
                component_patch,
            }))
        }
    }
    fn azalea_write(&self, buf: &mut impl Write) -> io::Result<()> {
        match self {
            ItemStack::Empty => 0_i32.azalea_write_var(buf)?,
            ItemStack::Present(i) => {
                i.count.azalea_write_var(buf)?;
                i.kind.azalea_write(buf)?;
                i.component_patch.azalea_write(buf)?;
            }
        };
        Ok(())
    }
}

impl From<ItemStackData> for ItemStack {
    fn from(item: ItemStackData) -> Self {
        if item.is_empty() {
            ItemStack::Empty
        } else {
            ItemStack::Present(item)
        }
    }
}
impl From<ItemKind> for ItemStack {
    fn from(item: ItemKind) -> Self {
        ItemStack::new(item, 1)
    }
}
impl From<(ItemKind, i32)> for ItemStack {
    fn from(item: (ItemKind, i32)) -> Self {
        ItemStack::new(item.0, item.1)
    }
}
impl From<ItemKind> for ItemStackData {
    fn from(item: ItemKind) -> Self {
        ItemStackData::new(item, 1)
    }
}
impl From<(ItemKind, i32)> for ItemStackData {
    fn from(item: (ItemKind, i32)) -> Self {
        ItemStackData::new(item.0, item.1)
    }
}

#[derive(Default)]
pub struct DataComponentPatch {
    components: Box<IndexMap<DataComponentKind, Option<DataComponentUnion>>>,
}

static EMPTY_DATA_COMPONENT_PATCH: LazyLock<DataComponentPatch> =
    LazyLock::new(DataComponentPatch::default);

impl DataComponentPatch {
    pub fn get<T: components::DataComponentTrait>(&self) -> Option<&T> {
        let component = self.get_kind(T::KIND)?;
        let component_any = component as &dyn Any;
        component_any.downcast_ref::<T>()
    }

    pub fn get_kind(
        &self,
        kind: DataComponentKind,
    ) -> Option<&dyn components::EncodableDataComponent> {
        self.components.get(&kind).and_then(|c| {
            c.as_ref().map(|c| {
                unsafe { c.as_kind(kind) }
            })
        })
    }

    pub fn has<T: components::DataComponentTrait>(&self) -> bool {
        self.has_kind(T::KIND)
    }

    pub fn has_kind(&self, kind: DataComponentKind) -> bool {
        self.get_kind(kind).is_some()
    }

    pub fn iter<'a>(
        &'a self,
    ) -> impl Iterator<
        Item = (
            DataComponentKind,
            Option<&'a dyn components::EncodableDataComponent>,
        ),
    > + 'a {
        self.components.iter().map(|(&kind, component)| {
            component.as_ref().map_or_else(
                || (kind, None),
                |c| (kind, unsafe { Some(c.as_kind(kind)) }),
            )
        })
    }
    pub unsafe fn unchecked_insert_component(
        &mut self,
        kind: DataComponentKind,
        value: Option<DataComponentUnion>,
    ) {
        let existing = self.components.insert(kind, value);
        if let Some(Some(mut existing)) = existing {
            unsafe { existing.drop_as(kind) };
        }
    }
}

impl Drop for DataComponentPatch {
    fn drop(&mut self) {
        for (kind, component) in self.components.iter_mut() {
            if let Some(component) = component {
                unsafe { component.drop_as(*kind) };
            }
        }
    }
}

impl AzBuf for DataComponentPatch {
    fn azalea_read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        let components_with_data_count = u32::azalea_read_var(buf)?;
        let components_without_data_count = u32::azalea_read_var(buf)?;

        if components_without_data_count == 0 && components_with_data_count == 0 {
            return Ok(DataComponentPatch::default());
        }

        let mut components = DataComponentPatch::default();

        for _ in 0..components_with_data_count {
            let component_kind = DataComponentKind::azalea_read(buf)?;
            let component_data = DataComponentUnion::azalea_read_as(component_kind, buf)?;
            unsafe { components.unchecked_insert_component(component_kind, Some(component_data)) };
        }

        for _ in 0..components_without_data_count {
            let component_kind = DataComponentKind::azalea_read(buf)?;
            unsafe { components.unchecked_insert_component(component_kind, None) };
        }

        Ok(components)
    }
    fn azalea_write(&self, buf: &mut impl Write) -> io::Result<()> {
        let mut components_with_data_count: u32 = 0;
        let mut components_without_data_count: u32 = 0;
        for component in self.components.values() {
            if component.is_some() {
                components_with_data_count += 1;
            } else {
                components_without_data_count += 1;
            }
        }

        components_with_data_count.azalea_write_var(buf)?;
        components_without_data_count.azalea_write_var(buf)?;

        let mut component_buf = Vec::new();
        for (kind, component) in self.components.iter() {
            if let Some(component) = component {
                kind.azalea_write(buf)?;

                component_buf.clear();
                unsafe { component.azalea_write_as(*kind, &mut component_buf) }?;
                buf.write_all(&component_buf)?;
            }
        }

        for (kind, component) in self.components.iter() {
            if component.is_none() {
                kind.azalea_write(buf)?;
            }
        }

        Ok(())
    }
}

impl DataComponentPatch {
    pub fn azalea_write_delimited(&self, buf: &mut impl Write) -> io::Result<()> {
        let mut components_with_data_count: u32 = 0;
        let mut components_without_data_count: u32 = 0;
        for component in self.components.values() {
            if component.is_some() {
                components_with_data_count += 1;
            } else {
                components_without_data_count += 1;
            }
        }

        components_with_data_count.azalea_write_var(buf)?;
        components_without_data_count.azalea_write_var(buf)?;

        let mut component_buf = Vec::new();
        for (kind, component) in self.components.iter() {
            if let Some(component) = component {
                kind.azalea_write(buf)?;

                component_buf.clear();
                unsafe { component.azalea_write_as(*kind, &mut component_buf) }?;
                (component_buf.len() as u32).azalea_write_var(buf)?;
                buf.write_all(&component_buf)?;
            }
        }

        for (kind, component) in self.components.iter() {
            if component.is_none() {
                kind.azalea_write(buf)?;
            }
        }

        Ok(())
    }

    pub fn azalea_read_delimited(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        let components_with_data_count = u32::azalea_read_var(buf)?;
        let components_without_data_count = u32::azalea_read_var(buf)?;

        if components_without_data_count == 0 && components_with_data_count == 0 {
            return Ok(DataComponentPatch::default());
        }

        let mut components = DataComponentPatch::default();

        for _ in 0..components_with_data_count {
            let component_kind = DataComponentKind::azalea_read(buf)?;
            let length = u32::azalea_read_var(buf)? as usize;
            let data: &[u8] = *buf.get_ref();
            let start = buf.position() as usize;
            let end = start
                .checked_add(length)
                .filter(|end| *end <= data.len())
                .ok_or_else(|| {
                    BufReadError::Custom(format!(
                        "component of {length} bytes runs past the end of the buffer"
                    ))
                })?;
            let mut inner = Cursor::new(&data[start..end]);
            let component_data = DataComponentUnion::azalea_read_as(component_kind, &mut inner)?;
            buf.set_position(end as u64);
            unsafe { components.unchecked_insert_component(component_kind, Some(component_data)) };
        }

        for _ in 0..components_without_data_count {
            let component_kind = DataComponentKind::azalea_read(buf)?;
            unsafe { components.unchecked_insert_component(component_kind, None) };
        }

        Ok(components)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub struct UntrustedItemStack(pub ItemStack);

impl From<ItemStack> for UntrustedItemStack {
    fn from(stack: ItemStack) -> Self {
        UntrustedItemStack(stack)
    }
}

impl AzBuf for UntrustedItemStack {
    fn azalea_read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        let count = i32::azalea_read_var(buf)?;
        if count <= 0 {
            return Ok(UntrustedItemStack(ItemStack::Empty));
        }
        let kind = ItemKind::azalea_read(buf)?;
        let component_patch = DataComponentPatch::azalea_read_delimited(buf)?;
        Ok(UntrustedItemStack(ItemStack::from(ItemStackData {
            count,
            kind,
            component_patch,
        })))
    }
    fn azalea_write(&self, buf: &mut impl Write) -> io::Result<()> {
        match &self.0 {
            ItemStack::Empty => 0_i32.azalea_write_var(buf)?,
            ItemStack::Present(i) => {
                i.count.azalea_write_var(buf)?;
                i.kind.azalea_write(buf)?;
                i.component_patch.azalea_write_delimited(buf)?;
            }
        };
        Ok(())
    }
}

impl Clone for DataComponentPatch {
    fn clone(&self) -> Self {
        let mut components = IndexMap::with_capacity(self.components.len());
        for (kind, component) in self.components.iter() {
            components.insert(
                *kind,
                component.as_ref().map(|c| unsafe { c.clone_as(*kind) }),
            );
        }
        DataComponentPatch {
            components: Box::new(components),
        }
    }
}
impl Debug for DataComponentPatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.components.keys()).finish()
    }
}
impl PartialEq for DataComponentPatch {
    fn eq(&self, other: &Self) -> bool {
        if self.components.len() != other.components.len() {
            return false;
        }
        for (kind, component) in self.components.iter() {
            let Some(other_component) = other.components.get(kind) else {
                return false;
            };
            if let Some(component) = component {
                let Some(other_component) = other_component else {
                    return false;
                };
                if !unsafe { component.eq_as(other_component, *kind) } {
                    return false;
                }
            } else if other_component.is_some() {
                return false;
            }
        }
        true
    }
}

impl Serialize for DataComponentPatch {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_map(Some(self.components.len()))?;
        for (kind, component) in self.components.iter() {
            if let Some(component) = component {
                unsafe { component.serialize_entry_as(&mut s, *kind) }?;
            } else {
                #[derive(Serialize)]
                struct EmptyComponent;
                s.serialize_entry(&format!("!{kind}"), &EmptyComponent)?;
            }
        }
        s.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::MapId;

    #[test]
    fn test_get_component() {
        let item = ItemStack::from(ItemKind::Map).with_component(MapId { id: 1 });
        let map_id = item.get_component::<MapId>().unwrap();
        assert_eq!(map_id.id, 1);
    }
}
