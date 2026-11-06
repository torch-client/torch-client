use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::gui::hud_layout::{Layer, Rect};
use crate::gui::painter::Painter;
use crate::gui::render::GuiInput;
use crate::gui::widgets::Button;
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::platform::storage::TUTORIAL;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Spot {
    Credits,
    EditProfile,
    Accounts,
    Version,
    Video,
    Tab(Layer),
    ChatButton,
    Swap,
    LookArea,
    ZoomArea,
    NewArea,
    Reset,
    Done,
    EditHud,
    Storage,
    Restart,
    ShaderButton,
    ShaderFolder,
    ShaderList,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Home,
    Title,
    Accounts,
    Multiplayer,
    Video,
    #[cfg(all(feature = "shader_support", not(target_arch = "wasm32")))]
    ShaderPacks,
    #[cfg(feature = "hud_editor")]
    Editor(Layer),
    #[cfg(feature = "mobile_ui")]
    Chat,
    Options,
    Storage,
}

struct Step {
    place: Place,
    spot: Option<Spot>,
    text: &'static str,
}

static STEPS: std::sync::LazyLock<Vec<Step>> = std::sync::LazyLock::new(steps);

fn steps() -> Vec<Step> {
    #[allow(unused_mut)]
    let mut steps = vec![
        Step {
            place: Place::Home,
            spot: None,
            text: "A quick tour. Skip leaves it at any point.",
        },
        Step {
            place: Place::Title,
            spot: Some(Spot::Credits),
            text: "Credits: the licence and the project's links.",
        },
        Step {
            place: Place::Title,
            spot: Some(Spot::EditProfile),
            text: "Edit Profile sets the name you join with.",
        },
        Step {
            place: Place::Accounts,
            spot: Some(Spot::Accounts),
            text: "Add an account, then Select it to play as it.",
        },
        Step {
            place: Place::Multiplayer,
            spot: Some(Spot::Version),
            text: "Picking a version here forces it. Left alone, the client matches each server.",
        },
        Step {
            place: Place::Video,
            spot: Some(Spot::Video),
            text: "Video Settings: render distance, GUI scale, brightness.",
        },
    ];
    #[cfg(all(feature = "shader_support", not(target_arch = "wasm32")))]
    steps.extend([
    Step {
        place: Place::Video,
        spot: Some(Spot::ShaderButton),
        text: "Shaders: open Shader Packs at the bottom of Video Settings.",
    },
    Step {
        place: Place::ShaderPacks,
        spot: Some(Spot::ShaderFolder),
        text: if cfg!(target_os = "android") {
            "Put a shader pack zip in this folder. R reads the folder again."
        } else {
            "Put a shader pack zip in this folder, or drop it on the window. R reads the folder again."
        },
    },
    Step {
        place: Place::ShaderPacks,
        spot: Some(Spot::ShaderList),
        text: "Tap a pack to turn it on. (none) turns shaders off.",
    },
    ]);
    #[cfg(all(feature = "hud_editor", feature = "mobile_ui"))]
    steps.extend([
        Step {
            place: Place::Editor(Layer::Areas),
            spot: Some(Spot::Tab(Layer::Areas)),
            text: "The HUD editor. Areas are touch zones.",
        },
        Step {
            place: Place::Editor(Layer::Areas),
            spot: Some(Spot::LookArea),
            text: "Look area: only turns the camera.",
        },
        Step {
            place: Place::Editor(Layer::Areas),
            spot: Some(Spot::ZoomArea),
            text: "Zoom: double tap and hold. Triple tap locks it.",
        },
        Step {
            place: Place::Editor(Layer::Areas),
            spot: None,
            text: "Elsewhere: tap attacks, hold mines, double tap places.",
        },
        Step {
            place: Place::Editor(Layer::Areas),
            spot: Some(Spot::NewArea),
            text: "Adds an area. Drag to move, drag an edge to resize.",
        },
        Step {
            place: Place::Editor(Layer::Controls),
            spot: Some(Spot::Tab(Layer::Controls)),
            text: "Touch buttons. Drag to move, pinch to resize.",
        },
        Step {
            place: Place::Editor(Layer::Controls),
            spot: Some(Spot::ChatButton),
            text: "Chat. Its SWAP button turns chat portrait.",
        },
    ]);
    #[cfg(feature = "mobile_ui")]
    steps.push(Step {
        place: Place::Chat,
        spot: Some(Spot::Swap),
        text: "SWAP: chat like a messaging app. Tap again to go back.",
    });
    #[cfg(feature = "hud_editor")]
    steps.extend([
        Step {
            place: Place::Editor(Layer::Extras),
            spot: Some(Spot::Tab(Layer::Extras)),
            text: "The HUD itself: hotbar, chat, coordinates.",
        },
        Step {
            place: Place::Editor(Layer::Extras),
            spot: Some(Spot::Reset),
            text: "Resets only the open tab.",
        },
        Step {
            place: Place::Editor(Layer::Extras),
            spot: Some(Spot::Done),
            text: "Saves the layout.",
        },
        Step {
            place: Place::Options,
            spot: Some(Spot::EditHud),
            text: "Reopen the editor from here.",
        },
    ]);
    steps.extend([
        Step {
            place: Place::Options,
            spot: Some(Spot::Storage),
            text: "Resets live in Manage Storage.",
        },
        Step {
            place: Place::Storage,
            spot: Some(Spot::Restart),
            text: "Replays this tour.",
        },
    ]);
    steps
}

const DIM: u32 = 0xB000_0000;
const LINE: u32 = 0xFFFF_FF55;
const PLATE: u32 = 0xF010_1010;
const TEXT: u32 = 0xFFFFFF;
const PAD: f32 = 3.0;
const GAP: f32 = 6.0;
const INSET: f32 = 4.0;
const LINE_H: f32 = 10.0;
const CAPTION_W: f32 = 200.0;
const BUTTON_W: f32 = 56.0;
const BUTTON_H: f32 = 20.0;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static SPOTS: Mutex<Vec<(Spot, Rect)>> = Mutex::new(Vec::new());

#[derive(Default)]
pub struct Tutorial {
    checked: bool,
    run: Option<Run>,
}

#[derive(Clone, Copy)]
struct Run {
    step: usize,
    home: Screen,
    in_game: bool,
    arrived: bool,
}

impl Run {
    fn shown(&self, i: usize) -> bool {
        let swap = crate::platform::orientation::SUPPORTED;
        match (STEPS[i].place, STEPS[i].spot) {
            (Place::Title | Place::Accounts | Place::Multiplayer, _) => !self.in_game,
            #[cfg(feature = "mobile_ui")]
            (Place::Chat, _) => self.in_game && swap,
            (_, Some(Spot::ChatButton)) => swap,
            _ => true,
        }
    }

    fn next(&self) -> Option<usize> {
        (self.step + 1..STEPS.len()).find(|&i| self.shown(i))
    }

    fn prev(&self) -> Option<usize> {
        (0..self.step).rev().find(|&i| self.shown(i))
    }
}

#[cfg(self_update)]
pub fn running() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

pub fn mark(spot: Spot, r: Rect) {
    if ACTIVE.load(Ordering::Relaxed)
        && let Ok(mut spots) = SPOTS.lock()
    {
        spots.push((spot, r));
    }
}

pub fn mark_in(p: &Painter, spot: Spot, r: Rect) {
    let (s, x, y) = p.xform();
    mark(spot, Rect::new(x + r.x * s, y + r.y * s, r.w * s, r.h * s));
}

pub fn restart(state: &mut GuiState) {
    TUTORIAL.store("");
    start(state);
}

fn start(state: &mut GuiState) {
    if state.screen == Screen::Title {
        state.options_parent = Screen::Title;
    }
    state.tutorial.run = Some(Run {
        step: 0,
        home: state.screen,
        in_game: !state.in_menu(),
        arrived: false,
    });
}

fn finish(state: &mut GuiState, go_home: bool) {
    if let Some(run) = state.tutorial.run.take() {
        TUTORIAL.store("1");
        if go_home && state.screen != run.home {
            state.nav = Some(run.home);
        }
    }
}

fn want(place: Place, home: Screen) -> Screen {
    match place {
        Place::Home => home,
        Place::Title => Screen::Title,
        Place::Accounts => Screen::Accounts,
        Place::Multiplayer => Screen::Multiplayer,
        Place::Video => Screen::VideoSettings,
        #[cfg(all(feature = "shader_support", not(target_arch = "wasm32")))]
        Place::ShaderPacks => Screen::ShaderPacks,
        #[cfg(feature = "hud_editor")]
        Place::Editor(_) => Screen::HudEditor,
        #[cfg(feature = "mobile_ui")]
        Place::Chat => Screen::Chat,
        Place::Options => Screen::Options,
        Place::Storage => Screen::ManageStorage,
    }
}

#[cfg_attr(not(feature = "hud_editor"), allow(unused_variables))]
fn go(state: &mut GuiState, screen: Screen, home: Screen) {
    if state.screen == Screen::Chat {
        state.chat.close();
    }
    match screen {
        Screen::Chat => state.chat.open(""),
        #[cfg(feature = "hud_editor")]
        Screen::HudEditor => {
            state.hud_state.parent = home;
            state.hud_state.over_menu = state.in_menu();
        }
        Screen::Accounts => state.profile.open(),
        Screen::VideoSettings => {
            state.video_scroll = 0.0;
            state.video_scroll_drag = None;
        }
        #[cfg(feature = "shader_support")]
        Screen::ShaderPacks => state.shaderpacks.refresh(),
        _ => {}
    }
    state.nav = Some(screen);
}

pub fn begin(state: &mut GuiState, input: &GuiInput) -> Option<GuiInput> {
    if !state.tutorial.checked && state.screen == Screen::Title {
        state.tutorial.checked = true;
        if TUTORIAL.load().is_none_or(|s| s.trim().is_empty()) {
            start(state);
        }
    }
    if let Ok(mut spots) = SPOTS.lock() {
        spots.clear();
    }
    ACTIVE.store(state.tutorial.run.is_some(), Ordering::Relaxed);
    let run = state.tutorial.run?;
    #[cfg(feature = "hud_editor")]
    if state.screen == Screen::HudEditor {
        if let Place::Editor(layer) = STEPS[run.step].place {
            state.hud_state.frames.layer = layer;
        }
        #[cfg(feature = "mobile_ui")]
        state.hud_state.set_touches(std::iter::empty());
    }
    if state.screen == Screen::VideoSettings && STEPS[run.step].spot == Some(Spot::ShaderButton) {
        state.video_scroll = f32::MAX;
    }
    Some(GuiInput {
        size: input.size,
        scale: input.scale,
        time: input.time,
        device_scale: input.device_scale,
        ..Default::default()
    })
}

pub fn draw(p: &mut Painter, state: &mut GuiState, input: &GuiInput) {
    let Some(mut run) = state.tutorial.run else {
        return;
    };
    let step = &STEPS[run.step];
    let screen = want(step.place, run.home);
    if state.screen == screen {
        run.arrived = true;
    } else if run.arrived {
        finish(state, false);
        return;
    } else if state.nav.is_none() {
        go(state, screen, run.home);
    }

    let target = step
        .spot
        .filter(|_| state.screen == screen)
        .and_then(|s| match s {
            Spot::ChatButton => {
                state.hud_state.frames.screen[crate::gui::hud_layout::ElementId::ChatButton.index()]
            }
            _ => SPOTS
                .lock()
                .ok()?
                .iter()
                .find(|(k, _)| *k == s)
                .map(|(_, r)| *r),
        });
    let (vw, vh) = (input.size.x, input.size.y);
    match target {
        Some(r) => {
            let (x0, y0, x1, y1) = (r.x - PAD, r.y - PAD, r.x + r.w + PAD, r.y + r.h + PAD);
            p.fill(0.0, 0.0, vw, y0.max(0.0), DIM);
            p.fill(0.0, y1, vw, (vh - y1).max(0.0), DIM);
            p.fill(0.0, y0, x0.max(0.0), y1 - y0, DIM);
            p.fill(x1, y0, (vw - x1).max(0.0), y1 - y0, DIM);
            p.outline(x0, y0, x1 - x0, y1 - y0, LINE);
        }
        None => p.fill(0.0, 0.0, vw, vh, DIM),
    }

    let w = CAPTION_W.min(vw - 2.0 * INSET);
    let lines = crate::gui::widgets::wrap(p, &[step.text], w - INSET);
    let h = lines.len() as f32 * LINE_H + 3.0 * INSET + BUTTON_H;
    let centred = (((vw - w) / 2.0).floor(), ((vh - h) / 2.0).floor());
    let (x, y) = match target {
        Some(r) if r.y + r.h + PAD + GAP + h <= vh => (centred.0, r.y + r.h + PAD + GAP),
        Some(r) if r.y - PAD - GAP - h >= 0.0 => (centred.0, r.y - PAD - GAP - h),
        Some(r) if r.x + r.w / 2.0 < vw / 2.0 => ((r.x + r.w + PAD + GAP).min(vw - w), centred.1),
        Some(r) => ((r.x - PAD - GAP - w).max(0.0), centred.1),
        None => centred,
    };
    let (x, y) = (x.floor(), y.floor());
    p.fill(x, y, w, h, PLATE);
    p.outline(x, y, w, h, LINE);
    for (i, line) in lines.iter().enumerate() {
        p.text_plain(line, x + INSET, y + INSET + i as f32 * LINE_H, TEXT, true);
    }

    let ctx = ScreenCtx { input, vw, vh };
    let by = y + h - BUTTON_H - INSET;
    let next_step = run.next();
    let prev_step = run.prev();
    let skip =
        next_step.is_some() && Button::new(x + INSET, by, BUTTON_W, BUTTON_H, "Skip").draw(p, &ctx);
    let back = prev_step.is_some()
        && Button::new(
            (x + (w - BUTTON_W) / 2.0).floor(),
            by,
            BUTTON_W,
            BUTTON_H,
            "Back",
        )
        .draw(p, &ctx);
    let next = Button::new(
        x + w - BUTTON_W - INSET,
        by,
        BUTTON_W,
        BUTTON_H,
        if next_step.is_some() { "Next" } else { "Done" },
    )
    .draw(p, &ctx);

    if skip || (next && next_step.is_none()) {
        finish(state, true);
        return;
    }
    if let Some(i) = next_step.filter(|_| next).or(prev_step.filter(|_| back)) {
        run.step = i;
        run.arrived = false;
    }
    state.tutorial.run = Some(run);
}
