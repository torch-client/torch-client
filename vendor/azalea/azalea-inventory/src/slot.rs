use std::{
    any::Any,
    borrow::Cow,
    fmt::{self, Debug},
    io::{self, Cursor, Write},
    num::NonZeroU32,
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

    pub fn known_kind(&self) -> Option<ItemKind> {
        self.as_present().and_then(ItemStackData::known_kind)
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

#[derive(Clone, Debug, Serialize)]
pub struct ItemStackData {
    #[serde(rename = "id")]
    pub kind: ItemKind,
    #[serde(skip_serializing_if = "is_one")]
    pub count: i32,
    #[serde(rename = "components", skip_serializing_if = "is_default")]
    pub component_patch: DataComponentPatch,
}

impl PartialEq for ItemStackData {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.count == other.count
            && self.component_patch == other.component_patch
            && self.modded_kind() == other.modded_kind()
    }
}

fn is_one(count: &i32) -> bool {
    *count == 1
}

impl ItemStackData {
    pub fn new(item: ItemKind, count: i32) -> Self {
        ItemStackData {
            count,
            kind: item,
            component_patch: Default::default(),
        }
    }

    pub fn modded_kind(&self) -> Option<NonZeroU32> {
        self.component_patch.inner.modded_kind
    }

    pub fn set_modded_kind(&mut self, id: Option<NonZeroU32>) {
        self.component_patch.inner.modded_kind = id;
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

    pub fn is_modded(&self) -> bool {
        self.modded_kind().is_some()
    }

    pub fn known_kind(&self) -> Option<ItemKind> {
        (!self.is_modded()).then_some(self.kind)
    }

    pub fn is_same_item_and_components(&self, other: &ItemStackData) -> bool {
        self.kind == other.kind
            && self.modded_kind() == other.modded_kind()
            && self.component_patch == other.component_patch
    }

    pub fn get_component<'a, T: components::DataComponentTrait>(&'a self) -> Option<Cow<'a, T>> {
        if let Some(c) = self.component_patch.get::<T>() {
            Some(Cow::Borrowed(c))
        } else {
            get_default_component::<T>(self.kind).map(|c| Cow::Owned(c))
        }
    }

    pub fn get_unless_removed<'a, T: components::DataComponentTrait>(
        &'a self,
    ) -> Option<Cow<'a, T>> {
        match self.component_patch.iter().find(|(kind, _)| *kind == T::KIND) {
            Some((_, None)) => None,
            _ => self.get_component::<T>(),
        }
    }
}

pub fn read_item_kind(
    buf: &mut Cursor<&[u8]>,
) -> Result<(ItemKind, Option<NonZeroU32>), BufReadError> {
    use azalea_registry::unknown::{Space, read_id};
    Ok(match read_id(buf, Space::Items)? {
        Ok(kind) => (kind, None),
        Err(raw) => match NonZeroU32::new(raw) {
            Some(raw) => (ItemKind::Barrier, Some(raw)),
            None => (ItemKind::Air, None),
        },
    })
}

pub fn write_item_kind(
    kind: ItemKind,
    modded: Option<NonZeroU32>,
    buf: &mut impl Write,
) -> io::Result<()> {
    match modded {
        Some(raw) => raw.get().azalea_write_var(buf),
        None => kind.azalea_write(buf),
    }
}

impl ItemStack {
    pub fn read_strict(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        Self::read_counted(buf, true)
    }

    fn read_counted(buf: &mut Cursor<&[u8]>, strict: bool) -> Result<Self, BufReadError> {
        let count = i32::azalea_read_var(buf)?;
        if count <= 0 {
            return Ok(ItemStack::Empty);
        }
        let (kind, modded_kind) = read_item_kind(buf)?;
        let mut component_patch = match DataComponentPatch::azalea_read(buf) {
            Ok(patch) => patch,
            Err(e) if strict => return Err(e),
            Err(e) => {
                tracing::warn!(
                    "dropping unparseable item component patch for {kind:?} (keeping count/kind): {e}"
                );
                DataComponentPatch::default()
            }
        };
        component_patch.inner.modded_kind = modded_kind;
        Ok(ItemStack::from(ItemStackData {
            count,
            kind,
            component_patch,
        }))
    }

    pub fn read_template(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        let (kind, modded_kind) = read_item_kind(buf)?;
        let count = i32::azalea_read_var(buf)?;
        let mut component_patch = DataComponentPatch::azalea_read(buf)?;
        component_patch.inner.modded_kind = modded_kind;
        Ok(ItemStack::from(ItemStackData {
            count,
            kind,
            component_patch,
        }))
    }

    pub fn write_template(&self, buf: &mut impl Write) -> io::Result<()> {
        let ItemStack::Present(i) = self else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "an ItemStackTemplate cannot be empty",
            ));
        };
        write_item_kind(i.kind, i.modded_kind(), buf)?;
        i.count.azalea_write_var(buf)?;
        i.component_patch.azalea_write(buf)
    }
}

impl AzBuf for ItemStack {
    fn azalea_read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        Self::read_counted(buf, false)
    }
    fn azalea_write(&self, buf: &mut impl Write) -> io::Result<()> {
        match self {
            ItemStack::Empty => 0_i32.azalea_write_var(buf)?,
            ItemStack::Present(i) => {
                i.count.azalea_write_var(buf)?;
                write_item_kind(i.kind, i.modded_kind(), buf)?;
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
    inner: Box<PatchInner>,
}

#[derive(Default)]
struct PatchInner {
    components: IndexMap<DataComponentKind, Option<DataComponentUnion>>,
    modded_kind: Option<NonZeroU32>,
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
        self.inner.components.get(&kind).and_then(|c| {
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
        self.inner.components.iter().map(|(&kind, component)| {
            component.as_ref().map_or_else(
                || (kind, None),
                |c| (kind, unsafe { Some(c.as_kind(kind)) }),
            )
        })
    }
    pub fn serializable(&self) -> impl Iterator<Item = (DataComponentKind, Option<SerializeComponent<'_>>)> + '_ {
        self.inner.components
            .iter()
            .map(|(&kind, c)| (kind, c.as_ref().map(|value| SerializeComponent { kind, value })))
    }

    pub unsafe fn unchecked_insert_component(
        &mut self,
        kind: DataComponentKind,
        value: Option<DataComponentUnion>,
    ) {
        let existing = self.inner.components.insert(kind, value);
        if let Some(Some(mut existing)) = existing {
            unsafe { existing.drop_as(kind) };
        }
    }

    pub fn clear_kind(&mut self, kind: DataComponentKind) {
        if let Some(Some(mut existing)) = self.inner.components.shift_remove(&kind) {
            unsafe { existing.drop_as(kind) };
        }
    }
}

pub struct SerializeComponent<'a> {
    kind: DataComponentKind,
    value: &'a DataComponentUnion,
}

impl Serialize for SerializeComponent<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        unsafe { self.value.serialize_as(serializer, self.kind) }
    }
}

impl Drop for DataComponentPatch {
    fn drop(&mut self) {
        for (kind, component) in self.inner.components.iter_mut() {
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
        for component in self.inner.components.values() {
            if component.is_some() {
                components_with_data_count += 1;
            } else {
                components_without_data_count += 1;
            }
        }

        components_with_data_count.azalea_write_var(buf)?;
        components_without_data_count.azalea_write_var(buf)?;

        let mut component_buf = Vec::new();
        for (kind, component) in self.inner.components.iter() {
            if let Some(component) = component {
                kind.azalea_write(buf)?;

                component_buf.clear();
                unsafe { component.azalea_write_as(*kind, &mut component_buf) }?;
                buf.write_all(&component_buf)?;
            }
        }

        for (kind, component) in self.inner.components.iter() {
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
        for component in self.inner.components.values() {
            if component.is_some() {
                components_with_data_count += 1;
            } else {
                components_without_data_count += 1;
            }
        }

        components_with_data_count.azalea_write_var(buf)?;
        components_without_data_count.azalea_write_var(buf)?;

        let mut component_buf = Vec::new();
        for (kind, component) in self.inner.components.iter() {
            if let Some(component) = component {
                kind.azalea_write(buf)?;

                component_buf.clear();
                unsafe { component.azalea_write_as(*kind, &mut component_buf) }?;
                (component_buf.len() as u32).azalea_write_var(buf)?;
                buf.write_all(&component_buf)?;
            }
        }

        for (kind, component) in self.inner.components.iter() {
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
        let (kind, modded_kind) = read_item_kind(buf)?;
        let mut component_patch = DataComponentPatch::azalea_read_delimited(buf)?;
        component_patch.inner.modded_kind = modded_kind;
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
                write_item_kind(i.kind, i.modded_kind(), buf)?;
                i.component_patch.azalea_write_delimited(buf)?;
            }
        };
        Ok(())
    }
}

impl Clone for DataComponentPatch {
    fn clone(&self) -> Self {
        let mut components = IndexMap::with_capacity(self.inner.components.len());
        for (kind, component) in self.inner.components.iter() {
            components.insert(
                *kind,
                component.as_ref().map(|c| unsafe { c.clone_as(*kind) }),
            );
        }
        DataComponentPatch {
            inner: Box::new(PatchInner {
                components,
                modded_kind: self.inner.modded_kind,
            }),
        }
    }
}
impl Debug for DataComponentPatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.inner.components.keys()).finish()
    }
}
impl PartialEq for DataComponentPatch {
    fn eq(&self, other: &Self) -> bool {
        if self.inner.components.len() != other.inner.components.len() {
            return false;
        }
        for (kind, component) in self.inner.components.iter() {
            let Some(other_component) = other.inner.components.get(kind) else {
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
        let mut s = serializer.serialize_map(Some(self.inner.components.len()))?;
        for (kind, component) in self.inner.components.iter() {
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

    const MODDED: u32 = 60000;

    fn empty_patch(bytes: &mut Vec<u8>) {
        0_u32.azalea_write_var(bytes).unwrap();
        0_u32.azalea_write_var(bytes).unwrap();
    }

    fn assert_modded(stack: &ItemStack) {
        let ItemStack::Present(data) = stack else {
            panic!("a modded item read as an empty slot");
        };
        assert_eq!(data.kind, ItemKind::Barrier);
        assert_eq!(data.count, 3);
        assert_eq!(data.modded_kind(), NonZeroU32::new(MODDED));
    }

    #[test]
    fn an_unknown_item_keeps_its_id_through_the_counted_codec() {
        let mut bytes = Vec::new();
        3_i32.azalea_write_var(&mut bytes).unwrap();
        MODDED.azalea_write_var(&mut bytes).unwrap();
        empty_patch(&mut bytes);

        let stack = ItemStack::azalea_read(&mut Cursor::new(&bytes[..])).unwrap();
        assert_modded(&stack);

        let mut written = Vec::new();
        stack.azalea_write(&mut written).unwrap();
        assert_eq!(written, bytes);
    }

    #[test]
    fn an_unknown_item_keeps_its_id_through_the_template_codec() {
        let mut bytes = Vec::new();
        MODDED.azalea_write_var(&mut bytes).unwrap();
        3_i32.azalea_write_var(&mut bytes).unwrap();
        empty_patch(&mut bytes);

        let stack = ItemStack::read_template(&mut Cursor::new(&bytes[..])).unwrap();
        assert_modded(&stack);

        let mut written = Vec::new();
        stack.write_template(&mut written).unwrap();
        assert_eq!(written, bytes);
    }

    #[test]
    fn a_modded_item_is_not_a_barrier_or_another_modded_item() {
        let barrier = ItemStackData::new(ItemKind::Barrier, 1);
        let mut modded = barrier.clone();
        modded.set_modded_kind(NonZeroU32::new(MODDED));
        let mut other = barrier.clone();
        other.set_modded_kind(NonZeroU32::new(MODDED + 1));

        assert!(!modded.is_same_item_and_components(&barrier));
        assert!(!modded.is_same_item_and_components(&other));
        assert!(modded.is_same_item_and_components(&modded.clone()));
        assert_eq!(modded.split(1).modded_kind(), NonZeroU32::new(MODDED));
    }
}
