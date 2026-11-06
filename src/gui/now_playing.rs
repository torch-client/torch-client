use crate::gui::painter::Painter;
use crate::text::{Font, Span, Style, styled};
use crate::util::dye::DYES;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MusicToast {
    #[default]
    Never,
    Pause,
    PauseAndToast,
}

impl MusicToast {
    pub fn serialized_name(self) -> &'static str {
        match self {
            MusicToast::Never => "never",
            MusicToast::Pause => "pause",
            MusicToast::PauseAndToast => "pause_and_toast",
        }
    }

    pub fn from_name(name: &str) -> Option<MusicToast> {
        [
            MusicToast::Never,
            MusicToast::Pause,
            MusicToast::PauseAndToast,
        ]
        .into_iter()
        .find(|state| state.serialized_name() == name)
    }

    pub fn next(self) -> MusicToast {
        match self {
            MusicToast::Never => MusicToast::Pause,
            MusicToast::Pause => MusicToast::PauseAndToast,
            MusicToast::PauseAndToast => MusicToast::Never,
        }
    }

    pub fn in_pause(self) -> bool {
        self != MusicToast::Never
    }

    pub fn as_toast(self) -> bool {
        self == MusicToast::PauseAndToast
    }
}

const PADDING: f32 = 7.0;
const NOTES_SIZE: f32 = 16.0;
pub const HEIGHT: f32 = 30.0;
const NOTES_SPACE: f32 = 30.0;
const NOTE_STEP: f32 = 0.025;
const NOTES_FRAME: f32 = 0.1;
const NOTES_FRAMES: usize = 8;
const LINE_HEIGHT: f32 = 9.0;

const NOTE_DYES: [usize; 12] = [0, 8, 3, 11, 9, 13, 5, 4, 1, 6, 14, 2];
const NOTE_DURATION: u32 = 30;

fn note_dye(index: usize) -> u32 {
    let dye = NOTE_DYES[index];
    if dye == 0 {
        return 0xE6_E6_E6;
    }
    let src = DYES[dye].texture_diffuse;
    let channel =
        |shift: u32| ((((src >> shift) & 0xFF) as f32 * 1.25).floor() as i32 & 255) as u32;
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

fn note_color(tick: u32) -> u32 {
    let value = (tick / NOTE_DURATION) as usize;
    let from = note_dye(value % NOTE_DYES.len());
    let to = note_dye((value + 1) % NOTE_DYES.len());
    let step = (tick % NOTE_DURATION) as f32 / NOTE_DURATION as f32;
    let lerp = |shift: u32| {
        let (a, b) = (
            ((from >> shift) & 0xFF) as i32,
            ((to >> shift) & 0xFF) as i32,
        );
        ((a + (step * (b - a) as f32).floor() as i32) & 255) as u32
    };
    0xFF00_0000 | (lerp(16) << 16) | (lerp(8) << 8) | lerp(0)
}

pub struct Slot {
    pub width: f32,
    pub update: bool,
    pub show: bool,
}

impl Slot {
    pub fn update(&mut self, visible_for: f32, display_time: f32) {
        if self.update {
            self.show = visible_for < display_time;
        }
    }
}

pub enum Poll {
    Same,
    Started,
    Stopped,
}

pub struct Music {
    song: Option<crate::audio::NowPlaying>,
    title: Vec<Span>,
    width: f32,
    pub mode: MusicToast,
    pub toast_shown: bool,
    note_tick: u32,
    note_last: f32,
    note_color: u32,
}

impl Default for Music {
    fn default() -> Music {
        Music {
            song: None,
            title: Vec::new(),
            width: PADDING + NOTES_SPACE,
            mode: MusicToast::Never,
            toast_shown: false,
            note_tick: 0,
            note_last: 0.0,
            note_color: 0xFFFF_FFFF,
        }
    }
}

impl Music {
    pub fn playing(&self) -> bool {
        self.song.is_some()
    }

    pub fn poll(&mut self, font: &Font) -> Poll {
        let song = crate::audio::now_playing();
        if song.as_ref().map(|s| s.serial) == self.song.as_ref().map(|s| s.serial) {
            return Poll::Same;
        }
        self.title = match &song {
            Some(song) => styled(
                &crate::gui::tooltip::translate(&song.key, &[]),
                Style::colored(DYES[8].text_color),
            ),
            None => Vec::new(),
        };
        self.width = NOTES_SPACE + font.width(&self.title) + PADDING;
        let started = song.is_some();
        self.song = song;
        if started {
            Poll::Started
        } else {
            Poll::Stopped
        }
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn tick_notes(&mut self, now: f32) {
        if self.song.is_some() && now > self.note_last + NOTE_STEP {
            self.note_tick = self.note_tick.wrapping_add(1);
            self.note_last = now;
            self.note_color = note_color(self.note_tick);
        }
    }

    pub fn draw(&self, p: &mut Painter, x: f32, y: f32, now: f32) {
        if self.song.is_none() {
            return;
        }
        p.sprite("toast/now_playing", x, y, self.width, HEIGHT);
        let frame = (now / NOTES_FRAME) as usize % NOTES_FRAMES;
        p.sprite_part_tinted(
            "icon/music_notes",
            0.0,
            frame as f32 * NOTES_SIZE,
            NOTES_SIZE,
            NOTES_SIZE,
            x + PADDING,
            y + PADDING,
            self.note_color,
        );
        p.text(
            &self.title,
            x + NOTES_SPACE,
            y + HEIGHT / 2.0 - (LINE_HEIGHT / 2.0).floor(),
            true,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_colors_start_on_white_and_blend_toward_light_gray() {
        assert_eq!(note_color(0), 0xFFE6_E6E6);
        assert_ne!(note_color(15), note_color(0));
        assert_eq!(note_color(NOTE_DURATION * 12), note_color(0));
    }
}
