use std::sync::Arc;

use crate::play::dialog::{Click, Dialog, Submit};
use crate::session::SharedMutex;

pub enum Followup {
    None,
    Suggest(String),
    Link(String),
    Dialog(Arc<Dialog>),
    Confirm(crate::gui::confirmcommand::CommandPrompt),
}

pub fn dispatch(click: Click, shared: &SharedMutex) -> Followup {
    match click {
        Click::RunCommand(command) => {
            let command = command.strip_prefix('/').unwrap_or(&command).to_string();
            if let Some(prompt) = crate::gui::confirmcommand::check(&command, shared) {
                return Followup::Confirm(prompt);
            }
            crate::play::dialog::queue_submit(shared, Submit::RunCommand(command));
            Followup::None
        }
        Click::Custom { id, payload } => {
            crate::play::dialog::queue_submit(shared, Submit::Custom { id, payload });
            Followup::None
        }
        Click::CopyToClipboard(value) => {
            crate::gui::render::clipboard_set(&value);
            Followup::None
        }
        Click::ChangePage(page) => {
            crate::log_debug!("gui", "ignoring a change_page click to page {page}");
            Followup::None
        }
        Click::SuggestCommand(command) => Followup::Suggest(command),
        Click::OpenUrl(url) => Followup::Link(url),
        Click::ShowDialog(dialog) => Followup::Dialog(dialog),
    }
}
