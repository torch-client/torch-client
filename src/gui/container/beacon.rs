use super::{
    BeaconPick, GuiState, InvAction, Layout, Painter, ScreenCtx, SlotStack, Snapshot, button,
    dim_background, slots, window_origin,
};
use crate::gui::tooltip;
use crate::play::mob_effects;
use crate::text::{Span, Style};
use azalea_registry::Registry;
use azalea_registry::builtin::MobEffect;
use std::str::FromStr;

const LABEL_RGB: u32 = (-2039584i32 as u32) & 0x00FF_FFFF;

const TIER_EFFECTS: [&[(&str, &str)]; 3] = [
    &[("speed", "mob_effect/speed"), ("haste", "mob_effect/haste")],
    &[
        ("resistance", "mob_effect/resistance"),
        ("jump_boost", "mob_effect/jump_boost"),
    ],
    &[("strength", "mob_effect/strength")],
];
const TIER3_EFFECT: (&str, &str) = ("regeneration", "mob_effect/regeneration");

pub fn draw(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let (left, top) = window_origin(Layout::Beacon, ctx);
    dim_background(p, ctx.vw, ctx.vh);
    p.blit_sheet("container/beacon", 0.0, 0.0, 230.0, 219.0, left, top);

    draw_centered_label(p, primary_label(), left + 62.0, top + 10.0);
    draw_centered_label(p, secondary_label(), left + 169.0, top + 10.0);

    for (id, x) in [
        ("netherite_ingot", 20.0),
        ("emerald", 41.0),
        ("diamond", 63.0),
        ("gold_ingot", 86.0),
        ("iron_ingot", 108.0),
    ] {
        p.item_icon(id, left + x, top + 109.0);
    }

    let levels = snap.container_data[0] as i32;
    let mouse = ctx.mouse();
    let ui = &mut state.container;

    if ui.beacon == BeaconPick::default() {
        ui.beacon.primary = decode_effect(snap.container_data[1]);
        ui.beacon.secondary = decode_effect(snap.container_data[2]);
    }

    for (tier, effects) in TIER_EFFECTS.iter().enumerate() {
        let total_w = row_width(effects.len());
        for (c, &(name, sprite)) in effects.iter().enumerate() {
            let x = left + 76.0 + c as f32 * 24.0 - total_w / 2.0;
            let y = top + 22.0 + tier as f32 * 25.0;
            let id = effect_id(name);
            let active = (tier as i32) < levels;
            let selected = ui.beacon.primary == id;
            if power_button(p, ctx, mouse, x, y, active, selected, sprite, name, false) {
                ui.beacon.primary = id;
            }
        }
    }

    let total_w = row_width(2);
    let y = top + 47.0;
    {
        let x = left + 167.0 - total_w / 2.0;
        let id = effect_id(TIER3_EFFECT.0);
        let active = levels >= 4;
        let selected = ui.beacon.secondary == id;
        if power_button(
            p,
            ctx,
            mouse,
            x,
            y,
            active,
            selected,
            TIER3_EFFECT.1,
            TIER3_EFFECT.0,
            false,
        ) {
            ui.beacon.secondary = id;
        }
    }
    if let Some(primary) = ui.beacon.primary
        && let Some((name, sprite)) = effect_by_id(primary)
    {
        let x = left + 167.0 + 24.0 - total_w / 2.0;
        let active = levels >= 4;
        let selected = ui.beacon.secondary == Some(primary);
        if power_button(p, ctx, mouse, x, y, active, selected, sprite, name, true) {
            ui.beacon.secondary = Some(primary);
        }
    }

    let confirm_active = filled(snap, 0) && ui.beacon.primary.is_some();
    if action_button(
        p,
        ctx,
        left + 164.0,
        top + 107.0,
        confirm_active,
        "container/beacon/confirm",
    ) {
        out.push(InvAction::SetBeacon {
            primary: to_effect_option(ui.beacon.primary),
            secondary: to_effect_option(ui.beacon.secondary),
        });
        out.push(InvAction::Close);
    }
    if action_button(
        p,
        ctx,
        left + 190.0,
        top + 107.0,
        true,
        "container/beacon/cancel",
    ) {
        out.push(InvAction::Close);
    }

    slots::panel(
        p,
        Layout::Beacon,
        (left, top),
        ctx,
        snap,
        &mut state.slots,
        out,
    )
}

#[allow(clippy::too_many_arguments)]
fn power_button(
    p: &mut Painter,
    ctx: &ScreenCtx,
    mouse: Option<bevy::prelude::Vec2>,
    x: f32,
    y: f32,
    active: bool,
    selected: bool,
    icon: &str,
    effect_name: &str,
    is_upgrade: bool,
) -> bool {
    let (hovered, clicked) = button(ctx, x, y, 22.0, 22.0);
    p.sprite(button_sprite(active, selected, hovered), x, y, 22.0, 22.0);
    p.sprite(icon, x + 2.0, y + 2.0, 18.0, 18.0);

    if active
        && hovered
        && let Some(m) = mouse
    {
        let mut name = mob_effects::display_name(effect_name, 0);
        if is_upgrade {
            name.push_str(" II");
        }
        let line = vec![Span {
            text: name,
            style: Style::default(),
        }];
        tooltip::draw_lines(p, &[line], m.x, m.y, ctx.vw, ctx.vh);
    }

    active && clicked
}

fn action_button(
    p: &mut Painter,
    ctx: &ScreenCtx,
    x: f32,
    y: f32,
    active: bool,
    icon: &str,
) -> bool {
    let (hovered, clicked) = button(ctx, x, y, 22.0, 22.0);
    p.sprite(button_sprite(active, false, hovered), x, y, 22.0, 22.0);
    p.sprite(icon, x + 2.0, y + 2.0, 18.0, 18.0);
    active && clicked
}

fn button_sprite(active: bool, selected: bool, hovered: bool) -> &'static str {
    if !active {
        "container/beacon/button_disabled"
    } else if selected {
        "container/beacon/button_selected"
    } else if hovered {
        "container/beacon/button_highlighted"
    } else {
        "container/beacon/button"
    }
}

fn effect_id(name: &str) -> Option<i16> {
    MobEffect::from_str(name).ok().map(|e| e.to_u32() as i16)
}

fn effect_by_id(id: i16) -> Option<(&'static str, &'static str)> {
    TIER_EFFECTS
        .iter()
        .flat_map(|tier| tier.iter())
        .chain(std::iter::once(&TIER3_EFFECT))
        .find(|&&(name, _)| effect_id(name) == Some(id))
        .copied()
}

fn decode_effect(raw: i16) -> Option<i16> {
    (raw > 0).then(|| raw - 1)
}

fn to_effect_option(id: Option<i16>) -> Option<u32> {
    id.filter(|&id| id >= 0).map(|id| id as u32)
}

fn filled(snap: &Snapshot, i: usize) -> bool {
    snap.menu_slots.get(i).is_some_and(|s| !s.is_empty())
}

fn row_width(count: usize) -> f32 {
    count as f32 * 22.0 + (count as f32 - 1.0) * 2.0
}

fn draw_centered_label(p: &mut Painter, text: &str, cx: f32, y: f32) {
    let w = p.atlas.font.width_str(text);
    p.text_plain(text, (cx - w / 2.0).floor(), y, LABEL_RGB, false);
}

fn primary_label() -> &'static str {
    use std::sync::OnceLock;
    static LABEL: OnceLock<String> = OnceLock::new();
    LABEL.get_or_init(|| tooltip::translate("block.minecraft.beacon.primary", &[]))
}

fn secondary_label() -> &'static str {
    use std::sync::OnceLock;
    static LABEL: OnceLock<String> = OnceLock::new();
    LABEL.get_or_init(|| tooltip::translate("block.minecraft.beacon.secondary", &[]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_label_colour_matches_java() {
        assert_eq!(LABEL_RGB, 0xE0E0E0);
    }

    #[test]
    fn row_width_matches_totalwidth() {
        assert_eq!(row_width(1), 22.0);
        assert_eq!(row_width(2), 46.0);
    }

    #[test]
    fn effect_ids_round_trip() {
        for tier in TIER_EFFECTS {
            for &(name, _) in tier {
                let id = effect_id(name).unwrap_or_else(|| panic!("{name} did not resolve"));
                assert!(id >= 0, "{name} resolved to a negative id");
                assert_eq!(effect_by_id(id).map(|(n, _)| n), Some(name));
            }
        }
        assert_eq!(effect_by_id(-1), None);
    }

    #[test]
    fn decode_effect_undoes_the_plus_one_encoding() {
        assert_eq!(decode_effect(0), None);
        assert_eq!(decode_effect(1), Some(0));
        assert_eq!(decode_effect(5), Some(4));
    }

    #[test]
    fn no_effect_is_none_on_the_wire() {
        assert_eq!(to_effect_option(None), None);
        let speed = effect_id("speed");
        assert_eq!(to_effect_option(speed), speed.map(|id| id as u32));
        assert!(speed.is_some());
    }
}
