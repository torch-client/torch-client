use std::str::FromStr;

use azalea_registry::builtin::{BlockKind, ItemKind};

const RENAMED: [(&str, &str); 15] = [
    ("redstone", "redstone_wire"),
    ("string", "tripwire"),
    ("wheat_seeds", "wheat"),
    ("cocoa_beans", "cocoa"),
    ("carrot", "carrots"),
    ("potato", "potatoes"),
    ("torchflower_seeds", "torchflower_crop"),
    ("pitcher_pod", "pitcher_crop"),
    ("beetroot_seeds", "beetroots"),
    ("sweet_berries", "sweet_berry_bush"),
    ("glow_berries", "cave_vines"),
    ("pumpkin_seeds", "pumpkin_stem"),
    ("melon_seeds", "melon_stem"),
    ("powder_snow_bucket", "powder_snow"),
    ("wheat", ""),
];

pub fn placed_block(id: &str) -> Option<BlockKind> {
    let block = match RENAMED.iter().find(|(item, _)| *item == id) {
        Some((_, "")) => return None,
        Some((_, block)) => block,
        None => id,
    };
    BlockKind::from_str(block)
        .ok()
        .filter(|b| *b != BlockKind::Air)
}

pub fn block_item(block: BlockKind) -> Option<ItemKind> {
    let id = block.to_str().trim_start_matches("minecraft:");
    let item = |s: &str| ItemKind::from_str(s).ok().filter(|i| *i != ItemKind::Air);
    if let Some((renamed, _)) = RENAMED.iter().find(|(_, b)| *b == id) {
        return item(renamed);
    }
    item(id).or_else(|| {
        let standing = match id.strip_prefix("wall_") {
            Some(rest) => rest.to_owned(),
            None => id.replacen("_wall_", "_", 1),
        };
        (standing != id).then(|| item(&standing)).flatten()
    })
}
