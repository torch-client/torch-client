use super::{
    CONTAINER_W, GuiState, INV_H, INV_W, InvAction, Layout, Painter, ScreenCtx, SlotStack,
    Snapshot, close_window, dim_background, draw_labels, draw_labels_centered, error_icon, filled,
    open_window, slots, window_origin,
};

pub fn draw_chest(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    rows: u8,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let layout = Layout::Chest(rows);
    let (_, img_h) = layout.size();
    let (left, top) = window_origin(layout, ctx);

    dim_background(p, ctx.vw, ctx.vh);
    let top_h = rows as f32 * 18.0 + 17.0;
    p.blit_sheet(
        "container/generic_54",
        0.0,
        0.0,
        CONTAINER_W,
        top_h,
        left,
        top,
    );
    p.blit_sheet(
        "container/generic_54",
        0.0,
        126.0,
        CONTAINER_W,
        96.0,
        left,
        top + top_h,
    );

    let hovered = slots::panel(p, layout, (left, top), ctx, snap, &mut state.slots, out);
    draw_labels(p, snap, left, top, img_h, 8.0);
    hovered
}

pub fn draw_crafting_table(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    crate::gui::recipe_book_ghost::draw_screen(
        p,
        state,
        ctx,
        snap,
        Layout::Crafting,
        (5.0, -49.0),
        true,
        out,
        |p, state, frame, out| {
            let (left, top) = frame.origin;
            p.blit_sheet(
                "container/crafting_table",
                0.0,
                0.0,
                INV_W,
                INV_H,
                left,
                top,
            );
            let hovered = slots::panel(
                p,
                Layout::Crafting,
                (left, top),
                &frame.ctx,
                snap,
                &mut state.slots,
                out,
            );
            draw_labels(p, snap, left, top, INV_H, 29.0);
            hovered
        },
    )
}

pub fn draw_furnace(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    kind: crate::session::FurnaceKind,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    crate::gui::recipe_book_ghost::draw_screen(
        p,
        state,
        ctx,
        snap,
        Layout::Furnace,
        (20.0, -49.0),
        true,
        out,
        |p, state, frame, out| {
            draw_furnace_window(p, state, &frame.ctx, snap, kind, frame.origin, out)
        },
    )
}

fn draw_furnace_window(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    kind: crate::session::FurnaceKind,
    (left, top): (f32, f32),
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let (sheet, lit, burn) = furnace_sprites(kind);
    p.blit_sheet(sheet, 0.0, 0.0, INV_W, INV_H, left, top);

    let d = &snap.container_data;
    let (lit_time, lit_duration) = (d[0].max(0) as f32, d[1].max(0) as f32);
    let (cook, cook_total) = (d[2].max(0) as f32, d[3].max(0) as f32);
    if lit_time > 0.0 {
        let duration = if lit_duration == 0.0 {
            200.0
        } else {
            lit_duration
        };
        let h = ((lit_time / duration).clamp(0.0, 1.0) * 13.0).ceil() + 1.0;
        p.sprite_part(
            lit,
            0.0,
            14.0 - h,
            14.0,
            h,
            left + 56.0,
            top + 36.0 + 14.0 - h,
        );
    }
    if cook_total != 0.0 && cook != 0.0 {
        let w = ((cook / cook_total).clamp(0.0, 1.0) * 24.0).ceil();
        if w > 0.0 {
            p.sprite_part(burn, 0.0, 0.0, w, 16.0, left + 79.0, top + 34.0);
        }
    }

    let hovered = slots::panel(
        p,
        Layout::Furnace,
        (left, top),
        ctx,
        snap,
        &mut state.slots,
        out,
    );
    draw_labels_centered(p, snap, left, top, INV_W, INV_H);
    hovered
}

fn furnace_sprites(
    kind: crate::session::FurnaceKind,
) -> (&'static str, &'static str, &'static str) {
    use crate::session::FurnaceKind as K;
    match kind {
        K::Furnace => (
            "container/furnace",
            "container/furnace/lit_progress",
            "container/furnace/burn_progress",
        ),
        K::BlastFurnace => (
            "container/blast_furnace",
            "container/blast_furnace/lit_progress",
            "container/blast_furnace/burn_progress",
        ),
        K::Smoker => (
            "container/smoker",
            "container/smoker/lit_progress",
            "container/smoker/burn_progress",
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_simple(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
    layout: Layout,
    sheet: &str,
    centered_title: bool,
) -> Option<SlotStack> {
    let (hovered, left, top) = simple(p, state, ctx, snap, out, layout, sheet);
    if centered_title {
        draw_labels_centered(p, snap, left, top, INV_W, INV_H);
    } else {
        draw_labels(p, snap, left, top, layout.size().1, 8.0);
    }
    hovered
}

pub fn draw_crafter(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let d = &snap.container_data;
    let mask = (0..9).fold(0u16, |m, i| m | (u16::from(d[i] == 1) << i));
    let layout = Layout::Crafter(mask);
    let queued = out.len();
    let (hovered, left, top) = simple(p, state, ctx, snap, out, layout, "container/crafter");
    draw_labels_centered(p, snap, left, top, INV_W, INV_H);

    let redstone = if d[9] == 1 {
        "container/crafter/powered_redstone"
    } else {
        "container/crafter/unpowered_redstone"
    };
    p.sprite(redstone, left + 97.0, top + 35.0, 16.0, 16.0);

    #[cfg(feature = "audio")]
    for action in &out[queued..] {
        if let InvAction::SlotStateChanged { enabled, .. } = *action {
            let pitch = if enabled { 1.0 } else { 0.75 };
            crate::audio::play(
                "ui.button.click",
                crate::audio::SoundCategory::Players,
                0.4,
                pitch,
            );
        }
    }
    #[cfg(not(feature = "audio"))]
    let _ = queued;

    let toggle_hint = (0..9).find(|&i| {
        let (sx, sy) = layout.slot_pos(i);
        ctx.hovering(
            left + sx - 1.0,
            top + sy - 1.0,
            slots::SLOT_PITCH,
            slots::SLOT_PITCH,
        )
    });
    if let Some(i) = toggle_hint
        && !layout.slot_disabled(i)
        && snap.slot(i).is_empty()
        && snap.gamemode != crate::session::Gamemode::Spectator
        && state.slots.cursor(snap).is_empty()
        && let Some(m) = ctx.mouse()
    {
        let line = vec![crate::text::Span::new(crate::gui::tooltip::label!(
            "gui.togglable_slot"
        ))];
        crate::gui::tooltip::draw_lines(p, &[line], m.x, m.y, ctx.vw, ctx.vh);
    }
    hovered
}

pub fn draw_grindstone(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let (left, top) = open_window(p, ctx, Layout::Grindstone, "container/grindstone");
    if (filled(snap, 0) || filled(snap, 1)) && !filled(snap, 2) {
        error_icon(p, "container/grindstone/error", left + 92.0, top + 31.0);
    }
    let hovered = slots::panel(
        p,
        Layout::Grindstone,
        (left, top),
        ctx,
        snap,
        &mut state.slots,
        out,
    );
    draw_labels(p, snap, left, top, INV_H, 8.0);
    hovered
}

pub fn draw_smithing(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let (left, top) = open_window(p, ctx, Layout::Smithing, "container/smithing");
    if snap.container_data[0] != 0 {
        error_icon(p, "container/smithing/error", left + 65.0, top + 46.0);
    }
    state.puppet.stand.draw(p, left, top, snap.slot(3));
    close_window(
        p,
        state,
        ctx,
        snap,
        out,
        Layout::Smithing,
        (left, top),
        (44.0, 15.0),
    )
}

pub fn draw_cartography(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let (left, top) = open_window(p, ctx, Layout::Cartography, "container/cartography_table");

    let (scaling, duplicating, locking) = match item_in(snap, 1) {
        "paper" => (true, false, false),
        "map" => (false, true, false),
        "glass_pane" => (false, false, true),
        _ => (false, false, false),
    };
    let has_result = filled(snap, 2);

    if (locking && !has_result) || (scaling && !has_result) {
        error_icon(
            p,
            "container/cartography_table/error",
            left + 35.0,
            top + 31.0,
        );
    }
    if duplicating {
        p.sprite(
            "container/cartography_table/duplicated_map",
            left + 67.0,
            top + 13.0,
            50.0,
            66.0,
        );
        p.sprite(
            "container/cartography_table/duplicated_map",
            left + 117.0,
            top + 13.0,
            50.0,
            66.0,
        );
    } else if scaling {
        p.sprite(
            "container/cartography_table/scaled_map",
            left + 67.0,
            top + 13.0,
            66.0,
            66.0,
        );
    } else {
        p.sprite(
            "container/cartography_table/map",
            left + 67.0,
            top + 13.0,
            66.0,
            66.0,
        );
        if locking {
            p.sprite(
                "container/cartography_table/locked",
                left + 118.0,
                top + 60.0,
                10.0,
                14.0,
            );
        }
    }

    close_window(
        p,
        state,
        ctx,
        snap,
        out,
        Layout::Cartography,
        (left, top),
        (8.0, 4.0),
    )
}

pub fn draw_brewing_stand(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let (left, top) = open_window(p, ctx, Layout::BrewingStand, "container/brewing_stand");

    let ticks = snap.container_data[0].max(0) as f32;
    let fuel = snap.container_data[1].max(0) as f32;

    let fuel_w = ((18.0 * fuel / 20.0).ceil()).clamp(0.0, 18.0);
    if fuel_w > 0.0 {
        p.sprite_part(
            "container/brewing_stand/fuel_length",
            0.0,
            0.0,
            fuel_w,
            4.0,
            left + 60.0,
            top + 44.0,
        );
    }
    if ticks > 0.0 {
        let h = (28.0 * (1.0 - ticks / 400.0)) as i32 as f32;
        if h > 0.0 {
            p.sprite_part(
                "container/brewing_stand/brew_progress",
                0.0,
                0.0,
                9.0,
                h,
                left + 97.0,
                top + 16.0,
            );
        }
        const BUBBLE_LENGTHS: [f32; 7] = [29.0, 24.0, 20.0, 16.0, 11.0, 6.0, 0.0];
        let h = BUBBLE_LENGTHS[(ticks as usize / 2) % 7];
        if h > 0.0 {
            p.sprite_part(
                "container/brewing_stand/bubbles",
                0.0,
                29.0 - h,
                12.0,
                h,
                left + 63.0,
                top + 14.0 + 29.0 - h,
            );
        }
    }

    let hovered = slots::panel(
        p,
        Layout::BrewingStand,
        (left, top),
        ctx,
        snap,
        &mut state.slots,
        out,
    );
    draw_labels_centered(p, snap, left, top, INV_W, INV_H);
    hovered
}

fn simple(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
    layout: Layout,
    sheet: &str,
) -> (Option<SlotStack>, f32, f32) {
    let (left, top) = window_origin(layout, ctx);
    let (w, h) = layout.size();
    dim_background(p, ctx.vw, ctx.vh);
    p.blit_sheet(sheet, 0.0, 0.0, w, h, left, top);
    let hovered = slots::panel(p, layout, (left, top), ctx, snap, &mut state.slots, out);
    (hovered, left, top)
}

fn item_in(snap: &Snapshot, i: usize) -> &str {
    snap.menu_slots.get(i).map_or("", |s| s.item)
}
