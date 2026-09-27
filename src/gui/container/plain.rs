use super::{
    CONTAINER_W, GuiState, INV_H, INV_W, InvAction, Layout, Painter, ScreenCtx, SlotStack,
    Snapshot, dim_background, draw_inventory_label, draw_labels, draw_labels_centered, draw_title,
    error_icon, slots, window_origin,
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
    let (hovered, left, top) = simple(
        p,
        state,
        ctx,
        snap,
        out,
        Layout::Crafting,
        "container/crafting_table",
    );
    draw_labels(p, snap, left, top, INV_H, 29.0);
    hovered
}

pub fn draw_furnace(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    kind: crate::session::FurnaceKind,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let (left, top) = window_origin(Layout::Furnace, ctx);
    dim_background(p, ctx.vw, ctx.vh);
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

pub fn draw_hopper(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let (hovered, left, top) = simple(p, state, ctx, snap, out, Layout::Hopper, "container/hopper");
    draw_labels(p, snap, left, top, Layout::Hopper.size().1, 8.0);
    hovered
}

pub fn draw_grindstone(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let (left, top) = window_origin(Layout::Grindstone, ctx);
    dim_background(p, ctx.vw, ctx.vh);
    p.blit_sheet("container/grindstone", 0.0, 0.0, INV_W, INV_H, left, top);
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
    let (left, top) = window_origin(Layout::Smithing, ctx);
    dim_background(p, ctx.vw, ctx.vh);
    p.blit_sheet("container/smithing", 0.0, 0.0, INV_W, INV_H, left, top);
    if snap.container_data[0] != 0 {
        error_icon(p, "container/smithing/error", left + 65.0, top + 46.0);
    }
    let hovered = slots::panel(
        p,
        Layout::Smithing,
        (left, top),
        ctx,
        snap,
        &mut state.slots,
        out,
    );
    draw_title(p, &snap.container_title, left + 44.0, top + 15.0);
    draw_inventory_label(p, left, top, INV_H);
    hovered
}

pub fn draw_cartography(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let (left, top) = window_origin(Layout::Cartography, ctx);
    dim_background(p, ctx.vw, ctx.vh);
    p.blit_sheet(
        "container/cartography_table",
        0.0,
        0.0,
        INV_W,
        INV_H,
        left,
        top,
    );

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

    let hovered = slots::panel(
        p,
        Layout::Cartography,
        (left, top),
        ctx,
        snap,
        &mut state.slots,
        out,
    );
    draw_title(p, &snap.container_title, left + 8.0, top + 4.0);
    draw_inventory_label(p, left, top, INV_H);
    hovered
}

pub fn draw_brewing_stand(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let (left, top) = window_origin(Layout::BrewingStand, ctx);
    dim_background(p, ctx.vw, ctx.vh);
    p.blit_sheet("container/brewing_stand", 0.0, 0.0, INV_W, INV_H, left, top);

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

fn filled(snap: &Snapshot, i: usize) -> bool {
    snap.menu_slots.get(i).is_some_and(|s| !s.is_empty())
}

fn item_in(snap: &Snapshot, i: usize) -> &str {
    snap.menu_slots.get(i).map_or("", |s| s.item)
}
