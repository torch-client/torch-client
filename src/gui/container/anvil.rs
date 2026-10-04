use std::borrow::Cow;

use super::{draw_labels, error_icon, window_origin};
use crate::gui::painter::Painter;
use crate::gui::screens::dim_background;
use crate::gui::slots::{self, INV_H, INV_W, Layout};
use crate::gui::tooltip::{self, label};
use crate::gui::widgets::{TextBox, allowed_char};
use crate::gui::{GuiState, ScreenCtx, Snapshot};
use crate::session::{Gamemode, InvAction, SlotStack};

const RESULT: usize = 2;
const MAX_NAME_LENGTH: usize = 50;
const FIELD: (f32, f32, f32, f32) = (62.0, 24.0, 103.0, 12.0);
const TOO_EXPENSIVE: i32 = 40;
const COST_COLOR: u32 = 0x80FF20;
const ERROR_COLOR: u32 = 0xFF6060;

pub struct AnvilUi {
    field: TextBox,
    input: SlotStack,
    sent: Option<String>,
    editable: bool,
    cost_label: Option<(i16, String)>,
}

impl Default for AnvilUi {
    fn default() -> AnvilUi {
        let mut field = TextBox::new(MAX_NAME_LENGTH, "");
        field.color = 0xFFFFFF;
        AnvilUi {
            field,
            input: SlotStack::default(),
            sent: None,
            editable: false,
            cost_label: None,
        }
    }
}

impl AnvilUi {
    pub fn editable(&self) -> bool {
        self.editable
    }
}

pub fn draw(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let (left, top) = window_origin(Layout::Anvil, ctx);
    dim_background(p, ctx.vw, ctx.vh);
    p.blit_sheet("container/anvil", 0.0, 0.0, INV_W, INV_H, left, top);
    let input = snap.slot(0);
    if (!input.is_empty() || !snap.slot(1).is_empty()) && snap.slot(RESULT).is_empty() {
        error_icon(p, "container/anvil/error", left + 99.0, top + 45.0);
    }
    let sprite = if input.is_empty() {
        "container/anvil/text_field_disabled"
    } else {
        "container/anvil/text_field"
    };
    p.sprite(sprite, left + 59.0, top + 20.0, 110.0, 16.0);

    let ui = &mut state.container.anvil;
    ui.input.cooldown = input.cooldown;
    if !input.is_empty() && *input != ui.input {
        ui.input = input.clone();
        let name = hover_name(input);
        ui.field.set_text(&name);
        ui.editable = true;
        on_name_changed(&mut ui.sent, input, &name, &name, out);
    }

    let (fx, fy, fw, fh) = (left + FIELD.0, top + FIELD.1, FIELD.2, FIELD.3);
    ui.field.focused = true;
    if ui.editable {
        ui.field
            .handle_mouse(ctx.input, &p.atlas.font, fx, fy, fw, fh);
        if ui.field.handle_input(ctx.input) {
            let hover = hover_name(input);
            on_name_changed(&mut ui.sent, input, &ui.field.text, &hover, out);
        }
    }
    let frame = p.frame;
    ui.field.draw(p, fx, fy - 2.0, fw, fh, frame);

    let hovered = slots::panel(
        p,
        Layout::Anvil,
        (left, top),
        ctx,
        snap,
        &mut state.slots,
        out,
    );
    let cost = snap.container_data[0];
    let creative = snap.gamemode == Gamemode::Creative;
    if !may_pickup(cost as i32, creative, snap.xp_level) {
        out.retain(|a| !matches!(a, InvAction::Click(op) if slots::takes_slot(op, RESULT as u16)));
    }

    draw_labels(p, snap, left, top, INV_H, 60.0);
    let has_result = !snap.slot(RESULT).is_empty();
    if let Some((line, color)) = cost_line(cost as i32, creative, snap.xp_level, has_result) {
        let text = match line {
            Line::TooExpensive => label!("container.repair.expensive"),
            Line::Cost => cost_text(&mut state.container.anvil.cost_label, cost),
        };
        let tx = INV_W - 8.0 - p.atlas.font.width_str(text) - 2.0;
        p.fill(
            left + tx - 2.0,
            top + 67.0,
            INV_W - 8.0 - (tx - 2.0),
            12.0,
            0x4F00_0000,
        );
        p.text_plain(text, left + tx, top + 69.0, color, true);
    }
    hovered
}

fn hover_name(stack: &SlotStack) -> String {
    tooltip::styled_hover_name(stack)
        .into_iter()
        .map(|s| s.text)
        .collect()
}

fn on_name_changed(
    sent: &mut Option<String>,
    input: &SlotStack,
    text: &str,
    hover: &str,
    out: &mut Vec<InvAction>,
) {
    if input.is_empty() {
        return;
    }
    let Some(name) = rename_value(text, hover, input.custom_name.is_some()) else {
        return;
    };
    if sent.as_deref() != Some(&*name) {
        let name = name.into_owned();
        out.push(InvAction::RenameItem(name.clone()));
        *sent = Some(name);
    }
}

fn rename_value<'a>(text: &'a str, hover: &str, custom: bool) -> Option<Cow<'a, str>> {
    let name = if !custom && text == hover { "" } else { text };
    let name = if name.chars().all(allowed_char) {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(name.chars().filter(|&c| allowed_char(c)).collect())
    };
    (name.encode_utf16().count() <= MAX_NAME_LENGTH).then_some(name)
}

fn may_pickup(cost: i32, creative: bool, level: u32) -> bool {
    (creative || i64::from(level) >= i64::from(cost)) && cost > 0
}

#[derive(Debug, PartialEq)]
enum Line {
    TooExpensive,
    Cost,
}

fn cost_line(cost: i32, creative: bool, level: u32, has_result: bool) -> Option<(Line, u32)> {
    if cost <= 0 {
        return None;
    }
    if cost >= TOO_EXPENSIVE && !creative {
        return Some((Line::TooExpensive, ERROR_COLOR));
    }
    if !has_result {
        return None;
    }
    let color = if may_pickup(cost, creative, level) {
        COST_COLOR
    } else {
        ERROR_COLOR
    };
    Some((Line::Cost, color))
}

fn cost_text(cache: &mut Option<(i16, String)>, cost: i16) -> &str {
    if cache.as_ref().is_none_or(|(c, _)| *c != cost) {
        let text = tooltip::translate("container.repair.cost", &[cost.to_string()]);
        *cache = Some((cost, text));
    }
    cache.as_ref().map_or("", |(_, s)| s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_name_sends_blank() {
        assert_eq!(rename_value("Stone", "Stone", false).as_deref(), Some(""));
        assert_eq!(
            rename_value("Stone", "Stone", true).as_deref(),
            Some("Stone")
        );
        assert_eq!(
            rename_value("Rock", "Stone", false).as_deref(),
            Some("Rock")
        );
    }

    #[test]
    fn names_are_filtered_and_measured_in_utf16() {
        assert_eq!(rename_value("a\u{a7}b", "", true).as_deref(), Some("ab"));
        assert!(rename_value(&"x".repeat(50), "", true).is_some());
        assert!(rename_value(&"x".repeat(51), "", true).is_none());
        assert!(rename_value(&"\u{1F600}".repeat(26), "", true).is_none());
    }

    #[test]
    fn the_cost_line_follows_extract_labels() {
        assert_eq!(cost_line(0, false, 99, true), None);
        assert_eq!(
            cost_line(40, false, 99, false),
            Some((Line::TooExpensive, ERROR_COLOR))
        );
        assert_eq!(cost_line(40, true, 0, true), Some((Line::Cost, COST_COLOR)));
        assert_eq!(cost_line(5, false, 99, false), None);
        assert_eq!(cost_line(5, false, 5, true), Some((Line::Cost, COST_COLOR)));
        assert_eq!(
            cost_line(5, false, 4, true),
            Some((Line::Cost, ERROR_COLOR))
        );
    }
}
