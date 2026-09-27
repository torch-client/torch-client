use super::{
    Digits, GuiState, INV_H, INV_W, InvAction, Layout, Painter, ScreenCtx, SlotStack, Snapshot,
    dim_background, draw_labels, hovering, slots, window_origin,
};
use crate::gui::tooltip;
use crate::text::{Span, Style};
use crate::util::javarandom::JavaRandom;

pub fn draw(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let (left, top) = window_origin(Layout::Enchantment, ctx);
    dim_background(p, ctx.vw, ctx.vh);
    p.blit_sheet(
        "container/enchanting_table",
        0.0,
        0.0,
        INV_W,
        INV_H,
        left,
        top,
    );

    let seed = snap.container_data[3] as i32;
    let gold_count = snap.menu_slots.get(1).map_or(0, |s| s.count) as i32;
    let infinite_materials = snap.is_creative();
    let mouse = ctx.mouse();

    for i in 0..3usize {
        let row_y = top + 14.0 + 19.0 * i as f32;
        let cost = snap.container_data[i] as i32;

        if cost == 0 {
            p.sprite(
                "container/enchanting_table/enchantment_slot_disabled",
                left + 60.0,
                row_y,
                108.0,
                19.0,
            );
            continue;
        }

        let cost_digits = Digits::new(cost as u32);
        let cost_w = p.atlas.font.width_str(cost_digits.as_str());
        let text_width = 86.0 - cost_w;
        let name = enchantment_name(seed, i, text_width, |s| p.sga_width(s));

        let cannot_afford =
            (gold_count < i as i32 + 1 || snap.xp_level < cost as u32) && !infinite_materials;
        let hovered = !cannot_afford && hovering(ctx, left + 60.0, row_y, 108.0, 19.0);

        let (slot_sprite, level_sprite, name_rgb) = if cannot_afford {
            (
                "container/enchanting_table/enchantment_slot_disabled",
                DISABLED_LEVEL_SPRITES[i],
                DISABLED_NAME_RGB,
            )
        } else if hovered {
            (
                "container/enchanting_table/enchantment_slot_highlighted",
                ENABLED_LEVEL_SPRITES[i],
                HOVERED_NAME_RGB,
            )
        } else {
            (
                "container/enchanting_table/enchantment_slot",
                ENABLED_LEVEL_SPRITES[i],
                PLAIN_NAME_RGB,
            )
        };
        let cost_rgb = if cannot_afford {
            DISABLED_COST_RGB
        } else {
            ENABLED_COST_RGB
        };

        p.sprite(slot_sprite, left + 60.0, row_y, 108.0, 19.0);
        p.sprite(level_sprite, left + 61.0, row_y + 1.0, 16.0, 16.0);
        p.text_sga(name.as_str(), left + 80.0, row_y + 2.0, name_rgb);
        p.text_plain(
            cost_digits.as_str(),
            left + 80.0 + 86.0 - cost_w,
            row_y + 2.0 + 7.0,
            cost_rgb,
            false,
        );

        if hovered && ctx.input.left_click {
            out.push(InvAction::ButtonClick(i as u8));
        }

        let level_clue = snap.container_data[7 + i];
        let enchant_clue = snap.container_data[4 + i];
        if hovered && level_clue >= 0 && enchant_clue >= 0 {
            if let Some(m) = mouse {
                draw_clue_tooltip(
                    p,
                    ctx,
                    m.x,
                    m.y,
                    enchant_clue,
                    snap.enchant_clues[i].as_deref(),
                    level_clue as i32,
                    i as i32 + 1,
                    cost,
                    gold_count,
                    infinite_materials,
                    snap.xp_level as i32,
                );
            }
        }
    }

    let hovered_stack = slots::panel(
        p,
        Layout::Enchantment,
        (left, top),
        ctx,
        snap,
        &mut state.slots,
        out,
    );
    draw_labels(p, snap, left, top, INV_H, 8.0);
    hovered_stack
}

const DISABLED_NAME_RGB: u32 = (((-9937334i32 as u32) & 16711422) >> 1) & 0x00FF_FFFF;

const PLAIN_NAME_RGB: u32 = (-9937334i32 as u32) & 0x00FF_FFFF;
const DISABLED_COST_RGB: u32 = (-12550384i32 as u32) & 0x00FF_FFFF;
const HOVERED_NAME_RGB: u32 = (-128i32 as u32) & 0x00FF_FFFF;
const ENABLED_COST_RGB: u32 = (-8323296i32 as u32) & 0x00FF_FFFF;

const ENABLED_LEVEL_SPRITES: [&str; 3] = [
    "container/enchanting_table/level_1",
    "container/enchanting_table/level_2",
    "container/enchanting_table/level_3",
];
const DISABLED_LEVEL_SPRITES: [&str; 3] = [
    "container/enchanting_table/level_1_disabled",
    "container/enchanting_table/level_2_disabled",
    "container/enchanting_table/level_3_disabled",
];

const WORDS: [&str; 62] = [
    "the",
    "elder",
    "scrolls",
    "klaatu",
    "berata",
    "niktu",
    "xyzzy",
    "bless",
    "curse",
    "light",
    "darkness",
    "fire",
    "air",
    "earth",
    "water",
    "hot",
    "dry",
    "cold",
    "wet",
    "ignite",
    "snuff",
    "embiggen",
    "twist",
    "shorten",
    "stretch",
    "fiddle",
    "destroy",
    "imbue",
    "galvanize",
    "enchant",
    "free",
    "limited",
    "range",
    "of",
    "towards",
    "inside",
    "sphere",
    "cube",
    "self",
    "other",
    "ball",
    "mental",
    "physical",
    "grow",
    "shrink",
    "demon",
    "elemental",
    "spirit",
    "animal",
    "creature",
    "beast",
    "humanoid",
    "undead",
    "fresh",
    "stale",
    "phnglui",
    "mglwnafh",
    "cthulhu",
    "rlyeh",
    "wgahnagl",
    "fhtagn",
    "baguette",
];

const NAME_CAP: usize = 64;

pub struct NameBuf {
    buf: [u8; NAME_CAP],
    len: usize,
}

impl NameBuf {
    fn new() -> NameBuf {
        NameBuf {
            buf: [0; NAME_CAP],
            len: 0,
        }
    }

    fn push_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let room = NAME_CAP - self.len;
        let n = bytes.len().min(room);
        self.buf[self.len..self.len + n].copy_from_slice(&bytes[..n]);
        self.len += n;
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

    fn truncate_to_width(&mut self, max_width: f32, width_of: &impl Fn(&str) -> f32) {
        while self.len > 0 && width_of(self.as_str()) > max_width {
            self.len -= 1;
        }
    }
}

fn enchantment_name(
    seed: i32,
    row: usize,
    max_width: f32,
    width_of: impl Fn(&str) -> f32,
) -> NameBuf {
    let mut rng = JavaRandom::new(seed as i64);
    let mut name = NameBuf::new();
    for _ in 0..=row {
        name = NameBuf::new();
        let word_count = rng.next_int(2) + 3;
        for i in 0..word_count {
            if i != 0 {
                name.push_str(" ");
            }
            let idx = rng.next_int(WORDS.len() as u32) as usize;
            name.push_str(WORDS[idx]);
        }
    }
    name.truncate_to_width(max_width, &width_of);
    name
}

#[allow(clippy::too_many_arguments)]
fn draw_clue_tooltip(
    p: &mut Painter,
    ctx: &ScreenCtx,
    mx: f32,
    my: f32,
    enchant_id: i16,
    clue_name: Option<&str>,
    level: i32,
    cost: i32,
    min_level: i32,
    gold_count: i32,
    infinite_materials: bool,
    xp_level: i32,
) {
    const WHITE: u32 = 0xFFFFFF;
    const GRAY: u32 = 0xAAAAAA;
    const RED: u32 = 0xFF5555;

    let mut full_name = String::new();
    match clue_name {
        Some(name) => full_name.push_str(&translate_enchantment(name)),
        None => {
            full_name.push_str("Enchantment #");
            full_name.push_str(Digits::new(enchant_id.max(0) as u32).as_str());
        }
    }
    if level != 1 {
        full_name.push(' ');
        full_name.push_str(&roman(level));
    }
    let clue = tooltip::translate("container.enchant.clue", &[full_name]);
    let mut lines = vec![vec![Span {
        text: clue,
        style: Style {
            color: WHITE,
            ..Style::default()
        },
    }]];

    if !infinite_materials {
        lines.push(vec![]);
        if xp_level < min_level {
            let text = tooltip::translate(
                "container.enchant.level.requirement",
                &[Digits::new(min_level as u32).as_str().to_string()],
            );
            lines.push(vec![Span {
                text,
                style: Style {
                    color: RED,
                    ..Style::default()
                },
            }]);
        } else {
            let lapis_key = if cost == 1 {
                "container.enchant.lapis.one"
            } else {
                "container.enchant.lapis.many"
            };
            let lapis_args: &[String] = if cost == 1 {
                &[]
            } else {
                &[Digits::new(cost as u32).as_str().to_string()]
            };
            let lapis_color = if gold_count >= cost { GRAY } else { RED };
            lines.push(vec![Span {
                text: tooltip::translate(lapis_key, lapis_args),
                style: Style {
                    color: lapis_color,
                    ..Style::default()
                },
            }]);

            let level_key = if cost == 1 {
                "container.enchant.level.one"
            } else {
                "container.enchant.level.many"
            };
            let level_args: &[String] = if cost == 1 {
                &[]
            } else {
                &[Digits::new(cost as u32).as_str().to_string()]
            };
            lines.push(vec![Span {
                text: tooltip::translate(level_key, level_args),
                style: Style {
                    color: GRAY,
                    ..Style::default()
                },
            }]);
        }
    }

    tooltip::draw_lines(p, &lines, mx, my, ctx.vw, ctx.vh);
}

fn translate_enchantment(id: &str) -> String {
    let mut key = String::with_capacity(22 + id.len());
    key.push_str("enchantment.minecraft.");
    key.push_str(id);
    tooltip::translate(&key, &[])
}

fn roman(n: i32) -> String {
    if n <= 0 || n > 3999 {
        return n.to_string();
    }
    const TABLE: [(i32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut n = n;
    let mut out = String::new();
    for (value, sym) in TABLE {
        while n >= value {
            out.push_str(sym);
            n -= value;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_colors_convert_correctly() {
        assert_eq!(DISABLED_NAME_RGB, 0x342F25);
        assert_eq!(DISABLED_COST_RGB, 0x407F10);
        assert_eq!(HOVERED_NAME_RGB, 0xFFFF80);
        assert_eq!(ENABLED_COST_RGB, 0x80FF20);
    }

    #[test]
    fn a_seed_and_row_always_pick_the_same_name() {
        let a = enchantment_name(12345, 1, 1000.0, |_| 0.0);
        let b = enchantment_name(12345, 1, 1000.0, |_| 0.0);
        assert_eq!(a.as_str(), b.as_str());
    }

    #[test]
    fn later_rows_consume_more_of_the_stream() {
        let row0 = enchantment_name(999, 0, 1000.0, |_| 0.0);
        let row2 = enchantment_name(999, 2, 1000.0, |_| 0.0);
        assert_ne!(row0.as_str(), row2.as_str());
    }

    #[test]
    fn names_have_three_or_four_words() {
        for seed in [0, 1, -1, 42, i32::MAX, i32::MIN] {
            let n = enchantment_name(seed, 0, 1000.0, |_| 0.0);
            let words = n.as_str().split(' ').count();
            assert!((3..=4).contains(&words), "seed {seed} gave {words} words");
        }
    }

    #[test]
    fn truncation_stops_at_the_requested_width() {
        let n = enchantment_name(7, 0, 5.0, |s| s.len() as f32);
        assert!(n.as_str().len() <= 5);
    }

    #[test]
    fn roman_numerals_match_the_usual_table() {
        assert_eq!(roman(1), "I");
        assert_eq!(roman(4), "IV");
        assert_eq!(roman(9), "IX");
        assert_eq!(roman(40), "XL");
    }
}
