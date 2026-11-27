use crate::gui::focus;
use crate::gui::op_blocks::{cycle, step};
use crate::gui::options::{LAYOUT, draw_title, generic_value};
use crate::gui::painter::Painter;
use crate::gui::tooltip::{label, translate};
use crate::gui::widgets::{Button, FOOTER_GAP, FOOTER_W, TextBox, WIDGET_HEIGHT, centered_row_x};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::singleplayer::backed::{Control, is_backed};
use crate::text::{LINE_HEIGHT, Span, Style};

const PORT_LOWER_BOUND: i32 = 1024;
const PORT_HIGHER_BOUND: i32 = 65535;
const FALLBACK_PORT: i32 = 25564;
const PORT_MAX_LEN: usize = 32;
const PORT_COLOR_VALID: u32 = 0xE0E0E0;
const PORT_COLOR_INVALID: u32 = 0xDF5050;
const GRAY: u32 = 0xAAAAAA;

const WIDGET_W: f32 = 150.0;
const CONTENT_SPACING: f32 = 8.0;
const PORT_ROW_SPACING: f32 = 4.0;
const LAN_Y: f32 = 0.0;
const PORT_LABEL_Y: f32 = LAN_Y + WIDGET_HEIGHT + CONTENT_SPACING;
const PORT_BOX_Y: f32 = PORT_LABEL_Y + LINE_HEIGHT + PORT_ROW_SPACING;
const OTHER_PLAYERS_Y: f32 = PORT_BOX_Y + WIDGET_HEIGHT + CONTENT_SPACING;
const ROW_Y: f32 = OTHER_PLAYERS_Y + LINE_HEIGHT + CONTENT_SPACING;
const CONTENT_H: f32 = ROW_Y + WIDGET_HEIGHT;
const ROW_W: f32 = 2.0 * WIDGET_W + CONTENT_SPACING;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LanGameMode {
    #[default]
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl LanGameMode {
    const CYCLE: [LanGameMode; 4] = [
        LanGameMode::Survival,
        LanGameMode::Spectator,
        LanGameMode::Creative,
        LanGameMode::Adventure,
    ];

    fn short_name(self) -> &'static str {
        match self {
            LanGameMode::Survival => label!("selectWorld.gameMode.survival"),
            LanGameMode::Creative => label!("selectWorld.gameMode.creative"),
            LanGameMode::Adventure => label!("selectWorld.gameMode.adventure"),
            LanGameMode::Spectator => label!("selectWorld.gameMode.spectator"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LanSettings {
    pub game_mode: LanGameMode,
    pub allow_commands: bool,
    pub port: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PortError {
    Invalid,
    Unavailable,
}

impl PortError {
    fn message(self) -> String {
        let bounds = [PORT_LOWER_BOUND.to_string(), PORT_HIGHER_BOUND.to_string()];
        match self {
            PortError::Invalid => translate("lanServer.port.invalid", &bounds),
            PortError::Unavailable => translate("lanServer.port.unavailable", &bounds),
        }
    }
}

#[derive(Default)]
pub struct State {
    hardcore: bool,
    initial_lan: bool,
    wanted_lan: bool,
    game_mode: LanGameMode,
    initial_game_mode: LanGameMode,
    commands: bool,
    initial_commands: bool,
    port: i32,
    initial_port: i32,
    port_box: TextBox,
    seen: String,
    error: Option<PortError>,
    pub result: Option<Option<LanSettings>>,
}

impl State {
    pub fn open(&mut self, hardcore: bool, current: Option<&LanSettings>) {
        self.hardcore = hardcore;
        self.port = available_port();
        self.initial_port = 0;
        self.initial_lan = current.is_some();
        self.wanted_lan = self.initial_lan;
        if let Some(lan) = current {
            self.port = i32::from(lan.port);
            self.initial_port = self.port;
        }
        self.game_mode = current.map_or(LanGameMode::Survival, |l| l.game_mode);
        self.initial_game_mode = self.game_mode;
        self.commands = current.is_some_and(|l| l.allow_commands);
        self.initial_commands = self.commands;
        self.port_box = TextBox::bordered(PORT_MAX_LEN, "");
        self.seen.clear();
        self.error = None;
        self.result = None;
        if self.initial_lan {
            self.port_box.set_text(&self.port.to_string());
            self.respond();
        }
        self.update_port_controls();
    }

    fn try_parse_port(&mut self, value: &str) -> Option<PortError> {
        if value.chars().all(char::is_whitespace) {
            self.port = available_port();
            return None;
        }
        match value.parse::<i32>() {
            Ok(parsed) if (PORT_LOWER_BOUND..=PORT_HIGHER_BOUND).contains(&parsed) => {
                if parsed != self.initial_port && !port_available(parsed) {
                    Some(PortError::Unavailable)
                } else {
                    self.port = parsed;
                    None
                }
            }
            Ok(_) => Some(PortError::Invalid),
            Err(_) => {
                self.port = available_port();
                Some(PortError::Invalid)
            }
        }
    }

    fn set_port_error(&mut self, error: Option<PortError>) {
        self.error = error;
        self.port_box.color = if error.is_none() {
            PORT_COLOR_VALID
        } else {
            PORT_COLOR_INVALID
        };
    }

    fn respond(&mut self) {
        if self.port_box.text == self.seen {
            return;
        }
        self.seen.clone_from(&self.port_box.text);
        let value = self.seen.clone();
        let error = self.try_parse_port(&value);
        self.set_port_error(error);
        self.port_box.placeholder = self.port.to_string();
    }

    fn update_port_controls(&mut self) {
        let desired = if self.wanted_lan && self.initial_lan {
            self.initial_port.to_string()
        } else {
            String::new()
        };
        if self.port_box.text != desired {
            self.port_box.set_text(&desired);
            self.respond();
        }
        self.port_box.placeholder = if self.wanted_lan {
            self.port.to_string()
        } else {
            String::new()
        };
        if !self.wanted_lan {
            if self.port_box.focused {
                focus::clear();
            }
            self.port_box.focused = false;
            self.set_port_error(None);
        }
    }

    fn lan_port_changed(&self) -> bool {
        self.wanted_lan && self.initial_lan && self.port != self.initial_port
    }

    fn has_settings_changes(&self) -> bool {
        self.wanted_lan != self.initial_lan
            || self.game_mode != self.initial_game_mode
            || self.commands != self.initial_commands
            || self.lan_port_changed()
    }

    fn apply_active(&self) -> bool {
        (!self.wanted_lan || self.error.is_none()) && self.has_settings_changes()
    }
}

enum Action {
    Apply,
    Close,
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    background(p, ctx);
    let lan = &mut state.worlds.lan;
    let mut action = None;
    let mut tip: Option<String> = None;

    let other_players = [Span {
        text: translate("menu.multiplayerOptions.otherPlayers.header", &[]),
        style: Style {
            bold: true,
            underline: true,
            ..Style::colored(0xFFFFFF)
        },
    }];
    let port_label = label!("lanServer.port");
    let other_players_w = p.atlas.font.width(&other_players);
    let port_row_w = p.atlas.font.width_str(port_label).max(WIDGET_W);

    let column_w = ROW_W.max(other_players_w).max(port_row_w);
    let column_x = centered_row_x(ctx.vw, column_w);
    let center = |w: f32| column_x + ((column_w - w) / 2.0).floor();
    let top = LAYOUT.content_y(ctx.vh, CONTENT_H);

    let lan_x = center(WIDGET_W);
    let lan_y = top + LAN_Y;
    let caption = generic_value(
        label!("menu.multiplayerOptions.lan"),
        crate::gui::op_blocks::on_off(lan.wanted_lan),
    );
    if cycle(
        p,
        ctx,
        &Button::new(lan_x, lan_y, WIDGET_W, WIDGET_HEIGHT, &caption),
    ) != 0
    {
        lan.wanted_lan = !lan.wanted_lan;
        lan.update_port_controls();
    }
    if ctx.hovering(lan_x, lan_y, WIDGET_W, WIDGET_HEIGHT) {
        tip = Some(if lan.wanted_lan {
            translate("menu.multiplayerOptions.network.lan.tooltip", &[])
        } else {
            translate("menu.multiplayerOptions.network.off.tooltip", &[])
        });
    }

    let port_x = center(port_row_w);
    let label_color = if lan.wanted_lan { 0xFFFFFF } else { GRAY };
    p.text_plain(port_label, port_x, top + PORT_LABEL_Y, label_color, true);
    let box_y = top + PORT_BOX_Y;
    if lan.wanted_lan {
        lan.port_box
            .update(p, ctx, port_x, box_y, WIDGET_W, WIDGET_HEIGHT);
        lan.respond();
    } else {
        focus::next(false);
        lan.port_box.draw(p, port_x, box_y, WIDGET_W, WIDGET_HEIGHT);
    }
    if let Some(error) = lan.error
        && ctx.hovering(port_x, box_y, WIDGET_W, WIDGET_HEIGHT)
    {
        tip = Some(error.message());
    }

    p.text(
        &other_players,
        center(other_players_w),
        top + OTHER_PLAYERS_Y,
        true,
    );

    let row_x = center(ROW_W);
    let row_y = top + ROW_Y;
    let caption = generic_value(label!("selectWorld.gameMode"), lan.game_mode.short_name());
    let button = Button {
        active: !lan.hardcore,
        ..Button::new(row_x, row_y, WIDGET_W, WIDGET_HEIGHT, &caption)
    };
    let delta = cycle(p, ctx, &button);
    if delta != 0 {
        lan.game_mode = step(&LanGameMode::CYCLE, lan.game_mode, delta);
    }
    if lan.hardcore && ctx.hovering(row_x, row_y, WIDGET_W, WIDGET_HEIGHT) {
        tip = Some(translate(
            "options.worldOptions.game_mode.disabled.tooltip",
            &[],
        ));
    }

    let commands_x = row_x + WIDGET_W + CONTENT_SPACING;
    let caption = generic_value(
        label!("selectWorld.allowCommands"),
        crate::gui::op_blocks::on_off(lan.commands),
    );
    let button = Button {
        active: !lan.hardcore,
        ..Button::new(commands_x, row_y, WIDGET_W, WIDGET_HEIGHT, &caption)
    };
    if cycle(p, ctx, &button) != 0 {
        lan.commands = !lan.commands;
    }
    if lan.hardcore && ctx.hovering(commands_x, row_y, WIDGET_W, WIDGET_HEIGHT) {
        tip = Some(translate(
            "options.worldOptions.allow_commands.disabled.tooltip",
            &[],
        ));
    }

    let backed = is_backed(Control::OpenToLan);
    let footer_x = centered_row_x(ctx.vw, 2.0 * FOOTER_W + FOOTER_GAP);
    let footer_y = LAYOUT.footer_y(ctx.vh);
    let apply = Button {
        active: lan.apply_active() && backed,
        ..Button::new(
            footer_x,
            footer_y,
            FOOTER_W,
            WIDGET_HEIGHT,
            label!("menu.multiplayerOptions.applyChanges"),
        )
    };
    if apply.draw(p, ctx) {
        action = Some(Action::Apply);
    }
    if !backed && ctx.hovering(footer_x, footer_y, FOOTER_W, WIDGET_HEIGHT) {
        tip = Some(super::unsupported_text());
    }
    let cancel_x = footer_x + FOOTER_W + FOOTER_GAP;
    let cancel = Button::new(
        cancel_x,
        footer_y,
        FOOTER_W,
        WIDGET_HEIGHT,
        label!("gui.cancel"),
    );
    if cancel.draw(p, ctx) || ctx.input.escape {
        action = Some(Action::Close);
    }

    draw_title(p, ctx, label!("options.multiplayer.title"));

    if let Some(text) = tip {
        super::hover_tooltip(p, ctx, &text);
    }

    match action {
        Some(Action::Apply) => {
            lan.result = Some(lan.wanted_lan.then(|| LanSettings {
                game_mode: lan.game_mode,
                allow_commands: lan.commands,
                port: lan.port as u16,
            }));
            state.nav = Some(Screen::None);
        }
        Some(Action::Close) => state.nav = Some(Screen::Pause),
        None => {}
    }
}

fn background(p: &mut Painter, ctx: &ScreenCtx) {
    let footer_top = ctx.vh - LAYOUT.footer_h;
    let content_h = (footer_top - LAYOUT.header_h).max(0.0);
    p.sprite(
        "menu_list_background",
        0.0,
        LAYOUT.header_h,
        ctx.vw,
        (content_h - 2.0).max(0.0),
    );
    p.sprite("header_separator", 0.0, LAYOUT.header_h - 2.0, ctx.vw, 2.0);
    p.sprite("footer_separator", 0.0, footer_top - 2.0, ctx.vw, 2.0);
}

fn available_port() -> i32 {
    std::net::TcpListener::bind(("0.0.0.0", 0))
        .and_then(|l| l.local_addr())
        .map_or(FALLBACK_PORT, |a| i32::from(a.port()))
}

fn port_available(port: i32) -> bool {
    let Ok(port) = u16::try_from(port) else {
        return false;
    };
    std::net::TcpListener::bind(("0.0.0.0", port))
        .and_then(|l| l.local_addr())
        .is_ok_and(|a| a.port() == port)
}
