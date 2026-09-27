use azalea::block::{BlockState, BlockTrait};
use parking_lot::RwLock;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Flame {
    Flame,
    Soul,
    Copper,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Furnace {
    Furnace,
    Blast,
    Smoker,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Leaf {
    BiomeTinted,
    AzaleaTint,
    Cherry,
    PaleOak,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WireSide {
    None,
    Side,
    Up,
}

#[derive(Clone, Copy)]
pub enum Ambient {
    Torch { flame: Flame, offset: [f32; 3] },
    Candle { wicks: [[f32; 3]; 4], count: u8 },
    Fire,
    Campfire,
    Furnace { kind: Furnace, front: [f32; 3] },
    BrewingStand,
    WitherRose,
    EndPortal,
    NetherPortal { axis_x: bool },
    EnderChest,
    RespawnAnchor,
    EnchantingTable,
    EndRod { step: [f32; 3] },
    Mycelium,
    Leaves { leaf: Leaf, chance: f32 },
    WetSponge,
    Beehive,
    CryingObsidian,
    PointedDripstone,
    SporeBlossom,
    BubbleColumn { drag: bool },
    DriedGhast { waterlogged: bool },
    SculkSensor,
    RedstoneWire { power: u8, sides: [WireSide; 4] },
    RedstoneTorch,
    RedstoneOre,
    Repeater { step: [f32; 3], delay: u8 },
    Lever { offset: [f32; 3] },
    FallingDust,
    FireflyBush,
}

pub const HORIZONTAL: [[f32; 3]; 4] = [
    [0.0, 0.0, -1.0],
    [0.0, 0.0, 1.0],
    [-1.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
];

const CANDLE_OFFSETS: [[[f32; 3]; 4]; 4] = [
    [
        [8.0, 8.0, 8.0],
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
    ],
    [
        [6.0, 7.0, 8.0],
        [10.0, 8.0, 7.0],
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
    ],
    [
        [8.0, 5.0, 10.0],
        [6.0, 7.0, 8.0],
        [9.0, 8.0, 7.0],
        [0.0, 0.0, 0.0],
    ],
    [
        [7.0, 5.0, 9.0],
        [10.0, 7.0, 9.0],
        [6.0, 7.0, 6.0],
        [9.0, 8.0, 6.0],
    ],
];

static CACHE: RwLock<Vec<Option<Option<Ambient>>>> = RwLock::new(Vec::new());

pub fn of(state: BlockState) -> Option<Ambient> {
    let id = state.id() as usize;
    {
        let guard = CACHE.read();
        if let Some(Some(hit)) = guard.get(id) {
            return *hit;
        }
    }
    let built = build(state);
    let mut guard = CACHE.write();
    if guard.len() <= id {
        guard.resize(id + 1, None);
    }
    guard[id] = Some(built);
    built
}

fn build(state: BlockState) -> Option<Ambient> {
    const PREFIX: usize = "minecraft:".len();
    let name = &state.as_block_kind().to_str()[PREFIX..];

    let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
    let prop = |n: &str| block.get_property(n);
    let flag = |n: &str| prop(n) == Some("true");
    let int = |n: &str| prop(n).and_then(|v| v.parse::<u8>().ok()).unwrap_or(0);

    if let Some(flame) = torch_flame(name) {
        let offset = if name.ends_with("wall_torch") {
            let step = facing_step(prop("facing").unwrap_or("north"));
            [0.5 - 0.27 * step[0], 0.7 + 0.22, 0.5 - 0.27 * step[2]]
        } else {
            [0.5, 0.7, 0.5]
        };
        return Some(Ambient::Torch { flame, offset });
    }

    if name == "redstone_torch" || name == "redstone_wall_torch" {
        return flag("lit").then_some(Ambient::RedstoneTorch);
    }

    if name == "candle" || name.ends_with("_candle") {
        if !flag("lit") {
            return None;
        }
        let count = int("candles").clamp(1, 4);
        let mut wicks = CANDLE_OFFSETS[count as usize - 1];
        for w in wicks.iter_mut() {
            *w = [w[0] / 16.0, w[1] / 16.0, w[2] / 16.0];
        }
        return Some(Ambient::Candle { wicks, count });
    }
    if name == "candle_cake" || name.ends_with("_candle_cake") {
        if !flag("lit") {
            return None;
        }
        return Some(Ambient::Candle {
            wicks: [[0.5, 1.0, 0.5]; 4],
            count: 1,
        });
    }

    match name {
        "fire" | "soul_fire" => return Some(Ambient::Fire),
        "furnace" | "blast_furnace" | "smoker" => {
            if !flag("lit") {
                return None;
            }
            let kind = match name {
                "furnace" => Furnace::Furnace,
                "blast_furnace" => Furnace::Blast,
                _ => Furnace::Smoker,
            };
            let front = facing_step(prop("facing").unwrap_or("north"));
            return Some(Ambient::Furnace { kind, front });
        }
        "campfire" | "soul_campfire" => {
            return flag("lit").then_some(Ambient::Campfire);
        }
        "brewing_stand" => return Some(Ambient::BrewingStand),
        "wither_rose" => return Some(Ambient::WitherRose),
        "end_portal" => return Some(Ambient::EndPortal),
        "nether_portal" => {
            return Some(Ambient::NetherPortal {
                axis_x: prop("axis") == Some("x"),
            });
        }
        "ender_chest" => return Some(Ambient::EnderChest),
        "respawn_anchor" => {
            return (int("charge") != 0).then_some(Ambient::RespawnAnchor);
        }
        "enchanting_table" => return Some(Ambient::EnchantingTable),
        "end_rod" => {
            return Some(Ambient::EndRod {
                step: facing_step(prop("facing").unwrap_or("up")),
            });
        }
        "mycelium" => return Some(Ambient::Mycelium),
        "wet_sponge" => return Some(Ambient::WetSponge),
        "beehive" | "bee_nest" => {
            return (int("honey_level") >= 5).then_some(Ambient::Beehive);
        }
        "crying_obsidian" => return Some(Ambient::CryingObsidian),
        "pointed_dripstone" => {
            let tip = prop("vertical_direction") == Some("down")
                && prop("thickness") == Some("tip")
                && !flag("waterlogged");
            return tip.then_some(Ambient::PointedDripstone);
        }
        "spore_blossom" => return Some(Ambient::SporeBlossom),
        "bubble_column" => {
            return Some(Ambient::BubbleColumn { drag: flag("drag") });
        }
        "dried_ghast" => {
            return Some(Ambient::DriedGhast {
                waterlogged: flag("waterlogged"),
            });
        }
        "sculk_sensor" | "calibrated_sculk_sensor" => {
            return (prop("sculk_sensor_phase") == Some("active")).then_some(Ambient::SculkSensor);
        }
        "redstone_wire" => {
            let power = int("power");
            if power == 0 {
                return None;
            }
            let side = |n: &str| match prop(n) {
                Some("up") => WireSide::Up,
                Some("side") => WireSide::Side,
                _ => WireSide::None,
            };
            return Some(Ambient::RedstoneWire {
                power,
                sides: [side("north"), side("south"), side("west"), side("east")],
            });
        }
        "redstone_ore" | "deepslate_redstone_ore" => {
            return flag("lit").then_some(Ambient::RedstoneOre);
        }
        "repeater" => {
            if !flag("powered") {
                return None;
            }
            return Some(Ambient::Repeater {
                step: facing_step(prop("facing").unwrap_or("north")),
                delay: int("delay").max(1),
            });
        }
        "lever" => {
            if !flag("powered") {
                return None;
            }
            let facing = facing_step(prop("facing").unwrap_or("north"));
            let attach = match prop("face") {
                Some("ceiling") => [0.0, 1.0, 0.0],
                Some("floor") => [0.0, -1.0, 0.0],
                _ => facing,
            };
            return Some(Ambient::Lever {
                offset: [
                    0.5 - 0.1 * facing[0] - 0.2 * attach[0],
                    0.5 - 0.1 * facing[1] - 0.2 * attach[1],
                    0.5 - 0.1 * facing[2] - 0.2 * attach[2],
                ],
            });
        }
        "firefly_bush" => return Some(Ambient::FireflyBush),
        _ => {}
    }

    if let Some((leaf, chance)) = leaf_particle(name) {
        return Some(Ambient::Leaves { leaf, chance });
    }
    if is_falling(name) {
        return Some(Ambient::FallingDust);
    }
    None
}

fn torch_flame(name: &str) -> Option<Flame> {
    match name {
        "torch" | "wall_torch" => Some(Flame::Flame),
        "soul_torch" | "soul_wall_torch" => Some(Flame::Soul),
        "copper_torch" | "copper_wall_torch" => Some(Flame::Copper),
        _ => None,
    }
}

fn leaf_particle(name: &str) -> Option<(Leaf, f32)> {
    match name {
        "cherry_leaves" => Some((Leaf::Cherry, 0.1)),
        "pale_oak_leaves" => Some((Leaf::PaleOak, 0.02)),
        "azalea_leaves" | "flowering_azalea_leaves" => Some((Leaf::AzaleaTint, 0.01)),
        _ if name.ends_with("_leaves") => Some((Leaf::BiomeTinted, 0.01)),
        _ => None,
    }
}

fn is_falling(name: &str) -> bool {
    matches!(
        name,
        "sand"
            | "red_sand"
            | "gravel"
            | "suspicious_sand"
            | "suspicious_gravel"
            | "anvil"
            | "chipped_anvil"
            | "damaged_anvil"
            | "dragon_egg"
    ) || name.ends_with("_concrete_powder")
}

fn facing_step(value: &str) -> [f32; 3] {
    match value {
        "north" => [0.0, 0.0, -1.0],
        "south" => [0.0, 0.0, 1.0],
        "west" => [-1.0, 0.0, 0.0],
        "east" => [1.0, 0.0, 0.0],
        "down" => [0.0, -1.0, 0.0],
        _ => [0.0, 1.0, 0.0],
    }
}
