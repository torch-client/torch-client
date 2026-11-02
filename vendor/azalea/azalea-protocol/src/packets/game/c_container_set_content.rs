use std::io::{self, Cursor, Write};

use azalea_buf::{AzBuf, AzBufVar, BufReadError};
use azalea_inventory::ItemStack;
use azalea_protocol_macros::ClientboundGamePacket;

#[derive(ClientboundGamePacket, Clone, Debug, PartialEq)]
pub struct ClientboundContainerSetContent {
    pub container_id: i32,
    pub state_id: u32,
    pub items: Vec<ItemStack>,
    pub carried_item: ItemStack,
}

impl AzBuf for ClientboundContainerSetContent {
    fn azalea_read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        let container_id = i32::azalea_read_var(buf)?;
        let state_id = u32::azalea_read_var(buf)?;
        let len = u32::azalea_read_var(buf)? as usize;
        let mut items = Vec::with_capacity(len.min(256));
        for _ in 0..len {
            items.push(ItemStack::read_strict(buf)?);
        }
        Ok(Self {
            container_id,
            state_id,
            items,
            carried_item: ItemStack::read_strict(buf)?,
        })
    }
    fn azalea_write(&self, buf: &mut impl Write) -> io::Result<()> {
        self.container_id.azalea_write_var(buf)?;
        self.state_id.azalea_write_var(buf)?;
        self.items.azalea_write(buf)?;
        self.carried_item.azalea_write(buf)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use azalea_buf::AzBuf;

    use super::ClientboundContainerSetContent;

    #[test]
    fn test_read_write_container_set_content() {
        let contents = [
            1, 2, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 1, 196, 6, 0, 0, 0,
        ];
        let mut buf = Cursor::new(contents.as_slice());
        let packet = ClientboundContainerSetContent::azalea_read(&mut buf).unwrap();
        println!("{packet:?}");

        assert_eq!(buf.position(), contents.len() as u64);

        let mut buf = Vec::new();
        packet.write(&mut buf).unwrap();
        assert_eq!(buf, contents);
    }
}
