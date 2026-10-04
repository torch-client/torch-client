use crate::gui::{tooltip, widgets};
use crate::session::{MerchantOffer, MerchantOffers};

use super::{
    ContainerUi, Digits, GuiState, InvAction, Layout, Painter, ScreenCtx, Scroller, SlotStack,
    Snapshot, TitleCache, dim_background, draw_inventory_label_at, draw_title, first_row, hovering,
    scroll_rows, slots, window_origin,
};

const INVENTORY_LABEL_X: f32 = 107.0;

const VISIBLE_ROWS: usize = 7;
const ROW_W: f32 = 88.0;
const ROW_H: f32 = 20.0;

const XP_THRESHOLDS: [u32; 5] = [0, 10, 70, 150, 250];

pub fn draw(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let layout = Layout::Merchant;
    let (left, top) = window_origin(layout, ctx);
    let (img_w, img_h) = layout.size();
    dim_background(p, ctx.vw, ctx.vh);
    p.blit_sheet("container/villager", 0.0, 0.0, img_w, img_h, left, top);

    let Some(offers) = snap.merchant.as_deref().filter(|o| !o.offers.is_empty()) else {
        let hovered = slots::panel(p, layout, (left, top), ctx, snap, &mut state.slots, out);
        draw_merchant_labels(p, &mut state.container, snap, None, left, top, img_w, img_h);
        return hovered;
    };

    let ui = &mut state.container;
    let count = offers.offers.len();
    let active = count > VISIBLE_ROWS;

    if hovering(
        ctx,
        left + 5.0,
        top + 18.0,
        ROW_W,
        VISIBLE_ROWS as f32 * ROW_H,
    ) {
        ui.scroll = scroll_rows(ui.scroll, ctx.input.scroll, count, VISIBLE_ROWS);
    }

    Scroller {
        x: left + 94.0,
        y: top + 18.0,
        w: 6.0,
        h: 27.0,
        travel: 113.0,
        sprite: "container/villager/scroller",
        disabled: "container/villager/scroller_disabled",
    }
    .run(p, ctx, ui, active);

    let start = first_row(ui.scroll, count, VISIBLE_ROWS);
    let selected = ui.selected.max(0) as usize;
    let mut hovered_stack = None;

    for (row, offer) in offers
        .offers
        .iter()
        .enumerate()
        .skip(start)
        .take(VISIBLE_ROWS)
    {
        let slot = row - start;
        let row_x = left + 5.0;
        let row_y = top + 18.0 + slot as f32 * ROW_H;

        let row_hovered = hovering(ctx, row_x, row_y, ROW_W, ROW_H);
        if widgets::Button::new(row_x, row_y, ROW_W, ROW_H, "").draw(p, ctx) {
            ui.selected = row as i16;
            out.push(InvAction::SelectTrade(row as u32));
        }
        if row_hovered && let Some(m) = ctx.mouse() {
            let rel = m.x - row_x;
            hovered_stack = if rel < 20.0 {
                Some(offer.cost_a.clone())
            } else if rel > 30.0 && rel < 50.0 && !offer.cost_b.is_empty() {
                Some(offer.cost_b.clone())
            } else if rel > 65.0 {
                Some(offer.result.clone())
            } else {
                None
            };
        }

        let item_y = row_y + 1.0;
        draw_cost_a(p, offer, row_x + 5.0, item_y);
        if !offer.cost_b.is_empty() {
            slots::draw_stack(p, &offer.cost_b, row_x + 35.0, item_y);
        }
        let arrow = if offer.out_of_stock {
            "container/villager/trade_arrow_out_of_stock"
        } else {
            "container/villager/trade_arrow"
        };
        p.sprite(arrow, row_x + 55.0, item_y + 3.0, 10.0, 9.0);
        slots::draw_stack(p, &offer.result, row_x + 68.0, item_y);
    }

    if offers.show_progress {
        draw_xp_bar(
            p,
            left,
            top,
            offers.level,
            offers.xp,
            snap.merchant_future_xp,
        );
    }

    let sold_out = offers.offers.get(selected).is_some_and(|o| o.out_of_stock);
    if sold_out {
        p.sprite(
            "container/villager/out_of_stock",
            left + 182.0,
            top + 35.0,
            28.0,
            21.0,
        );
    }

    let slot_hovered = slots::panel(p, layout, (left, top), ctx, snap, &mut state.slots, out);
    draw_merchant_labels(
        p,
        &mut state.container,
        snap,
        Some(offers),
        left,
        top,
        img_w,
        img_h,
    );

    if sold_out
        && offers.can_restock
        && hovering(ctx, left + 186.0, top + 35.0, 22.0, 21.0)
        && let Some(m) = ctx.mouse()
    {
        let line = vec![crate::text::Span {
            text: restock_label().to_string(),
            style: crate::text::Style::default(),
        }];
        tooltip::draw_lines(p, &[line], m.x, m.y, ctx.vw, ctx.vh);
    }

    slot_hovered.or(hovered_stack)
}

#[allow(clippy::too_many_arguments)]
fn draw_merchant_labels(
    p: &mut Painter,
    ui: &mut ContainerUi,
    snap: &Snapshot,
    offers: Option<&MerchantOffers>,
    left: f32,
    top: f32,
    img_w: f32,
    img_h: f32,
) {
    match offers.filter(|o| o.show_progress && (1..=5).contains(&o.level)) {
        Some(offers) => {
            let title = titled_with_level(ui, snap, offers.level);
            let w = p.atlas.font.width_str(title);
            let x = (49.0 + img_w / 2.0 - w / 2.0).floor();
            let title = title.to_string();
            p.text_plain(&title, left + x, top + 6.0, super::LABEL_COLOR, false);
        }
        None => {
            let title_w = super::title_width(p, &snap.container_title);
            let title_x = (49.0 + img_w / 2.0 - title_w / 2.0).floor();
            draw_title(p, &snap.container_title, left + title_x, top + 6.0);
        }
    }
    draw_inventory_label_at(p, left, top, img_h, INVENTORY_LABEL_X);

    let trades = trades_label();
    let trades_w = p.atlas.font.width_str(trades);
    p.text_plain(
        trades,
        left + 5.0 - trades_w / 2.0 + 48.0,
        top + 6.0,
        super::LABEL_COLOR,
        false,
    );
}

fn trades_label() -> &'static str {
    use std::sync::OnceLock;
    static LABEL: OnceLock<String> = OnceLock::new();
    LABEL.get_or_init(|| tooltip::translate("merchant.trades", &[]))
}

fn restock_label() -> &'static str {
    use std::sync::OnceLock;
    static LABEL: OnceLock<String> = OnceLock::new();
    LABEL.get_or_init(|| tooltip::translate("merchant.deprecated", &[]))
}

fn titled_with_level<'a>(ui: &'a mut ContainerUi, snap: &Snapshot, level: u32) -> &'a str {
    let fresh = ui
        .merchant_title
        .as_ref()
        .is_some_and(|c| c.container_id == snap.container_id && c.level == level);
    if !fresh {
        let plain: String = snap
            .container_title
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        let mut level_key = String::from("merchant.level.");
        level_key.push_str(Digits::new(level).as_str());
        let level_name = tooltip::translate(&level_key, &[]);
        let text = tooltip::translate("merchant.title", &[plain, level_name]);
        ui.merchant_title = Some(TitleCache {
            container_id: snap.container_id,
            level,
            text,
        });
    }
    ui.merchant_title.as_ref().map_or("", |c| c.text.as_str())
}

fn draw_cost_a(p: &mut Painter, offer: &MerchantOffer, x: f32, y: f32) {
    if !is_discounted(offer) {
        slots::draw_stack(p, &offer.cost_a, x, y);
        return;
    }
    if offer.cost_a.is_empty() {
        return;
    }
    p.item_icon(&offer.cost_a.model_key(), x, y);
    draw_price(p, offer.base_cost_a.count, x, y);
    draw_price(p, offer.cost_a.count, x + 14.0, y);
    p.sprite(
        "container/villager/discount_strikethrough",
        x + 7.0,
        y + 12.0,
        9.0,
        2.0,
    );
}

fn draw_price(p: &mut Painter, count: u8, x: f32, y: f32) {
    let digits = Digits::new(count as u32);
    let w = p.atlas.font.width_str(digits.as_str());
    p.text_plain(digits.as_str(), x + 17.0 - w, y + 9.0, 0xFFFFFF, true);
}

fn is_discounted(offer: &MerchantOffer) -> bool {
    offer.base_cost_a.count != offer.cost_a.count
}

fn draw_xp_bar(p: &mut Painter, left: f32, top: f32, level: u32, xp: u32, future: u32) {
    if level >= 5 {
        return;
    }
    let (bar_x, bar_y) = (left + 136.0, top + 16.0);
    p.sprite(
        "container/villager/experience_bar_background",
        bar_x,
        bar_y,
        102.0,
        5.0,
    );
    if level == 0 {
        return;
    }
    let (min, max) = (
        XP_THRESHOLDS[level as usize - 1],
        XP_THRESHOLDS[level as usize],
    );
    if xp < min {
        return;
    }
    let w = xp_bar_width(xp, min, max);
    if w > 0.0 {
        p.sprite_part(
            "container/villager/experience_bar_current",
            0.0,
            0.0,
            w,
            5.0,
            bar_x,
            bar_y,
        );
    }
    let future_w = future_bar_width(future, min, max, w);
    if future_w > 0.0 {
        p.sprite_part(
            "container/villager/experience_bar_result",
            w,
            0.0,
            future_w,
            5.0,
            bar_x + w,
            bar_y,
        );
    }
}

fn xp_bar_width(xp: u32, min: u32, max: u32) -> f32 {
    if max <= min {
        return 0.0;
    }
    let multiplier = 102.0 / (max - min) as f32;
    (multiplier * (xp - min) as f32).floor().min(102.0)
}

fn future_bar_width(future: u32, min: u32, max: u32, earned: f32) -> f32 {
    if future == 0 || max <= min {
        return 0.0;
    }
    let multiplier = 102.0 / (max - min) as f32;
    (multiplier * future as f32)
        .floor()
        .min(102.0 - earned)
        .max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xp_bar_fills_between_its_thresholds() {
        assert_eq!(xp_bar_width(0, 0, 10), 0.0);
        assert_eq!(xp_bar_width(9, 0, 10), (102.0_f32 / 10.0 * 9.0).floor());
        assert_eq!(xp_bar_width(10, 0, 10), 102.0);
        assert_eq!(xp_bar_width(5, 10, 10), 0.0);
    }

    #[test]
    fn the_future_segment_fills_only_what_is_left() {
        let earned = xp_bar_width(40, 10, 70);
        assert_eq!(future_bar_width(0, 10, 70, earned), 0.0);
        assert_eq!(
            future_bar_width(5, 10, 70, earned),
            (102.0_f32 / 60.0 * 5.0).floor()
        );
        assert_eq!(future_bar_width(500, 10, 70, earned), 102.0 - earned);
        assert_eq!(future_bar_width(10, 10, 70, 102.0), 0.0);
    }

    #[test]
    fn discount_is_only_a_count_mismatch() {
        let mut offer = MerchantOffer::default();
        offer.base_cost_a.count = 3;
        offer.cost_a.count = 3;
        assert!(!is_discounted(&offer));
        offer.cost_a.count = 2;
        assert!(is_discounted(&offer));
    }
}
