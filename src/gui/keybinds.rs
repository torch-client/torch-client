use std::sync::LazyLock;

use bevy::input::ButtonInput;
use bevy::prelude::{KeyCode, MouseButton};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Bound {
    #[default]
    Unbound,
    Key(KeyCode),
    Mouse(MouseButton),
}

const UNBOUND_NAME: &str = "Not Bound";

#[rustfmt::skip]
const KEYS: &[(KeyCode, &str, &str)] = &[
    (KeyCode::KeyA, "key.keyboard.a", "A"),
    (KeyCode::KeyB, "key.keyboard.b", "B"),
    (KeyCode::KeyC, "key.keyboard.c", "C"),
    (KeyCode::KeyD, "key.keyboard.d", "D"),
    (KeyCode::KeyE, "key.keyboard.e", "E"),
    (KeyCode::KeyF, "key.keyboard.f", "F"),
    (KeyCode::KeyG, "key.keyboard.g", "G"),
    (KeyCode::KeyH, "key.keyboard.h", "H"),
    (KeyCode::KeyI, "key.keyboard.i", "I"),
    (KeyCode::KeyJ, "key.keyboard.j", "J"),
    (KeyCode::KeyK, "key.keyboard.k", "K"),
    (KeyCode::KeyL, "key.keyboard.l", "L"),
    (KeyCode::KeyM, "key.keyboard.m", "M"),
    (KeyCode::KeyN, "key.keyboard.n", "N"),
    (KeyCode::KeyO, "key.keyboard.o", "O"),
    (KeyCode::KeyP, "key.keyboard.p", "P"),
    (KeyCode::KeyQ, "key.keyboard.q", "Q"),
    (KeyCode::KeyR, "key.keyboard.r", "R"),
    (KeyCode::KeyS, "key.keyboard.s", "S"),
    (KeyCode::KeyT, "key.keyboard.t", "T"),
    (KeyCode::KeyU, "key.keyboard.u", "U"),
    (KeyCode::KeyV, "key.keyboard.v", "V"),
    (KeyCode::KeyW, "key.keyboard.w", "W"),
    (KeyCode::KeyX, "key.keyboard.x", "X"),
    (KeyCode::KeyY, "key.keyboard.y", "Y"),
    (KeyCode::KeyZ, "key.keyboard.z", "Z"),
    (KeyCode::Digit0, "key.keyboard.0", "0"),
    (KeyCode::Digit1, "key.keyboard.1", "1"),
    (KeyCode::Digit2, "key.keyboard.2", "2"),
    (KeyCode::Digit3, "key.keyboard.3", "3"),
    (KeyCode::Digit4, "key.keyboard.4", "4"),
    (KeyCode::Digit5, "key.keyboard.5", "5"),
    (KeyCode::Digit6, "key.keyboard.6", "6"),
    (KeyCode::Digit7, "key.keyboard.7", "7"),
    (KeyCode::Digit8, "key.keyboard.8", "8"),
    (KeyCode::Digit9, "key.keyboard.9", "9"),
    (KeyCode::F1, "key.keyboard.f1", "F1"),
    (KeyCode::F2, "key.keyboard.f2", "F2"),
    (KeyCode::F3, "key.keyboard.f3", "F3"),
    (KeyCode::F4, "key.keyboard.f4", "F4"),
    (KeyCode::F5, "key.keyboard.f5", "F5"),
    (KeyCode::F6, "key.keyboard.f6", "F6"),
    (KeyCode::F7, "key.keyboard.f7", "F7"),
    (KeyCode::F8, "key.keyboard.f8", "F8"),
    (KeyCode::F9, "key.keyboard.f9", "F9"),
    (KeyCode::F10, "key.keyboard.f10", "F10"),
    (KeyCode::F11, "key.keyboard.f11", "F11"),
    (KeyCode::F12, "key.keyboard.f12", "F12"),
    (KeyCode::F13, "key.keyboard.f13", "F13"),
    (KeyCode::F14, "key.keyboard.f14", "F14"),
    (KeyCode::F15, "key.keyboard.f15", "F15"),
    (KeyCode::F16, "key.keyboard.f16", "F16"),
    (KeyCode::F17, "key.keyboard.f17", "F17"),
    (KeyCode::F18, "key.keyboard.f18", "F18"),
    (KeyCode::F19, "key.keyboard.f19", "F19"),
    (KeyCode::F20, "key.keyboard.f20", "F20"),
    (KeyCode::F21, "key.keyboard.f21", "F21"),
    (KeyCode::F22, "key.keyboard.f22", "F22"),
    (KeyCode::F23, "key.keyboard.f23", "F23"),
    (KeyCode::F24, "key.keyboard.f24", "F24"),
    (KeyCode::F25, "key.keyboard.f25", "F25"),
    (KeyCode::Space, "key.keyboard.space", "Space"),
    (KeyCode::Enter, "key.keyboard.enter", "Enter"),
    (KeyCode::Tab, "key.keyboard.tab", "Tab"),
    (KeyCode::Backspace, "key.keyboard.backspace", "Backspace"),
    (KeyCode::Delete, "key.keyboard.delete", "Delete"),
    (KeyCode::Insert, "key.keyboard.insert", "Insert"),
    (KeyCode::Home, "key.keyboard.home", "Home"),
    (KeyCode::End, "key.keyboard.end", "End"),
    (KeyCode::PageUp, "key.keyboard.page.up", "Page Up"),
    (KeyCode::PageDown, "key.keyboard.page.down", "Page Down"),
    (KeyCode::ArrowLeft, "key.keyboard.left", "Left Arrow"),
    (KeyCode::ArrowRight, "key.keyboard.right", "Right Arrow"),
    (KeyCode::ArrowUp, "key.keyboard.up", "Up Arrow"),
    (KeyCode::ArrowDown, "key.keyboard.down", "Down Arrow"),
    (KeyCode::Escape, "key.keyboard.escape", "Escape"),
    (KeyCode::CapsLock, "key.keyboard.caps.lock", "Caps Lock"),
    (KeyCode::NumLock, "key.keyboard.num.lock", "Num Lock"),
    (KeyCode::ScrollLock, "key.keyboard.scroll.lock", "Scroll Lock"),
    (KeyCode::PrintScreen, "key.keyboard.print.screen", "Print Screen"),
    (KeyCode::Pause, "key.keyboard.pause", "Pause"),
    (KeyCode::ShiftLeft, "key.keyboard.left.shift", "Left Shift"),
    (KeyCode::ShiftRight, "key.keyboard.right.shift", "Right Shift"),
    (KeyCode::ControlLeft, "key.keyboard.left.control", "Left Control"),
    (KeyCode::ControlRight, "key.keyboard.right.control", "Right Control"),
    (KeyCode::AltLeft, "key.keyboard.left.alt", "Left Alt"),
    (KeyCode::AltRight, "key.keyboard.right.alt", "Right Alt"),
    (KeyCode::SuperLeft, "key.keyboard.left.win", "Left Win"),
    (KeyCode::SuperRight, "key.keyboard.right.win", "Right Win"),
    (KeyCode::ContextMenu, "key.keyboard.menu", "Menu"),
    (KeyCode::Minus, "key.keyboard.minus", "-"),
    (KeyCode::Equal, "key.keyboard.equal", "="),
    (KeyCode::BracketLeft, "key.keyboard.left.bracket", "["),
    (KeyCode::BracketRight, "key.keyboard.right.bracket", "]"),
    (KeyCode::Backslash, "key.keyboard.backslash", "\\"),
    (KeyCode::Semicolon, "key.keyboard.semicolon", ";"),
    (KeyCode::Quote, "key.keyboard.apostrophe", "'"),
    (KeyCode::Comma, "key.keyboard.comma", ","),
    (KeyCode::Period, "key.keyboard.period", "."),
    (KeyCode::Slash, "key.keyboard.slash", "/"),
    (KeyCode::Backquote, "key.keyboard.grave.accent", "`"),
    (KeyCode::Numpad0, "key.keyboard.keypad.0", "Keypad 0"),
    (KeyCode::Numpad1, "key.keyboard.keypad.1", "Keypad 1"),
    (KeyCode::Numpad2, "key.keyboard.keypad.2", "Keypad 2"),
    (KeyCode::Numpad3, "key.keyboard.keypad.3", "Keypad 3"),
    (KeyCode::Numpad4, "key.keyboard.keypad.4", "Keypad 4"),
    (KeyCode::Numpad5, "key.keyboard.keypad.5", "Keypad 5"),
    (KeyCode::Numpad6, "key.keyboard.keypad.6", "Keypad 6"),
    (KeyCode::Numpad7, "key.keyboard.keypad.7", "Keypad 7"),
    (KeyCode::Numpad8, "key.keyboard.keypad.8", "Keypad 8"),
    (KeyCode::Numpad9, "key.keyboard.keypad.9", "Keypad 9"),
    (KeyCode::NumpadAdd, "key.keyboard.keypad.add", "Keypad +"),
    (KeyCode::NumpadSubtract, "key.keyboard.keypad.subtract", "Keypad -"),
    (KeyCode::NumpadMultiply, "key.keyboard.keypad.multiply", "Keypad *"),
    (KeyCode::NumpadDivide, "key.keyboard.keypad.divide", "Keypad /"),
    (KeyCode::NumpadDecimal, "key.keyboard.keypad.decimal", "Keypad Decimal"),
    (KeyCode::NumpadEnter, "key.keyboard.keypad.enter", "Keypad Enter"),
    (KeyCode::NumpadEqual, "key.keyboard.keypad.equal", "Keypad ="),
];

const MOUSE: &[(MouseButton, &str, &str)] = &[
    (MouseButton::Left, "key.mouse.left", "Left Button"),
    (MouseButton::Right, "key.mouse.right", "Right Button"),
    (MouseButton::Middle, "key.mouse.middle", "Middle Button"),
    (MouseButton::Back, "key.mouse.4", "Button 4"),
    (MouseButton::Forward, "key.mouse.5", "Button 5"),
];

#[cfg(feature = "click_gui")]
const MOUSE_TAG: u32 = 1 << 16;

impl Bound {
    pub fn serialized_name(self) -> &'static str {
        match self {
            Bound::Unbound => "key.keyboard.unknown",
            Bound::Key(code) => KEYS
                .iter()
                .find(|(k, ..)| *k == code)
                .map(|(_, name, _)| *name)
                .unwrap_or("key.keyboard.unknown"),
            Bound::Mouse(button) => MOUSE
                .iter()
                .find(|(b, ..)| *b == button)
                .map(|(_, name, _)| *name)
                .unwrap_or("key.keyboard.unknown"),
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Bound::Unbound => UNBOUND_NAME,
            Bound::Key(code) => KEYS
                .iter()
                .find(|(k, ..)| *k == code)
                .map(|(.., display)| *display)
                .unwrap_or(UNBOUND_NAME),
            Bound::Mouse(button) => MOUSE
                .iter()
                .find(|(b, ..)| *b == button)
                .map(|(.., display)| *display)
                .unwrap_or(UNBOUND_NAME),
        }
    }

    pub(crate) fn from_serialized_name(name: &str) -> Bound {
        if let Some((code, ..)) = KEYS.iter().find(|(_, n, _)| *n == name) {
            return Bound::Key(*code);
        }
        if let Some((button, ..)) = MOUSE.iter().find(|(_, n, _)| *n == name) {
            return Bound::Mouse(*button);
        }
        Bound::Unbound
    }

    #[cfg(feature = "click_gui")]
    pub fn to_bits(self) -> u32 {
        match self {
            Bound::Unbound => 0,
            Bound::Key(code) => KEYS
                .iter()
                .position(|(k, ..)| *k == code)
                .map_or(0, |i| 1 + i as u32),
            Bound::Mouse(button) => MOUSE
                .iter()
                .position(|(b, ..)| *b == button)
                .map_or(0, |i| MOUSE_TAG + i as u32),
        }
    }

    #[cfg(feature = "click_gui")]
    pub fn from_bits(bits: u32) -> Bound {
        if bits == 0 {
            return Bound::Unbound;
        }
        if bits >= MOUSE_TAG {
            return MOUSE
                .get((bits - MOUSE_TAG) as usize)
                .map_or(Bound::Unbound, |(b, ..)| Bound::Mouse(*b));
        }
        KEYS.get((bits - 1) as usize)
            .map_or(Bound::Unbound, |(k, ..)| Bound::Key(*k))
    }

    fn bindable(self) -> bool {
        !matches!(self, Bound::Key(KeyCode::Escape) | Bound::Unbound)
    }
}

pub struct BindDef {
    pub action: Action,
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub default: Bound,
}

const MOVEMENT: &str = "Movement";
const MISC: &str = "Miscellaneous";
const MULTIPLAYER: &str = "Multiplayer";
const GAMEPLAY: &str = "Gameplay";
const INVENTORY: &str = "Inventory";
const SPECTATOR: &str = "Spectator";
const DEBUG: &str = "Debug";
const CLIENT: &str = "Client";

#[rustfmt::skip]
pub static BINDS: LazyLock<Vec<BindDef>> = LazyLock::new(|| {
    #[cfg(target_arch = "wasm32")]
    const SPRINT: KeyCode = KeyCode::KeyR;
    #[cfg(not(target_arch = "wasm32"))]
    const SPRINT: KeyCode = KeyCode::ControlLeft;

    #[allow(unused_mut)]
    let mut binds = vec![
    BindDef { action: Action::Forward, id: "key.forward", name: "Walk Forward", category: MOVEMENT, default: Bound::Key(KeyCode::KeyW) },
    BindDef { action: Action::Left, id: "key.left", name: "Strafe Left", category: MOVEMENT, default: Bound::Key(KeyCode::KeyA) },
    BindDef { action: Action::Back, id: "key.back", name: "Walk Backward", category: MOVEMENT, default: Bound::Key(KeyCode::KeyS) },
    BindDef { action: Action::Right, id: "key.right", name: "Strafe Right", category: MOVEMENT, default: Bound::Key(KeyCode::KeyD) },
    BindDef { action: Action::Jump, id: "key.jump", name: "Jump", category: MOVEMENT, default: Bound::Key(KeyCode::Space) },
    BindDef { action: Action::Sneak, id: "key.sneak", name: "Sneak", category: MOVEMENT, default: Bound::Key(KeyCode::ShiftLeft) },
    BindDef { action: Action::Sprint, id: "key.sprint", name: "Sprint", category: MOVEMENT, default: Bound::Key(SPRINT) },

    BindDef { action: Action::TogglePerspective, id: "key.togglePerspective", name: "Toggle Perspective", category: MISC, default: Bound::Key(KeyCode::F5) },
    BindDef { action: Action::ToggleGui, id: "key.toggleGui", name: "Toggle GUI", category: MISC, default: Bound::Key(KeyCode::F1) },
    BindDef { action: Action::Fullscreen, id: "key.fullscreen", name: "Toggle Fullscreen", category: MISC, default: Bound::Key(KeyCode::F11) },
    BindDef { action: Action::Screenshot, id: "key.screenshot", name: "Take Screenshot", category: MISC, default: Bound::Key(KeyCode::F2) },

    BindDef { action: Action::Chat, id: "key.chat", name: "Open Chat", category: MULTIPLAYER, default: Bound::Key(KeyCode::KeyT) },
    BindDef { action: Action::PlayerList, id: "key.playerlist", name: "List Players", category: MULTIPLAYER, default: Bound::Key(KeyCode::Tab) },
    BindDef { action: Action::Command, id: "key.command", name: "Open Command", category: MULTIPLAYER, default: Bound::Key(KeyCode::Slash) },

    BindDef { action: Action::Attack, id: "key.attack", name: "Attack/Destroy", category: GAMEPLAY, default: Bound::Mouse(MouseButton::Left) },
    BindDef { action: Action::Use, id: "key.use", name: "Use Item/Place Block", category: GAMEPLAY, default: Bound::Mouse(MouseButton::Right) },
    BindDef { action: Action::PickItem, id: "key.pickItem", name: "Pick Block", category: GAMEPLAY, default: Bound::Mouse(MouseButton::Middle) },
    BindDef { action: Action::SpectatorAction, id: "key.spectatorHotbar", name: "Spectator Menu", category: SPECTATOR, default: Bound::Mouse(MouseButton::Middle) },

    BindDef { action: Action::Inventory, id: "key.inventory", name: "Open/Close Inventory", category: INVENTORY, default: Bound::Key(KeyCode::KeyE) },
    BindDef { action: Action::SwapOffhand, id: "key.swapOffhand", name: "Swap Item With Off Hand", category: INVENTORY, default: Bound::Key(KeyCode::KeyF) },
    BindDef { action: Action::Drop, id: "key.drop", name: "Drop Selected Item", category: INVENTORY, default: Bound::Key(KeyCode::KeyQ) },
    BindDef { action: Action::Hotbar1, id: "key.hotbar.1", name: "Hotbar Slot 1", category: INVENTORY, default: Bound::Key(KeyCode::Digit1) },
    BindDef { action: Action::Hotbar2, id: "key.hotbar.2", name: "Hotbar Slot 2", category: INVENTORY, default: Bound::Key(KeyCode::Digit2) },
    BindDef { action: Action::Hotbar3, id: "key.hotbar.3", name: "Hotbar Slot 3", category: INVENTORY, default: Bound::Key(KeyCode::Digit3) },
    BindDef { action: Action::Hotbar4, id: "key.hotbar.4", name: "Hotbar Slot 4", category: INVENTORY, default: Bound::Key(KeyCode::Digit4) },
    BindDef { action: Action::Hotbar5, id: "key.hotbar.5", name: "Hotbar Slot 5", category: INVENTORY, default: Bound::Key(KeyCode::Digit5) },
    BindDef { action: Action::Hotbar6, id: "key.hotbar.6", name: "Hotbar Slot 6", category: INVENTORY, default: Bound::Key(KeyCode::Digit6) },
    BindDef { action: Action::Hotbar7, id: "key.hotbar.7", name: "Hotbar Slot 7", category: INVENTORY, default: Bound::Key(KeyCode::Digit7) },
    BindDef { action: Action::Hotbar8, id: "key.hotbar.8", name: "Hotbar Slot 8", category: INVENTORY, default: Bound::Key(KeyCode::Digit8) },
    BindDef { action: Action::Hotbar9, id: "key.hotbar.9", name: "Hotbar Slot 9", category: INVENTORY, default: Bound::Key(KeyCode::Digit9) },

    BindDef { action: Action::DebugModifier, id: "key.debug.modifier", name: "Debug Modifier Key", category: DEBUG, default: Bound::Key(KeyCode::F3) },
    BindDef { action: Action::ReloadChunks, id: "key.debug.reloadChunk", name: "Reload Chunks", category: DEBUG, default: Bound::Key(KeyCode::KeyA) },
    BindDef { action: Action::ChunkBorders, id: "key.debug.showChunkBorders", name: "Show Chunk Boundaries", category: DEBUG, default: Bound::Key(KeyCode::KeyG) },
    BindDef { action: Action::AdvancedTooltips, id: "key.debug.showAdvancedTooltips", name: "Show Advanced Tooltips", category: DEBUG, default: Bound::Key(KeyCode::KeyH) },

    BindDef { action: Action::Freecam, id: "key.client.freecam", name: "Detached Camera", category: CLIENT, default: Bound::Key(KeyCode::KeyC) },
    ];
    #[cfg(target_arch = "wasm32")]
    binds.push(BindDef { action: Action::Pause, id: "key.client.pause", name: "Pause Menu", category: CLIENT, default: Bound::Key(KeyCode::Backquote) });
    #[cfg(feature = "click_gui")]
    binds.push(BindDef { action: Action::ClickGui, id: "key.client.clickGui", name: "Click GUI", category: CLIENT, default: Bound::Key(KeyCode::ShiftRight) });
    binds
});

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Forward,
    Left,
    Back,
    Right,
    Jump,
    Sneak,
    Sprint,
    TogglePerspective,
    ToggleGui,
    Fullscreen,
    Screenshot,
    Chat,
    PlayerList,
    Command,
    Attack,
    Use,
    PickItem,
    SpectatorAction,
    Inventory,
    SwapOffhand,
    Drop,
    Hotbar1,
    Hotbar2,
    Hotbar3,
    Hotbar4,
    Hotbar5,
    Hotbar6,
    Hotbar7,
    Hotbar8,
    Hotbar9,
    DebugModifier,
    ReloadChunks,
    ChunkBorders,
    AdvancedTooltips,
    Freecam,
    #[cfg(target_arch = "wasm32")]
    Pause,
    #[cfg(feature = "click_gui")]
    ClickGui,
}

pub const HOTBAR: [Action; 9] = [
    Action::Hotbar1,
    Action::Hotbar2,
    Action::Hotbar3,
    Action::Hotbar4,
    Action::Hotbar5,
    Action::Hotbar6,
    Action::Hotbar7,
    Action::Hotbar8,
    Action::Hotbar9,
];

#[derive(Clone, Debug)]
pub struct KeyBinds {
    bound: Vec<Bound>,
    pub capturing: Option<usize>,
    pub escape_guard: bool,
}

impl Default for KeyBinds {
    fn default() -> KeyBinds {
        let stored = crate::platform::storage::KEYBINDS
            .load()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
        let bound = BINDS
            .iter()
            .map(|def| {
                stored
                    .as_ref()
                    .and_then(|json| json.get(def.id))
                    .and_then(|v| v.as_str())
                    .map(Bound::from_serialized_name)
                    .unwrap_or(def.default)
            })
            .collect();
        KeyBinds {
            bound,
            capturing: None,
            escape_guard: false,
        }
    }
}

impl KeyBinds {
    pub fn bound(&self, action: Action) -> Bound {
        self.row(action as usize)
    }

    pub fn row(&self, i: usize) -> Bound {
        self.bound.get(i).copied().unwrap_or_default()
    }

    pub fn set(&mut self, i: usize, key: Bound) {
        if let Some(slot) = self.bound.get_mut(i) {
            *slot = key;
            self.save();
        }
    }

    pub fn is_default(&self, i: usize) -> bool {
        BINDS.get(i).is_some_and(|def| def.default == self.row(i))
    }

    pub fn any_rebound(&self) -> bool {
        (0..BINDS.len()).any(|i| !self.is_default(i))
    }

    pub fn reset(&mut self, i: usize) {
        if let Some(def) = BINDS.get(i) {
            self.set(i, def.default);
        }
    }

    pub fn reset_all(&mut self) {
        for (slot, def) in self.bound.iter_mut().zip(BINDS.iter()) {
            *slot = def.default;
        }
        self.save();
    }

    pub fn collisions(&self, i: usize) -> Vec<&'static str> {
        let key = self.row(i);
        if key == Bound::Unbound {
            return Vec::new();
        }
        (0..BINDS.len())
            .filter(|j| {
                *j != i && self.row(*j) == key && !(self.is_default(*j) && self.is_default(i))
            })
            .map(|j| BINDS[j].name)
            .collect()
    }

    pub fn key(&self, action: Action) -> Option<KeyCode> {
        match self.bound(action) {
            Bound::Key(code) => Some(code),
            _ => None,
        }
    }

    pub fn down(
        &self,
        action: Action,
        keys: &ButtonInput<KeyCode>,
        buttons: &ButtonInput<MouseButton>,
    ) -> bool {
        match self.bound(action) {
            Bound::Unbound => false,
            Bound::Key(code) => keys.pressed(code),
            Bound::Mouse(button) => buttons.pressed(button),
        }
    }

    pub fn just(
        &self,
        action: Action,
        keys: &ButtonInput<KeyCode>,
        buttons: &ButtonInput<MouseButton>,
    ) -> bool {
        match self.bound(action) {
            Bound::Unbound => false,
            Bound::Key(code) => keys.just_pressed(code),
            Bound::Mouse(button) => buttons.just_pressed(button),
        }
    }

    pub fn resolve_capture(&mut self, key: Option<KeyCode>, button: Option<MouseButton>) -> bool {
        let Some(i) = self.capturing else {
            return false;
        };
        if key == Some(KeyCode::Escape) {
            self.capturing = None;
            self.escape_guard = true;
            self.set(i, Bound::Unbound);
            return true;
        }
        let pressed = match (key, button) {
            (Some(code), _) => Bound::Key(code),
            (None, Some(button)) => Bound::Mouse(button),
            (None, None) => return true,
        };
        if !pressed.bindable() || pressed.serialized_name() == "key.keyboard.unknown" {
            return true;
        }
        self.capturing = None;
        self.set(i, pressed);
        true
    }

    fn save(&self) {
        let map: serde_json::Map<String, serde_json::Value> = BINDS
            .iter()
            .zip(&self.bound)
            .map(|(def, key)| (def.id.to_string(), key.serialized_name().into()))
            .collect();
        crate::platform::storage::KEYBINDS.store(&serde_json::Value::Object(map).to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_are_in_action_order() {
        for (i, def) in BINDS.iter().enumerate() {
            assert_eq!(def.action as usize, i, "{}", def.id);
        }
        assert_eq!(HOTBAR[8] as usize, Action::Hotbar9 as usize);
    }

    #[test]
    fn names_round_trip() {
        for (code, name, _) in KEYS {
            assert_eq!(Bound::from_serialized_name(name), Bound::Key(*code));
            assert_eq!(Bound::Key(*code).serialized_name(), *name);
        }
        for (button, name, _) in MOUSE {
            assert_eq!(Bound::from_serialized_name(name), Bound::Mouse(*button));
        }
        assert_eq!(Bound::from_serialized_name("nonsense"), Bound::Unbound);
        assert_eq!(Bound::Unbound.display_name(), UNBOUND_NAME);
    }

    #[test]
    fn shared_defaults_are_not_a_collision() {
        let mut binds = KeyBinds {
            bound: BINDS.iter().map(|d| d.default).collect(),
            capturing: None,
            escape_guard: false,
        };
        let left = Action::Left as usize;
        let reload = Action::ReloadChunks as usize;
        assert_eq!(binds.row(left), binds.row(reload));
        assert!(binds.collisions(left).is_empty());

        binds.bound[Action::Jump as usize] = Bound::Key(KeyCode::KeyA);
        assert_eq!(
            binds.collisions(Action::Jump as usize),
            vec!["Strafe Left", "Reload Chunks"]
        );
    }
}
