use azalea::block::BlockState;
use azalea::block::properties::{RailShape as StraightShape, Shape as AnyShape};
use azalea_registry::builtin::BlockKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum RailShape {
    NorthSouth = 1,
    EastWest,
    AscendingEast,
    AscendingWest,
    AscendingNorth,
    AscendingSouth,
    SouthEast,
    SouthWest,
    NorthWest,
    NorthEast,
}

impl RailShape {
    pub(crate) fn from_u8(v: u8) -> Option<RailShape> {
        use RailShape::*;
        Some(match v {
            1 => NorthSouth,
            2 => EastWest,
            3 => AscendingEast,
            4 => AscendingWest,
            5 => AscendingNorth,
            6 => AscendingSouth,
            7 => SouthEast,
            8 => SouthWest,
            9 => NorthWest,
            10 => NorthEast,
            _ => return None,
        })
    }

    fn is_slope(self) -> bool {
        use RailShape::*;
        matches!(
            self,
            AscendingEast | AscendingWest | AscendingNorth | AscendingSouth
        )
    }

    fn exits(self) -> ([i32; 3], [i32; 3]) {
        const X_NEG: [i32; 3] = [-1, 0, 0];
        const X_POS: [i32; 3] = [1, 0, 0];
        const Z_NEG: [i32; 3] = [0, 0, -1];
        const Z_POS: [i32; 3] = [0, 0, 1];
        const X_NEG_BELOW: [i32; 3] = [-1, -1, 0];
        const X_POS_BELOW: [i32; 3] = [1, -1, 0];
        const Z_NEG_BELOW: [i32; 3] = [0, -1, -1];
        const Z_POS_BELOW: [i32; 3] = [0, -1, 1];
        match self {
            RailShape::NorthSouth => (Z_NEG, Z_POS),
            RailShape::EastWest => (X_NEG, X_POS),
            RailShape::AscendingEast => (X_NEG_BELOW, X_POS),
            RailShape::AscendingWest => (X_NEG, X_POS_BELOW),
            RailShape::AscendingNorth => (Z_NEG, Z_POS_BELOW),
            RailShape::AscendingSouth => (Z_NEG_BELOW, Z_POS),
            RailShape::SouthEast => (Z_POS, X_POS),
            RailShape::SouthWest => (Z_POS, X_NEG),
            RailShape::NorthWest => (Z_NEG, X_NEG),
            RailShape::NorthEast => (Z_NEG, X_POS),
        }
    }
}

pub(crate) fn shape_of(state: BlockState) -> Option<RailShape> {
    match BlockKind::from(state) {
        BlockKind::Rail => Some(match state.property::<AnyShape>()? {
            AnyShape::NorthSouth => RailShape::NorthSouth,
            AnyShape::EastWest => RailShape::EastWest,
            AnyShape::AscendingEast => RailShape::AscendingEast,
            AnyShape::AscendingWest => RailShape::AscendingWest,
            AnyShape::AscendingNorth => RailShape::AscendingNorth,
            AnyShape::AscendingSouth => RailShape::AscendingSouth,
            AnyShape::SouthEast => RailShape::SouthEast,
            AnyShape::SouthWest => RailShape::SouthWest,
            AnyShape::NorthWest => RailShape::NorthWest,
            AnyShape::NorthEast => RailShape::NorthEast,
        }),
        BlockKind::PoweredRail | BlockKind::DetectorRail | BlockKind::ActivatorRail => {
            Some(match state.property::<StraightShape>()? {
                StraightShape::NorthSouth => RailShape::NorthSouth,
                StraightShape::EastWest => RailShape::EastWest,
                StraightShape::AscendingEast => RailShape::AscendingEast,
                StraightShape::AscendingWest => RailShape::AscendingWest,
                StraightShape::AscendingNorth => RailShape::AscendingNorth,
                StraightShape::AscendingSouth => RailShape::AscendingSouth,
            })
        }
        _ => None,
    }
}

fn floor(v: f64) -> i32 {
    v.floor() as i32
}

fn rail_under(
    rail_at: &impl Fn([i32; 3]) -> Option<RailShape>,
    xt: i32,
    yt: i32,
    zt: i32,
) -> Option<(RailShape, i32)> {
    let yt = if rail_at([xt, yt - 1, zt]).is_some() {
        yt - 1
    } else {
        yt
    };
    rail_at([xt, yt, zt]).map(|shape| (shape, yt))
}

pub(crate) fn pos(
    rail_at: &impl Fn([i32; 3]) -> Option<RailShape>,
    x: f64,
    y: f64,
    z: f64,
) -> Option<[f64; 3]> {
    let (xt, zt) = (floor(x), floor(z));
    let (shape, yt) = rail_under(rail_at, xt, floor(y), zt)?;
    let (exit0, exit1) = shape.exits();
    let end = |exit: [i32; 3]| {
        [
            f64::from(xt) + 0.5 + f64::from(exit[0]) * 0.5,
            f64::from(yt) + 0.0625 + f64::from(exit[1]) * 0.5,
            f64::from(zt) + 0.5 + f64::from(exit[2]) * 0.5,
        ]
    };
    let ([x0, y0, z0], [x1, y1, z1]) = (end(exit0), end(exit1));
    let xd = x1 - x0;
    let yd = (y1 - y0) * 2.0;
    let zd = z1 - z0;
    let progress = if xd == 0.0 {
        z - f64::from(zt)
    } else if zd == 0.0 {
        x - f64::from(xt)
    } else {
        ((x - x0) * xd + (z - z0) * zd) * 2.0
    };
    let mut out_y = y0 + yd * progress;
    if yd < 0.0 {
        out_y += 1.0;
    } else if yd > 0.0 {
        out_y += 0.5;
    }
    Some([x0 + xd * progress, out_y, z0 + zd * progress])
}

pub(crate) fn pos_offs(
    rail_at: &impl Fn([i32; 3]) -> Option<RailShape>,
    x: f64,
    y: f64,
    z: f64,
    offs: f64,
) -> Option<[f64; 3]> {
    let (xt, zt) = (floor(x), floor(z));
    let (shape, yt) = rail_under(rail_at, xt, floor(y), zt)?;
    let mut y = f64::from(if shape.is_slope() { yt + 1 } else { yt });
    let (exit0, exit1) = shape.exits();
    let xd = f64::from(exit1[0] - exit0[0]);
    let zd = f64::from(exit1[2] - exit0[2]);
    let length = (xd * xd + zd * zd).sqrt();
    let x = x + xd / length * offs;
    let z = z + zd / length * offs;
    let left_by =
        |exit: [i32; 3]| exit[1] != 0 && floor(x) - xt == exit[0] && floor(z) - zt == exit[2];
    if left_by(exit0) {
        y += f64::from(exit0[1]);
    } else if left_by(exit1) {
        y += f64::from(exit1[1]);
    }
    pos(rail_at, x, y, z)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(rails: &'static [([i32; 3], RailShape)]) -> impl Fn([i32; 3]) -> Option<RailShape> {
        move |at| rails.iter().find(|(p, _)| *p == at).map(|(_, s)| *s)
    }

    #[test]
    fn shape_bytes_round_trip() {
        for v in 1..=10u8 {
            assert_eq!(RailShape::from_u8(v).map(|s| s as u8), Some(v));
        }
        assert_eq!(RailShape::from_u8(0), None);
        assert_eq!(RailShape::from_u8(11), None);
    }

    #[test]
    fn flat_rail_centres_the_cart() {
        let rails = track(&[([0, 64, 0], RailShape::EastWest)]);
        let p = pos(&rails, 0.25, 64.0625, 0.9).unwrap();
        assert_eq!(p, [0.25, 64.0625, 0.5]);
        assert_eq!(pos(&rails, 0.25, 66.5, 0.9), None);
    }

    #[test]
    fn rail_below_is_found() {
        let rails = track(&[([0, 64, 0], RailShape::NorthSouth)]);
        let p = pos(&rails, 0.5, 65.2, 0.25).unwrap();
        assert_eq!(p, [0.5, 64.0625, 0.25]);
    }

    #[test]
    fn slope_rises_toward_its_high_end() {
        let rails = track(&[([0, 64, 0], RailShape::AscendingEast)]);
        let west = pos(&rails, 0.0, 64.5, 0.5).unwrap();
        let east = pos(&rails, 1.0 - 1.0e-9, 64.5, 0.5).unwrap();
        assert!((west[1] - 64.0625).abs() < 1.0e-6, "{west:?}");
        assert!((east[1] - 65.0625).abs() < 1.0e-6, "{east:?}");
    }

    #[test]
    fn corner_projects_onto_the_diagonal() {
        let rails = track(&[([0, 64, 0], RailShape::SouthEast)]);
        let p = pos(&rails, 0.75, 64.0625, 0.75).unwrap();
        assert!(
            (p[0] - 0.75).abs() < 1.0e-9 && (p[2] - 0.75).abs() < 1.0e-9,
            "{p:?}"
        );
    }

    #[test]
    fn offset_steps_onto_the_next_rail() {
        let rails = track(&[
            ([0, 64, 0], RailShape::EastWest),
            ([1, 64, 0], RailShape::AscendingEast),
        ]);
        let ahead = pos_offs(&rails, 0.9, 64.0625, 0.5, 0.3).unwrap();
        assert!((ahead[0] - 1.2).abs() < 1.0e-9, "{ahead:?}");
        assert!((ahead[1] - (64.0625 + 0.2)).abs() < 1.0e-6, "{ahead:?}");
        assert_eq!(pos_offs(&rails, 0.1, 64.0625, 0.5, -0.3), None);
    }
}
