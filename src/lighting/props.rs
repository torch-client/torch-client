use std::collections::HashMap;

use azalea::block::{BlockState, BlockTrait};
use parking_lot::RwLock;

use super::Direction;

pub type FaceMask = [u16; 16];

const EMPTY_FACE: FaceMask = [0; 16];
#[cfg(test)]
const FULL_FACE: FaceMask = [0xFFFF; 16];

#[derive(Clone, Copy, Debug)]
struct Box16 {
    x0: i32,
    y0: i32,
    z0: i32,
    x1: i32,
    y1: i32,
    z1: i32,
}

const fn b(x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32) -> Box16 {
    Box16 {
        x0,
        y0,
        z0,
        x1,
        y1,
        z1,
    }
}

impl Box16 {
    const fn rot_y_90(self) -> Self {
        Self {
            x0: 16 - self.z1,
            z0: self.x0,
            x1: 16 - self.z0,
            z1: self.x1,
            ..self
        }
    }

    const fn invert_y(self) -> Self {
        Self {
            y0: 16 - self.y1,
            y1: 16 - self.y0,
            ..self
        }
    }

    fn face(self, dir: Direction) -> FaceMask {
        let (u0, u1, v0, v1) = match dir {
            Direction::Down if self.y0 == 0 => (self.x0, self.x1, self.z0, self.z1),
            Direction::Up if self.y1 == 16 => (self.x0, self.x1, self.z0, self.z1),
            Direction::North if self.z0 == 0 => (self.x0, self.x1, self.y0, self.y1),
            Direction::South if self.z1 == 16 => (self.x0, self.x1, self.y0, self.y1),
            Direction::West if self.x0 == 0 => (self.z0, self.z1, self.y0, self.y1),
            Direction::East if self.x1 == 16 => (self.z0, self.z1, self.y0, self.y1),
            _ => return EMPTY_FACE,
        };
        let row = ((1u32 << u1) - (1u32 << u0)) as u16;
        let mut mask = EMPTY_FACE;
        for (v, cell) in mask.iter_mut().enumerate() {
            if (v as i32) >= v0 && (v as i32) < v1 {
                *cell = row;
            }
        }
        mask
    }
}

#[inline]
fn covers(a: &FaceMask, b: &FaceMask) -> bool {
    a.iter().zip(b.iter()).all(|(p, q)| p | q == 0xFFFF)
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct LightProps {
    pub emission: u8,
    pub dampening: u8,
    pub empty_shape: bool,
    faces: [FaceMask; 6],
}

impl LightProps {
    #[inline]
    pub fn opacity(&self) -> u8 {
        self.dampening.max(1)
    }
}

pub fn shape_occludes(from: &LightProps, to: &LightProps, dir: Direction) -> bool {
    if from.empty_shape && to.empty_shape {
        return false;
    }
    let a = if from.empty_shape {
        &EMPTY_FACE
    } else {
        &from.faces[dir.index()]
    };
    let b = if to.empty_shape {
        &EMPTY_FACE
    } else {
        &to.faces[dir.opposite().index()]
    };
    covers(a, b)
}

pub fn edge_occluded(top: &LightProps, bottom: &LightProps) -> bool {
    if bottom.dampening != 0 {
        return true;
    }
    shape_occludes(top, bottom, Direction::Down)
}

static CACHE: RwLock<Option<Interner>> = RwLock::new(None);

#[derive(Default)]
struct Interner {
    by_state: Vec<Option<&'static LightProps>>,
    unique: HashMap<LightProps, &'static LightProps>,
}

pub fn air() -> &'static LightProps {
    static AIR: std::sync::OnceLock<LightProps> = std::sync::OnceLock::new();
    AIR.get_or_init(|| LightProps {
        emission: 0,
        dampening: 0,
        empty_shape: true,
        faces: [EMPTY_FACE; 6],
    })
}

pub fn of(state: BlockState) -> &'static LightProps {
    let id = state.id() as usize;
    {
        let guard = CACHE.read();
        if let Some(interner) = guard.as_ref()
            && let Some(Some(hit)) = interner.by_state.get(id)
        {
            return hit;
        }
    }
    let built = build(state);
    let mut guard = CACHE.write();
    let interner = guard.get_or_insert_with(Interner::default);
    if interner.by_state.len() <= id {
        interner.by_state.resize(id + 1, None);
    }
    if let Some(hit) = interner.by_state[id] {
        return hit;
    }
    let interned = *interner
        .unique
        .entry(built.clone())
        .or_insert_with(|| &*Box::leak(Box::new(built)));
    interner.by_state[id] = Some(interned);
    interned
}

fn build(state: BlockState) -> LightProps {
    let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
    let name = block.id();
    let p = block.property_map();

    let emission = emission(name, &p);
    let shape = occlusion_shape(name, &p);
    let mut faces = [EMPTY_FACE; 6];
    if let Some(boxes) = &shape {
        for dir in super::Direction::ALL {
            let mut mask = EMPTY_FACE;
            for bx in boxes {
                let f = bx.face(dir);
                for (m, v) in mask.iter_mut().zip(f.iter()) {
                    *m |= v;
                }
            }
            faces[dir.index()] = mask;
        }
    }

    LightProps {
        emission,
        dampening: dampening(state, name, &p),
        empty_shape: shape.is_none(),
        faces,
    }
}

fn dampening(state: BlockState, name: &str, p: &HashMap<&str, &str>) -> u8 {
    if name == "tinted_glass" {
        return 15;
    }
    if name.ends_with("_leaves") {
        return 1;
    }
    if matches!(name, "water" | "lava" | "bubble_column") {
        return 1;
    }

    let baked = crate::blocks::registry().baked(state.id(), name, p);
    if baked.is_solid {
        return 15;
    }
    if propagates_skylight_down(name, p, baked.full_cube) {
        0
    } else {
        1
    }
}

fn propagates_skylight_down(name: &str, p: &HashMap<&str, &str>, full_cube: bool) -> bool {
    if name == "glass" || name.ends_with("_stained_glass") || name.ends_with("copper_grate") {
        return true;
    }
    if name == "chorus_plant" {
        return false;
    }
    let waterlogged = p.get("waterlogged").is_some_and(|v| *v == "true");
    !full_cube && !waterlogged
}

fn int_prop(p: &HashMap<&str, &str>, key: &str) -> i32 {
    p.get(key).and_then(|v| v.parse().ok()).unwrap_or(0)
}

fn bool_prop(p: &HashMap<&str, &str>, key: &str) -> bool {
    p.get(key).is_some_and(|v| *v == "true")
}

fn lit(p: &HashMap<&str, &str>, level: u8) -> u8 {
    if bool_prop(p, "lit") { level } else { 0 }
}

fn emission(name: &str, p: &HashMap<&str, &str>) -> u8 {
    if name == "candle" || name.ends_with("_candle") {
        return if bool_prop(p, "lit") {
            3 * int_prop(p, "candles").max(1) as u8
        } else {
            0
        };
    }
    if name == "candle_cake" || name.ends_with("_candle_cake") {
        return lit(p, 3);
    }
    match name {
        "lava"
        | "lava_cauldron"
        | "fire"
        | "glowstone"
        | "jack_o_lantern"
        | "end_portal"
        | "end_gateway"
        | "sea_lantern"
        | "beacon"
        | "conduit"
        | "lantern"
        | "shroomlight"
        | "ochre_froglight"
        | "verdant_froglight"
        | "pearlescent_froglight" => 15,
        "copper_lantern"
        | "exposed_copper_lantern"
        | "weathered_copper_lantern"
        | "oxidized_copper_lantern"
        | "waxed_copper_lantern"
        | "waxed_exposed_copper_lantern"
        | "waxed_weathered_copper_lantern"
        | "waxed_oxidized_copper_lantern" => 15,
        "torch" | "wall_torch" | "copper_torch" | "copper_wall_torch" | "end_rod" => 14,
        "nether_portal" => 11,
        "soul_fire" | "soul_torch" | "soul_wall_torch" | "soul_lantern" | "crying_obsidian" => 10,
        "enchanting_table" | "ender_chest" => 7,
        "sculk_catalyst" => 6,
        "amethyst_cluster" => 5,
        "large_amethyst_bud" => 4,
        "magma_block" => 3,
        "firefly_bush" => 2,
        "medium_amethyst_bud" => 2,
        "brown_mushroom"
        | "brewing_stand"
        | "end_portal_frame"
        | "dragon_egg"
        | "small_amethyst_bud"
        | "sculk_sensor"
        | "calibrated_sculk_sensor" => 1,

        "furnace" | "smoker" | "blast_furnace" => lit(p, 13),
        "redstone_ore" | "deepslate_redstone_ore" => lit(p, 9),
        "redstone_torch" | "redstone_wall_torch" => lit(p, 7),
        "redstone_lamp" | "campfire" | "copper_bulb" | "waxed_copper_bulb" => lit(p, 15),
        "soul_campfire" => lit(p, 10),
        "exposed_copper_bulb" | "waxed_exposed_copper_bulb" => lit(p, 12),
        "weathered_copper_bulb" | "waxed_weathered_copper_bulb" => lit(p, 8),
        "oxidized_copper_bulb" | "waxed_oxidized_copper_bulb" => lit(p, 4),

        "light" => int_prop(p, "level") as u8,
        "glow_lichen" => {
            let attached = ["north", "east", "south", "west", "up", "down"]
                .iter()
                .any(|f| bool_prop(p, f));
            if attached { 7 } else { 0 }
        }
        "cave_vines" | "cave_vines_plant" => {
            if bool_prop(p, "berries") {
                14
            } else {
                0
            }
        }
        "sea_pickle" => {
            if bool_prop(p, "waterlogged") {
                (3 + 3 * int_prop(p, "pickles").max(1)) as u8
            } else {
                0
            }
        }
        "respawn_anchor" => (int_prop(p, "charge") * 15 / 4) as u8,
        "trial_spawner" => match p.get("trial_spawner_state").copied().unwrap_or("inactive") {
            "waiting_for_players" => 4,
            "active" | "waiting_for_reward_ejection" | "ejecting_reward" => 8,
            _ => 0,
        },
        "vault" => match p.get("vault_state").copied().unwrap_or("inactive") {
            "active" | "unlocking" | "ejecting" => 12,
            _ => 6,
        },
        _ => 0,
    }
}

fn occlusion_shape(name: &str, p: &HashMap<&str, &str>) -> Option<Vec<Box16>> {
    if name.ends_with("_slab") {
        return match p.get("type").copied().unwrap_or("bottom") {
            "bottom" => Some(vec![b(0, 0, 0, 16, 8, 16)]),
            "top" => Some(vec![b(0, 8, 0, 16, 16, 16)]),
            _ => None,
        };
    }
    if name.ends_with("_stairs") {
        return Some(stair_shape(p));
    }
    if name.ends_with("_shelf") {
        let north = [
            b(0, 12, 11, 16, 16, 13),
            b(0, 0, 13, 16, 16, 16),
            b(0, 0, 11, 16, 4, 13),
        ];
        return Some(rotate_to_facing(&north, p));
    }
    Some(match name {
        "snow" => vec![b(0, 0, 0, 16, 2 * int_prop(p, "layers").max(1), 16)],
        "sculk_sensor" | "calibrated_sculk_sensor" | "sculk_shrieker" => {
            vec![b(0, 0, 0, 16, 8, 16)]
        }
        "stonecutter" => vec![b(0, 0, 0, 16, 9, 16)],
        "daylight_detector" => vec![b(0, 0, 0, 16, 6, 16)],
        "enchanting_table" => vec![b(0, 0, 0, 16, 12, 16)],
        "farmland" | "dirt_path" => vec![b(0, 0, 0, 16, 15, 16)],
        "end_portal_frame" => {
            let mut boxes = vec![b(0, 0, 0, 16, 13, 16)];
            if bool_prop(p, "eye") {
                boxes.push(b(4, 13, 4, 12, 16, 12));
            }
            boxes
        }
        "lectern" => vec![b(0, 0, 0, 16, 2, 16), b(4, 2, 4, 12, 14, 12)],
        "piston_head" => rotate_to_facing_any(&[b(0, 0, 0, 16, 16, 4), b(6, 6, 4, 10, 10, 16)], p),
        "piston" | "sticky_piston" => {
            if !bool_prop(p, "extended") {
                return None;
            }
            rotate_to_facing_any(&[b(0, 0, 4, 16, 16, 16)], p)
        }
        _ => return None,
    })
}

fn stair_shape(p: &HashMap<&str, &str>) -> Vec<Box16> {
    const OUTER: [Box16; 2] = [b(0, 0, 0, 16, 8, 16), b(0, 8, 0, 8, 16, 8)];
    let mut boxes: Vec<Box16> = OUTER.to_vec();
    let turns = match p.get("shape").copied().unwrap_or("straight") {
        "straight" => 1,
        "inner_left" | "inner_right" => 2,
        _ => 0,
    };
    for _ in 0..turns {
        let rotated: Vec<Box16> = boxes.iter().map(|bx| bx.rot_y_90()).collect();
        boxes.extend(rotated);
    }
    if p.get("half").copied().unwrap_or("bottom") == "top" {
        for bx in boxes.iter_mut() {
            *bx = bx.invert_y();
        }
    }
    let facing_turns = match p.get("shape").copied().unwrap_or("straight") {
        "inner_left" => 3,
        "outer_right" => 1,
        _ => 0,
    };
    rotate_quarter_turns(&boxes, facing_turns + facing_turns_of(p))
}

fn facing_turns_of(p: &HashMap<&str, &str>) -> u32 {
    match p.get("facing").copied().unwrap_or("north") {
        "east" => 1,
        "south" => 2,
        "west" => 3,
        _ => 0,
    }
}

fn rotate_quarter_turns(north: &[Box16], turns: u32) -> Vec<Box16> {
    let mut boxes = north.to_vec();
    for _ in 0..turns % 4 {
        for bx in boxes.iter_mut() {
            *bx = bx.rot_y_90();
        }
    }
    boxes
}

fn rotate_to_facing(north: &[Box16], p: &HashMap<&str, &str>) -> Vec<Box16> {
    rotate_quarter_turns(north, facing_turns_of(p))
}

fn rotate_to_facing_any(north: &[Box16], p: &HashMap<&str, &str>) -> Vec<Box16> {
    match p.get("facing").copied().unwrap_or("north") {
        "up" => north
            .iter()
            .map(|bx| Box16 {
                y0: 16 - bx.z1,
                y1: 16 - bx.z0,
                z0: bx.y0,
                z1: bx.y1,
                ..*bx
            })
            .collect(),
        "down" => north
            .iter()
            .map(|bx| Box16 {
                y0: bx.z0,
                y1: bx.z1,
                z0: bx.y0,
                z1: bx.y1,
                ..*bx
            })
            .collect(),
        _ => rotate_to_facing(north, p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bottom_slab_seals_its_underside_and_not_its_top() {
        let boxes = [b(0, 0, 0, 16, 8, 16)];
        assert_eq!(boxes[0].face(Direction::Down), FULL_FACE);
        assert_eq!(boxes[0].face(Direction::Up), EMPTY_FACE);
        let north = boxes[0].face(Direction::North);
        assert_eq!(north[0], 0xFFFF);
        assert_eq!(north[7], 0xFFFF);
        assert_eq!(north[8], 0);
    }

    #[test]
    fn a_bottom_slab_stops_light_leaving_downward() {
        let bottom = LightProps {
            emission: 0,
            dampening: 0,
            empty_shape: false,
            faces: faces_of(&[b(0, 0, 0, 16, 8, 16)]),
        };
        let air = air();
        assert!(shape_occludes(&bottom, air, Direction::Down));
        assert!(!shape_occludes(&bottom, air, Direction::Up));
    }

    #[test]
    fn a_top_slab_over_a_bottom_slab_leaves_the_seam_open() {
        let top = b(0, 8, 0, 16, 16, 16);
        let bottom = b(0, 0, 0, 16, 8, 16);
        assert!(!covers(
            &top.face(Direction::Down),
            &bottom.face(Direction::Up)
        ));
    }

    fn faces_of(boxes: &[Box16]) -> [FaceMask; 6] {
        let mut out = [EMPTY_FACE; 6];
        for dir in super::super::Direction::ALL {
            for bx in boxes {
                let f = bx.face(dir);
                for (m, v) in out[dir.index()].iter_mut().zip(f.iter()) {
                    *m |= v;
                }
            }
        }
        out
    }

    #[test]
    fn the_quarter_turn_matches_shapes_rotate_horizontal() {
        let step = b(0, 8, 0, 8, 16, 8);
        let turned = step.rot_y_90();
        assert_eq!((turned.x0, turned.x1), (8, 16));
        assert_eq!((turned.z0, turned.z1), (0, 8));
    }

    #[test]
    fn a_straight_stair_is_solid_on_the_side_it_faces() {
        let p = HashMap::from([
            ("shape", "straight"),
            ("half", "bottom"),
            ("facing", "north"),
        ]);
        let boxes = stair_shape(&p);
        let mut north = EMPTY_FACE;
        let mut south = EMPTY_FACE;
        for bx in &boxes {
            for (m, v) in north.iter_mut().zip(bx.face(Direction::North).iter()) {
                *m |= v;
            }
            for (m, v) in south.iter_mut().zip(bx.face(Direction::South).iter()) {
                *m |= v;
            }
        }
        assert_eq!(north, FULL_FACE);
        assert_eq!(south[0], 0xFFFF);
        assert_eq!(south[15], 0);
    }
}
