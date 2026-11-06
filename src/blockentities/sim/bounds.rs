use azalea_protocol::packets::game::s_set_structure_block::{Mirror, Rotation};
use azalea_registry::builtin::BlockEntityKind;
use simdnbt::owned::{Nbt, NbtCompound};

use crate::blockentities::game_master::{
    MAX_OFFSET, MAX_SIZE, axes, mirror_legacy, rotation_legacy, rotation_named, vec3i,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    None,
    Box,
    BoxAndInvisibleBlocks,
}

const STRUCTURE_OFFSET: [i32; 3] = [0, 1, 1];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Data {
    pub mode: Mode,
    pub local: [i32; 3],
    pub size: [i32; 3],
    pub test_instance: bool,
    pub beam: Option<u32>,
    pub errors: Vec<[i32; 3]>,
}

pub fn decode(kind: BlockEntityKind, tag: &Nbt) -> Data {
    match kind {
        BlockEntityKind::StructureBlock => structure(tag),
        BlockEntityKind::TestInstanceBlock => test_instance(tag),
        _ => Data::default(),
    }
}

pub fn beam_color(data: &Data) -> Option<u32> {
    data.beam
}

fn structure(tag: &Nbt) -> Data {
    let tag: &NbtCompound = tag;
    let mode = tag.string("mode").map(|s| s.to_string());
    let show_air = tag.byte("showair").unwrap_or(0) != 0;
    let show_box = tag.byte("showboundingbox").unwrap_or(1) != 0;
    let mode = match mode.as_deref() {
        Some("SAVE") if show_air => Mode::BoxAndInvisibleBlocks,
        Some("SAVE") => Mode::Box,
        Some("LOAD") if show_box => Mode::Box,
        _ => Mode::None,
    };
    let pos = axes(tag, "pos", [0, 1, 0], -MAX_OFFSET, MAX_OFFSET);
    let size = axes(tag, "size", [0; 3], 0, MAX_SIZE);
    let turn = tag
        .string("rotation")
        .and_then(|s| rotation_legacy(&s.to_string()))
        .unwrap_or(Rotation::None);
    let flip = tag
        .string("mirror")
        .and_then(|s| mirror_legacy(&s.to_string()))
        .unwrap_or(Mirror::None);
    let (local, size) = structure_box(pos, size, turn, flip);
    Data {
        mode,
        local,
        size,
        ..Data::default()
    }
}

fn structure_box(
    pos: [i32; 3],
    size: [i32; 3],
    turn: Rotation,
    flip: Mirror,
) -> ([i32; 3], [i32; 3]) {
    let [xo, y0, zo] = pos;
    let y1 = y0 + size[1];
    let (xd, zd) = match flip {
        Mirror::LeftRight => (size[0], -size[2]),
        Mirror::FrontBack => (-size[0], size[2]),
        Mirror::None => (size[0], size[2]),
    };
    let (x0, z0, x1, z1) = match turn {
        Rotation::Clockwise90 => {
            let x0 = if zd < 0 { xo } else { xo + 1 };
            let z0 = if xd < 0 { zo + 1 } else { zo };
            (x0, z0, x0 - zd, z0 + xd)
        }
        Rotation::Clockwise180 => {
            let x0 = if xd < 0 { xo } else { xo + 1 };
            let z0 = if zd < 0 { zo } else { zo + 1 };
            (x0, z0, x0 - xd, z0 - zd)
        }
        Rotation::Counterclockwise90 => {
            let x0 = if zd < 0 { xo + 1 } else { xo };
            let z0 = if xd < 0 { zo } else { zo + 1 };
            (x0, z0, x0 + zd, z0 - xd)
        }
        Rotation::None => {
            let x0 = if xd < 0 { xo + 1 } else { xo };
            let z0 = if zd < 0 { zo + 1 } else { zo };
            (x0, z0, x0 + xd, z0 + zd)
        }
    };
    (
        [x0.min(x1), y0.min(y1), z0.min(z1)],
        [(x1 - x0).abs(), (y1 - y0).abs(), (z1 - z0).abs()],
    )
}

fn test_instance(tag: &Nbt) -> Data {
    let mut out = Data {
        test_instance: true,
        ..Data::default()
    };
    if let Some(data) = tag.compound("data") {
        let size = vec3i(data, "size").unwrap_or([0; 3]);
        let turn = data
            .string("rotation")
            .and_then(|s| rotation_named(&s.to_string()))
            .unwrap_or(Rotation::None);
        let size = if matches!(turn, Rotation::Clockwise90 | Rotation::Counterclockwise90) {
            [size[2], size[1], size[0]]
        } else {
            size
        };
        out.mode = Mode::Box;
        out.local = STRUCTURE_OFFSET;
        out.size = size;
        let failed = data.get("error_message").is_some();
        out.beam = match data.string("status").map(|s| s.to_string()).as_deref() {
            Some("running") => Some(0xFF80_8080),
            Some("finished") if failed => Some(0xFFFF_0000),
            Some("finished") => Some(0xFF00_FF00),
            _ => None,
        };
    }
    if let Some(list) = tag.list("errors").and_then(|l| l.compounds()) {
        out.errors = list
            .iter()
            .filter_map(|c| c.int_array("pos"))
            .filter(|a| a.len() == 3)
            .map(|a| [a[0], a[1], a[2]])
            .collect();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrotated_box_starts_at_the_offset() {
        let (local, size) = structure_box([2, 3, 4], [5, 6, 7], Rotation::None, Mirror::None);
        assert_eq!((local, size), ([2, 3, 4], [5, 6, 7]));
    }

    #[test]
    fn clockwise_90_swaps_the_horizontal_axes() {
        let (local, size) =
            structure_box([0, 0, 0], [3, 2, 5], Rotation::Clockwise90, Mirror::None);
        assert_eq!(size, [5, 2, 3]);
        assert_eq!(local, [-4, 0, 0]);
    }

    #[test]
    fn mirror_extends_the_box_backwards() {
        let (local, size) = structure_box([0, 0, 0], [3, 2, 5], Rotation::None, Mirror::FrontBack);
        assert_eq!((local, size), ([-2, 0, 0], [3, 2, 5]));
    }

    #[test]
    fn default_data_draws_nothing() {
        assert_eq!(Data::default().mode, Mode::None);
        assert_eq!(beam_color(&Data::default()), None);
    }
}
