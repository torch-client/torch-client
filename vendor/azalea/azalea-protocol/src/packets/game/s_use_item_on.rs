use std::io::{self, Cursor, Write};

use azalea_buf::{AzBuf, BufReadError};
use azalea_core::{
    direction::Direction,
    hit_result::BlockHitResult,
    position::{BlockPos, Vec3, Vec3f32},
};
use azalea_protocol_macros::ServerboundGamePacket;

use crate::packets::game::s_interact::InteractionHand;

#[derive(AzBuf, Clone, Debug, PartialEq, ServerboundGamePacket)]
pub struct ServerboundUseItemOn {
    pub hand: InteractionHand,
    pub block_hit: BlockHit,
    #[var]
    pub seq: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockHit {
    pub block_pos: BlockPos,
    pub direction: Direction,
    pub location: Vec3,
    pub inside: bool,
    pub world_border: bool,
}

impl AzBuf for BlockHit {
    fn azalea_read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        let block_pos = BlockPos::azalea_read(buf)?;
        let direction = Direction::azalea_read(buf)?;
        let cursor = Vec3f32::azalea_read(buf)?;
        let inside = bool::azalea_read(buf)?;
        let world_border = bool::azalea_read(buf)?;
        Ok(Self {
            block_pos,
            direction,
            location: Vec3 {
                x: f64::from(block_pos.x) + f64::from(cursor.x),
                y: f64::from(block_pos.y) + f64::from(cursor.y),
                z: f64::from(block_pos.z) + f64::from(cursor.z),
            },
            inside,
            world_border,
        })
    }
    fn azalea_write(&self, buf: &mut impl Write) -> io::Result<()> {
        self.block_pos.azalea_write(buf)?;
        self.direction.azalea_write(buf)?;
        let cursor = Vec3 {
            x: self.location.x - f64::from(self.block_pos.x),
            y: self.location.y - f64::from(self.block_pos.y),
            z: self.location.z - f64::from(self.block_pos.z),
        };
        Vec3f32::from(cursor).azalea_write(buf)?;
        self.inside.azalea_write(buf)?;
        self.world_border.azalea_write(buf)?;
        Ok(())
    }
}

impl From<&BlockHitResult> for BlockHit {
    fn from(hit_result: &BlockHitResult) -> Self {
        Self {
            block_pos: hit_result.block_pos,
            direction: hit_result.direction,
            location: hit_result.location,
            inside: hit_result.inside,
            world_border: hit_result.world_border,
        }
    }
}
