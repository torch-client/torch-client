use crate::gui::ScreenCtx;
use crate::gui::painter::Painter;
use crate::session::Gamemode;
use crate::text::{Span, Style};

pub const SHEET: &str = "container/gamemode_switcher";
const SHEET_W: f32 = 125.0;
const SHEET_H: f32 = 75.0;
const SLOT_SPRITE: &str = "gamemode_switcher/slot";
const SELECTION_SPRITE: &str = "gamemode_switcher/selection";

const SLOT_AREA: f32 = 26.0;
const SLOT_AREA_PADDED: f32 = 31.0;
const ALL_SLOTS_WIDTH: i32 = Icon::VALUES.len() as i32 * SLOT_AREA_PADDED as i32 - 5;
const ICON_TOP_LEFT: f32 = 5.0;
const HELP_TIPS_OFFSET_Y: f32 = 5.0;
const AQUA: u32 = 0x55FFFF;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Icon {
    #[default]
    Creative,
    Survival,
    Adventure,
    Spectator,
}

impl Icon {
    const VALUES: [Icon; 4] = [
        Icon::Creative,
        Icon::Survival,
        Icon::Adventure,
        Icon::Spectator,
    ];

    fn name_key(self) -> &'static str {
        match self {
            Icon::Creative => "gameMode.creative",
            Icon::Survival => "gameMode.survival",
            Icon::Adventure => "gameMode.adventure",
            Icon::Spectator => "gameMode.spectator",
        }
    }

    fn item(self) -> &'static str {
        match self {
            Icon::Creative => "grass_block",
            Icon::Survival => "iron_sword",
            Icon::Adventure => "map",
            Icon::Spectator => "ender_eye",
        }
    }

    pub fn mode(self) -> Gamemode {
        match self {
            Icon::Creative => Gamemode::Creative,
            Icon::Survival => Gamemode::Survival,
            Icon::Adventure => Gamemode::Adventure,
            Icon::Spectator => Gamemode::Spectator,
        }
    }

    fn from_mode(mode: Gamemode) -> Icon {
        match mode {
            Gamemode::Creative => Icon::Creative,
            Gamemode::Survival => Icon::Survival,
            Gamemode::Adventure => Icon::Adventure,
            Gamemode::Spectator => Icon::Spectator,
        }
    }

    fn next(self) -> Icon {
        match self {
            Icon::Creative => Icon::Survival,
            Icon::Survival => Icon::Adventure,
            Icon::Adventure => Icon::Spectator,
            Icon::Spectator => Icon::Creative,
        }
    }
}

#[derive(Default)]
pub struct SwitcherState {
    hovered: Icon,
    first_mouse: Option<(i32, i32)>,
}

impl SwitcherState {
    pub fn open(&mut self, current: Gamemode, previous: Option<Gamemode>) {
        let default = previous.unwrap_or(if current == Gamemode::Creative {
            Gamemode::Survival
        } else {
            Gamemode::Creative
        });
        self.hovered = Icon::from_mode(default);
        self.first_mouse = None;
    }

    pub fn select_next(&mut self) {
        self.first_mouse = None;
        self.hovered = self.hovered.next();
    }

    pub fn hovered(&self) -> Gamemode {
        self.hovered.mode()
    }
}

pub fn draw(p: &mut Painter, state: &mut SwitcherState, ctx: &ScreenCtx, select_key: &str) {
    let cx = (ctx.vw / 2.0).floor();
    let cy = (ctx.vh / 2.0).floor();

    p.blit_sheet(
        SHEET,
        0.0,
        0.0,
        SHEET_W,
        SHEET_H,
        cx - 62.0,
        cy - SLOT_AREA_PADDED - 27.0,
    );

    let name = crate::gui::tooltip::translate(state.hovered.name_key(), &[]);
    p.text_centered(&name, cx, cy - SLOT_AREA_PADDED - 20.0, 0xFFFFFF, true);

    let template =
        crate::gui::tooltip::translate("debug.gamemodes.select_next", &["%s".to_string()]);
    let (before, after) = template.split_once("%s").unwrap_or((template.as_str(), ""));
    let spans = [
        Span {
            text: before.to_string(),
            style: Style::colored(0xFFFFFF),
        },
        Span {
            text: select_key.to_string(),
            style: Style::colored(AQUA),
        },
        Span {
            text: after.to_string(),
            style: Style::colored(0xFFFFFF),
        },
    ];
    let w = p.atlas.font.width(&spans);
    p.text(
        &spans,
        (cx - w / 2.0).floor(),
        cy + HELP_TIPS_OFFSET_Y,
        true,
    );

    let mouse = ctx
        .mouse()
        .map(|m| (m.x.floor() as i32, m.y.floor() as i32));
    if state.first_mouse.is_none() {
        state.first_mouse = mouse;
    }
    let moved = mouse.is_some() && mouse != state.first_mouse;

    let selected = state.hovered;
    let x0 = cx - (ALL_SLOTS_WIDTH / 2) as f32;
    let y = cy - SLOT_AREA_PADDED;
    for (i, icon) in Icon::VALUES.into_iter().enumerate() {
        let x = x0 + i as f32 * SLOT_AREA_PADDED;
        p.sprite(SLOT_SPRITE, x, y, SLOT_AREA, SLOT_AREA);
        if icon == selected {
            p.sprite(SELECTION_SPRITE, x, y, SLOT_AREA, SLOT_AREA);
        }
        p.item_icon(icon.item(), x + ICON_TOP_LEFT, y + ICON_TOP_LEFT);
        if moved && ctx.hovering(x, y, SLOT_AREA, SLOT_AREA) {
            state.hovered = icon;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_selection() {
        let mut s = SwitcherState::default();
        s.open(Gamemode::Survival, Some(Gamemode::Adventure));
        assert_eq!(s.hovered(), Gamemode::Adventure);
        s.open(Gamemode::Creative, None);
        assert_eq!(s.hovered(), Gamemode::Survival);
        s.open(Gamemode::Spectator, None);
        assert_eq!(s.hovered(), Gamemode::Creative);
    }

    #[test]
    fn select_next_wraps() {
        let mut s = SwitcherState::default();
        s.open(Gamemode::Survival, Some(Gamemode::Spectator));
        s.select_next();
        assert_eq!(s.hovered(), Gamemode::Creative);
        s.select_next();
        assert_eq!(s.hovered(), Gamemode::Survival);
    }

    #[test]
    fn slot_row_width() {
        assert_eq!(ALL_SLOTS_WIDTH, 119);
    }
}
