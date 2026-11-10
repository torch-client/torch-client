#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum Kind {
    Coal,
    Iron,
    Gold,
    Redstone,
    Diamond,
    Lapis,
    Copper,
    Emerald,
    Quartz,
    Debris,
}

impl Kind {
    pub const ALL: [Kind; 10] = [
        Kind::Coal,
        Kind::Iron,
        Kind::Gold,
        Kind::Redstone,
        Kind::Diamond,
        Kind::Lapis,
        Kind::Copper,
        Kind::Emerald,
        Kind::Quartz,
        Kind::Debris,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Kind::Coal => "Coal",
            Kind::Iron => "Iron",
            Kind::Gold => "Gold",
            Kind::Redstone => "Redstone",
            Kind::Diamond => "Diamond",
            Kind::Lapis => "Lapis",
            Kind::Copper => "Copper",
            Kind::Emerald => "Emerald",
            Kind::Quartz => "Quartz",
            Kind::Debris => "Ancient Debris",
        }
    }

    pub const fn block(self) -> &'static str {
        match self {
            Kind::Coal => "coal_ore",
            Kind::Iron => "iron_ore",
            Kind::Gold => "gold_ore",
            Kind::Redstone => "redstone_ore",
            Kind::Diamond => "diamond_ore",
            Kind::Lapis => "lapis_ore",
            Kind::Copper => "copper_ore",
            Kind::Emerald => "emerald_ore",
            Kind::Quartz => "nether_quartz_ore",
            Kind::Debris => "ancient_debris",
        }
    }
}

#[derive(Clone, Copy)]
pub enum Count {
    Once,
    Fixed(i32),
    Uniform(i32, i32),
    Rarity(i32),
}

#[derive(Clone, Copy)]
pub enum Height {
    Uniform(i32, i32),
    Trapezoid(i32, i32),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Overworld,
    Netherrack,
    NetherBase,
}

#[derive(Clone, Copy)]
pub enum Biomes {
    All,
    Only(&'static [&'static str]),
    Except(&'static [&'static str]),
}

pub struct Row {
    #[cfg_attr(not(test), allow(dead_code))]
    pub name: &'static str,
    pub kind: Kind,
    pub step: i32,
    pub index: i32,
    pub scattered: bool,
    pub size: i32,
    pub discard: f32,
    pub count: Count,
    pub height: Height,
    pub target: Target,
    pub biomes: Biomes,
}

const ORES_STEP: i32 = 6;
const NETHER_STEP: i32 = 7;

const fn ore(
    name: &'static str,
    kind: Kind,
    index: i32,
    size: i32,
    discard: f32,
    count: Count,
    height: Height,
) -> Row {
    Row {
        name,
        kind,
        step: ORES_STEP,
        index,
        scattered: false,
        size,
        discard,
        count,
        height,
        target: Target::Overworld,
        biomes: Biomes::All,
    }
}

const fn nether(
    name: &'static str,
    kind: Kind,
    index: i32,
    size: i32,
    count: Count,
    biomes: Biomes,
) -> Row {
    Row {
        name,
        kind,
        step: NETHER_STEP,
        index,
        scattered: false,
        size,
        discard: 0.0,
        count,
        height: Height::Uniform(10, 117),
        target: Target::Netherrack,
        biomes,
    }
}

const fn debris(name: &'static str, index: i32, size: i32, height: Height) -> Row {
    Row {
        name,
        kind: Kind::Debris,
        step: NETHER_STEP,
        index,
        scattered: true,
        size,
        discard: 1.0,
        count: Count::Once,
        height,
        target: Target::NetherBase,
        biomes: Biomes::All,
    }
}

const fn only(row: Row, biomes: &'static [&'static str]) -> Row {
    Row {
        biomes: Biomes::Only(biomes),
        ..row
    }
}

const DELTAS: &[&str] = &["basalt_deltas"];
const DRIPSTONE: &[&str] = &["dripstone_caves"];
const BADLANDS: &[&str] = &["badlands", "eroded_badlands", "wooded_badlands"];
const MOUNTAINS: &[&str] = &[
    "cherry_grove",
    "frozen_peaks",
    "grove",
    "jagged_peaks",
    "meadow",
    "snowy_slopes",
    "stony_peaks",
    "windswept_forest",
    "windswept_gravelly_hills",
    "windswept_hills",
];

pub static OVERWORLD: [Row; 19] = {
    use Count::{Fixed, Rarity, Uniform as Between};
    use Height::{Trapezoid, Uniform};
    use Kind::*;
    [
        ore(
            "ore_coal_upper",
            Coal,
            9,
            17,
            0.0,
            Fixed(30),
            Uniform(136, 319),
        ),
        ore(
            "ore_coal_lower",
            Coal,
            10,
            17,
            0.5,
            Fixed(20),
            Trapezoid(0, 192),
        ),
        ore(
            "ore_iron_upper",
            Iron,
            11,
            9,
            0.0,
            Fixed(90),
            Trapezoid(80, 384),
        ),
        ore(
            "ore_iron_middle",
            Iron,
            12,
            9,
            0.0,
            Fixed(10),
            Trapezoid(-24, 56),
        ),
        ore(
            "ore_iron_small",
            Iron,
            13,
            4,
            0.0,
            Fixed(10),
            Uniform(-64, 72),
        ),
        ore("ore_gold", Gold, 14, 9, 0.5, Fixed(4), Trapezoid(-64, 32)),
        ore(
            "ore_gold_lower",
            Gold,
            15,
            9,
            0.5,
            Between(0, 1),
            Uniform(-64, -48),
        ),
        ore(
            "ore_redstone",
            Redstone,
            16,
            8,
            0.0,
            Fixed(4),
            Uniform(-64, 15),
        ),
        ore(
            "ore_redstone_lower",
            Redstone,
            17,
            8,
            0.0,
            Fixed(8),
            Trapezoid(-96, -32),
        ),
        ore(
            "ore_diamond",
            Diamond,
            18,
            4,
            0.5,
            Fixed(7),
            Trapezoid(-144, 16),
        ),
        ore(
            "ore_diamond_medium",
            Diamond,
            19,
            8,
            0.5,
            Fixed(2),
            Uniform(-64, -4),
        ),
        ore(
            "ore_diamond_large",
            Diamond,
            20,
            12,
            0.7,
            Rarity(9),
            Trapezoid(-144, 16),
        ),
        ore(
            "ore_diamond_buried",
            Diamond,
            21,
            8,
            1.0,
            Fixed(4),
            Trapezoid(-144, 16),
        ),
        ore("ore_lapis", Lapis, 22, 7, 0.0, Fixed(2), Trapezoid(-32, 32)),
        ore(
            "ore_lapis_buried",
            Lapis,
            23,
            7,
            1.0,
            Fixed(4),
            Uniform(-64, 64),
        ),
        only(
            ore(
                "ore_copper_large",
                Copper,
                24,
                20,
                0.0,
                Fixed(16),
                Trapezoid(-16, 112),
            ),
            DRIPSTONE,
        ),
        Row {
            biomes: Biomes::Except(DRIPSTONE),
            ..ore(
                "ore_copper",
                Copper,
                25,
                10,
                0.0,
                Fixed(16),
                Trapezoid(-16, 112),
            )
        },
        only(
            ore(
                "ore_gold_extra",
                Gold,
                28,
                9,
                0.0,
                Fixed(50),
                Uniform(32, 256),
            ),
            BADLANDS,
        ),
        only(
            ore(
                "ore_emerald",
                Emerald,
                33,
                3,
                0.0,
                Fixed(100),
                Trapezoid(-16, 480),
            ),
            MOUNTAINS,
        ),
    ]
};

pub static NETHER: [Row; 6] = {
    use Count::Fixed;
    use Height::{Trapezoid, Uniform};
    use Kind::*;
    [
        nether(
            "ore_gold_deltas",
            Gold,
            13,
            10,
            Fixed(20),
            Biomes::Only(DELTAS),
        ),
        nether(
            "ore_quartz_deltas",
            Quartz,
            14,
            14,
            Fixed(32),
            Biomes::Only(DELTAS),
        ),
        nether(
            "ore_gold_nether",
            Gold,
            19,
            10,
            Fixed(10),
            Biomes::Except(DELTAS),
        ),
        nether(
            "ore_quartz_nether",
            Quartz,
            20,
            14,
            Fixed(16),
            Biomes::Except(DELTAS),
        ),
        debris("ore_ancient_debris_large", 21, 3, Trapezoid(8, 24)),
        debris("ore_debris_small", 22, 2, Uniform(8, 119)),
    ]
};
