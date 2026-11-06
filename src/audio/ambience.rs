use super::attributes::{AmbientSounds, Mood};
use super::{LoopId, Scene, SoundCategory};

const FADE_TICKS: i32 = 40;

const SKY_MOOD_RECOVERY_RATE: f32 = 0.001;

const UNDERWATER_ENTER: &str = "ambient.underwater.enter";
const UNDERWATER_EXIT: &str = "ambient.underwater.exit";
const UNDERWATER_LOOP: &str = "ambient.underwater.loop";
const UNDERWATER_ADDITIONS: &str = "ambient.underwater.loop.additions";
const UNDERWATER_RARE: &str = "ambient.underwater.loop.additions.rare";
const UNDERWATER_ULTRA_RARE: &str = "ambient.underwater.loop.additions.ultra_rare";

fn fade_volume(fade: i32) -> f32 {
    (fade as f32 / FADE_TICKS as f32).clamp(0.0, 1.0)
}

struct BiomeLoop {
    event: &'static str,
    id: LoopId,
    fade: i32,
    direction: i32,
}

#[derive(Default)]
pub(super) struct Ambience {
    loops: Vec<BiomeLoop>,
    previous: Option<&'static str>,
    moodiness: f32,
    was_underwater: bool,
    water_loop: Option<(LoopId, i32)>,
}

impl Ambience {
    pub fn new_player(&mut self, stopped: bool) {
        self.moodiness = 0.0;
        self.was_underwater = false;
        if let Some((id, _)) = self.water_loop.take() {
            super::end_loop(id);
        }
        if stopped {
            for sound in self.loops.drain(..) {
                super::end_loop(sound.id);
            }
            self.previous = None;
        }
    }

    pub fn tick(&mut self, scene: &Scene, ambient: Option<&AmbientSounds>) {
        self.tick_underwater(scene);
        self.tick_biome_loops(ambient.and_then(|ambient| ambient.looped));
        let Some(ambient) = ambient else {
            return;
        };
        for addition in &ambient.additions {
            if super::with_rng(|rng| rng.next_f64()).is_some_and(|roll| roll < addition.tick_chance)
            {
                super::play(addition.event, SoundCategory::Ambient, 1.0, 1.0);
            }
        }
        if let Some(mood) = &ambient.mood {
            self.tick_mood(scene, mood);
        }
    }

    fn tick_underwater(&mut self, scene: &Scene) {
        let (was, now) = (self.was_underwater, scene.underwater);
        self.was_underwater = now;

        if !scene.spectator {
            if !was && now {
                super::play_at(
                    UNDERWATER_ENTER,
                    SoundCategory::Ambient,
                    scene.feet,
                    1.0,
                    1.0,
                );
                if self.water_loop.is_none() {
                    self.water_loop =
                        super::start_loop(UNDERWATER_LOOP, SoundCategory::Ambient, 0.0, 1.0)
                            .map(|id| (id, 0));
                }
            }
            if was && !now {
                super::play_at(
                    UNDERWATER_EXIT,
                    SoundCategory::Ambient,
                    scene.feet,
                    1.0,
                    1.0,
                );
            }
        }
        if was && !now {
            for event in [UNDERWATER_ADDITIONS, UNDERWATER_RARE, UNDERWATER_ULTRA_RARE] {
                super::stop(Some(SoundCategory::Ambient), Some(event));
            }
        }

        if let Some((id, fade)) = &mut self.water_loop {
            let before = fade_volume(*fade);
            *fade = (*fade + if now { 1 } else { -2 }).min(FADE_TICKS);
            if *fade < 0 {
                super::end_loop(*id);
                self.water_loop = None;
            } else if fade_volume(*fade) != before {
                super::update_loop(*id, fade_volume(*fade), 1.0);
            }
        }

        if now {
            let roll = super::random_f32();
            let event = match roll {
                _ if roll < 1.0e-4 => UNDERWATER_ULTRA_RARE,
                _ if roll < 0.001 => UNDERWATER_RARE,
                _ if roll < 0.01 => UNDERWATER_ADDITIONS,
                _ => return,
            };
            super::play(event, SoundCategory::Ambient, 1.0, 1.0);
        }
    }

    fn tick_biome_loops(&mut self, current: Option<&'static str>) {
        self.loops.retain_mut(|sound| {
            if sound.fade < 0 {
                super::end_loop(sound.id);
                return false;
            }
            let before = fade_volume(sound.fade);
            sound.fade += sound.direction;
            if fade_volume(sound.fade) != before {
                super::update_loop(sound.id, fade_volume(sound.fade), 1.0);
            }
            true
        });

        if current == self.previous {
            return;
        }
        for sound in &mut self.loops {
            sound.fade = sound.fade.min(FADE_TICKS);
            sound.direction = -1;
        }
        let Some(event) = current else {
            self.previous = None;
            return;
        };
        if let Some(sound) = self.loops.iter_mut().find(|sound| sound.event == event) {
            sound.fade = sound.fade.max(0);
            sound.direction = 1;
        } else if let Some(id) = super::start_loop(event, SoundCategory::Ambient, 0.0, 1.0) {
            self.loops.push(BiomeLoop {
                event,
                id,
                fade: 0,
                direction: 1,
            });
        } else {
            return;
        }
        self.previous = current;
    }

    fn tick_mood(&mut self, scene: &Scene, mood: &Mood) {
        let span = (mood.block_search_extent * 2 + 1) as u32;
        let extent = f64::from(mood.block_search_extent);
        let Some([dx, dy, dz]) = super::with_rng(|rng| {
            [rng.next_int(span), rng.next_int(span), rng.next_int(span)]
                .map(|d| f64::from(d) - extent)
        }) else {
            return;
        };
        let [x, _, z] = scene.feet;
        let eye_y = scene.eye[1];
        let block = [
            (x + dx).floor() as i32,
            (eye_y + dy).floor() as i32,
            (z + dz).floor() as i32,
        ];
        let Some((block_light, sky_light)) =
            crate::client::worldsync::light_at(block[0], block[1], block[2])
        else {
            return;
        };
        let sky_light = if crate::renderer::dimension::current().has_skylight() {
            sky_light
        } else {
            0
        };
        if sky_light > 0 {
            self.moodiness -= f32::from(sky_light) / 15.0 * SKY_MOOD_RECOVERY_RATE;
        } else {
            self.moodiness -= (f32::from(block_light) - 1.0) / mood.tick_delay as f32;
        }

        if self.moodiness < 1.0 {
            self.moodiness = self.moodiness.max(0.0);
            return;
        }
        let direction = [
            f64::from(block[0]) + 0.5 - x,
            f64::from(block[1]) + 0.5 - eye_y,
            f64::from(block[2]) + 0.5 - z,
        ];
        let distance = direction.iter().map(|d| d * d).sum::<f64>().sqrt();
        let scale = (distance + mood.offset) / distance;
        let at = [
            x + direction[0] * scale,
            eye_y + direction[1] * scale,
            z + direction[2] * scale,
        ];
        super::play_at(mood.event, SoundCategory::Ambient, at, 1.0, 1.0);
        self.moodiness = 0.0;
    }
}
