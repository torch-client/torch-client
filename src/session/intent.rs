use super::*;

pub enum InvAction {
    Click(azalea_inventory::operations::ClickOperation),
    CreativeSet {
        slot: u16,
        stack: SlotStack,
    },
    CreativeDrop {
        stack: SlotStack,
    },
    ButtonClick(u8),
    SelectTrade(u32),
    SetBeacon {
        primary: Option<u32>,
        secondary: Option<u32>,
    },
    RenameItem(String),
    SlotStateChanged {
        slot: u8,
        enabled: bool,
    },
    Close,
}

#[derive(Clone, Copy)]
pub enum DropRequest {
    HeldItem,
    HeldStack,
}

#[derive(Clone, Copy)]
pub struct CommandBlockOpen {
    pub pos: azalea_core::position::BlockPos,
    pub mode: CommandBlockMode,
    pub conditional: bool,
}

pub use azalea_protocol::packets::game::s_set_command_block::Mode as CommandBlockMode;

#[derive(Clone, Default)]
pub struct CommandBlockData {
    pub command: String,
    pub track_output: bool,
    pub last_output: String,
    pub automatic: bool,
}

pub struct CommandBlockUpdate {
    pub pos: azalea_core::position::BlockPos,
    pub command: String,
    pub mode: CommandBlockMode,
    pub track_output: bool,
    pub conditional: bool,
    pub automatic: bool,
}

#[derive(Clone)]
pub struct SignEditRequest {
    pub pos: azalea_core::position::BlockPos,
    pub front: bool,
    pub kind: SignEditKind,
    pub wood: &'static str,
    pub face: crate::blockentities::feed::SignFace,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SignEditKind {
    Standing { wall: bool },
    Hanging,
}

impl SignEditKind {
    pub fn from_block(block: &str) -> Option<(SignEditKind, &'static str)> {
        use crate::blockentities::render::sign;
        use crate::blockentities::text::SignKind;
        let kind = match SignKind::of(crate::blockentities::feed::kind_for_block(block)?)? {
            SignKind::Standing => SignEditKind::Standing {
                wall: sign::is_wall(block),
            },
            SignKind::Hanging => SignEditKind::Hanging,
        };
        Some((kind, sign::known_wood(block).unwrap_or("oak")))
    }

    pub fn sign_kind(self) -> crate::blockentities::text::SignKind {
        match self {
            SignEditKind::Standing { .. } => crate::blockentities::text::SignKind::Standing,
            SignEditKind::Hanging => crate::blockentities::text::SignKind::Hanging,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ServerLink {
    pub label: Vec<crate::text::Span>,
    pub url: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InteractionHand {
    Main,
    Off,
}

impl InteractionHand {
    pub fn edit_book_slot(self, selected: u8) -> u32 {
        match self {
            InteractionHand::Main => selected as u32,
            InteractionHand::Off => 40,
        }
    }

    pub fn hotbar_index(self, selected: u8) -> usize {
        match self {
            InteractionHand::Main => selected as usize,
            InteractionHand::Off => 9,
        }
    }
}

#[derive(Clone)]
pub struct BookEditRequest {
    pub hand: InteractionHand,
    pub author: String,
}

#[derive(Clone)]
pub struct EditBookRequest {
    pub slot: u32,
    pub pages: Vec<String>,
    pub title: Option<String>,
}

#[derive(Clone)]
pub struct SignUpdateRequest {
    pub pos: azalea_core::position::BlockPos,
    pub front: bool,
    pub lines: [String; 4],
}
