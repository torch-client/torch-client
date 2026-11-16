use crate::client::assets::transfer::{self, Offer, Part};
use crate::gui::options::{
    COLUMN_STEP, COLUMN_W, GRID_LEFT_OFFSET, LAYOUT, draw_done, draw_title, generic_value,
};
use crate::gui::painter::Painter;
use crate::gui::tooltip::label;
use crate::gui::widgets::{Button, WIDGET_HEIGHT};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::util::bytes::Bytes;

const TITLE: &str = "Export Assets";
const EXPORT: &str = "Export";
const ROW_SPACING: f32 = 4.0;
const LINE_H: f32 = 12.0;

pub struct State {
    on: [bool; 6],
    offer: Option<Offer>,
    pub import_line: Option<(String, bool)>,
}

impl Default for State {
    fn default() -> State {
        State {
            on: Part::ALL.map(Part::default_on),
            offer: None,
            import_line: None,
        }
    }
}

impl State {
    pub fn open(&mut self) {
        self.offer = Some(transfer::offer());
    }

    fn available(&self, part: Part) -> bool {
        self.offer
            .as_ref()
            .is_some_and(|o| o.available[part.index()])
    }

    fn chosen(&self, part: Part) -> bool {
        self.on[part.index()] && self.available(part)
    }
}

fn language_caption(state: &State) -> String {
    let Some(offer) = &state.offer else {
        return generic_value(Part::Language.caption(), on_off(false));
    };
    if offer.language == crate::client::language::ENGLISH {
        return format!("{}: English (built in)", Part::Language.caption());
    }
    let name = crate::client::language::list()
        .and_then(|list| {
            list.iter()
                .find(|l| l.code == offer.language)
                .map(|l| l.name.clone())
        })
        .unwrap_or_else(|| offer.language.clone());
    generic_value(
        &format!("{}: {name}", Part::Language.caption()),
        on_off(state.chosen(Part::Language)),
    )
}

fn on_off(on: bool) -> &'static str {
    if on {
        label!("options.on")
    } else {
        label!("options.off")
    }
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    draw_title(p, ctx, TITLE);
    let st = &mut state.asset_transfer;
    if st.offer.is_none() {
        st.open();
    }

    let rows = 4.0;
    let grid_h = rows * WIDGET_HEIGHT + (rows - 1.0) * ROW_SPACING;
    let content_h = grid_h + ROW_SPACING + LINE_H + WIDGET_HEIGHT;
    let left = (ctx.vw / 2.0 - GRID_LEFT_OFFSET).floor();
    let top = LAYOUT.content_y(ctx.vh, content_h);
    let row_y = |row: usize| top + row as f32 * (WIDGET_HEIGHT + ROW_SPACING);

    for (i, part) in Part::ALL
        .iter()
        .copied()
        .filter(|p| *p != Part::Language)
        .enumerate()
    {
        let x = left + (i % 2) as f32 * COLUMN_STEP;
        let caption = generic_value(part.caption(), on_off(st.chosen(part)));
        let button = Button {
            active: st.available(part),
            ..Button::new(x, row_y(i / 2), COLUMN_W, WIDGET_HEIGHT, &caption)
        };
        if button.draw(p, ctx) {
            st.on[part.index()] = !st.on[part.index()];
        }
    }
    let wide = COLUMN_STEP + COLUMN_W;
    let caption = language_caption(st);
    let language = Button {
        active: st.available(Part::Language),
        ..Button::new(left, row_y(3), wide, WIDGET_HEIGHT, &caption)
    };
    if language.draw(p, ctx) {
        let i = Part::Language.index();
        st.on[i] = !st.on[i];
    }

    let size: u64 = Part::ALL
        .iter()
        .filter(|p| st.chosen(**p))
        .map(|p| st.offer.as_ref().map_or(0, |o| o.sizes[p.index()]))
        .sum();
    let any = Part::ALL.iter().any(|p| st.chosen(*p));
    let line_y = top + grid_h + ROW_SPACING;
    p.text_centered(
        &format!("About {}", Bytes(size)),
        ctx.vw / 2.0,
        line_y + 2.0,
        0xA0A0A0,
        true,
    );

    let export = Button {
        active: any,
        ..Button::new(left, line_y + LINE_H, wide, WIDGET_HEIGHT, EXPORT)
    };
    if export.draw(p, ctx) {
        let mut on = [false; 6];
        for part in Part::ALL {
            on[part.index()] = st.chosen(part);
        }
        let bytes = transfer::export(on);
        if let Err(e) = crate::platform::download::save(
            &transfer::file_name(),
            &bytes,
            "application/octet-stream",
        ) {
            crate::log_warn!("assets", "could not export the assets: {e}");
        }
    }

    if draw_done(p, ctx) {
        state.nav = Some(Screen::ManageStorage);
    }
}
