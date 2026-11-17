use azalea_inventory::components::{
    AdventureModePredicate, BlockPredicate, DataComponentMatchers, DyeColor, FireworkExplosion,
    FireworkExplosionShape, Fireworks, PartialOrFullProfile, PartialProfile, Profile,
    TropicalFishPattern,
};
use azalea_inventory::{ItemStack, ItemStackData};
use azalea_registry::HolderSet;
use azalea_registry::builtin::{BlockKind, EntityKind, ItemKind};

use super::*;

fn text(line: &[Span]) -> String {
    line.iter().map(|s| s.text.as_str()).collect()
}

fn explosion(shape: FireworkExplosionShape, colors: Vec<i32>) -> FireworkExplosion {
    FireworkExplosion {
        shape,
        colors,
        fade_colors: Vec::new(),
        has_trail: false,
        has_twinkle: false,
    }
}

#[test]
fn splice_keeps_argument_styles_and_fills_default_ones() {
    let name = vec![Span::colored("Stone", 0x55FFFF)];
    let count = vec![Span::new("3")];
    let template = format!("{MARK}0{MARK} x{MARK}1{MARK}");
    let line = splice(&template, &[name, count], Style::colored(GRAY));
    assert_eq!(text(&line), "Stone x3");
    assert_eq!(line[0].style.color, 0x55FFFF);
    assert_eq!(line[1].style.color, GRAY);
    assert_eq!(line[2].style.color, GRAY);
}

#[test]
fn splice_without_marks_is_one_span() {
    let line = splice("Unknown", &[], Style::default());
    assert_eq!(line.len(), 1);
    assert_eq!(line[0].text, "Unknown");
}

#[test]
fn translate_with_leaves_no_marks_behind() {
    let line = translate_with("no.such.key", &[vec![Span::new("x")]], Style::default());
    assert!(!text(&line).contains(MARK));
}

#[test]
fn container_lists_five_stacks_then_the_remainder() {
    let mut items = vec![ItemStack::Empty];
    items.extend((0..7).map(|_| ItemStack::Present(ItemStackData::new(ItemKind::Stone, 3))));
    items.push(ItemStack::Empty);
    let mut out = Vec::new();
    container_rows(&items, |_| vec![Span::new("n")], &mut out);
    assert_eq!(out.len(), 6);
    assert!(out[..5].iter().all(|line| !line[0].style.italic));
    assert!(out[5][0].style.italic);
}

#[test]
fn container_with_five_stacks_has_no_remainder_row() {
    let items: Vec<ItemStack> = (0..5)
        .map(|_| ItemStack::Present(ItemStackData::new(ItemKind::Stone, 1)))
        .collect();
    let mut out = Vec::new();
    container_rows(&items, |_| vec![Span::new("n")], &mut out);
    assert_eq!(out.len(), 5);
}

#[test]
fn pot_sides_print_front_left_right_back() {
    let items = [
        ItemKind::AnglerPotterySherd,
        ItemKind::ArcherPotterySherd,
        ItemKind::Brick,
        ItemKind::PrizePotterySherd,
    ];
    let mut out = Vec::new();
    pot_rows(&items, |k| k.to_str().to_owned(), &mut out);
    let rows: Vec<String> = out.iter().map(|l| text(l)).collect();
    assert_eq!(
        rows,
        [
            "",
            "minecraft:prize_pottery_sherd",
            "minecraft:archer_pottery_sherd",
            "minecraft:brick",
            "minecraft:angler_pottery_sherd",
        ]
    );
    assert!(out[1..].iter().all(|l| l[0].style.color == GRAY));
}

#[test]
fn undecorated_pot_prints_nothing() {
    let mut out = Vec::new();
    pot_rows(&[ItemKind::Brick; 4], |k| k.to_str().to_owned(), &mut out);
    pot_rows(&[], |k| k.to_str().to_owned(), &mut out);
    assert!(out.is_empty());
}

#[test]
fn common_tropical_fish_is_one_row_and_others_two() {
    let mut out = Vec::new();
    tropical_fish_rows(
        TropicalFishPattern::Stripey,
        DyeColor::Orange,
        DyeColor::Gray,
        &mut out,
    );
    assert_eq!(out.len(), 1);
    assert!(text(&out[0]).ends_with('0') || !text(&out[0]).contains("predefined"));
    out.clear();
    tropical_fish_rows(
        TropicalFishPattern::Kob,
        DyeColor::White,
        DyeColor::White,
        &mut out,
    );
    assert_eq!(out.len(), 2);
    assert!(!text(&out[1]).contains(", "));
    assert!(
        out.iter()
            .all(|l| l[0].style.italic && l[0].style.color == GRAY)
    );
}

#[test]
fn firework_colors_match_only_the_exact_dye_value() {
    let custom = translate("item.minecraft.firework_star.custom_color", &[]);
    assert_ne!(firework::color_name(15790320), custom);
    assert_eq!(firework::color_name(15790321), custom);
}

#[test]
fn star_rows_follow_set_fields() {
    let mut star = explosion(FireworkExplosionShape::Creeper, vec![11743532, 1]);
    star.fade_colors = vec![1973019];
    star.has_trail = true;
    let mut out = Vec::new();
    firework::explosion_rows(&star, &mut out);
    assert_eq!(out.len(), 4);
    assert!(text(&out[1]).contains(", "));
    assert!(out.iter().all(|l| l[0].style.color == GRAY));
}

#[test]
fn rocket_groups_equal_neighbouring_stars() {
    let ball = explosion(FireworkExplosionShape::SmallBall, vec![15790320]);
    let star = explosion(FireworkExplosionShape::Star, Vec::new());
    let rocket = Fireworks {
        flight_duration: 2,
        explosions: vec![ball.clone(), ball.clone(), star, ball],
    };
    let mut out = Vec::new();
    firework::fireworks_rows(&rocket, &mut out);
    assert_eq!(out.len(), 6);
    assert!(text(&out[0]).ends_with(" 2"));
    assert_eq!(out[2][0].text, "  ");
}

#[test]
fn rocket_without_flight_skips_the_duration() {
    let mut out = Vec::new();
    firework::fireworks_rows(&Fireworks::new(), &mut out);
    assert!(out.is_empty());
}

#[test]
fn honey_level_is_zero_to_five() {
    assert_eq!(honey_level("0"), Some(0));
    assert_eq!(honey_level("5"), Some(5));
    assert_eq!(honey_level("6"), None);
    assert_eq!(honey_level("full"), None);
}

#[test]
fn hostile_mobs_are_not_allowed_in_peaceful() {
    assert!(!allowed_in_peaceful(EntityKind::Zombie));
    assert!(!allowed_in_peaceful(EntityKind::Creeper));
    assert!(allowed_in_peaceful(EntityKind::Pig));
    assert!(allowed_in_peaceful(EntityKind::Shulker));
}

#[test]
fn empty_spawner_explains_how_to_set_it() {
    let mut out = Vec::new();
    spawner_rows(None, &mut out);
    assert_eq!(out.len(), 3);
    assert!(out[0].is_empty());
    assert_eq!(out[2][1].style.color, BLUE);
}

#[test]
fn predicate_without_blocks_is_unknown() {
    let predicate = AdventureModePredicate {
        predicates: vec![
            BlockPredicate {
                blocks: Some(HolderSet::Direct {
                    contents: vec![BlockKind::Stone, BlockKind::Stone, BlockKind::Dirt],
                }),
                properties: None,
                nbt: None,
                components: DataComponentMatchers::default(),
            },
            BlockPredicate {
                blocks: None,
                properties: None,
                nbt: None,
                components: DataComponentMatchers::default(),
            },
        ],
    };
    let mut out = Vec::new();
    predicate_rows("item.canBreak", &predicate, &mut out);
    assert_eq!(out.len(), 3);
    assert_eq!(out[2][0].style.color, GRAY);
}

#[test]
fn predicate_blocks_print_once_each() {
    let predicate = AdventureModePredicate {
        predicates: vec![BlockPredicate {
            blocks: Some(HolderSet::Direct {
                contents: vec![BlockKind::Stone, BlockKind::Dirt, BlockKind::Stone],
            }),
            properties: None,
            nbt: None,
            components: DataComponentMatchers::default(),
        }],
    };
    let mut out = Vec::new();
    predicate_rows("item.canPlace", &predicate, &mut out);
    assert_eq!(out.len(), 4);
    assert!(out[2..].iter().all(|l| l[0].style.color == DARK_GRAY));
}

#[test]
fn only_a_name_or_an_id_alone_is_dynamic() {
    let profile = |name: Option<&str>| Profile {
        unpack: Box::new(PartialOrFullProfile::Partial(PartialProfile {
            name: name.map(str::to_owned),
            ..PartialProfile::default()
        })),
        ..Profile::default()
    };
    let mut out = Vec::new();
    profile_rows(&profile(Some("Steve")), &mut out);
    assert_eq!(out.len(), 1);
    out.clear();
    profile_rows(&profile(None), &mut out);
    assert!(out.is_empty());
}
