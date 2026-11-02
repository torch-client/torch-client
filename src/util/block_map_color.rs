const WHEAT_RIPE_AGE: u8 = 6;

const NONE: u8 = 0;
const GRASS: u8 = 1;
const SAND: u8 = 2;
const WOOL: u8 = 3;
const FIRE: u8 = 4;
const ICE: u8 = 5;
const METAL: u8 = 6;
const PLANT: u8 = 7;
const SNOW: u8 = 8;
const CLAY: u8 = 9;
const DIRT: u8 = 10;
const STONE: u8 = 11;
const WATER: u8 = 12;
const WOOD: u8 = 13;
const QUARTZ: u8 = 14;
const COLOR_ORANGE: u8 = 15;
const COLOR_MAGENTA: u8 = 16;
const COLOR_LIGHT_BLUE: u8 = 17;
const COLOR_YELLOW: u8 = 18;
const COLOR_LIGHT_GREEN: u8 = 19;
const COLOR_PINK: u8 = 20;
const COLOR_GRAY: u8 = 21;
const COLOR_LIGHT_GRAY: u8 = 22;
const COLOR_CYAN: u8 = 23;
const COLOR_PURPLE: u8 = 24;
const COLOR_BLUE: u8 = 25;
const COLOR_BROWN: u8 = 26;
const COLOR_GREEN: u8 = 27;
const COLOR_RED: u8 = 28;
const COLOR_BLACK: u8 = 29;
const GOLD: u8 = 30;
const DIAMOND: u8 = 31;
const LAPIS: u8 = 32;
const EMERALD: u8 = 33;
const PODZOL: u8 = 34;
const NETHER: u8 = 35;
const TERRACOTTA_WHITE: u8 = 36;
const TERRACOTTA_ORANGE: u8 = 37;
const TERRACOTTA_MAGENTA: u8 = 38;
const TERRACOTTA_LIGHT_BLUE: u8 = 39;
const TERRACOTTA_YELLOW: u8 = 40;
const TERRACOTTA_LIGHT_GREEN: u8 = 41;
const TERRACOTTA_PINK: u8 = 42;
const TERRACOTTA_GRAY: u8 = 43;
const TERRACOTTA_LIGHT_GRAY: u8 = 44;
const TERRACOTTA_CYAN: u8 = 45;
const TERRACOTTA_PURPLE: u8 = 46;
const TERRACOTTA_BLUE: u8 = 47;
const TERRACOTTA_BROWN: u8 = 48;
const TERRACOTTA_GREEN: u8 = 49;
const TERRACOTTA_RED: u8 = 50;
const TERRACOTTA_BLACK: u8 = 51;
const CRIMSON_NYLIUM: u8 = 52;
const CRIMSON_STEM: u8 = 53;
const CRIMSON_HYPHAE: u8 = 54;
const WARPED_NYLIUM: u8 = 55;
const WARPED_STEM: u8 = 56;
const WARPED_HYPHAE: u8 = 57;
const WARPED_WART_BLOCK: u8 = 58;
const DEEPSLATE: u8 = 59;
const RAW_IRON: u8 = 60;
const GLOW_LICHEN: u8 = 61;

pub fn material(name: &str, prop: impl Fn(&str) -> Option<&'static str>) -> u8 {
    match name {
        "acacia_log" => {
            if prop("axis") == Some("y") {
                COLOR_ORANGE
            } else {
                STONE
            }
        }
        "bamboo_block" => {
            if prop("axis") == Some("y") {
                COLOR_YELLOW
            } else {
                PLANT
            }
        }
        "birch_log" => {
            if prop("axis") == Some("y") {
                SAND
            } else {
                QUARTZ
            }
        }
        "cherry_log" => {
            if prop("axis") == Some("y") {
                TERRACOTTA_WHITE
            } else {
                TERRACOTTA_GRAY
            }
        }
        "jungle_log" => {
            if prop("axis") == Some("y") {
                DIRT
            } else {
                PODZOL
            }
        }
        "mangrove_log" => {
            if prop("axis") == Some("y") {
                COLOR_RED
            } else {
                PODZOL
            }
        }
        "oak_log" => {
            if prop("axis") == Some("y") {
                WOOD
            } else {
                PODZOL
            }
        }
        "pale_oak_log" => {
            if prop("axis") == Some("y") {
                QUARTZ
            } else {
                STONE
            }
        }
        "spruce_log" => {
            if prop("axis") == Some("y") {
                PODZOL
            } else {
                COLOR_BROWN
            }
        }
        "stripped_cherry_log" => {
            if prop("axis") == Some("y") {
                TERRACOTTA_WHITE
            } else {
                TERRACOTTA_PINK
            }
        }
        "black_bed" => {
            if prop("part") == Some("foot") {
                COLOR_BLACK
            } else {
                WOOL
            }
        }
        "blue_bed" => {
            if prop("part") == Some("foot") {
                COLOR_BLUE
            } else {
                WOOL
            }
        }
        "brown_bed" => {
            if prop("part") == Some("foot") {
                COLOR_BROWN
            } else {
                WOOL
            }
        }
        "cyan_bed" => {
            if prop("part") == Some("foot") {
                COLOR_CYAN
            } else {
                WOOL
            }
        }
        "gray_bed" => {
            if prop("part") == Some("foot") {
                COLOR_GRAY
            } else {
                WOOL
            }
        }
        "green_bed" => {
            if prop("part") == Some("foot") {
                COLOR_GREEN
            } else {
                WOOL
            }
        }
        "light_blue_bed" => {
            if prop("part") == Some("foot") {
                COLOR_LIGHT_BLUE
            } else {
                WOOL
            }
        }
        "light_gray_bed" => {
            if prop("part") == Some("foot") {
                COLOR_LIGHT_GRAY
            } else {
                WOOL
            }
        }
        "lime_bed" => {
            if prop("part") == Some("foot") {
                COLOR_LIGHT_GREEN
            } else {
                WOOL
            }
        }
        "magenta_bed" => {
            if prop("part") == Some("foot") {
                COLOR_MAGENTA
            } else {
                WOOL
            }
        }
        "orange_bed" => {
            if prop("part") == Some("foot") {
                COLOR_ORANGE
            } else {
                WOOL
            }
        }
        "pink_bed" => {
            if prop("part") == Some("foot") {
                COLOR_PINK
            } else {
                WOOL
            }
        }
        "purple_bed" => {
            if prop("part") == Some("foot") {
                COLOR_PURPLE
            } else {
                WOOL
            }
        }
        "red_bed" => {
            if prop("part") == Some("foot") {
                COLOR_RED
            } else {
                WOOL
            }
        }
        "white_bed" => {
            if prop("part") == Some("foot") {
                SNOW
            } else {
                WOOL
            }
        }
        "yellow_bed" => {
            if prop("part") == Some("foot") {
                COLOR_YELLOW
            } else {
                WOOL
            }
        }
        "wheat" => {
            let age = prop("age").and_then(|age| age.parse::<u8>().ok());
            if age.is_some_and(|age| age >= WHEAT_RIPE_AGE) {
                COLOR_YELLOW
            } else {
                PLANT
            }
        }
        "barrier" | "light" => {
            if prop("waterlogged") == Some("true") {
                WATER
            } else {
                NONE
            }
        }

        "acacia_button"
        | "activator_rail"
        | "air"
        | "bamboo_button"
        | "birch_button"
        | "black_candle_cake"
        | "black_stained_glass_pane"
        | "blue_candle_cake"
        | "blue_stained_glass_pane"
        | "brown_candle_cake"
        | "brown_stained_glass_pane"
        | "cake"
        | "candle_cake"
        | "cave_air"
        | "cherry_button"
        | "comparator"
        | "copper_bars"
        | "copper_chain"
        | "copper_torch"
        | "copper_wall_torch"
        | "creeper_head"
        | "creeper_wall_head"
        | "crimson_button"
        | "cyan_candle_cake"
        | "cyan_stained_glass_pane"
        | "dark_oak_button"
        | "detector_rail"
        | "dragon_head"
        | "dragon_wall_head"
        | "end_rod"
        | "exposed_copper_bars"
        | "exposed_copper_chain"
        | "flower_pot"
        | "glass"
        | "glass_pane"
        | "gray_candle_cake"
        | "gray_stained_glass_pane"
        | "green_candle_cake"
        | "green_stained_glass_pane"
        | "iron_bars"
        | "iron_chain"
        | "jungle_button"
        | "ladder"
        | "lever"
        | "light_blue_candle_cake"
        | "light_blue_stained_glass_pane"
        | "light_gray_candle_cake"
        | "light_gray_stained_glass_pane"
        | "lime_candle_cake"
        | "lime_stained_glass_pane"
        | "magenta_candle_cake"
        | "magenta_stained_glass_pane"
        | "mangrove_button"
        | "nether_portal"
        | "oak_button"
        | "orange_candle_cake"
        | "orange_stained_glass_pane"
        | "oxidized_copper_bars"
        | "oxidized_copper_chain"
        | "pale_oak_button"
        | "piglin_head"
        | "piglin_wall_head"
        | "pink_candle_cake"
        | "pink_stained_glass_pane"
        | "player_head"
        | "player_wall_head"
        | "polished_blackstone_button"
        | "potted_acacia_sapling"
        | "potted_allium"
        | "potted_azalea_bush"
        | "potted_azure_bluet"
        | "potted_bamboo"
        | "potted_birch_sapling"
        | "potted_blue_orchid"
        | "potted_brown_mushroom"
        | "potted_cactus"
        | "potted_cherry_sapling"
        | "potted_closed_eyeblossom"
        | "potted_cornflower"
        | "potted_crimson_fungus"
        | "potted_crimson_roots"
        | "potted_dandelion"
        | "potted_dark_oak_sapling"
        | "potted_dead_bush"
        | "potted_fern"
        | "potted_flowering_azalea_bush"
        | "potted_golden_dandelion"
        | "potted_jungle_sapling"
        | "potted_lily_of_the_valley"
        | "potted_mangrove_propagule"
        | "potted_oak_sapling"
        | "potted_open_eyeblossom"
        | "potted_orange_tulip"
        | "potted_oxeye_daisy"
        | "potted_pale_oak_sapling"
        | "potted_pink_tulip"
        | "potted_poppy"
        | "potted_red_mushroom"
        | "potted_red_tulip"
        | "potted_spruce_sapling"
        | "potted_torchflower"
        | "potted_warped_fungus"
        | "potted_warped_roots"
        | "potted_white_tulip"
        | "potted_wither_rose"
        | "powered_rail"
        | "purple_candle_cake"
        | "purple_stained_glass_pane"
        | "rail"
        | "red_candle_cake"
        | "red_stained_glass_pane"
        | "redstone_torch"
        | "redstone_wall_torch"
        | "redstone_wire"
        | "repeater"
        | "skeleton_skull"
        | "skeleton_wall_skull"
        | "soul_torch"
        | "soul_wall_torch"
        | "spruce_button"
        | "stone_button"
        | "structure_void"
        | "test_instance_block"
        | "torch"
        | "tripwire"
        | "tripwire_hook"
        | "void_air"
        | "wall_torch"
        | "warped_button"
        | "waxed_copper_bars"
        | "waxed_copper_chain"
        | "waxed_exposed_copper_bars"
        | "waxed_exposed_copper_chain"
        | "waxed_oxidized_copper_bars"
        | "waxed_oxidized_copper_chain"
        | "waxed_weathered_copper_bars"
        | "waxed_weathered_copper_chain"
        | "weathered_copper_bars"
        | "weathered_copper_chain"
        | "white_candle_cake"
        | "white_stained_glass_pane"
        | "wither_skeleton_skull"
        | "wither_skeleton_wall_skull"
        | "yellow_candle_cake"
        | "yellow_stained_glass_pane"
        | "zombie_head"
        | "zombie_wall_head" => NONE,
        "grass_block" | "slime_block" => GRASS,
        "birch_door"
        | "birch_fence"
        | "birch_fence_gate"
        | "birch_hanging_sign"
        | "birch_planks"
        | "birch_pressure_plate"
        | "birch_shelf"
        | "birch_sign"
        | "birch_slab"
        | "birch_stairs"
        | "birch_trapdoor"
        | "birch_wall_hanging_sign"
        | "birch_wall_sign"
        | "birch_wood"
        | "bone_block"
        | "candle"
        | "chiseled_sandstone"
        | "cut_sandstone"
        | "cut_sandstone_slab"
        | "end_stone"
        | "end_stone_brick_slab"
        | "end_stone_brick_stairs"
        | "end_stone_brick_wall"
        | "end_stone_bricks"
        | "glowstone"
        | "ochre_froglight"
        | "sand"
        | "sandstone"
        | "sandstone_slab"
        | "sandstone_stairs"
        | "sandstone_wall"
        | "scaffolding"
        | "smooth_sandstone"
        | "smooth_sandstone_slab"
        | "smooth_sandstone_stairs"
        | "stripped_birch_log"
        | "stripped_birch_wood"
        | "suspicious_sand"
        | "turtle_egg" => SAND,
        "cobweb" | "mushroom_stem" | "white_candle" => WOOL,
        "fire" | "lava" | "redstone_block" | "tnt" => FIRE,
        "blue_ice" | "frosted_ice" | "ice" | "packed_ice" => ICE,
        "anvil"
        | "brewing_stand"
        | "chipped_anvil"
        | "closed_eyeblossom"
        | "copper_lantern"
        | "damaged_anvil"
        | "exposed_copper_lantern"
        | "grindstone"
        | "heavy_core"
        | "heavy_weighted_pressure_plate"
        | "iron_block"
        | "iron_door"
        | "iron_trapdoor"
        | "lantern"
        | "lodestone"
        | "oxidized_copper_lantern"
        | "pale_oak_leaves"
        | "pale_oak_sapling"
        | "soul_lantern"
        | "waxed_copper_lantern"
        | "waxed_exposed_copper_lantern"
        | "waxed_oxidized_copper_lantern"
        | "waxed_weathered_copper_lantern"
        | "weathered_copper_lantern" => METAL,
        "acacia_leaves"
        | "acacia_sapling"
        | "allium"
        | "attached_melon_stem"
        | "attached_pumpkin_stem"
        | "azalea"
        | "azalea_leaves"
        | "azure_bluet"
        | "bamboo"
        | "beetroots"
        | "big_dripleaf"
        | "big_dripleaf_stem"
        | "birch_leaves"
        | "birch_sapling"
        | "blue_orchid"
        | "bush"
        | "cactus"
        | "carrots"
        | "cave_vines"
        | "cave_vines_plant"
        | "cocoa"
        | "cornflower"
        | "dandelion"
        | "dark_oak_leaves"
        | "dark_oak_sapling"
        | "fern"
        | "firefly_bush"
        | "flowering_azalea"
        | "flowering_azalea_leaves"
        | "golden_dandelion"
        | "jungle_leaves"
        | "jungle_sapling"
        | "large_fern"
        | "lilac"
        | "lily_of_the_valley"
        | "lily_pad"
        | "mangrove_leaves"
        | "mangrove_propagule"
        | "melon_stem"
        | "oak_leaves"
        | "oak_sapling"
        | "orange_tulip"
        | "oxeye_daisy"
        | "peony"
        | "pink_petals"
        | "pink_tulip"
        | "pitcher_crop"
        | "pitcher_plant"
        | "poppy"
        | "potatoes"
        | "pumpkin_stem"
        | "red_tulip"
        | "rose_bush"
        | "short_grass"
        | "small_dripleaf"
        | "spore_blossom"
        | "spruce_leaves"
        | "spruce_sapling"
        | "sugar_cane"
        | "sunflower"
        | "sweet_berry_bush"
        | "tall_grass"
        | "torchflower"
        | "torchflower_crop"
        | "vine"
        | "white_tulip"
        | "wildflowers"
        | "wither_rose" => PLANT,
        "powder_snow"
        | "snow"
        | "snow_block"
        | "white_carpet"
        | "white_concrete"
        | "white_concrete_powder"
        | "white_glazed_terracotta"
        | "white_shulker_box"
        | "white_stained_glass"
        | "white_wool" => SNOW,
        "clay"
        | "infested_chiseled_stone_bricks"
        | "infested_cobblestone"
        | "infested_cracked_stone_bricks"
        | "infested_mossy_stone_bricks"
        | "infested_stone"
        | "infested_stone_bricks" => CLAY,
        "brown_mushroom_block"
        | "coarse_dirt"
        | "dirt"
        | "dirt_path"
        | "farmland"
        | "granite"
        | "granite_slab"
        | "granite_stairs"
        | "granite_wall"
        | "hanging_roots"
        | "jukebox"
        | "jungle_door"
        | "jungle_fence"
        | "jungle_fence_gate"
        | "jungle_hanging_sign"
        | "jungle_planks"
        | "jungle_pressure_plate"
        | "jungle_shelf"
        | "jungle_sign"
        | "jungle_slab"
        | "jungle_stairs"
        | "jungle_trapdoor"
        | "jungle_wall_hanging_sign"
        | "jungle_wall_sign"
        | "jungle_wood"
        | "packed_mud"
        | "polished_granite"
        | "polished_granite_slab"
        | "polished_granite_stairs"
        | "rooted_dirt"
        | "stripped_jungle_log"
        | "stripped_jungle_wood" => DIRT,
        "andesite"
        | "andesite_slab"
        | "andesite_stairs"
        | "andesite_wall"
        | "bedrock"
        | "blast_furnace"
        | "cauldron"
        | "chiseled_stone_bricks"
        | "coal_ore"
        | "cobblestone"
        | "cobblestone_slab"
        | "cobblestone_stairs"
        | "cobblestone_wall"
        | "copper_ore"
        | "cracked_stone_bricks"
        | "crafter"
        | "diamond_ore"
        | "dispenser"
        | "dropper"
        | "emerald_ore"
        | "ender_chest"
        | "furnace"
        | "gold_ore"
        | "gravel"
        | "hopper"
        | "iron_ore"
        | "lapis_ore"
        | "lava_cauldron"
        | "mossy_cobblestone"
        | "mossy_cobblestone_slab"
        | "mossy_cobblestone_stairs"
        | "mossy_cobblestone_wall"
        | "mossy_stone_brick_slab"
        | "mossy_stone_brick_stairs"
        | "mossy_stone_brick_wall"
        | "mossy_stone_bricks"
        | "moving_piston"
        | "observer"
        | "pale_oak_wood"
        | "piston"
        | "piston_head"
        | "polished_andesite"
        | "polished_andesite_slab"
        | "polished_andesite_stairs"
        | "powder_snow_cauldron"
        | "redstone_ore"
        | "smoker"
        | "smooth_stone"
        | "smooth_stone_slab"
        | "spawner"
        | "sticky_piston"
        | "stone"
        | "stone_brick_slab"
        | "stone_brick_stairs"
        | "stone_brick_wall"
        | "stone_bricks"
        | "stone_pressure_plate"
        | "stone_slab"
        | "stone_stairs"
        | "stonecutter"
        | "suspicious_gravel"
        | "trial_spawner"
        | "vault"
        | "water_cauldron" => STONE,
        "bubble_column" | "frogspawn" | "kelp" | "kelp_plant" | "seagrass" | "tall_seagrass"
        | "water" => WATER,
        "bamboo_sapling"
        | "barrel"
        | "beehive"
        | "black_banner"
        | "black_wall_banner"
        | "blue_banner"
        | "blue_wall_banner"
        | "bookshelf"
        | "brown_banner"
        | "brown_wall_banner"
        | "cartography_table"
        | "chest"
        | "chiseled_bookshelf"
        | "composter"
        | "crafting_table"
        | "cyan_banner"
        | "cyan_wall_banner"
        | "daylight_detector"
        | "dead_bush"
        | "fletching_table"
        | "gray_banner"
        | "gray_wall_banner"
        | "green_banner"
        | "green_wall_banner"
        | "lectern"
        | "light_blue_banner"
        | "light_blue_wall_banner"
        | "light_gray_banner"
        | "light_gray_wall_banner"
        | "lime_banner"
        | "lime_wall_banner"
        | "loom"
        | "magenta_banner"
        | "magenta_wall_banner"
        | "note_block"
        | "oak_door"
        | "oak_fence"
        | "oak_fence_gate"
        | "oak_hanging_sign"
        | "oak_planks"
        | "oak_pressure_plate"
        | "oak_shelf"
        | "oak_sign"
        | "oak_slab"
        | "oak_stairs"
        | "oak_trapdoor"
        | "oak_wall_hanging_sign"
        | "oak_wall_sign"
        | "oak_wood"
        | "orange_banner"
        | "orange_wall_banner"
        | "petrified_oak_slab"
        | "pink_banner"
        | "pink_wall_banner"
        | "purple_banner"
        | "purple_wall_banner"
        | "red_banner"
        | "red_wall_banner"
        | "smithing_table"
        | "spruce_wall_hanging_sign"
        | "stripped_oak_log"
        | "stripped_oak_wood"
        | "trapped_chest"
        | "white_banner"
        | "white_wall_banner"
        | "yellow_banner"
        | "yellow_wall_banner" => WOOD,
        "chiseled_quartz_block"
        | "diorite"
        | "diorite_slab"
        | "diorite_stairs"
        | "diorite_wall"
        | "pale_oak_door"
        | "pale_oak_fence"
        | "pale_oak_fence_gate"
        | "pale_oak_hanging_sign"
        | "pale_oak_planks"
        | "pale_oak_pressure_plate"
        | "pale_oak_shelf"
        | "pale_oak_sign"
        | "pale_oak_slab"
        | "pale_oak_stairs"
        | "pale_oak_trapdoor"
        | "pale_oak_wall_hanging_sign"
        | "pale_oak_wall_sign"
        | "polished_diorite"
        | "polished_diorite_slab"
        | "polished_diorite_stairs"
        | "quartz_block"
        | "quartz_bricks"
        | "quartz_pillar"
        | "quartz_slab"
        | "quartz_stairs"
        | "sea_lantern"
        | "smooth_quartz"
        | "smooth_quartz_slab"
        | "smooth_quartz_stairs"
        | "stripped_pale_oak_log"
        | "stripped_pale_oak_wood"
        | "target" => QUARTZ,
        "acacia_door"
        | "acacia_fence"
        | "acacia_fence_gate"
        | "acacia_hanging_sign"
        | "acacia_planks"
        | "acacia_pressure_plate"
        | "acacia_shelf"
        | "acacia_sign"
        | "acacia_slab"
        | "acacia_stairs"
        | "acacia_trapdoor"
        | "acacia_wall_hanging_sign"
        | "acacia_wall_sign"
        | "carved_pumpkin"
        | "chiseled_copper"
        | "chiseled_red_sandstone"
        | "copper_block"
        | "copper_bulb"
        | "copper_chest"
        | "copper_door"
        | "copper_golem_statue"
        | "copper_grate"
        | "copper_trapdoor"
        | "creaking_heart"
        | "cut_copper"
        | "cut_copper_slab"
        | "cut_copper_stairs"
        | "cut_red_sandstone"
        | "cut_red_sandstone_slab"
        | "honey_block"
        | "honeycomb_block"
        | "jack_o_lantern"
        | "lightning_rod"
        | "open_eyeblossom"
        | "orange_candle"
        | "orange_carpet"
        | "orange_concrete"
        | "orange_concrete_powder"
        | "orange_glazed_terracotta"
        | "orange_shulker_box"
        | "orange_stained_glass"
        | "orange_wool"
        | "pumpkin"
        | "raw_copper_block"
        | "red_sand"
        | "red_sandstone"
        | "red_sandstone_slab"
        | "red_sandstone_stairs"
        | "red_sandstone_wall"
        | "smooth_red_sandstone"
        | "smooth_red_sandstone_slab"
        | "smooth_red_sandstone_stairs"
        | "stripped_acacia_log"
        | "stripped_acacia_wood"
        | "terracotta"
        | "waxed_chiseled_copper"
        | "waxed_copper_block"
        | "waxed_copper_bulb"
        | "waxed_copper_chest"
        | "waxed_copper_door"
        | "waxed_copper_golem_statue"
        | "waxed_copper_grate"
        | "waxed_copper_trapdoor"
        | "waxed_cut_copper"
        | "waxed_cut_copper_slab"
        | "waxed_cut_copper_stairs"
        | "waxed_lightning_rod" => COLOR_ORANGE,
        "magenta_candle"
        | "magenta_carpet"
        | "magenta_concrete"
        | "magenta_concrete_powder"
        | "magenta_glazed_terracotta"
        | "magenta_shulker_box"
        | "magenta_stained_glass"
        | "magenta_wool"
        | "purpur_block"
        | "purpur_pillar"
        | "purpur_slab"
        | "purpur_stairs" => COLOR_MAGENTA,
        "light_blue_candle"
        | "light_blue_carpet"
        | "light_blue_concrete"
        | "light_blue_concrete_powder"
        | "light_blue_glazed_terracotta"
        | "light_blue_shulker_box"
        | "light_blue_stained_glass"
        | "light_blue_wool"
        | "soul_fire" => COLOR_LIGHT_BLUE,
        "bamboo_door"
        | "bamboo_fence"
        | "bamboo_fence_gate"
        | "bamboo_hanging_sign"
        | "bamboo_mosaic"
        | "bamboo_mosaic_slab"
        | "bamboo_mosaic_stairs"
        | "bamboo_planks"
        | "bamboo_pressure_plate"
        | "bamboo_shelf"
        | "bamboo_sign"
        | "bamboo_slab"
        | "bamboo_stairs"
        | "bamboo_trapdoor"
        | "bamboo_wall_hanging_sign"
        | "bamboo_wall_sign"
        | "bee_nest"
        | "hay_block"
        | "horn_coral"
        | "horn_coral_block"
        | "horn_coral_fan"
        | "horn_coral_wall_fan"
        | "short_dry_grass"
        | "sponge"
        | "stripped_bamboo_block"
        | "tall_dry_grass"
        | "wet_sponge"
        | "yellow_candle"
        | "yellow_carpet"
        | "yellow_concrete"
        | "yellow_concrete_powder"
        | "yellow_glazed_terracotta"
        | "yellow_shulker_box"
        | "yellow_stained_glass"
        | "yellow_wool" => COLOR_YELLOW,
        "lime_candle"
        | "lime_carpet"
        | "lime_concrete"
        | "lime_concrete_powder"
        | "lime_glazed_terracotta"
        | "lime_shulker_box"
        | "lime_stained_glass"
        | "lime_wool"
        | "melon" => COLOR_LIGHT_GREEN,
        "brain_coral"
        | "brain_coral_block"
        | "brain_coral_fan"
        | "brain_coral_wall_fan"
        | "cactus_flower"
        | "cherry_leaves"
        | "cherry_sapling"
        | "pearlescent_froglight"
        | "pink_candle"
        | "pink_carpet"
        | "pink_concrete"
        | "pink_concrete_powder"
        | "pink_glazed_terracotta"
        | "pink_shulker_box"
        | "pink_stained_glass"
        | "pink_wool" => COLOR_PINK,
        "acacia_wood"
        | "dead_brain_coral"
        | "dead_brain_coral_block"
        | "dead_brain_coral_fan"
        | "dead_brain_coral_wall_fan"
        | "dead_bubble_coral"
        | "dead_bubble_coral_block"
        | "dead_bubble_coral_fan"
        | "dead_bubble_coral_wall_fan"
        | "dead_fire_coral"
        | "dead_fire_coral_block"
        | "dead_fire_coral_fan"
        | "dead_fire_coral_wall_fan"
        | "dead_horn_coral"
        | "dead_horn_coral_block"
        | "dead_horn_coral_fan"
        | "dead_horn_coral_wall_fan"
        | "dead_tube_coral"
        | "dead_tube_coral_block"
        | "dead_tube_coral_fan"
        | "dead_tube_coral_wall_fan"
        | "dried_ghast"
        | "gray_candle"
        | "gray_carpet"
        | "gray_concrete"
        | "gray_concrete_powder"
        | "gray_glazed_terracotta"
        | "gray_shulker_box"
        | "gray_stained_glass"
        | "gray_wool"
        | "tinted_glass" => COLOR_GRAY,
        "jigsaw"
        | "light_gray_candle"
        | "light_gray_carpet"
        | "light_gray_concrete"
        | "light_gray_concrete_powder"
        | "light_gray_glazed_terracotta"
        | "light_gray_shulker_box"
        | "light_gray_stained_glass"
        | "light_gray_wool"
        | "pale_hanging_moss"
        | "pale_moss_block"
        | "pale_moss_carpet"
        | "structure_block"
        | "test_block" => COLOR_LIGHT_GRAY,
        "calibrated_sculk_sensor"
        | "cyan_candle"
        | "cyan_carpet"
        | "cyan_concrete"
        | "cyan_concrete_powder"
        | "cyan_glazed_terracotta"
        | "cyan_shulker_box"
        | "cyan_stained_glass"
        | "cyan_wool"
        | "nether_sprouts"
        | "prismarine"
        | "prismarine_slab"
        | "prismarine_stairs"
        | "prismarine_wall"
        | "sculk_sensor"
        | "twisting_vines"
        | "twisting_vines_plant"
        | "warped_fungus"
        | "warped_roots" => COLOR_CYAN,
        "amethyst_block"
        | "amethyst_cluster"
        | "bubble_coral"
        | "bubble_coral_block"
        | "bubble_coral_fan"
        | "bubble_coral_wall_fan"
        | "budding_amethyst"
        | "chorus_flower"
        | "chorus_plant"
        | "large_amethyst_bud"
        | "medium_amethyst_bud"
        | "mycelium"
        | "purple_candle"
        | "purple_carpet"
        | "purple_concrete"
        | "purple_concrete_powder"
        | "purple_glazed_terracotta"
        | "purple_stained_glass"
        | "purple_wool"
        | "repeating_command_block"
        | "shulker_box"
        | "small_amethyst_bud" => COLOR_PURPLE,
        "blue_candle"
        | "blue_carpet"
        | "blue_concrete"
        | "blue_concrete_powder"
        | "blue_glazed_terracotta"
        | "blue_shulker_box"
        | "blue_stained_glass"
        | "blue_wool"
        | "tube_coral"
        | "tube_coral_block"
        | "tube_coral_fan"
        | "tube_coral_wall_fan" => COLOR_BLUE,
        "brown_candle"
        | "brown_carpet"
        | "brown_concrete"
        | "brown_concrete_powder"
        | "brown_glazed_terracotta"
        | "brown_mushroom"
        | "brown_shulker_box"
        | "brown_stained_glass"
        | "brown_wool"
        | "command_block"
        | "dark_oak_door"
        | "dark_oak_fence"
        | "dark_oak_fence_gate"
        | "dark_oak_hanging_sign"
        | "dark_oak_log"
        | "dark_oak_planks"
        | "dark_oak_pressure_plate"
        | "dark_oak_shelf"
        | "dark_oak_sign"
        | "dark_oak_slab"
        | "dark_oak_stairs"
        | "dark_oak_trapdoor"
        | "dark_oak_wall_hanging_sign"
        | "dark_oak_wall_sign"
        | "dark_oak_wood"
        | "leaf_litter"
        | "soul_sand"
        | "soul_soil"
        | "stripped_dark_oak_log"
        | "stripped_dark_oak_wood" => COLOR_BROWN,
        "chain_command_block"
        | "dried_kelp_block"
        | "end_portal_frame"
        | "green_candle"
        | "green_carpet"
        | "green_concrete"
        | "green_concrete_powder"
        | "green_glazed_terracotta"
        | "green_shulker_box"
        | "green_stained_glass"
        | "green_wool"
        | "moss_block"
        | "moss_carpet"
        | "sea_pickle" => COLOR_GREEN,
        "brick_slab"
        | "brick_stairs"
        | "brick_wall"
        | "bricks"
        | "enchanting_table"
        | "fire_coral"
        | "fire_coral_block"
        | "fire_coral_fan"
        | "fire_coral_wall_fan"
        | "mangrove_door"
        | "mangrove_fence"
        | "mangrove_fence_gate"
        | "mangrove_hanging_sign"
        | "mangrove_planks"
        | "mangrove_pressure_plate"
        | "mangrove_shelf"
        | "mangrove_sign"
        | "mangrove_slab"
        | "mangrove_stairs"
        | "mangrove_trapdoor"
        | "mangrove_wall_hanging_sign"
        | "mangrove_wall_sign"
        | "mangrove_wood"
        | "nether_wart"
        | "nether_wart_block"
        | "red_candle"
        | "red_carpet"
        | "red_concrete"
        | "red_concrete_powder"
        | "red_glazed_terracotta"
        | "red_mushroom"
        | "red_mushroom_block"
        | "red_shulker_box"
        | "red_stained_glass"
        | "red_wool"
        | "shroomlight"
        | "sniffer_egg"
        | "stripped_mangrove_log"
        | "stripped_mangrove_wood" => COLOR_RED,
        "ancient_debris"
        | "basalt"
        | "black_candle"
        | "black_carpet"
        | "black_concrete"
        | "black_concrete_powder"
        | "black_glazed_terracotta"
        | "black_shulker_box"
        | "black_stained_glass"
        | "black_wool"
        | "blackstone"
        | "blackstone_slab"
        | "blackstone_stairs"
        | "blackstone_wall"
        | "chiseled_polished_blackstone"
        | "coal_block"
        | "cracked_polished_blackstone_bricks"
        | "crying_obsidian"
        | "dragon_egg"
        | "end_gateway"
        | "end_portal"
        | "gilded_blackstone"
        | "netherite_block"
        | "obsidian"
        | "polished_basalt"
        | "polished_blackstone"
        | "polished_blackstone_brick_slab"
        | "polished_blackstone_brick_stairs"
        | "polished_blackstone_brick_wall"
        | "polished_blackstone_bricks"
        | "polished_blackstone_pressure_plate"
        | "polished_blackstone_slab"
        | "polished_blackstone_stairs"
        | "polished_blackstone_wall"
        | "respawn_anchor"
        | "sculk"
        | "sculk_catalyst"
        | "sculk_shrieker"
        | "sculk_vein"
        | "smooth_basalt" => COLOR_BLACK,
        "bell" | "gold_block" | "light_weighted_pressure_plate" | "raw_gold_block" => GOLD,
        "beacon"
        | "conduit"
        | "dark_prismarine"
        | "dark_prismarine_slab"
        | "dark_prismarine_stairs"
        | "diamond_block"
        | "prismarine_brick_slab"
        | "prismarine_brick_stairs"
        | "prismarine_bricks" => DIAMOND,
        "lapis_block" => LAPIS,
        "emerald_block" => EMERALD,
        "campfire"
        | "mangrove_roots"
        | "muddy_mangrove_roots"
        | "podzol"
        | "soul_campfire"
        | "spruce_door"
        | "spruce_fence"
        | "spruce_fence_gate"
        | "spruce_hanging_sign"
        | "spruce_planks"
        | "spruce_pressure_plate"
        | "spruce_shelf"
        | "spruce_sign"
        | "spruce_slab"
        | "spruce_stairs"
        | "spruce_trapdoor"
        | "spruce_wall_sign"
        | "spruce_wood"
        | "stripped_spruce_log"
        | "stripped_spruce_wood" => PODZOL,
        "chiseled_nether_bricks"
        | "cracked_nether_bricks"
        | "crimson_fungus"
        | "crimson_roots"
        | "magma_block"
        | "nether_brick_fence"
        | "nether_brick_slab"
        | "nether_brick_stairs"
        | "nether_brick_wall"
        | "nether_bricks"
        | "nether_gold_ore"
        | "nether_quartz_ore"
        | "netherrack"
        | "red_nether_brick_slab"
        | "red_nether_brick_stairs"
        | "red_nether_brick_wall"
        | "red_nether_bricks"
        | "weeping_vines"
        | "weeping_vines_plant" => NETHER,
        "calcite"
        | "cherry_door"
        | "cherry_fence"
        | "cherry_fence_gate"
        | "cherry_planks"
        | "cherry_pressure_plate"
        | "cherry_shelf"
        | "cherry_sign"
        | "cherry_slab"
        | "cherry_stairs"
        | "cherry_trapdoor"
        | "cherry_wall_sign"
        | "white_terracotta" => TERRACOTTA_WHITE,
        "chiseled_resin_bricks"
        | "orange_terracotta"
        | "redstone_lamp"
        | "resin_block"
        | "resin_brick_slab"
        | "resin_brick_stairs"
        | "resin_brick_wall"
        | "resin_bricks"
        | "resin_clump" => TERRACOTTA_ORANGE,
        "magenta_terracotta" => TERRACOTTA_MAGENTA,
        "light_blue_terracotta" => TERRACOTTA_LIGHT_BLUE,
        "yellow_terracotta" => TERRACOTTA_YELLOW,
        "lime_terracotta" => TERRACOTTA_LIGHT_GREEN,
        "cherry_hanging_sign"
        | "cherry_wall_hanging_sign"
        | "pink_terracotta"
        | "stripped_cherry_wood" => TERRACOTTA_PINK,
        "cherry_wood"
        | "chiseled_tuff"
        | "chiseled_tuff_bricks"
        | "gray_terracotta"
        | "polished_tuff"
        | "polished_tuff_slab"
        | "polished_tuff_stairs"
        | "polished_tuff_wall"
        | "tuff"
        | "tuff_brick_slab"
        | "tuff_brick_stairs"
        | "tuff_brick_wall"
        | "tuff_bricks"
        | "tuff_slab"
        | "tuff_stairs"
        | "tuff_wall" => TERRACOTTA_GRAY,
        "exposed_chiseled_copper"
        | "exposed_copper"
        | "exposed_copper_bulb"
        | "exposed_copper_chest"
        | "exposed_copper_door"
        | "exposed_copper_golem_statue"
        | "exposed_copper_grate"
        | "exposed_copper_trapdoor"
        | "exposed_cut_copper"
        | "exposed_cut_copper_slab"
        | "exposed_cut_copper_stairs"
        | "exposed_lightning_rod"
        | "light_gray_terracotta"
        | "mud_brick_slab"
        | "mud_brick_stairs"
        | "mud_brick_wall"
        | "mud_bricks"
        | "waxed_exposed_chiseled_copper"
        | "waxed_exposed_copper"
        | "waxed_exposed_copper_bulb"
        | "waxed_exposed_copper_chest"
        | "waxed_exposed_copper_door"
        | "waxed_exposed_copper_golem_statue"
        | "waxed_exposed_copper_grate"
        | "waxed_exposed_copper_trapdoor"
        | "waxed_exposed_cut_copper"
        | "waxed_exposed_cut_copper_slab"
        | "waxed_exposed_cut_copper_stairs"
        | "waxed_exposed_lightning_rod" => TERRACOTTA_LIGHT_GRAY,
        "cyan_terracotta" | "mud" => TERRACOTTA_CYAN,
        "purple_shulker_box" | "purple_terracotta" => TERRACOTTA_PURPLE,
        "blue_terracotta" => TERRACOTTA_BLUE,
        "brown_terracotta" | "dripstone_block" | "pointed_dripstone" => TERRACOTTA_BROWN,
        "green_terracotta" => TERRACOTTA_GREEN,
        "decorated_pot" | "red_terracotta" => TERRACOTTA_RED,
        "black_terracotta" => TERRACOTTA_BLACK,
        "crimson_nylium" => CRIMSON_NYLIUM,
        "crimson_door"
        | "crimson_fence"
        | "crimson_fence_gate"
        | "crimson_hanging_sign"
        | "crimson_planks"
        | "crimson_pressure_plate"
        | "crimson_shelf"
        | "crimson_sign"
        | "crimson_slab"
        | "crimson_stairs"
        | "crimson_stem"
        | "crimson_trapdoor"
        | "crimson_wall_hanging_sign"
        | "crimson_wall_sign"
        | "stripped_crimson_stem" => CRIMSON_STEM,
        "crimson_hyphae" | "stripped_crimson_hyphae" => CRIMSON_HYPHAE,
        "oxidized_chiseled_copper"
        | "oxidized_copper"
        | "oxidized_copper_bulb"
        | "oxidized_copper_chest"
        | "oxidized_copper_door"
        | "oxidized_copper_golem_statue"
        | "oxidized_copper_grate"
        | "oxidized_copper_trapdoor"
        | "oxidized_cut_copper"
        | "oxidized_cut_copper_slab"
        | "oxidized_cut_copper_stairs"
        | "oxidized_lightning_rod"
        | "warped_nylium"
        | "waxed_oxidized_chiseled_copper"
        | "waxed_oxidized_copper"
        | "waxed_oxidized_copper_bulb"
        | "waxed_oxidized_copper_chest"
        | "waxed_oxidized_copper_door"
        | "waxed_oxidized_copper_golem_statue"
        | "waxed_oxidized_copper_grate"
        | "waxed_oxidized_copper_trapdoor"
        | "waxed_oxidized_cut_copper"
        | "waxed_oxidized_cut_copper_slab"
        | "waxed_oxidized_cut_copper_stairs"
        | "waxed_oxidized_lightning_rod" => WARPED_NYLIUM,
        "stripped_warped_stem"
        | "warped_door"
        | "warped_fence"
        | "warped_fence_gate"
        | "warped_hanging_sign"
        | "warped_planks"
        | "warped_pressure_plate"
        | "warped_shelf"
        | "warped_sign"
        | "warped_slab"
        | "warped_stairs"
        | "warped_stem"
        | "warped_trapdoor"
        | "warped_wall_hanging_sign"
        | "warped_wall_sign"
        | "waxed_weathered_chiseled_copper"
        | "waxed_weathered_copper"
        | "waxed_weathered_copper_bulb"
        | "waxed_weathered_copper_chest"
        | "waxed_weathered_copper_door"
        | "waxed_weathered_copper_golem_statue"
        | "waxed_weathered_copper_grate"
        | "waxed_weathered_copper_trapdoor"
        | "waxed_weathered_cut_copper"
        | "waxed_weathered_cut_copper_slab"
        | "waxed_weathered_cut_copper_stairs"
        | "waxed_weathered_lightning_rod"
        | "weathered_chiseled_copper"
        | "weathered_copper"
        | "weathered_copper_bulb"
        | "weathered_copper_chest"
        | "weathered_copper_door"
        | "weathered_copper_golem_statue"
        | "weathered_copper_grate"
        | "weathered_copper_trapdoor"
        | "weathered_cut_copper"
        | "weathered_cut_copper_slab"
        | "weathered_cut_copper_stairs"
        | "weathered_lightning_rod" => WARPED_STEM,
        "stripped_warped_hyphae" | "warped_hyphae" => WARPED_HYPHAE,
        "warped_wart_block" => WARPED_WART_BLOCK,
        "chiseled_deepslate"
        | "cobbled_deepslate"
        | "cobbled_deepslate_slab"
        | "cobbled_deepslate_stairs"
        | "cobbled_deepslate_wall"
        | "cracked_deepslate_bricks"
        | "cracked_deepslate_tiles"
        | "deepslate"
        | "deepslate_brick_slab"
        | "deepslate_brick_stairs"
        | "deepslate_brick_wall"
        | "deepslate_bricks"
        | "deepslate_coal_ore"
        | "deepslate_copper_ore"
        | "deepslate_diamond_ore"
        | "deepslate_emerald_ore"
        | "deepslate_gold_ore"
        | "deepslate_iron_ore"
        | "deepslate_lapis_ore"
        | "deepslate_redstone_ore"
        | "deepslate_tile_slab"
        | "deepslate_tile_stairs"
        | "deepslate_tile_wall"
        | "deepslate_tiles"
        | "infested_deepslate"
        | "polished_deepslate"
        | "polished_deepslate_slab"
        | "polished_deepslate_stairs"
        | "polished_deepslate_wall"
        | "reinforced_deepslate" => DEEPSLATE,
        "raw_iron_block" => RAW_IRON,
        "glow_lichen" | "verdant_froglight" => GLOW_LICHEN,
        _ => NONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_props(_: &str) -> Option<&'static str> {
        None
    }

    #[test]
    fn plain_blocks() {
        assert_eq!(material("stone", no_props), STONE);
        assert_eq!(material("grass_block", no_props), GRASS);
        assert_eq!(material("white_wool", no_props), SNOW);
        assert_eq!(material("air", no_props), NONE);
        assert_eq!(material("not_a_block", no_props), NONE);
    }

    #[test]
    fn water_is_water() {
        assert_eq!(material("water", no_props), WATER);
    }

    #[test]
    fn log_axis_picks_top_or_side() {
        let y = |key: &str| (key == "axis").then_some("y");
        let x = |key: &str| (key == "axis").then_some("x");
        assert_eq!(material("oak_log", y), WOOD);
        assert_eq!(material("oak_log", x), PODZOL);
    }

    #[test]
    fn bed_part_picks_dye_or_wool() {
        let foot = |key: &str| (key == "part").then_some("foot");
        let head = |key: &str| (key == "part").then_some("head");
        assert_eq!(material("white_bed", foot), SNOW);
        assert_eq!(material("white_bed", head), WOOL);
    }

    #[test]
    fn wheat_ripens_at_age_six() {
        let age = |n: &'static str| move |key: &str| (key == "age").then_some(n);
        assert_eq!(material("wheat", age("5")), PLANT);
        assert_eq!(material("wheat", age("6")), COLOR_YELLOW);
    }
}
