use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

use azalea_inventory::ItemStack;
use azalea_inventory::components::{Consumable, Food};
use azalea_inventory::default_components::get_default_component;
use azalea_inventory::item::consume_effect::ConsumeEffect;
use azalea_registry::builtin::ItemKind;
use bevy::input::ButtonInput;
use bevy::prelude::{KeyCode, MouseButton};

use super::registry::{Id, better_tooltips as setting, options};
use super::store;
use crate::gui::keybinds::Bound;
use crate::gui::painter::{Painter, argb};
use crate::play::mob_effects;
use crate::session::SlotStack;
use crate::text::Font;

pub mod preview;
pub mod rows;

pub use preview::Preview;

options! {
    pub enum DisplayWhen {
        Keybind = "Keybind",
        Always = "Always",
    }
}

pub const DEFAULT_DISPLAY: DisplayWhen = if cfg!(any(target_arch = "wasm32", feature = "mobile_ui"))
{
    DisplayWhen::Always
} else {
    DisplayWhen::Keybind
};

const MAP_EDGE: f32 = crate::client::tracking::MAP_EDGE as f32;

static HELD: AtomicBool = AtomicBool::new(false);

pub fn poll_key(keys: &ButtonInput<KeyCode>, buttons: &ButtonInput<MouseButton>, elsewhere: bool) {
    let s = store();
    let held = s.enabled(Id::BetterTooltips)
        && s.choice(setting::DISPLAY_WHEN) == DisplayWhen::Keybind
        && match s.key(setting::PREVIEW_KEY) {
            Bound::Unbound => false,
            Bound::Key(code) => !elsewhere && keys.pressed(code),
            Bound::Mouse(button) => buttons.pressed(button),
        };
    HELD.store(held, Relaxed);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opts {
    pub previewing: bool,
    pub hint_key: Option<Bound>,
    pub containers: bool,
    pub compact: bool,
    pub books: bool,
    pub banners: bool,
    pub maps: bool,
    pub map_size: f32,
    pub buckets: bool,
    pub effects: bool,
    pub food: bool,
    pub reveal_tooltip: bool,
    pub reveal_components: bool,
}

impl Opts {
    pub const OFF: Opts = Opts {
        previewing: false,
        hint_key: None,
        containers: false,
        compact: false,
        books: false,
        banners: false,
        maps: false,
        map_size: MAP_EDGE,
        buckets: false,
        effects: false,
        food: false,
        reveal_tooltip: false,
        reveal_components: false,
    };

    pub fn now() -> Opts {
        let s = store();
        if !s.enabled(Id::BetterTooltips) {
            return Opts::OFF;
        }
        let (previewing, hint) = mode(s.choice(setting::DISPLAY_WHEN), HELD.load(Relaxed));
        Opts {
            previewing,
            hint_key: hint
                .then(|| s.key(setting::PREVIEW_KEY))
                .filter(|k| !matches!(k, Bound::Unbound)),
            containers: s.flag(setting::CONTAINERS),
            compact: s.flag(setting::COMPACT_CONTAINERS),
            books: s.flag(setting::BOOKS),
            banners: s.flag(setting::BANNERS),
            maps: s.flag(setting::MAPS),
            map_size: (MAP_EDGE * s.num(setting::MAP_SCALE)).round().max(1.0),
            buckets: s.flag(setting::BUCKETS),
            effects: s.flag(setting::STATUS_EFFECTS),
            food: s.flag(setting::FOOD_VALUES),
            reveal_tooltip: s.flag(setting::SHOW_HIDDEN_TOOLTIP),
            reveal_components: s.flag(setting::SHOW_HIDDEN_COMPONENTS),
        }
    }
}

fn mode(display: DisplayWhen, held: bool) -> (bool, bool) {
    match display {
        DisplayWhen::Always => (true, false),
        DisplayWhen::Keybind => (held, !held),
    }
}

pub fn reveals_tooltip() -> bool {
    let s = store();
    s.enabled(Id::BetterTooltips) && s.flag(setting::SHOW_HIDDEN_TOOLTIP)
}

pub fn reveals_components() -> bool {
    let s = store();
    s.enabled(Id::BetterTooltips) && s.flag(setting::SHOW_HIDDEN_COMPONENTS)
}

const SHANK: f32 = 9.0;
const SAT: f32 = 7.0;
const MAX_BARS: i32 = 10;
const LABEL_SCALE: f32 = 0.75;
const LABEL_COLOR: u32 = 0xAAAAAA;
const ROW_STEP: f32 = 10.0;

#[derive(Clone, Debug, PartialEq)]
pub struct FoodRows {
    hunger: i32,
    saturation: f32,
    rotten: bool,
    hunger_bars: i32,
    hunger_text: Option<String>,
    saturation_bars: i32,
    saturation_text: Option<String>,
    width: f32,
}

impl FoodRows {
    pub fn of(stack: &SlotStack, font: &Font) -> Option<FoodRows> {
        let (food, rotten) = match stack.raw.as_deref().and_then(ItemStack::as_present) {
            Some(data) => (
                data.get_unless_removed::<Food>()?.into_owned(),
                data.get_unless_removed::<Consumable>()
                    .is_some_and(|c| rotten(&c)),
            ),
            None => {
                let kind = ItemKind::from_str(stack.item).ok()?;
                (
                    get_default_component::<Food>(kind)?,
                    get_default_component::<Consumable>(kind).is_some_and(|c| rotten(&c)),
                )
            }
        };
        let hunger = food.nutrition;
        let saturation = food.saturation;

        let mut hunger_bars = hunger.checked_abs()?.unsigned_abs().div_ceil(2) as i32;
        if hunger_bars == 0 {
            return None;
        }
        let mut hunger_text = None;
        if hunger_bars > MAX_BARS {
            hunger_text = Some(format!("x{}", hunger.signum() * hunger_bars));
            hunger_bars = 1;
        }

        let mut saturation_bars = (saturation.abs() / 2.0).ceil() as i32;
        let mut saturation_text = None;
        if saturation_bars > MAX_BARS || saturation_bars == 0 {
            let sign = if saturation < 0.0 { -1 } else { 1 };
            saturation_text = Some(format!("x{}", sign * saturation_bars));
            saturation_bars = 1;
        }

        let row = |bars: i32, cell: f32, text: &Option<String>| match text {
            Some(t) => cell + 2.0 * LABEL_SCALE + font.width_str(t) * LABEL_SCALE,
            None => bars as f32 * cell,
        };
        let width = row(hunger_bars, SHANK, &hunger_text)
            .max(row(saturation_bars, SAT, &saturation_text))
            .ceil();

        Some(FoodRows {
            hunger,
            saturation,
            rotten,
            hunger_bars,
            hunger_text,
            saturation_bars,
            saturation_text,
            width,
        })
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn draw(&self, p: &mut Painter, x: f32, y: f32) {
        self.draw_hunger(p, x, y);
        self.draw_saturation(p, x, y + ROW_STEP);
    }

    fn draw_hunger(&self, p: &mut Painter, left: f32, y: f32) {
        let (full, half) = if self.rotten {
            ("hud/food_full_hunger", "hud/food_half_hunger")
        } else {
            ("hud/food_full", "hud/food_half")
        };
        let mut x = left + (self.hunger_bars - 1) as f32 * SHANK;
        for i in (0..self.hunger_bars * 2).step_by(2) {
            p.sprite("hud/food_empty", x, y, SHANK, SHANK);
            if self.hunger > i {
                let sprite = if self.hunger - 1 == i { half } else { full };
                p.sprite(sprite, x, y, SHANK, SHANK);
            }
            x -= SHANK;
        }
        if let Some(text) = &self.hunger_text {
            label(p, text, x + 2.0 * SHANK, y, 2.0);
        }
    }

    fn draw_saturation(&self, p: &mut Painter, left: f32, y: f32) {
        let amount = self.saturation.abs();
        let frames = if self.saturation >= 0.0 {
            &SAT_POSITIVE
        } else {
            &SAT_NEGATIVE
        };
        let mut x = left + (self.saturation_bars - 1) as f32 * SAT;
        for i in (0..self.saturation_bars * 2).step_by(2) {
            let i = i as f32;
            let of_bar = (amount - i) / 2.0;
            let alpha = if amount <= i { 0.5 } else { 1.0 };
            let frame = if of_bar >= 1.0 {
                3
            } else if of_bar > 0.5 {
                2
            } else if of_bar > 0.25 {
                1
            } else if of_bar > 0.0 {
                0
            } else {
                4
            };
            draw_icon(p, &frames[frame], x, y, alpha);
            x -= SAT;
        }
        if let Some(text) = &self.saturation_text {
            label(p, text, x + 2.0 * SAT, y, 1.0);
        }
    }
}

fn label(p: &mut Painter, text: &str, x: f32, y: f32, dy: f32) {
    p.scaled_in(LABEL_SCALE, x, y, |p| {
        p.text_plain(text, 2.0, dy, LABEL_COLOR, false);
    });
}

fn rotten(consumable: &Consumable) -> bool {
    consumable
        .on_consume_effects
        .iter()
        .any(|effect| match effect {
            ConsumeEffect::ApplyEffects { effects, .. } => effects.iter().any(|e| {
                let id = e.id.to_str();
                let id = id.strip_prefix("minecraft:").unwrap_or(id);
                mob_effects::category(id) == mob_effects::Category::Harmful
            }),
            _ => false,
        })
}

type Icon = [&'static str; 7];

const SAT_POSITIVE: [Icon; 5] = [
    [
        "..a....", ".aba...", "abbba..", ".abbc..", "..aabcc", "....dbc", "....dd.",
    ],
    [
        "..a....", ".aba...", "abbbc..", ".abbc..", "..adbcc", "....dbc", "....dd.",
    ],
    [
        "..a....", ".abe...", "abbbc..", ".abbc..", "..ddbcc", "....dbc", "....dd.",
    ],
    [
        "..e....", ".ebe...", "cbbbc..", ".dbbc..", "..ddbcc", "....dbc", "....dd.",
    ],
    EMPTY,
];
const SAT_NEGATIVE: [Icon; 5] = [
    [
        "..a....", ".aba...", "abbba..", ".abbf..", "..aabff", "....fbf", "....ff.",
    ],
    [
        "..a....", ".aba...", "abbbf..", ".abbf..", "..afbff", "....fbf", "....ff.",
    ],
    [
        "..a....", ".abg...", "abbbf..", ".abbf..", "..ffbff", "....fbf", "....ff.",
    ],
    [
        "..g....", ".gbg...", "hbbbf..", ".hbbf..", "..hhbff", "....hbf", "....hh.",
    ],
    EMPTY,
];
const EMPTY: Icon = [
    "..a....", ".aba...", "abbba..", ".abba..", "..aabaa", "....aba", "....aa.",
];

fn palette(c: u8) -> u32 {
    match c {
        b'a' => 0x0A0A0A,
        b'b' => 0x282828,
        b'c' => 0xC2A100,
        b'd' => 0x9F8609,
        b'e' => 0xDDC241,
        b'f' => 0xAB1200,
        b'g' => 0xE01700,
        _ => 0x9B1100,
    }
}

fn draw_icon(p: &mut Painter, icon: &Icon, x: f32, y: f32, alpha: f32) {
    for (row, line) in icon.iter().enumerate() {
        let bytes = line.as_bytes();
        let mut start = 0;
        while start < bytes.len() {
            let c = bytes[start];
            let mut end = start + 1;
            while end < bytes.len() && bytes[end] == c {
                end += 1;
            }
            if c != b'.' {
                let color = argb(alpha, palette(c));
                p.fill(
                    x + start as f32,
                    y + row as f32,
                    (end - start) as f32,
                    1.0,
                    color,
                );
            }
            start = end;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_shows_only_while_the_key_would_change_something() {
        assert_eq!(mode(DisplayWhen::Keybind, false), (false, true));
        assert_eq!(mode(DisplayWhen::Keybind, true), (true, false));
        assert_eq!(mode(DisplayWhen::Always, false), (true, false));
        assert_eq!(mode(DisplayWhen::Always, true), (true, false));
    }

    #[test]
    fn opts_differ_when_the_key_is_pressed() {
        let up = Opts {
            hint_key: Some(Bound::Key(KeyCode::AltLeft)),
            ..Opts::OFF
        };
        let down = Opts {
            previewing: true,
            ..Opts::OFF
        };
        assert_ne!(up, down);
        assert_ne!(
            Opts {
                map_size: 64.0,
                ..Opts::OFF
            },
            Opts::OFF
        );
    }

    #[test]
    fn icons_are_seven_by_seven() {
        for icon in SAT_POSITIVE.iter().chain(SAT_NEGATIVE.iter()) {
            for line in icon {
                assert_eq!(line.len(), 7);
                assert!(
                    line.bytes()
                        .all(|c| c == b'.' || (b'a'..=b'h').contains(&c))
                );
            }
        }
    }
}
