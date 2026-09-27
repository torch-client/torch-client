use super::{
    GuiState, INV_H, INV_W, InvAction, Layout, Painter, ScreenCtx, SlotStack, Snapshot,
    dim_background, draw_labels, slots, window_origin,
};

pub fn draw(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    columns: u8,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let layout = Layout::Horse(columns);
    let (left, top) = window_origin(layout, ctx);
    dim_background(p, ctx.vw, ctx.vh);
    p.blit_sheet("container/horse", 0.0, 0.0, INV_W, INV_H, left, top);

    if columns > 0 {
        p.sprite_part(
            "container/horse/chest_slots",
            0.0,
            0.0,
            columns as f32 * 18.0,
            54.0,
            left + 79.0,
            top + 17.0,
        );
    }

    p.sprite("container/slot", left + 7.0, top + 17.0, 18.0, 18.0);
    p.sprite("container/slot", left + 7.0, top + 35.0, 18.0, 18.0);

    let hovered = slots::panel(p, layout, (left, top), ctx, snap, &mut state.slots, out);
    draw_labels(p, snap, left, top, INV_H, 8.0);
    hovered
}
