use crate::gui::Snapshot;
use crate::gui::painter::Painter;
use crate::session::HeartKind;
use crate::util::javarandom::JavaRandom;

const HEART: f32 = 9.0;
const HEART_STRIDE: f32 = 8.0;
const SEED_STRIDE: i32 = 312_871;

#[derive(Clone, Copy)]
pub struct HealthState {
    last_value: i32,
    pub displayed_value: i32,
    last_update_tick: u64,
    blink_until_tick: u64,
}

impl HealthState {
    const DISPLAY_UPDATE_DELAY: u64 = 20;
    const DECREASE_BLINK_DURATION: u64 = 20;
    const INCREASE_BLINK_DURATION: u64 = 10;

    pub fn new(value: i32) -> HealthState {
        HealthState {
            last_value: value,
            displayed_value: value,
            last_update_tick: 0,
            blink_until_tick: 0,
        }
    }

    pub fn update(&mut self, value: i32, tick: u64) {
        if value != self.last_value {
            let duration = if value < self.last_value {
                Self::DECREASE_BLINK_DURATION
            } else {
                Self::INCREASE_BLINK_DURATION
            };
            self.blink_until_tick = tick + duration;
            self.last_value = value;
            self.last_update_tick = tick;
        }
        if tick - self.last_update_tick > Self::DISPLAY_UPDATE_DELAY {
            self.displayed_value = value;
        }
    }

    pub fn is_blinking(&self, tick: u64) -> bool {
        self.blink_until_tick > tick && (self.blink_until_tick - tick) % 6 >= 3
    }
}

const CONTAINER_SPRITES: [&str; 8] = [
    "hud/heart/container",
    "hud/heart/container_blinking",
    "hud/heart/container",
    "hud/heart/container_blinking",
    "hud/heart/container_hardcore",
    "hud/heart/container_hardcore_blinking",
    "hud/heart/container_hardcore",
    "hud/heart/container_hardcore_blinking",
];

const NORMAL_SPRITES: [&str; 8] = [
    "hud/heart/full",
    "hud/heart/full_blinking",
    "hud/heart/half",
    "hud/heart/half_blinking",
    "hud/heart/hardcore_full",
    "hud/heart/hardcore_full_blinking",
    "hud/heart/hardcore_half",
    "hud/heart/hardcore_half_blinking",
];

const POISONED_SPRITES: [&str; 8] = [
    "hud/heart/poisoned_full",
    "hud/heart/poisoned_full_blinking",
    "hud/heart/poisoned_half",
    "hud/heart/poisoned_half_blinking",
    "hud/heart/poisoned_hardcore_full",
    "hud/heart/poisoned_hardcore_full_blinking",
    "hud/heart/poisoned_hardcore_half",
    "hud/heart/poisoned_hardcore_half_blinking",
];

const WITHERED_SPRITES: [&str; 8] = [
    "hud/heart/withered_full",
    "hud/heart/withered_full_blinking",
    "hud/heart/withered_half",
    "hud/heart/withered_half_blinking",
    "hud/heart/withered_hardcore_full",
    "hud/heart/withered_hardcore_full_blinking",
    "hud/heart/withered_hardcore_half",
    "hud/heart/withered_hardcore_half_blinking",
];

const ABSORBING_SPRITES: [&str; 8] = [
    "hud/heart/absorbing_full",
    "hud/heart/absorbing_full_blinking",
    "hud/heart/absorbing_half",
    "hud/heart/absorbing_half_blinking",
    "hud/heart/absorbing_hardcore_full",
    "hud/heart/absorbing_hardcore_full_blinking",
    "hud/heart/absorbing_hardcore_half",
    "hud/heart/absorbing_hardcore_half_blinking",
];

const FROZEN_SPRITES: [&str; 8] = [
    "hud/heart/frozen_full",
    "hud/heart/frozen_full_blinking",
    "hud/heart/frozen_half",
    "hud/heart/frozen_half_blinking",
    "hud/heart/frozen_hardcore_full",
    "hud/heart/frozen_hardcore_full_blinking",
    "hud/heart/frozen_hardcore_half",
    "hud/heart/frozen_hardcore_half_blinking",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum HeartSprite {
    Container,
    Kind(HeartKind),
    Absorbing,
}

fn heart_sprite(sprite: HeartSprite, hardcore: bool, half: bool, blinking: bool) -> &'static str {
    let table = match sprite {
        HeartSprite::Container => &CONTAINER_SPRITES,
        HeartSprite::Absorbing => &ABSORBING_SPRITES,
        HeartSprite::Kind(HeartKind::Normal) => &NORMAL_SPRITES,
        HeartSprite::Kind(HeartKind::Poisoned) => &POISONED_SPRITES,
        HeartSprite::Kind(HeartKind::Withered) => &WITHERED_SPRITES,
        HeartSprite::Kind(HeartKind::Frozen) => &FROZEN_SPRITES,
    };
    table[(hardcore as usize) << 2 | (half as usize) << 1 | blinking as usize]
}

#[derive(Clone, Copy)]
pub struct HeartAnim {
    state: HealthState,
    epoch: u32,
}

impl Default for HeartAnim {
    fn default() -> Self {
        HeartAnim {
            state: HealthState::new(0),
            epoch: 0,
        }
    }
}

impl HeartAnim {
    pub fn frame(&mut self, snap: &Snapshot, tick: u64) -> (Rows, bool) {
        let current = snap.health.max(0.0).ceil() as i32;
        if self.epoch != snap.health_epoch {
            self.epoch = snap.health_epoch;
            self.state = HealthState::new(current);
        }
        let blink = self.state.is_blinking(tick);
        self.state.update(current, tick);
        (Rows::new(snap, self.state.displayed_value, tick), blink)
    }
}

#[derive(Clone, Copy)]
pub struct Rows {
    pub current: i32,
    pub old: i32,
    pub absorption: i32,
    pub max_health: f32,
    pub rows: i32,
    pub row_height: f32,
    pub regen_index: i32,
}

impl Rows {
    pub fn new(snap: &Snapshot, displayed: i32, tick: u64) -> Rows {
        let d = &snap.health_display;
        let current = snap.health.max(0.0).ceil() as i32;
        let max_health = d.max_health.max(displayed.max(current) as f32);
        let absorption = d.absorption.max(0.0).ceil() as i32;
        let rows = ((max_health + absorption as f32) / 20.0).ceil() as i32;
        Rows {
            current,
            old: displayed,
            absorption,
            max_health,
            rows,
            row_height: (10 - (rows - 2)).max(3) as f32,
            regen_index: if d.regenerating {
                (tick % (max_health + 5.0).ceil().max(1.0) as u64) as i32
            } else {
                -1
            },
        }
    }
}

pub fn draw_hearts(
    p: &mut Painter,
    snap: &Snapshot,
    rows: &Rows,
    blink: bool,
    left: f32,
    top: f32,
    rng: &mut JavaRandom,
) {
    let kind = HeartSprite::Kind(snap.health_display.kind);
    let hardcore = snap.hardcore;
    let health_containers = (rows.max_health / 2.0).ceil() as i32;
    let absorption_containers = (rows.absorption as f32 / 2.0).ceil() as i32;
    let max_health_halves = health_containers * 2;

    for index in (0..health_containers + absorption_containers).rev() {
        let x = left + (index % 10) as f32 * HEART_STRIDE;
        let mut y = top - (index / 10) as f32 * rows.row_height;
        if rows.current + rows.absorption <= 4 {
            y += rng.next_int(2) as f32;
        }
        if index < health_containers && index == rows.regen_index {
            y -= 2.0;
        }

        let sprite = heart_sprite(HeartSprite::Container, hardcore, false, blink);
        p.sprite(sprite, x, y, HEART, HEART);

        let halves = index * 2;
        if index >= health_containers {
            let absorbed = halves - max_health_halves;
            if absorbed < rows.absorption {
                let sprite = if snap.health_display.kind == HeartKind::Withered {
                    kind
                } else {
                    HeartSprite::Absorbing
                };
                let half = absorbed + 1 == rows.absorption;
                p.sprite(
                    heart_sprite(sprite, hardcore, half, false),
                    x,
                    y,
                    HEART,
                    HEART,
                );
            }
        }

        if blink && halves < rows.old {
            let half = halves + 1 == rows.old;
            p.sprite(heart_sprite(kind, hardcore, half, true), x, y, HEART, HEART);
        }
        if halves < rows.current {
            let half = halves + 1 == rows.current;
            p.sprite(
                heart_sprite(kind, hardcore, half, false),
                x,
                y,
                HEART,
                HEART,
            );
        }
    }
}

pub fn draw_armor(p: &mut Painter, armor: u8, rows: &Rows, left: f32, top: f32) {
    if armor == 0 {
        return;
    }
    let armor = armor as i32;
    let y = top - (rows.rows - 1) as f32 * rows.row_height - 10.0;
    for i in 0..10 {
        let x = left + i as f32 * HEART_STRIDE;
        let sprite = match (i * 2 + 1).cmp(&armor) {
            std::cmp::Ordering::Less => "hud/armor_full",
            std::cmp::Ordering::Equal => "hud/armor_half",
            std::cmp::Ordering::Greater => "hud/armor_empty",
        };
        p.sprite(sprite, x, y, HEART, HEART);
    }
}

pub fn draw_food(
    p: &mut Painter,
    snap: &Snapshot,
    right: f32,
    top: f32,
    tick: u64,
    rng: &mut JavaRandom,
) {
    let food = snap.food as i32;
    let (empty, half, full) = if snap.health_display.hunger_effect {
        (
            "hud/food_empty_hunger",
            "hud/food_half_hunger",
            "hud/food_full_hunger",
        )
    } else {
        ("hud/food_empty", "hud/food_half", "hud/food_full")
    };
    let shakes = snap.saturation <= 0.0 && tick % (food * 3 + 1).max(1) as u64 == 0;
    for i in 0..10 {
        let x = right - i as f32 * HEART_STRIDE - HEART;
        let y = if shakes {
            top + rng.next_int(3) as f32 - 1.0
        } else {
            top
        };
        p.sprite(empty, x, y, HEART, HEART);
        if i * 2 + 1 < food {
            p.sprite(full, x, y, HEART, HEART);
        } else if i * 2 + 1 == food {
            p.sprite(half, x, y, HEART, HEART);
        }
    }
}

pub fn vehicle_hearts(max_health: f32) -> i32 {
    ((max_health + 0.5) as i32 / 2).min(30)
}

pub fn vehicle_rows(hearts: i32) -> i32 {
    (hearts as f32 / 10.0).ceil() as i32
}

pub fn draw_vehicle_hearts(p: &mut Painter, health: f32, mut hearts: i32, right: f32, top: f32) {
    let current = health.max(0.0).ceil() as i32;
    let mut y = top;
    let mut base = 0;
    while hearts > 0 {
        let row_hearts = hearts.min(10);
        hearts -= row_hearts;
        for i in 0..row_hearts {
            let x = right - i as f32 * HEART_STRIDE - HEART;
            p.sprite("hud/heart/vehicle_container", x, y, HEART, HEART);
            let halves = i * 2 + 1 + base;
            if halves < current {
                p.sprite("hud/heart/vehicle_full", x, y, HEART, HEART);
            } else if halves == current {
                p.sprite("hud/heart/vehicle_half", x, y, HEART, HEART);
            }
        }
        y -= 10.0;
        base += 20;
    }
}

pub fn air_row_y(base: f32, vehicle_hearts: i32) -> f32 {
    let food_row = if vehicle_hearts == 0 { 10.0 } else { 0.0 };
    base - 10.0 - food_row - (vehicle_rows(vehicle_hearts) - 1) as f32 * 10.0
}

pub fn draw_freeze_overlay(p: &mut Painter, snap: &Snapshot, vw: f32, vh: f32) {
    let ticks = snap.health_display.ticks_frozen;
    if ticks <= 0 {
        return;
    }
    let percent =
        (ticks as f32 / crate::play::entity_feed::TICKS_REQUIRED_TO_FREEZE as f32).min(1.0);
    let alpha = (percent * 255.0) as u32;
    p.sprite_tinted(
        "powder_snow_outline",
        0.0,
        0.0,
        vw,
        vh,
        alpha << 24 | 0x00FF_FFFF,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    p: &mut Painter,
    snap: &Snapshot,
    rows: &Rows,
    blink: bool,
    tick: u64,
    left: f32,
    right: f32,
    base: f32,
    survival: bool,
    rng: &mut JavaRandom,
) -> i32 {
    let vehicle = snap.health_display.vehicle;
    let hearts = vehicle.map_or(0, |(_, max)| vehicle_hearts(max));

    if survival {
        draw_armor(p, snap.health_display.armor, rows, left, base);
        draw_hearts(p, snap, rows, blink, left, base, rng);
        if hearts == 0 {
            draw_food(p, snap, right, base, tick, rng);
        }
    }
    if let Some((health, _)) = vehicle
        && hearts > 0
    {
        draw_vehicle_hearts(p, health, hearts, right, base);
    }
    hearts
}

pub fn frame_random(tick: u64) -> JavaRandom {
    JavaRandom::new((tick as i32).wrapping_mul(SEED_STRIDE) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprite_table_matches_vanilla_order() {
        let kind = HeartSprite::Kind(HeartKind::Normal);
        assert_eq!(heart_sprite(kind, false, false, false), "hud/heart/full");
        assert_eq!(
            heart_sprite(kind, false, true, true),
            "hud/heart/half_blinking"
        );
        assert_eq!(
            heart_sprite(kind, true, true, false),
            "hud/heart/hardcore_half"
        );
        assert_eq!(
            heart_sprite(HeartSprite::Absorbing, true, false, true),
            "hud/heart/absorbing_hardcore_full_blinking"
        );
        assert_eq!(
            heart_sprite(HeartSprite::Container, false, true, false),
            heart_sprite(HeartSprite::Container, false, false, false)
        );
    }

    #[test]
    fn rows_tighten_past_two() {
        let row = |max: f32, absorption: f32| {
            let rows = ((max + absorption) / 20.0).ceil() as i32;
            ((10 - (rows - 2)).max(3), rows)
        };
        assert_eq!(row(20.0, 0.0), (11, 1));
        assert_eq!(row(40.0, 0.0), (10, 2));
        assert_eq!(row(40.0, 10.0), (9, 3));
        assert_eq!(row(200.0, 0.0), (3, 10));
    }

    #[test]
    fn air_line_sits_one_row_above() {
        let base = 100.0;
        assert_eq!(air_row_y(base, 0), 90.0);
        assert_eq!(air_row_y(base, 10), 90.0);
        assert_eq!(air_row_y(base, 11), 80.0);
        assert_eq!(air_row_y(base, 30), 70.0);
    }

    #[test]
    fn vehicle_hearts_round_and_cap() {
        assert_eq!(vehicle_hearts(20.0), 10);
        assert_eq!(vehicle_hearts(15.0), 7);
        assert_eq!(vehicle_hearts(53.0), 26);
        assert_eq!(vehicle_hearts(200.0), 30);
    }

    #[test]
    fn damage_blinks_then_settles() {
        let mut state = HealthState::new(20);
        state.update(14, 100);
        assert!(!state.is_blinking(100));
        assert!(state.is_blinking(103));
        assert_eq!(state.displayed_value, 20);
        assert!(!state.is_blinking(120));
        state.update(14, 121);
        assert_eq!(state.displayed_value, 14);

        state.update(20, 200);
        assert!(state.is_blinking(205));
        assert!(!state.is_blinking(208));
        assert!(!state.is_blinking(210));
    }
}
