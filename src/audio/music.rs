use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use super::attributes::Music;
use super::{SoundCategory, Tracked};

const STARTING_DELAY: i32 = 100;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MusicFrequency {
    #[default]
    Default,
    Frequent,
    Constant,
}

crate::util::named_cycle::named_cycle!(MusicFrequency);

impl MusicFrequency {
    const ALL: [MusicFrequency; 3] = [Self::Default, Self::Frequent, Self::Constant];

    fn max_delay(self) -> i32 {
        match self {
            MusicFrequency::Default => 20 * 1200,
            MusicFrequency::Frequent => 10 * 1200,
            MusicFrequency::Constant => 0,
        }
    }

    pub fn serialized_name(self) -> &'static str {
        match self {
            MusicFrequency::Default => "DEFAULT",
            MusicFrequency::Frequent => "FREQUENT",
            MusicFrequency::Constant => "CONSTANT",
        }
    }

    fn from_index(index: u8) -> MusicFrequency {
        match index {
            1 => MusicFrequency::Frequent,
            2 => MusicFrequency::Constant,
            _ => MusicFrequency::Default,
        }
    }

    fn index(self) -> u8 {
        self as u8
    }

    fn next_song_delay(self, music: Option<Music>) -> i32 {
        let Some(music) = music else {
            return self.max_delay();
        };
        if self == MusicFrequency::Constant {
            return 100;
        }
        let max = self.max_delay();
        next_int(music.min_delay.min(max), music.max_delay.min(max))
    }
}

static FREQUENCY: AtomicU8 = AtomicU8::new(0);

static RESELECT: AtomicBool = AtomicBool::new(false);

pub fn set_music_frequency(frequency: MusicFrequency) {
    FREQUENCY.store(frequency.index(), Ordering::Relaxed);
}

pub fn set_minutes_between_songs(frequency: MusicFrequency) {
    FREQUENCY.store(frequency.index(), Ordering::Relaxed);
    RESELECT.store(true, Ordering::Relaxed);
}

#[derive(Clone)]
pub struct NowPlaying {
    pub key: Arc<str>,
    pub serial: u32,
}

static NOW_PLAYING: Mutex<Option<NowPlaying>> = Mutex::new(None);
static SONGS_STARTED: AtomicU32 = AtomicU32::new(0);

pub fn now_playing() -> Option<NowPlaying> {
    NOW_PLAYING.lock().ok()?.clone()
}

fn set_now_playing(song: Option<NowPlaying>) {
    if let Ok(mut now) = NOW_PLAYING.lock() {
        *now = song;
    }
}

fn next_int(min: i32, max: i32) -> i32 {
    if min >= max {
        return min;
    }
    min + super::with_rng(|rng| rng.next_int((max - min + 1) as u32) as i32).unwrap_or(0)
}

pub(super) struct MusicManager {
    current: Option<(Tracked, &'static str)>,
    gain: f32,
    delay: i32,
}

impl Default for MusicManager {
    fn default() -> MusicManager {
        MusicManager {
            current: None,
            gain: 1.0,
            delay: STARTING_DELAY,
        }
    }
}

impl MusicManager {
    pub fn tick(&mut self, music: Option<Music>, volume: f32) {
        let reselect = RESELECT.swap(false, Ordering::Relaxed);
        let frequency = MusicFrequency::from_index(FREQUENCY.load(Ordering::Relaxed));
        if reselect {
            self.delay = frequency.next_song_delay(music);
        }
        if self.current.is_some() && self.gain != volume && !self.fade(volume) {
            return;
        }
        let Some(music) = music else {
            self.delay = self.delay.max(STARTING_DELAY);
            return;
        };
        if let Some((voice, event)) = &self.current {
            let replaced = music.replace && music.event != *event;
            if replaced {
                self.delay = next_int(0, music.min_delay / 2);
            }
            if replaced || !voice.active() {
                super::end_loop(voice.id());
                self.current = None;
                set_now_playing(None);
                self.delay = self.delay.min(frequency.next_song_delay(Some(music)));
            }
        }
        self.delay = self.delay.min(frequency.next_song_delay(Some(music)));
        if self.current.is_none() {
            let due = self.delay <= 0;
            self.delay = self.delay.wrapping_sub(1);
            if due {
                self.start_playing(music);
            }
        }
    }

    fn start_playing(&mut self, music: Music) {
        self.current = super::start_tracked(music.event, SoundCategory::Music, self.gain, None)
            .map(|voice| (voice, music.event));
        if let Some((voice, _)) = &self.current {
            set_now_playing(Some(NowPlaying {
                key: super::defs::language_key(voice.key()).into(),
                serial: SONGS_STARTED.fetch_add(1, Ordering::Relaxed) + 1,
            }));
        }
        self.delay = i32::MAX;
    }

    pub fn stop_playing(&mut self) {
        if let Some((voice, _)) = self.current.take() {
            super::end_loop(voice.id());
            set_now_playing(None);
        }
        self.delay = self.delay.wrapping_add(STARTING_DELAY);
    }

    pub fn stop_playing_track(&mut self, music: Music) {
        if self
            .current
            .as_ref()
            .is_some_and(|(_, event)| *event == music.event)
        {
            self.stop_playing();
        }
    }

    fn fade(&mut self, volume: f32) -> bool {
        let Some(id) = self.current.as_ref().map(|(voice, _)| voice.id()) else {
            return false;
        };
        if self.gain < volume {
            self.gain = (self.gain + self.gain.clamp(5.0e-4, 0.005)).min(volume);
        } else {
            self.gain = 0.03 * volume + 0.97 * self.gain;
            if (self.gain - volume).abs() < 1.0e-4 || self.gain < volume {
                self.gain = volume;
            }
        }
        self.gain = self.gain.clamp(0.0, 1.0);
        if self.gain <= 1.0e-4 {
            self.stop_playing();
            return false;
        }
        super::update_loop(id, self.gain, 1.0);
        true
    }
}

const JUKEBOX_VOLUME: f32 = 4.0;

const MAX_JUKEBOXES: usize = 8;

static JUKEBOXES: Mutex<Vec<([i32; 3], Tracked)>> = Mutex::new(Vec::new());

pub fn jukebox_start(pos: [i32; 3], event: &str) {
    jukebox_stop(pos);
    let Ok(mut playing) = JUKEBOXES.lock() else {
        return;
    };
    if playing.len() >= MAX_JUKEBOXES {
        return;
    }
    let at = crate::util::block_centre(pos).to_array();
    if let Some(voice) =
        super::start_tracked(event, SoundCategory::Records, JUKEBOX_VOLUME, Some(at))
    {
        playing.push((pos, voice));
    }
}

pub fn jukebox_stop(pos: [i32; 3]) {
    let Ok(mut playing) = JUKEBOXES.lock() else {
        return;
    };
    if let Some(slot) = playing.iter().position(|(at, _)| *at == pos) {
        super::end_loop(playing.swap_remove(slot).1.id());
    }
}

pub(super) fn clear_jukeboxes() {
    if let Ok(mut playing) = JUKEBOXES.lock() {
        for (_, voice) in playing.drain(..) {
            super::end_loop(voice.id());
        }
    }
}

pub(super) fn tick_jukeboxes() {
    let Ok(mut playing) = JUKEBOXES.lock() else {
        return;
    };
    playing.retain(|(pos, voice)| {
        if !voice.active() {
            super::end_loop(voice.id());
            return false;
        }
        let at = crate::util::block_centre(*pos).to_array();
        super::update_placed(voice.placed(), at, JUKEBOX_VOLUME, 1.0);
        true
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAME: Music = Music {
        event: "music.game",
        min_delay: 12000,
        max_delay: 24000,
        replace: false,
    };

    #[test]
    fn next_int_returns_the_minimum_for_an_empty_range() {
        assert_eq!(next_int(0, 0), 0);
        assert_eq!(next_int(7, 3), 7);
    }

    #[test]
    fn the_delay_stays_inside_the_tracks_range() {
        for _ in 0..64 {
            let delay = MusicFrequency::Default.next_song_delay(Some(GAME));
            assert!((12000..=24000).contains(&delay));
        }
    }

    #[test]
    fn frequency_caps_the_range_and_constant_is_a_hundred() {
        for _ in 0..64 {
            assert_eq!(MusicFrequency::Frequent.next_song_delay(Some(GAME)), 12000);
        }
        assert_eq!(MusicFrequency::Constant.next_song_delay(Some(GAME)), 100);
        assert_eq!(MusicFrequency::Constant.next_song_delay(None), 0);
        assert_eq!(MusicFrequency::Default.next_song_delay(None), 24000);
    }

    #[test]
    fn stopping_at_the_parked_delay_wraps_as_a_java_int() {
        let mut manager = MusicManager {
            delay: i32::MAX,
            ..MusicManager::default()
        };
        manager.stop_playing();
        assert!(manager.delay < 0);
    }

    #[test]
    fn no_situation_holds_the_delay_at_the_starting_delay() {
        let mut manager = MusicManager {
            delay: 3,
            ..MusicManager::default()
        };
        manager.tick(None, 1.0);
        assert_eq!(manager.delay, STARTING_DELAY);
    }
}
