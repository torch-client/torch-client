use bevy::math::{DVec3, Vec3};

use crate::audio::Heard;
use crate::gui::ScreenCtx;
use crate::gui::painter::Painter;

const DISPLAY_TIME: f32 = 3.0;

const LINE_HEIGHT: f32 = 9.0;

const BACKGROUND: u32 = 0xCC00_0000;

struct PlayedAt {
    at: Option<DVec3>,
    time: f32,
}

struct Row {
    key: &'static str,
    text: String,
    range: f32,
    played_at: Vec<PlayedAt>,
}

impl Row {
    fn closest(&self, ears: DVec3) -> Option<&PlayedAt> {
        let distance = |played: &PlayedAt| played.at.map_or(0.0, |at| at.distance_squared(ears));
        self.played_at
            .iter()
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))
    }

    fn audible_from(&self, ears: DVec3) -> bool {
        self.closest(ears).is_some_and(|played| {
            self.range.is_infinite()
                || played
                    .at
                    .is_none_or(|at| at.distance(ears) < f64::from(self.range))
        })
    }
}

#[derive(Default)]
pub struct Subtitles {
    rows: Vec<Row>,
    heard: Vec<Heard>,
    max_age: f32,
}

impl Subtitles {
    pub fn update(&mut self, now: f32, display_scale: f32) {
        self.max_age = DISPLAY_TIME * display_scale;
        let max_age = self.max_age;
        for row in &mut self.rows {
            row.played_at.retain(|played| now - played.time <= max_age);
        }
        crate::audio::take_heard(&mut self.heard);
        for heard in self.heard.drain(..) {
            let at = heard.at.map(DVec3::from_array);
            match self.rows.iter_mut().find(|row| row.key == heard.key) {
                Some(row) => {
                    row.played_at.retain(|played| played.at != at);
                    row.played_at.push(PlayedAt { at, time: now });
                }
                None => self.rows.push(Row {
                    key: heard.key,
                    text: crate::gui::tooltip::translate(heard.key, &[]),
                    range: heard.range,
                    played_at: vec![PlayedAt { at, time: now }],
                }),
            }
        }
    }

    pub fn draw(&self, p: &mut Painter, ctx: &ScreenCtx) {
        let now = ctx.input.time;
        let max_age = self.max_age;
        let (ears, forward, right) = crate::audio::listener();
        let ears = DVec3::from_array(ears);
        let forward = Vec3::from_array(forward).as_dvec3();
        let right = Vec3::from_array(right).as_dvec3();
        let font = &p.atlas.font;
        let widest = self
            .rows
            .iter()
            .filter(|row| row.audible_from(ears))
            .map(|row| font.width_str(&row.text))
            .reduce(f32::max);
        let Some(widest) = widest else {
            return;
        };
        let width = widest
            + font.width_str("<")
            + font.width_str(" ")
            + font.width_str(">")
            + font.width_str(" ");
        let half_width = (width / 2.0).floor();
        let half_height = (LINE_HEIGHT / 2.0).floor();
        let centre_x = ctx.vw.ceil() - half_width - 2.0;

        let mut line = 0.0;
        for row in self.rows.iter().filter(|row| row.audible_from(ears)) {
            let Some(played) = row.closest(ears) else {
                continue;
            };
            let centre_y = ctx.vh.ceil() - 35.0 - line * (LINE_HEIGHT + 1.0);
            let (left, top) = (centre_x - half_width, centre_y - half_height);
            p.fill(
                left - 1.0,
                top - 1.0,
                half_width * 2.0 + 2.0,
                half_height * 2.0 + 2.0,
                BACKGROUND,
            );

            let brightness =
                (255.0 - 180.0 * ((now - played.time) / max_age).clamp(0.0, 1.0)).floor() as u32;
            let color = brightness << 16 | brightness << 8 | brightness;

            if let Some(at) = played.at {
                let delta = (at - ears).normalize_or_zero();
                let rightness = right.dot(delta);
                if forward.dot(delta) <= 0.5 {
                    if rightness > 0.0 {
                        let x = centre_x + half_width - p.atlas.font.width_str(">");
                        p.text_plain(">", x, top, color, true);
                    } else if rightness < 0.0 {
                        p.text_plain("<", left, top, color, true);
                    }
                }
            }
            let text_x = centre_x - (p.atlas.font.width_str(&row.text) / 2.0).floor();
            p.text_plain(&row.text, text_x, top, color, true);
            line += 1.0;
        }
    }
}
