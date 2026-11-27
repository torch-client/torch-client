use crate::gui::painter::Painter;
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::text::Span;

pub mod buffet;
pub mod create;
pub mod datapacks;
pub mod edit;
pub mod experiments;
pub mod flat;
pub mod gamerules;
#[allow(
    dead_code,
    reason = "nothing opens it until the singleplayer backend exists"
)]
pub mod lan;
pub mod optimize;
pub mod select;

#[derive(Default)]
pub struct State {
    pub select: select::State,
    pub create: create::State,
    pub edit: edit::State,
    pub gamerules: gamerules::State,
    pub experiments: experiments::State,
    pub datapacks: datapacks::State,
    pub flat: flat::State,
    pub buffet: buffet::State,
    pub optimize: optimize::State,
    pub lan: lan::State,
}

pub(in crate::gui) const TOOLTIP_W: f32 = 170.0;

pub(in crate::gui) fn hover_spans(p: &mut Painter, ctx: &ScreenCtx, spans: &[Span]) {
    if let Some(m) = ctx.mouse() {
        let lines = p.atlas.font.wrap(spans, TOOLTIP_W);
        crate::gui::tooltip::draw_lines(p, &lines, m.x, m.y, ctx.vw, ctx.vh);
    }
}

pub(in crate::gui) fn hover_tooltip(p: &mut Painter, ctx: &ScreenCtx, text: &str) {
    hover_spans(p, ctx, &[Span::colored(text, 0xFFFFFF)]);
}

pub(in crate::gui) fn unsupported_text() -> String {
    crate::lang::tr!("singleplayer.unsupported").to_owned()
}

pub(in crate::gui) fn unsupported_tip(p: &mut Painter, ctx: &ScreenCtx) {
    hover_tooltip(p, ctx, &unsupported_text());
}

pub(in crate::gui) fn back_to_list(state: &mut GuiState) {
    state.worlds.select.reload();
    state.nav = Some(Screen::SelectWorld);
}
