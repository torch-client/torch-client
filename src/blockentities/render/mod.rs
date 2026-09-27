pub mod banner;
pub mod bed;
pub mod chest;
pub mod lectern;
pub mod sign;
pub mod skull;

use super::BeRegistry;

pub fn register_all(registry: &mut BeRegistry) {
    banner::register(registry);
    bed::register(registry);
    chest::register(registry);
    lectern::register(registry);
    sign::register(registry);
    skull::register(registry);
}

#[cfg(test)]
pub use test_support::{banner_state, test_state, test_states};

#[cfg(test)]
mod test_support {
    use std::collections::HashMap;
    use std::sync::Arc;

    use azalea_registry::builtin::BlockEntityKind;

    use crate::blockentities::BeState;
    use crate::blockentities::feed::{BlockEntityData, BlockStateInfo};

    pub fn test_state(block: &str, props: &[(&str, &str)]) -> BeState {
        let kind = kind_for(block);
        BeState {
            pos: [0, 64, 0],
            kind,
            state: Arc::new(BlockStateInfo {
                state_id: 0,
                block: block.to_string(),
                props: props
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<HashMap<_, _>>(),
            }),
            data: Arc::new(BlockEntityData::None),
            open: 0.0,
            anim: 0.0,
            draw_outline: false,
            phase: 0.0,
        }
    }

    fn kind_for(block: &str) -> BlockEntityKind {
        match block {
            "ender_chest" => BlockEntityKind::EnderChest,
            "trapped_chest" => BlockEntityKind::TrappedChest,
            _ if block.ends_with("chest") => BlockEntityKind::Chest,
            _ if block.ends_with("_hanging_sign") => BlockEntityKind::HangingSign,
            _ if block.ends_with("skull") || block.ends_with("head") => BlockEntityKind::Skull,
            _ if block.ends_with("_bed") => BlockEntityKind::Bed,
            "lectern" => BlockEntityKind::Lectern,
            _ if block.ends_with("_banner") => BlockEntityKind::Banner,
            _ => BlockEntityKind::Sign,
        }
    }

    const WOODS: &[&str] = &[
        "oak", "spruce", "birch", "acacia", "cherry", "jungle", "dark_oak", "pale_oak", "crimson",
        "warped", "mangrove", "bamboo",
    ];

    const SKULLS: &[&str] = &[
        "skeleton_skull",
        "skeleton_wall_skull",
        "wither_skeleton_skull",
        "wither_skeleton_wall_skull",
        "player_head",
        "player_wall_head",
        "zombie_head",
        "zombie_wall_head",
        "creeper_head",
        "creeper_wall_head",
        "dragon_head",
        "dragon_wall_head",
        "piglin_head",
        "piglin_wall_head",
    ];

    const CHESTS: &[&str] = &[
        "chest",
        "trapped_chest",
        "ender_chest",
        "copper_chest",
        "exposed_copper_chest",
        "weathered_copper_chest",
        "oxidized_copper_chest",
        "waxed_copper_chest",
        "waxed_exposed_copper_chest",
        "waxed_weathered_copper_chest",
        "waxed_oxidized_copper_chest",
    ];

    pub fn banner_state(block: &str, layers: &[(&str, u8)]) -> BeState {
        BeState {
            data: Arc::new(BlockEntityData::Banner(
                crate::blockentities::banner::BannerData {
                    layers: layers
                        .iter()
                        .map(|(asset, color)| crate::blockentities::banner::BannerLayer {
                            asset: Box::from(*asset),
                            color: *color,
                        })
                        .collect(),
                },
            )),
            ..test_state(
                block,
                if block.ends_with("_wall_banner") {
                    &[("facing", "north")]
                } else {
                    &[("rotation", "0")]
                },
            )
        }
    }

    pub fn test_states() -> Vec<BeState> {
        let mut out = Vec::new();
        for block in CHESTS {
            for chest_type in ["single", "left", "right"] {
                for facing in ["north", "south", "east", "west"] {
                    out.push(test_state(
                        block,
                        &[("type", chest_type), ("facing", facing)],
                    ));
                }
            }
        }
        for wood in WOODS {
            out.push(test_state(&format!("{wood}_sign"), &[("rotation", "0")]));
            out.push(test_state(
                &format!("{wood}_wall_sign"),
                &[("facing", "north")],
            ));
            for attached in ["true", "false"] {
                out.push(test_state(
                    &format!("{wood}_hanging_sign"),
                    &[("rotation", "3"), ("attached", attached)],
                ));
            }
            out.push(test_state(
                &format!("{wood}_wall_hanging_sign"),
                &[("facing", "east")],
            ));
        }
        for block in SKULLS {
            if block.contains("_wall_") {
                out.push(test_state(
                    block,
                    &[("facing", "north"), ("powered", "false")],
                ));
            } else {
                out.push(test_state(
                    block,
                    &[("rotation", "0"), ("powered", "false")],
                ));
            }
        }
        for color in COLORS {
            for block in [format!("{color}_banner"), format!("{color}_wall_banner")] {
                for count in [0, 1, 16] {
                    let layers: Vec<(&str, u8)> = PATTERNS
                        .iter()
                        .cycle()
                        .take(count)
                        .enumerate()
                        .map(|(i, asset)| (*asset, (i % 16) as u8))
                        .collect();
                    out.push(banner_state(&block, &layers));
                }
            }
        }
        for facing in ["north", "south", "east", "west"] {
            for has_book in ["true", "false"] {
                out.push(test_state(
                    "lectern",
                    &[("facing", facing), ("has_book", has_book)],
                ));
            }
        }
        for color in COLORS {
            for part in ["head", "foot"] {
                for facing in ["north", "south", "east", "west"] {
                    out.push(test_state(
                        &format!("{color}_bed"),
                        &[("facing", facing), ("part", part)],
                    ));
                }
            }
        }
        out
    }

    const PATTERNS: &[&str] = &[
        "base",
        "border",
        "bricks",
        "circle",
        "creeper",
        "cross",
        "curly_border",
        "diagonal_left",
        "diagonal_right",
        "flower",
        "globe",
        "gradient",
        "half_horizontal",
        "half_vertical",
        "mojang",
        "rhombus",
        "skull",
        "straight_cross",
    ];

    const COLORS: &[&str] = &[
        "white",
        "orange",
        "magenta",
        "light_blue",
        "yellow",
        "lime",
        "pink",
        "gray",
        "light_gray",
        "cyan",
        "purple",
        "blue",
        "brown",
        "green",
        "red",
        "black",
    ];
}
