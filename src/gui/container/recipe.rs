use super::banner;
use super::{
    GuiState, INV_H, INV_W, InvAction, Layout, Painter, ScreenCtx, Scroller, SlotStack, Snapshot,
    button, dim_background, draw_title, error_icon, first_row, hovering, scroll_rows, slots,
    window_origin,
};

const COLS: usize = 4;

pub fn draw_stonecutter(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    const ROWS: usize = 3;
    const VISIBLE: usize = COLS * ROWS;

    let (left, top) = window_origin(Layout::Stonecutter, ctx);
    dim_background(p, ctx.vw, ctx.vh);
    p.blit_sheet("container/stonecutter", 0.0, 0.0, INV_W, INV_H, left, top);

    let input = snap.menu_slots.first().map_or("", |s| s.item);
    let matches = |r: &&crate::session::StonecutterRecipe| {
        !input.is_empty() && r.input.binary_search_by(|k| (**k).cmp(input)).is_ok()
    };
    let count = snap.stonecutter_recipes.iter().filter(matches).count();

    let active = count > VISIBLE;
    let ui = &mut state.container;
    if hovering(ctx, left + 52.0, top + 14.0, 4.0 * 16.0, 3.0 * 18.0) {
        ui.scroll = scroll_rows(ui.scroll, ctx.input.scroll, rows_of(count), ROWS);
    }
    Scroller {
        x: left + 119.0,
        y: top + 15.0,
        w: 12.0,
        h: 15.0,
        travel: 41.0,
        sprite: "container/stonecutter/scroller",
        disabled: "container/stonecutter/scroller_disabled",
    }
    .run(p, ctx, ui, active);

    let start = first_row(ui.scroll, rows_of(count), ROWS) * COLS;
    let selected = snap.container_data[0];

    for (cell, recipe) in snap
        .stonecutter_recipes
        .iter()
        .filter(matches)
        .enumerate()
        .skip(start)
        .take(VISIBLE)
    {
        let slot = cell - start;
        let bx = left + 52.0 + (slot % COLS) as f32 * 16.0;
        let by = top + 14.0 + (slot / COLS) as f32 * 18.0;
        let (hovered, clicked) = button(ctx, bx, by, 16.0, 18.0);
        p.sprite(
            recipe_sprite(
                selected == cell as i16,
                hovered,
                "container/stonecutter/recipe_selected",
                "container/stonecutter/recipe_highlighted",
                "container/stonecutter/recipe",
            ),
            bx,
            by,
            16.0,
            18.0,
        );
        p.item_icon(recipe.result.item, bx, by + 2.0);
        if clicked {
            ui.selected = cell as i16;
            out.push(InvAction::ButtonClick(cell.min(255) as u8));
        }
    }

    let hovered = slots::panel(
        p,
        Layout::Stonecutter,
        (left, top),
        ctx,
        snap,
        &mut state.slots,
        out,
    );
    draw_title(p, &snap.container_title, left + 8.0, top + 5.0);
    super::draw_inventory_label(p, left, top, INV_H);
    hovered
}

pub fn draw_loom(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    const ROWS: usize = 4;
    const VISIBLE: usize = COLS * ROWS;

    let (left, top) = window_origin(Layout::Loom, ctx);
    dim_background(p, ctx.vw, ctx.vh);
    p.blit_sheet("container/loom", 0.0, 0.0, INV_W, INV_H, left, top);

    let patterns: &[Box<str>] = match snap
        .menu_slots
        .get(2)
        .and_then(|s| s.banner_patterns.as_deref())
    {
        Some(p) => p,
        None => banner::no_item_required(),
    };

    let has_inputs = filled(snap, 0) && filled(snap, 1);
    let count = if has_inputs { patterns.len() } else { 0 };
    let active = count > VISIBLE;

    let ui = &mut state.container;
    if hovering(ctx, left + 60.0, top + 13.0, 4.0 * 14.0, 4.0 * 14.0) {
        ui.scroll = scroll_rows(ui.scroll, ctx.input.scroll, rows_of(count), ROWS);
    }
    Scroller {
        x: left + 119.0,
        y: top + 13.0,
        w: 12.0,
        h: 15.0,
        travel: 41.0,
        sprite: "container/loom/scroller",
        disabled: "container/loom/scroller_disabled",
    }
    .run(p, ctx, ui, active);

    let start = first_row(ui.scroll, rows_of(count), ROWS) * COLS;
    let selected = snap.container_data[0];

    for (cell, pattern) in patterns
        .iter()
        .enumerate()
        .skip(start)
        .take(count.min(VISIBLE))
    {
        let slot = cell - start;
        if slot >= VISIBLE {
            break;
        }
        let bx = left + 60.0 + (slot % COLS) as f32 * 14.0;
        let by = top + 13.0 + (slot / COLS) as f32 * 14.0;
        let (hovered, clicked) = button(ctx, bx, by, 14.0, 14.0);
        p.sprite(
            recipe_sprite(
                selected == cell as i16,
                hovered,
                "container/loom/pattern_selected",
                "container/loom/pattern_highlighted",
                "container/loom/pattern",
            ),
            bx,
            by,
            14.0,
            14.0,
        );
        banner::swatch(p, pattern, bx + 4.0, by + 2.0);
        if clicked {
            ui.selected = cell as i16;
            out.push(InvAction::ButtonClick(cell.min(255) as u8));
        }
    }

    if has_inputs && count == 0 {
        error_icon(p, "container/loom/error", left + 138.0, top + 52.0);
    } else if has_inputs {
        banner::preview(p, stack(snap, 0), left + 141.0, top + 8.0);
    }

    let hovered = slots::panel(
        p,
        Layout::Loom,
        (left, top),
        ctx,
        snap,
        &mut state.slots,
        out,
    );
    draw_title(p, &snap.container_title, left + 8.0, top + 4.0);
    super::draw_inventory_label(p, left, top, INV_H);
    hovered
}

fn rows_of(count: usize) -> usize {
    count.div_ceil(COLS)
}

fn recipe_sprite(
    selected: bool,
    hovered: bool,
    on_selected: &'static str,
    on_hovered: &'static str,
    plain: &'static str,
) -> &'static str {
    if selected {
        on_selected
    } else if hovered {
        on_hovered
    } else {
        plain
    }
}

fn filled(snap: &Snapshot, i: usize) -> bool {
    snap.menu_slots.get(i).is_some_and(|s| !s.is_empty())
}

fn stack<'a>(snap: &'a Snapshot, i: usize) -> Option<&'a SlotStack> {
    snap.menu_slots.get(i).filter(|s| !s.is_empty())
}
