use std::io::{self, Cursor, Write};

use azalea_buf::{AzBuf, AzBufVar, BufReadError};
use azalea_core::entity_id::MinecraftEntityId;
use azalea_inventory::components::AttributeModifier;
use azalea_protocol_macros::ClientboundGamePacket;
use azalea_registry::{
    builtin::Attribute,
    unknown::{self, Space},
};

#[derive(ClientboundGamePacket, Clone, Debug, PartialEq)]
pub struct ClientboundUpdateAttributes {
    pub entity_id: MinecraftEntityId,
    pub values: Vec<AttributeSnapshot>,
}

impl AzBuf for ClientboundUpdateAttributes {
    fn azalea_read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        let entity_id = MinecraftEntityId::azalea_read_var(buf)?;
        let values = unknown::read_list_skipping(buf, |buf| {
            let attribute = unknown::read_id::<Attribute>(buf, Space::Attributes)?;
            let base = f64::azalea_read(buf)?;
            let modifiers = Vec::<AttributeModifier>::azalea_read(buf)?;
            Ok(attribute.ok().map(|attribute| AttributeSnapshot {
                attribute,
                base,
                modifiers,
            }))
        })?;
        Ok(Self { entity_id, values })
    }
    fn azalea_write(&self, buf: &mut impl Write) -> io::Result<()> {
        self.entity_id.azalea_write_var(buf)?;
        self.values.azalea_write(buf)
    }
}

#[derive(AzBuf, Clone, Debug, PartialEq)]
pub struct AttributeSnapshot {
    pub attribute: Attribute,
    pub base: f64,
    pub modifiers: Vec<AttributeModifier>,
}

#[cfg(test)]
mod tests {
    use azalea_core::attribute_modifier_operation::AttributeModifierOperation;
    use azalea_registry::identifier::Identifier;

    use super::*;

    fn snapshot(attribute: Attribute, base: f64) -> AttributeSnapshot {
        AttributeSnapshot {
            attribute,
            base,
            modifiers: vec![AttributeModifier {
                id: Identifier::new("example:speed"),
                amount: 0.5,
                operation: AttributeModifierOperation::AddValue,
            }],
        }
    }

    #[test]
    fn unknown_attribute_keeps_the_known_ones() {
        let first = snapshot(Attribute::Armor, 2.0);
        let last = snapshot(Attribute::MaxHealth, 20.0);

        let mut bytes = Vec::new();
        MinecraftEntityId(7).azalea_write_var(&mut bytes).unwrap();
        3_u32.azalea_write_var(&mut bytes).unwrap();
        first.azalea_write(&mut bytes).unwrap();
        9999_u32.azalea_write_var(&mut bytes).unwrap();
        1.5_f64.azalea_write(&mut bytes).unwrap();
        first.modifiers.azalea_write(&mut bytes).unwrap();
        last.azalea_write(&mut bytes).unwrap();

        let mut cursor = Cursor::new(&bytes[..]);
        let packet = ClientboundUpdateAttributes::azalea_read(&mut cursor).unwrap();

        assert_eq!(cursor.position(), bytes.len() as u64);
        assert_eq!(packet.entity_id, MinecraftEntityId(7));
        assert_eq!(packet.values, vec![first, last]);
        assert!(unknown::snapshot().attributes >= 1);
    }
}
