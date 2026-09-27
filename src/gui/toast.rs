use std::collections::VecDeque;

use crate::gui::ScreenCtx;
use crate::gui::painter::Painter;
use crate::text::{Font, Span, Style, styled};

const DEFAULT_WIDTH: f32 = 160.0;

const SLOT_HEIGHT: f32 = 32.0;

const SLOT_COUNT: u8 = 5;

const SLIDE: f32 = 0.6;

const ADVANCEMENT_SWAP: f32 = 1.5;
const ADVANCEMENT_FADE: f32 = 0.3;

const LINE_HEIGHT: f32 = 9.0;

const ADVANCEMENT_TITLE: u32 = 0xFFFF00;
const CHALLENGE_TITLE: u32 = 0xFF88FF;
const RECIPE_TITLE: u32 = 0x500050;
const RECIPE_DESCRIPTION: u32 = 0x000000;
const SYSTEM_TITLE: u32 = 0xFFFF00;

const SOUND_IN: &str = "ui.toast.in";
const SOUND_OUT: &str = "ui.toast.out";
const SOUND_CHALLENGE: &str = "ui.toast.challenge_complete";

#[derive(Clone, Copy)]
pub struct ToastSettings {
    pub advancement: bool,
    pub recipe: bool,
    pub system: bool,
    pub display_time: f32,
    pub hide_gui: bool,
}

impl ToastSettings {
    pub fn from_state(state: &crate::gui::GuiState) -> ToastSettings {
        ToastSettings {
            advancement: state.options.toast_advancement,
            recipe: state.options.toast_recipe,
            system: state.options.toast_system,
            display_time: state.options.toast_time as f32 / 10.0,
            hide_gui: state.hide_gui,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AdvancementFrame {
    Task,
    Challenge,
    Goal,
}

impl AdvancementFrame {
    fn display_key(self) -> &'static str {
        match self {
            AdvancementFrame::Task => "advancements.toast.task",
            AdvancementFrame::Challenge => "advancements.toast.challenge",
            AdvancementFrame::Goal => "advancements.toast.goal",
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SystemId {
    NarratorToggle,
    WorldBackup,
    PackLoadFailure,
    WorldAccessFailure,
    PackCopyFailure,
    FileDropFailure,
    PeriodicNotification,
    LowDiskSpace,
    ChunkLoadFailure,
    ChunkSaveFailure,
    UnsecureServerWarning,
}

pub enum ToastEvent {
    Advancement {
        title: Vec<Span>,
        icon: String,
        frame: AdvancementFrame,
    },
    Recipe {
        station: String,
        result: String,
    },
    System {
        id: SystemId,
        title: Vec<Span>,
        message: Vec<Span>,
        multiline: bool,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Visibility {
    Show,
    Hide,
}

impl Visibility {
    fn sound(self) -> &'static str {
        match self {
            Visibility::Show => SOUND_IN,
            Visibility::Hide => SOUND_OUT,
        }
    }
}

struct Advancement {
    header: Vec<Span>,
    header_is_challenge: bool,
    lines: Vec<Vec<Span>>,
    icon: String,
    hide: bool,
}

struct Recipe {
    items: Vec<(String, String)>,
    title: Vec<Span>,
    description: Vec<Span>,
    last_changed: f32,
    changed: bool,
    index: usize,
    hide: bool,
}

struct System {
    id: SystemId,
    title: Vec<Span>,
    lines: Vec<Vec<Span>>,
    width: f32,
    force_hide: bool,
    last_changed: f32,
    changed: bool,
    hide: bool,
}

enum Toast {
    Advancement(Advancement),
    Recipe(Recipe),
    System(System),
}

impl Toast {
    fn width(&self) -> f32 {
        match self {
            Toast::System(s) => s.width,
            _ => DEFAULT_WIDTH,
        }
    }

    fn height(&self) -> f32 {
        match self {
            Toast::System(s) => 20.0 + s.lines.len().max(1) as f32 * 12.0,
            _ => SLOT_HEIGHT,
        }
    }

    fn slots(&self) -> u8 {
        (self.height() / SLOT_HEIGHT).ceil() as u8
    }

    fn sound(&self) -> Option<&'static str> {
        match self {
            Toast::Advancement(a) => a.header_is_challenge.then_some(SOUND_CHALLENGE),
            _ => None,
        }
    }

    fn wanted_visibility(&self) -> Visibility {
        let hide = match self {
            Toast::Advancement(a) => a.hide,
            Toast::Recipe(r) => r.hide,
            Toast::System(s) => s.hide,
        };
        if hide {
            Visibility::Hide
        } else {
            Visibility::Show
        }
    }

    fn update(&mut self, t: f32, display_time: f32) {
        match self {
            Toast::Advancement(a) => a.hide = t >= display_time,
            Toast::Recipe(r) => {
                if r.changed {
                    r.last_changed = t;
                    r.changed = false;
                }
                r.hide = r.items.is_empty() || t - r.last_changed >= display_time;
                if !r.items.is_empty() {
                    let count = r.items.len() as f32;
                    let step = (display_time / count).max(1.0);
                    r.index = (t / step) as usize % r.items.len();
                }
            }
            Toast::System(s) => {
                if s.changed {
                    s.last_changed = t;
                    s.changed = false;
                }
                s.hide = s.force_hide || t - s.last_changed >= display_time;
            }
        }
    }

    fn draw(&self, p: &mut Painter, ox: f32, oy: f32, t: f32) {
        match self {
            Toast::Advancement(a) => a.draw(p, ox, oy, t),
            Toast::Recipe(r) => r.draw(p, ox, oy),
            Toast::System(s) => s.draw(p, ox, oy, self.height()),
        }
    }
}

impl Advancement {
    fn draw(&self, p: &mut Painter, ox: f32, oy: f32, t: f32) {
        p.sprite("toast/advancement", ox, oy, DEFAULT_WIDTH, SLOT_HEIGHT);
        if self.lines.len() == 1 {
            p.text(&self.header, ox + 30.0, oy + 7.0, false);
            p.text(&self.lines[0], ox + 30.0, oy + 18.0, false);
        } else if t < ADVANCEMENT_SWAP {
            let alpha = ((ADVANCEMENT_SWAP - t) / ADVANCEMENT_FADE).clamp(0.0, 1.0);
            p.text_faded(&self.header, ox + 30.0, oy + 11.0, false, alpha);
        } else {
            let alpha =
                ((t - ADVANCEMENT_SWAP) / ADVANCEMENT_FADE).clamp(0.0, 1.0) * (252.0 / 255.0);
            let mut y = oy + SLOT_HEIGHT / 2.0 - self.lines.len() as f32 * LINE_HEIGHT / 2.0;
            for line in &self.lines {
                p.text_faded(line, ox + 30.0, y, false, alpha);
                y += LINE_HEIGHT;
            }
        }
        p.item_icon(&self.icon, ox + 8.0, oy + 8.0);
    }
}

impl Recipe {
    fn draw(&self, p: &mut Painter, ox: f32, oy: f32) {
        p.sprite("toast/recipe", ox, oy, DEFAULT_WIDTH, SLOT_HEIGHT);
        p.text(&self.title, ox + 30.0, oy + 7.0, false);
        p.text(&self.description, ox + 30.0, oy + 18.0, false);
        let Some((station, result)) = self.items.get(self.index) else {
            return;
        };
        p.scaled(0.6, ox, oy, |p| p.item_icon(station, 3.0, 3.0));
        p.item_icon(result, ox + 8.0, oy + 8.0);
    }
}

impl System {
    fn draw(&self, p: &mut Painter, ox: f32, oy: f32, h: f32) {
        p.sprite("toast/system", ox, oy, self.width, h);
        if self.lines.is_empty() {
            p.text(&self.title, ox + 18.0, oy + 12.0, false);
            return;
        }
        p.text(&self.title, ox + 18.0, oy + 7.0, false);
        for (i, line) in self.lines.iter().enumerate() {
            p.text(line, ox + 18.0, oy + 18.0 + i as f32 * 12.0, false);
        }
    }
}

struct Instance {
    toast: Toast,
    first_slot: u8,
    slots: u8,
    anim_start: Option<f32>,
    became_visible_at: f32,
    visibility: Visibility,
    fully_visible_for: f32,
    visible_portion: f32,
    finished: bool,
}

impl Instance {
    fn new(toast: Toast, first_slot: u8, slots: u8) -> Instance {
        Instance {
            toast,
            first_slot,
            slots,
            anim_start: None,
            became_visible_at: 0.0,
            visibility: Visibility::Hide,
            fully_visible_for: 0.0,
            visible_portion: 0.0,
            finished: false,
        }
    }

    fn update(&mut self, now: f32, display_time: f32) -> Option<&'static str> {
        let previous = self.visibility;
        let mut start = match self.anim_start {
            Some(start) => start,
            None => {
                self.visibility = Visibility::Show;
                self.anim_start = Some(now);
                now
            }
        };

        if self.visibility == Visibility::Show && now - start <= SLIDE {
            self.became_visible_at = now;
        }
        self.fully_visible_for = now - self.became_visible_at;

        let progress = ((now - start) / SLIDE).clamp(0.0, 1.0);
        let progress = progress * progress;
        self.visible_portion = match self.visibility {
            Visibility::Hide => 1.0 - progress,
            Visibility::Show => progress,
        };

        self.toast.update(self.fully_visible_for, display_time);

        let wanted = self.toast.wanted_visibility();
        if wanted != self.visibility {
            start = now - (1.0 - self.visible_portion) * SLIDE;
            self.anim_start = Some(start);
            self.visibility = wanted;
        }

        self.finished = self.visibility == Visibility::Hide && now - start > SLIDE;
        (self.visibility != previous).then(|| self.visibility.sound())
    }

    fn x(&self, screen_w: f32) -> f32 {
        screen_w - self.toast.width() * self.visible_portion
    }

    fn y(&self) -> f32 {
        self.first_slot as f32 * self.toast.height()
    }
}

#[derive(Default)]
pub struct Toasts {
    visible: Vec<Instance>,
    queued: VecDeque<Toast>,
    occupied: u8,
}

impl Toasts {
    pub fn ingest(&mut self, events: Vec<ToastEvent>, font: &Font, opts: ToastSettings) {
        for event in events {
            match event {
                ToastEvent::Advancement { title, icon, frame } => {
                    if !opts.advancement {
                        continue;
                    }
                    let challenge = frame == AdvancementFrame::Challenge;
                    let color = if challenge {
                        CHALLENGE_TITLE
                    } else {
                        ADVANCEMENT_TITLE
                    };
                    let header = styled(
                        &crate::gui::tooltip::translate(frame.display_key(), &[]),
                        Style::colored(color),
                    );
                    self.add(Toast::Advancement(Advancement {
                        header,
                        header_is_challenge: challenge,
                        lines: font.wrap(&title, 125.0),
                        icon,
                        hide: false,
                    }));
                }
                ToastEvent::Recipe { station, result } => {
                    if !opts.recipe {
                        continue;
                    }
                    if let Some(recipe) = self.find_recipe() {
                        recipe.items.push((station, result));
                        recipe.changed = true;
                        continue;
                    }
                    self.add(Toast::Recipe(Recipe {
                        items: vec![(station, result)],
                        title: styled(
                            &crate::gui::tooltip::translate("recipe.toast.title", &[]),
                            Style::colored(RECIPE_TITLE),
                        ),
                        description: styled(
                            &crate::gui::tooltip::translate("recipe.toast.description", &[]),
                            Style::colored(RECIPE_DESCRIPTION),
                        ),
                        last_changed: 0.0,
                        changed: true,
                        index: 0,
                        hide: false,
                    }));
                }
                ToastEvent::System {
                    id,
                    title,
                    message,
                    multiline,
                } => {
                    if !opts.system {
                        continue;
                    }
                    if let Some(system) = self.find_system(id) {
                        system.title = recolor_default(title, SYSTEM_TITLE);
                        system.lines = system_lines(font, &message, multiline).0;
                        system.changed = true;
                        continue;
                    }
                    let (lines, body_w) = system_lines(font, &message, multiline);
                    let width = if multiline {
                        body_w.max(200.0) + 30.0
                    } else {
                        DEFAULT_WIDTH.max(30.0 + font.width(&title).max(body_w))
                    };
                    self.add(Toast::System(System {
                        id,
                        title: recolor_default(title, SYSTEM_TITLE),
                        lines,
                        width,
                        force_hide: false,
                        last_changed: 0.0,
                        changed: false,
                        hide: false,
                    }));
                }
            }
        }
    }

    fn add(&mut self, toast: Toast) {
        self.queued.push_back(toast);
    }

    fn find_recipe(&mut self) -> Option<&mut Recipe> {
        self.visible
            .iter_mut()
            .map(|i| &mut i.toast)
            .chain(self.queued.iter_mut())
            .find_map(|t| match t {
                Toast::Recipe(r) => Some(r),
                _ => None,
            })
    }

    fn find_system(&mut self, id: SystemId) -> Option<&mut System> {
        self.visible
            .iter_mut()
            .map(|i| &mut i.toast)
            .chain(self.queued.iter_mut())
            .find_map(|t| match t {
                Toast::System(s) if s.id == id => Some(s),
                _ => None,
            })
    }

    fn free_slots(&self) -> u8 {
        SLOT_COUNT - self.occupied.count_ones() as u8
    }

    fn find_free_slots(&self, needed: u8) -> Option<u8> {
        if self.free_slots() < needed {
            return None;
        }
        let mut run = 0;
        for i in 0..SLOT_COUNT {
            if self.occupied & (1 << i) != 0 {
                run = 0;
            } else {
                run += 1;
                if run == needed {
                    return Some(i + 1 - run);
                }
            }
        }
        None
    }

    fn update(&mut self, now: f32, display_time: f32) {
        let mut flip_sound = None;
        let occupied = &mut self.occupied;
        self.visible.retain_mut(|instance| {
            if let Some(sound) = instance.update(now, display_time)
                && flip_sound.is_none()
            {
                flip_sound = Some(sound);
            }
            if !instance.finished {
                return true;
            }
            for slot in instance.first_slot..instance.first_slot + instance.slots {
                *occupied &= !(1 << slot);
            }
            false
        });
        if let Some(sound) = flip_sound {
            play_ui(sound);
        }

        if self.queued.is_empty() || self.free_slots() == 0 {
            return;
        }
        let mut sounds: Vec<&'static str> = Vec::new();
        let mut kept = VecDeque::with_capacity(self.queued.len());
        while let Some(toast) = self.queued.pop_front() {
            let slots = toast.slots();
            let Some(first) = self.find_free_slots(slots) else {
                kept.push_back(toast);
                continue;
            };
            for slot in first..first + slots {
                self.occupied |= 1 << slot;
            }
            if let Some(sound) = toast.sound()
                && !sounds.contains(&sound)
            {
                sounds.push(sound);
            }
            self.visible.push(Instance::new(toast, first, slots));
        }
        self.queued = kept;
        for sound in sounds {
            play_ui(sound);
        }
    }
}

fn recolor_default(mut spans: Vec<Span>, rgb: u32) -> Vec<Span> {
    for span in &mut spans {
        if span.style.color == Style::default().color {
            span.style.color = rgb;
        }
    }
    spans
}

fn system_lines(font: &Font, message: &[Span], multiline: bool) -> (Vec<Vec<Span>>, f32) {
    if message.is_empty() {
        return (Vec::new(), 0.0);
    }
    if !multiline {
        return (vec![message.to_vec()], font.width(message));
    }
    let lines = font.wrap(message, 200.0);
    let width = lines.iter().map(|l| font.width(l)).fold(0.0f32, f32::max);
    (lines, width)
}

#[cfg(feature = "audio")]
fn play_ui(event: &str) {
    crate::audio::play(event, crate::audio::SoundCategory::Ambient, 1.0, 1.0);
}

#[cfg(not(feature = "audio"))]
fn play_ui(_event: &str) {}

pub fn draw(p: &mut Painter, ctx: &ScreenCtx, toasts: &mut Toasts, opts: ToastSettings) {
    toasts.update(ctx.input.time, opts.display_time);
    if opts.hide_gui || toasts.visible.is_empty() {
        return;
    }
    for instance in &toasts.visible {
        if instance.finished {
            continue;
        }
        instance.toast.draw(
            p,
            instance.x(ctx.vw),
            instance.y(),
            instance.fully_visible_for,
        );
    }
}
