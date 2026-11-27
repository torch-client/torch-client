use crate::gui::menu;
use crate::gui::painter::Painter;
use crate::gui::toast::{SystemId, ToastEvent, ToastSettings};
use crate::gui::tooltip::{label, translate};
use crate::gui::widgets::{Button, TextBox, WIDGET_HEIGHT};
use crate::gui::worlds::back_to_list;
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::singleplayer::backed::{Control, is_backed};
use crate::singleplayer::store::{self, WorldSummary};
use crate::text::Style;

const TITLE_Y: f32 = 15.0;
const WIDTH: f32 = 200.0;
const HALF_WIDTH: f32 = 98.0;
const ROW_SPACING: f32 = 4.0;
const NAME_MAX_LEN: usize = 32;
const GRAY: u32 = 0xAAAAAA;
const RED: u32 = 0xFF5555;
const MIB: f64 = 1_048_576.0;

const LABEL_Y: f32 = 25.0;
const NAME_Y: f32 = 39.0;
const RESET_ICON_Y: f32 = 64.0;
const FOLDER_Y: f32 = 89.0;
const BACKUP_Y: f32 = 114.0;
const BACKUP_FOLDER_Y: f32 = 139.0;
const OPTIMIZE_Y: f32 = 164.0;
const SPACER_Y: f32 = 189.0;
const BOTTOM_ROW_Y: f32 = 214.0;
const LAYOUT_H: f32 = 234.0;

#[derive(Default)]
pub struct State {
    folder: String,
    name: TextBox,
    has_icon: bool,
    error: Option<String>,
}

impl State {
    pub fn open(&mut self, world: &WorldSummary) {
        self.folder = world.folder.clone();
        self.name = TextBox::bordered(NAME_MAX_LEN, "");
        self.name.set_text(&world.settings.name);
        self.name.focus();
        self.has_icon = world.icon.is_some();
        self.error = None;
    }
}

enum Action {
    Rename,
    Close,
    ResetIcon,
    Backup,
    Optimize,
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    menu::background(p, ctx.vw, ctx.vh);

    let x = ((ctx.vw - WIDTH) / 2.0).trunc();
    let top = ((ctx.vh - LAYOUT_H) / 2.0).trunc();

    let edit = &mut state.worlds.edit;
    let mut action = None;
    let mut unsupported_hover = false;

    p.text_plain(
        label!("selectWorld.enterName"),
        x,
        top + LABEL_Y,
        GRAY,
        true,
    );
    edit.name
        .update(p, ctx, x, top + NAME_Y, WIDTH, WIDGET_HEIGHT);
    let can_save = !is_blank(&edit.name.text);

    let rows = [
        (
            label!("selectWorld.edit.resetIcon"),
            RESET_ICON_Y,
            edit.has_icon,
            is_backed(Control::ResetIcon),
        ),
        (label!("selectWorld.edit.openFolder"), FOLDER_Y, true, true),
        (
            label!("selectWorld.edit.backup"),
            BACKUP_Y,
            true,
            is_backed(Control::Backup),
        ),
        (
            label!("selectWorld.edit.backupFolder"),
            BACKUP_FOLDER_Y,
            true,
            true,
        ),
        (label!("selectWorld.edit.optimize"), OPTIMIZE_Y, true, true),
    ];
    for (i, &(text, y, active, backed)) in rows.iter().enumerate() {
        let button = Button {
            active: active && backed,
            ..Button::new(x, top + y, WIDTH, WIDGET_HEIGHT, text)
        };
        if !backed && ctx.hovering(x, top + y, WIDTH, WIDGET_HEIGHT) {
            unsupported_hover = true;
        }
        if !button.draw(p, ctx) {
            continue;
        }
        match i {
            0 => action = Some(Action::ResetIcon),
            1 => crate::platform::url::open_folder(&store::world_dir(&edit.folder)),
            2 => action = Some(Action::Backup),
            3 => crate::platform::url::open_folder(&store::backups_dir()),
            _ => action = Some(Action::Optimize),
        }
    }

    let row_y = top + BOTTOM_ROW_Y;
    let save = Button {
        active: can_save,
        ..Button::new(
            x,
            row_y,
            HALF_WIDTH,
            WIDGET_HEIGHT,
            label!("selectWorld.edit.save"),
        )
    };
    if save.draw(p, ctx) {
        action = Some(Action::Rename);
    }
    let cancel = Button::new(
        x + HALF_WIDTH + ROW_SPACING,
        row_y,
        HALF_WIDTH,
        WIDGET_HEIGHT,
        label!("gui.cancel"),
    );
    if cancel.draw(p, ctx) || ctx.input.escape {
        action = Some(Action::Close);
    }
    if ctx.input.enter && edit.name.focused && can_save {
        action = Some(Action::Rename);
    }

    if let Some(error) = &edit.error {
        p.text_centered(error, ctx.vw / 2.0, top + SPACER_Y + 6.0, RED, true);
    }
    p.text_centered(
        label!("selectWorld.edit.title"),
        ctx.vw / 2.0,
        TITLE_Y,
        0xFFFFFF,
        true,
    );

    if unsupported_hover {
        super::unsupported_tip(p, ctx);
    }

    match action {
        Some(Action::Rename) => match store::rename(&edit.folder, &edit.name.text) {
            Ok(()) => back_to_list(state),
            Err(e) => edit.error = Some(e),
        },
        Some(Action::Close) => back_to_list(state),
        Some(Action::ResetIcon) => match store::reset_icon(&edit.folder) {
            Ok(()) => edit.has_icon = false,
            Err(e) => edit.error = Some(e),
        },
        Some(Action::Backup) => {
            let toast = backup_toast(&edit.folder);
            let settings = ToastSettings::from_state(state);
            state.toasts.ingest(vec![toast], &p.atlas.font, settings);
            back_to_list(state);
        }
        Some(Action::Optimize) => {
            let (folder, name) = (edit.folder.clone(), edit.name.text.clone());
            state.worlds.optimize.open(&folder, &name);
            state.nav = Some(Screen::WorldOptimize);
        }
        None => {}
    }
}

fn backup_toast(folder: &str) -> ToastEvent {
    let (title, message) = match store::backup(folder) {
        Ok(size) => {
            let mib = (size as f64 / MIB).ceil() as i64;
            (
                translate("selectWorld.edit.backupCreated", &[folder.to_string()]),
                translate("selectWorld.edit.backupSize", &[mib.to_string()]),
            )
        }
        Err(e) => (translate("selectWorld.edit.backupFailed", &[]), e),
    };
    ToastEvent::System {
        id: SystemId::WorldBackup,
        title: crate::text::styled(&title, Style::default()),
        message: crate::text::styled(&message, Style::default()),
        multiline: false,
    }
}

fn is_blank(s: &str) -> bool {
    s.chars().all(char::is_whitespace)
}
