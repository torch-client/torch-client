use std::io::{self, Cursor, Write};

use azalea_buf::{AzBuf, AzBufVar, BufReadError};
use azalea_protocol_macros::ServerboundGamePacket;
use azalea_registry::identifier::Identifier;
use simdnbt::owned::Nbt;

#[derive(Clone, Debug, PartialEq, ServerboundGamePacket)]
pub struct ServerboundCustomClickAction {
    pub id: Identifier,
    pub payload: Nbt,
}

impl AzBuf for ServerboundCustomClickAction {
    fn azalea_read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        let id = Identifier::azalea_read(buf)?;
        let data = Vec::<u8>::azalea_read(buf)?;
        let payload = if data.is_empty() {
            Nbt::None
        } else {
            simdnbt::owned::read_unnamed(&mut Cursor::new(&data[..]))?
        };
        Ok(Self { id, payload })
    }

    fn azalea_write(&self, buf: &mut impl Write) -> io::Result<()> {
        self.id.azalea_write(buf)?;
        let mut data = Vec::new();
        self.payload.write_unnamed(&mut data);
        (data.len() as u32).azalea_write_var(buf)?;
        buf.write_all(&data)
    }
}
