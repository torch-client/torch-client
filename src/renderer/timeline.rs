#[derive(Clone, Copy)]
pub struct CubicCurve {
    a: f32,
    b: f32,
    c: f32,
}

const fn curve_from_controls(v1: f32, v2: f32) -> CubicCurve {
    CubicCurve {
        a: 3.0 * v1 - 3.0 * v2 + 1.0,
        b: -6.0 * v1 + 3.0 * v2,
        c: 3.0 * v1,
    }
}

impl CubicCurve {
    fn sample(&self, t: f32) -> f32 {
        ((self.a * t + self.b) * t + self.c) * t
    }

    fn sample_gradient(&self, t: f32) -> f32 {
        (3.0 * self.a * t + 2.0 * self.b) * t + self.c
    }
}

#[derive(Clone, Copy)]
pub enum Ease {
    Linear,
    Constant,
    CubicBezier { x: CubicCurve, y: CubicCurve },
}

pub const fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> Ease {
    Ease::CubicBezier {
        x: curve_from_controls(x1, x2),
        y: curve_from_controls(y1, y2),
    }
}

impl Ease {
    fn apply(&self, x: f32) -> f32 {
        match self {
            Ease::Linear => x,
            Ease::Constant => 0.0,
            Ease::CubicBezier { x: xc, y: yc } => {
                let mut t = x;
                for _ in 0..4 {
                    let gradient = xc.sample_gradient(t);
                    if gradient < 1.0e-5 {
                        break;
                    }
                    t -= (xc.sample(t) - x) / gradient;
                }
                yc.sample(t)
            }
        }
    }
}

pub trait Lerp: Copy {
    fn lerp(alpha: f32, from: Self, to: Self) -> Self;
}

impl Lerp for f32 {
    fn lerp(alpha: f32, from: f32, to: f32) -> f32 {
        from + alpha * (to - from)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Argb(pub u32);

impl Argb {
    pub const fn channel(self, shift: u32) -> u32 {
        (self.0 >> shift) & 0xFF
    }

    pub const fn alpha(self) -> u32 {
        self.channel(24)
    }

    pub fn multiply(self, rhs: Argb) -> Argb {
        if self.0 == 0xFFFF_FFFF {
            return rhs;
        }
        if rhs.0 == 0xFFFF_FFFF {
            return self;
        }
        let ch = |shift: u32| (self.channel(shift) * rhs.channel(shift) / 255) << shift;
        Argb(ch(24) | ch(16) | ch(8) | ch(0))
    }

    pub fn scale_rgb(self, r: f32, g: f32, b: f32) -> Argb {
        let ch = |shift: u32, scale: f32| {
            (((self.channel(shift) as f32 * scale) as i32).clamp(0, 255) as u32) << shift
        };
        Argb((self.alpha() << 24) | ch(16, r) | ch(8, g) | ch(0, b))
    }

    pub fn greyscale(self) -> Argb {
        let grey = (self.channel(16) as f32 * 0.3
            + self.channel(8) as f32 * 0.59
            + self.channel(0) as f32 * 0.11) as u32;
        Argb((self.alpha() << 24) | (grey << 16) | (grey << 8) | grey)
    }

    pub fn blend_to_gray(self, brightness: f32, factor: f32) -> Argb {
        let grey = self
            .greyscale()
            .scale_rgb(brightness, brightness, brightness);
        <Argb as Lerp>::lerp(factor, self, grey)
    }

    pub fn alpha_blend(self, source: Argb) -> Argb {
        let source_alpha = source.alpha();
        if source_alpha == 255 {
            return source;
        }
        if source_alpha == 0 {
            return self;
        }
        let alpha = source_alpha + self.alpha() * (255 - source_alpha) / 255;
        let ch = |shift: u32| {
            let blended = (source.channel(shift) * source_alpha
                + self.channel(shift) * (alpha - source_alpha))
                / alpha;
            (blended & 0xFF) << shift
        };
        Argb((alpha & 0xFF) << 24 | ch(16) | ch(8) | ch(0))
    }

    pub const fn opaque(self) -> Argb {
        Argb(self.0 | 0xFF00_0000)
    }

    pub fn alpha_f32(self) -> f32 {
        self.alpha() as f32 / 255.0
    }

    pub fn to_rgba_f32(self) -> [f32; 4] {
        [
            self.channel(16) as f32 / 255.0,
            self.channel(8) as f32 / 255.0,
            self.channel(0) as f32 / 255.0,
            self.channel(24) as f32 / 255.0,
        ]
    }

    pub fn to_color(self) -> bevy::prelude::Color {
        let [r, g, b, a] = self.to_rgba_f32();
        bevy::prelude::Color::srgba(r, g, b, a)
    }
}

impl Lerp for Argb {
    fn lerp(alpha: f32, from: Argb, to: Argb) -> Argb {
        let ch = |shift: u32| {
            let a = from.channel(shift) as f32;
            let b = to.channel(shift) as f32;
            ((a + alpha * (b - a)) as u32).min(255) << shift
        };
        Argb(ch(24) | ch(16) | ch(8) | ch(0))
    }
}

pub struct Track<T: 'static> {
    pub keyframes: &'static [(i32, T)],
    pub ease: Ease,
}

impl<T: Lerp + 'static> Track<T> {
    pub fn sample(&self, period: i32, ticks: f64) -> T {
        let kf = self.keyframes;
        if kf.len() == 1 {
            return kf[0].1;
        }
        let t = ticks.rem_euclid(period as f64);
        let (first_ticks, first_value) = kf[0];
        let (last_ticks, last_value) = kf[kf.len() - 1];

        let mut segment = (last_ticks, last_value, first_ticks + period, first_value);
        if t < first_ticks as f64 {
            segment = (last_ticks - period, last_value, first_ticks, first_value);
        } else {
            for pair in kf.windows(2) {
                if t < pair[1].0 as f64 {
                    segment = (pair[0].0, pair[0].1, pair[1].0, pair[1].1);
                    break;
                }
            }
        }

        let (from_ticks, from_value, to_ticks, to_value) = segment;
        if t <= from_ticks as f64 {
            return from_value;
        }
        if t >= to_ticks as f64 {
            return to_value;
        }
        let alpha = (t - from_ticks as f64) as f32 / (to_ticks - from_ticks) as f32;
        T::lerp(self.ease.apply(alpha), from_value, to_value)
    }
}

pub const DAY_PERIOD: i32 = 24000;

const CELESTIAL_EASE: Ease = cubic_bezier(0.362, 0.241, 0.638, 0.759);

static SUN_ANGLE: Track<f32> = Track {
    keyframes: &[(6000, 360.0), (6000, 0.0)],
    ease: CELESTIAL_EASE,
};

static MOON_ANGLE: Track<f32> = Track {
    keyframes: &[(6000, 540.0), (6000, 180.0)],
    ease: CELESTIAL_EASE,
};

static STAR_ANGLE: Track<f32> = Track {
    keyframes: &[(6000, 360.0), (6000, 0.0)],
    ease: CELESTIAL_EASE,
};

static STAR_BRIGHTNESS: Track<f32> = Track {
    keyframes: &[
        (92, 0.037),
        (627, 0.0),
        (11373, 0.0),
        (11732, 0.016),
        (11959, 0.044),
        (12399, 0.143),
        (12729, 0.258),
        (13228, 0.5),
        (22772, 0.5),
        (23032, 0.364),
        (23356, 0.225),
        (23758, 0.101),
    ],
    ease: Ease::Linear,
};

static SKY_COLOR: Track<Argb> = Track {
    keyframes: &[
        (133, Argb(0xFFFF_FFFF)),
        (11867, Argb(0xFFFF_FFFF)),
        (13670, Argb(0xFF00_0000)),
        (22330, Argb(0xFF00_0000)),
    ],
    ease: Ease::Linear,
};

static FOG_COLOR: Track<Argb> = Track {
    keyframes: &[
        (133, Argb(0xFFFF_FFFF)),
        (11867, Argb(0xFFFF_FFFF)),
        (13670, Argb(0xFF0F_0F16)),
        (22330, Argb(0xFF0F_0F16)),
    ],
    ease: Ease::Linear,
};

static SKY_LIGHT_FACTOR: Track<f32> = Track {
    keyframes: &[(730, 1.0), (11270, 1.0), (13140, 0.24), (22860, 0.24)],
    ease: Ease::Linear,
};

static SKY_LIGHT_COLOR: Track<Argb> = Track {
    keyframes: &[
        (730, Argb(0xFFFF_FFFF)),
        (11270, Argb(0xFFFF_FFFF)),
        (13140, Argb(0xFF7A_7AFF)),
        (22860, Argb(0xFF7A_7AFF)),
    ],
    ease: Ease::Linear,
};

static CLOUD_COLOR: Track<Argb> = Track {
    keyframes: &[
        (133, Argb(0xFFFF_FFFF)),
        (11867, Argb(0xFFFF_FFFF)),
        (13670, Argb(0xFF19_1926)),
        (22330, Argb(0xFF19_1926)),
    ],
    ease: Ease::Linear,
};

static SUNRISE_SUNSET_COLOR: Track<Argb> = Track {
    keyframes: &[
        (71, Argb(0x5FEF_A333)),
        (310, Argb(0x29F5_BA33)),
        (565, Argb(0x06FB_D433)),
        (730, Argb(0x00FF_E533)),
        (11270, Argb(0x00FF_E533)),
        (11397, Argb(0x04FC_D833)),
        (11522, Argb(0x0FF9_CB33)),
        (11690, Argb(0x29F5_BA33)),
        (11929, Argb(0x5FEF_A333)),
        (12243, Argb(0xB1E7_8733)),
        (12358, Argb(0xCCE4_7E33)),
        (12512, Argb(0xE9E0_7233)),
        (12613, Argb(0xF6DD_6B33)),
        (12732, Argb(0xFEDA_6333)),
        (12841, Argb(0xFED7_5C33)),
        (13035, Argb(0xECD2_5133)),
        (13252, Argb(0xC1CC_4733)),
        (13775, Argb(0x36BE_3733)),
        (13888, Argb(0x1FBB_3533)),
        (14039, Argb(0x09B7_3333)),
        (14192, Argb(0x00B3_3333)),
        (21807, Argb(0x00B2_3333)),
        (21961, Argb(0x09B7_3333)),
        (22112, Argb(0x1FBB_3533)),
        (22225, Argb(0x36BE_3733)),
        (22748, Argb(0xC1CC_4733)),
        (22965, Argb(0xECD2_5133)),
        (23159, Argb(0xFED7_5C33)),
        (23272, Argb(0xFEDA_6333)),
        (23488, Argb(0xE9E0_7233)),
        (23642, Argb(0xCCE4_7E33)),
        (23757, Argb(0xB1E7_8733)),
    ],
    ease: Ease::Linear,
};

const MOON_PERIOD: i32 = 192_000;

const MOON_PHASE_LENGTH: i32 = 24_000;

pub const MOON_PHASE_NAMES: [&str; 8] = [
    "full_moon",
    "waning_gibbous",
    "third_quarter",
    "waning_crescent",
    "new_moon",
    "waxing_crescent",
    "first_quarter",
    "waxing_gibbous",
];

const BASE_SKY_COLOR: Argb = Argb(0xFF78_A7FF);

const BASE_SKY_LIGHT_COLOR: Argb = Argb(0xFFFF_FFFF);

const BASE_FOG_COLOR: Argb = Argb(0xFFC0_D8FF);

const BASE_CLOUD_COLOR: Argb = Argb(0xCCFF_FFFF);

pub const CLOUD_HEIGHT: f32 = 192.33;

pub const HORIZON_HEIGHT: f64 = 63.0;

struct Timeline {
    period: i32,
    numbers: std::collections::HashMap<String, Track<f32>>,
    colors: std::collections::HashMap<String, Track<Argb>>,
    phases: Vec<(i32, String)>,
}

static DAY: std::sync::OnceLock<Timeline> = std::sync::OnceLock::new();
static MOON: std::sync::OnceLock<Timeline> = std::sync::OnceLock::new();

fn day() -> &'static Timeline {
    DAY.get_or_init(|| load("day", DAY_PERIOD))
}

fn moon() -> &'static Timeline {
    MOON.get_or_init(|| load("moon", MOON_PERIOD))
}

fn parse_ease(value: Option<&serde_json::Value>) -> Ease {
    match value {
        None => Ease::Linear,
        Some(v) if v.as_str() == Some("constant") => Ease::Constant,
        Some(v) if v.as_str() == Some("linear") => Ease::Linear,
        Some(v) => {
            let Some(points) = v.get("cubic_bezier").and_then(|p| p.as_array()) else {
                return Ease::Linear;
            };
            let at = |i: usize| points.get(i).and_then(serde_json::Value::as_f64);
            match (at(0), at(1), at(2), at(3)) {
                (Some(x1), Some(y1), Some(x2), Some(y2)) => {
                    cubic_bezier(x1 as f32, y1 as f32, x2 as f32, y2 as f32)
                }
                _ => Ease::Linear,
            }
        }
    }
}

fn parse_color(value: &serde_json::Value) -> Option<Argb> {
    if let Some(packed) = crate::util::datapack::hex_color(value) {
        return Some(Argb(packed));
    }
    Some(Argb(value.as_i64()? as u32))
}

fn load(id: &str, fallback_period: i32) -> Timeline {
    let mut out = Timeline {
        period: fallback_period,
        numbers: std::collections::HashMap::new(),
        colors: std::collections::HashMap::new(),
        phases: Vec::new(),
    };
    let Some(json) = crate::util::datapack::entry("timeline", id) else {
        crate::log_info!("timeline", "no {id}.json; using the transcribed tracks");
        return out;
    };
    if let Some(period) = json.get("period_ticks").and_then(serde_json::Value::as_i64) {
        out.period = period as i32;
    }

    for (key, track) in json["tracks"].as_object().into_iter().flatten() {
        let name = key.strip_prefix("minecraft:").unwrap_or(key);
        let Some(attribute) = name.strip_prefix("visual/") else {
            continue;
        };
        let ease = parse_ease(track.get("ease"));
        let Some(keyframes) = track["keyframes"].as_array() else {
            continue;
        };
        let ticks = |kf: &serde_json::Value| kf.get("ticks")?.as_i64().map(|t| t as i32);

        if attribute == "moon_phase" {
            out.phases = keyframes
                .iter()
                .filter_map(|kf| Some((ticks(kf)?, kf.get("value")?.as_str()?.to_owned())))
                .collect();
            continue;
        }

        if attribute.ends_with("_color") {
            let parsed: Vec<(i32, Argb)> = keyframes
                .iter()
                .filter_map(|kf| Some((ticks(kf)?, parse_color(kf.get("value")?)?)))
                .collect();
            if !parsed.is_empty() {
                let track = Track {
                    keyframes: Box::leak(parsed.into_boxed_slice()),
                    ease,
                };
                out.colors.insert(attribute.to_owned(), track);
            }
        } else {
            let parsed: Vec<(i32, f32)> = keyframes
                .iter()
                .filter_map(|kf| Some((ticks(kf)?, kf.get("value")?.as_f64()? as f32)))
                .collect();
            if !parsed.is_empty() {
                let track = Track {
                    keyframes: Box::leak(parsed.into_boxed_slice()),
                    ease,
                };
                out.numbers.insert(attribute.to_owned(), track);
            }
        }
    }

    crate::log_info!(
        "timeline",
        "{id}.json: period {}, {} colour and {} scalar tracks, {} moon phases",
        out.period,
        out.colors.len(),
        out.numbers.len(),
        out.phases.len()
    );
    out
}

struct Resolved {
    period: i32,
    moon_period: i32,
    sun_angle: &'static Track<f32>,
    moon_angle: &'static Track<f32>,
    star_angle: &'static Track<f32>,
    star_brightness: &'static Track<f32>,
    sky_light_factor: &'static Track<f32>,
    sky_color: &'static Track<Argb>,
    fog_color: &'static Track<Argb>,
    cloud_color: &'static Track<Argb>,
    sky_light_color: &'static Track<Argb>,
    sunrise_color: &'static Track<Argb>,
    base_sky_color: Argb,
    base_fog_color: Argb,
    base_cloud_color: Argb,
    base_sky_light_color: Argb,
    phase_starts: &'static [i32],
}

static RESOLVED: std::sync::OnceLock<Resolved> = std::sync::OnceLock::new();

fn resolved() -> &'static Resolved {
    RESOLVED.get_or_init(|| {
        let day = day();
        let moon = moon();
        let scalar = |attribute: &str, fallback: &'static Track<f32>| -> &'static Track<f32> {
            day.numbers.get(attribute).unwrap_or(fallback)
        };
        let color = |attribute: &str, fallback: &'static Track<Argb>| -> &'static Track<Argb> {
            day.colors.get(attribute).unwrap_or(fallback)
        };

        let overworld = crate::util::datapack::entry("dimension_type", "overworld");
        let base = |attribute: &str, fallback: Argb| {
            overworld
                .as_ref()
                .and_then(|json| {
                    json.get("attributes")?
                        .get(format!("minecraft:visual/{attribute}"))
                })
                .and_then(crate::util::datapack::hex_color)
                .map_or(fallback, Argb)
        };

        Resolved {
            period: day.period,
            moon_period: moon.period,
            sun_angle: scalar("sun_angle", &SUN_ANGLE),
            moon_angle: scalar("moon_angle", &MOON_ANGLE),
            star_angle: scalar("star_angle", &STAR_ANGLE),
            star_brightness: scalar("star_brightness", &STAR_BRIGHTNESS),
            sky_light_factor: scalar("sky_light_factor", &SKY_LIGHT_FACTOR),
            sky_color: color("sky_color", &SKY_COLOR),
            fog_color: color("fog_color", &FOG_COLOR),
            cloud_color: color("cloud_color", &CLOUD_COLOR),
            sky_light_color: color("sky_light_color", &SKY_LIGHT_COLOR),
            sunrise_color: color("sunrise_sunset_color", &SUNRISE_SUNSET_COLOR),
            base_sky_color: base("sky_color", BASE_SKY_COLOR),
            base_fog_color: base("fog_color", BASE_FOG_COLOR),
            base_cloud_color: base("cloud_color", BASE_CLOUD_COLOR),
            base_sky_light_color: base("sky_light_color", BASE_SKY_LIGHT_COLOR),
            phase_starts: Box::leak(
                moon.phases
                    .iter()
                    .map(|(at, _)| *at)
                    .collect::<Vec<i32>>()
                    .into_boxed_slice(),
            ),
        }
    })
}

fn moon_phase_at(resolved: &Resolved, ticks: f64) -> usize {
    let t = ticks.rem_euclid(resolved.moon_period as f64) as i32;
    if resolved.phase_starts.is_empty() {
        return (t / MOON_PHASE_LENGTH) as usize;
    }
    resolved
        .phase_starts
        .iter()
        .rposition(|at| *at <= t)
        .unwrap_or(resolved.phase_starts.len() - 1)
}

#[derive(Clone, Copy, Debug)]
pub struct SkyState {
    pub sun_angle: f32,
    pub moon_angle: f32,
    pub star_angle: f32,
    pub star_brightness: f32,
    pub sky_color: Argb,
    pub fog_color: Argb,
    pub cloud_color: Argb,
    pub sunrise_color: Argb,
    pub moon_phase: usize,
    pub sky_light_factor: f32,
    pub sky_light_color: Argb,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackBases {
    pub sky_color: Argb,
    pub fog_color: Argb,
    pub cloud_color: Argb,
    pub sky_light_color: Argb,
}

pub fn datapack_bases() -> TrackBases {
    let r = resolved();
    TrackBases {
        sky_color: r.base_sky_color,
        fog_color: r.base_fog_color,
        cloud_color: r.base_cloud_color,
        sky_light_color: r.base_sky_light_color,
    }
}

pub fn sample(ticks: f64) -> SkyState {
    sample_with(ticks, &datapack_bases())
}

pub fn sample_with(ticks: f64, bases: &TrackBases) -> SkyState {
    const DEG: f32 = std::f32::consts::PI / 180.0;
    let r = resolved();
    let p = r.period;
    SkyState {
        sun_angle: r.sun_angle.sample(p, ticks) * DEG,
        moon_angle: r.moon_angle.sample(p, ticks) * DEG,
        star_angle: r.star_angle.sample(p, ticks) * DEG,
        star_brightness: r.star_brightness.sample(p, ticks).clamp(0.0, 1.0),
        sky_color: bases.sky_color.multiply(r.sky_color.sample(p, ticks)),
        fog_color: bases.fog_color.multiply(r.fog_color.sample(p, ticks)),
        cloud_color: bases.cloud_color.multiply(r.cloud_color.sample(p, ticks)),
        sunrise_color: r.sunrise_color.sample(p, ticks),
        moon_phase: moon_phase_at(r, ticks),
        sky_light_factor: r.sky_light_factor.sample(p, ticks).clamp(0.0, 1.0),
        sky_light_color: bases
            .sky_light_color
            .multiply(r.sky_light_color.sample(p, ticks)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noon_puts_the_sun_overhead() {
        let s = sample(6000.0);
        assert!(s.sun_angle.abs() < 1e-4, "{}", s.sun_angle);
        assert!((s.moon_angle - std::f32::consts::PI).abs() < 1e-4);
        assert_eq!(s.star_brightness, 0.0);
    }

    #[test]
    fn the_bezier_reproduces_the_old_celestial_angle() {
        for tick in (0..24000).step_by(37) {
            let f = ((tick as f64 / 24000.0) - 0.25).rem_euclid(1.0);
            let g = 0.5 - (f * std::f64::consts::PI).cos() / 2.0;
            let expected = (f * 2.0 / 3.0 + g / 3.0) * 360.0;
            let got = SUN_ANGLE.sample(DAY_PERIOD, tick as f64) as f64;
            assert!(
                (got - expected).abs() < 0.6,
                "tick {tick}: {got} vs {expected}"
            );
        }
    }

    #[test]
    fn midnight_is_black() {
        let s = sample(18000.0);
        assert_eq!(s.sky_color, Argb(0xFF00_0000));
        assert_eq!(s.star_brightness, 0.5);
        assert_eq!(s.sunrise_color.alpha(), 0);
        assert_eq!(s.cloud_color, Argb(0xCC19_1926));
    }

    #[test]
    fn midday_leaves_the_base_colours_alone() {
        let s = sample(6000.0);
        assert_eq!(s.sky_color, BASE_SKY_COLOR);
        assert_eq!(s.fog_color, BASE_FOG_COLOR);
        assert_eq!(s.cloud_color, BASE_CLOUD_COLOR);
    }

    #[test]
    fn the_sunset_fan_peaks_in_the_evening() {
        let s = sample(12732.0);
        assert_eq!(s.sunrise_color, Argb(0xFEDA_6333));
        assert!(sample(6000.0).sunrise_color.alpha() == 0);
    }

    #[test]
    fn the_moon_cycles_over_eight_days() {
        assert_eq!(sample(0.0).moon_phase, 0);
        assert_eq!(sample(30000.0).moon_phase, 1);
        assert_eq!(sample(192_000.0).moon_phase, 0);
        assert_eq!(sample(191_999.0).moon_phase, 7);
    }

    #[test]
    fn the_wrap_segment_covers_the_gap_at_tick_zero() {
        let before = STAR_BRIGHTNESS.sample(DAY_PERIOD, 23_900.0);
        let after = STAR_BRIGHTNESS.sample(DAY_PERIOD, 30.0);
        assert!(before < 0.101 && before > 0.037, "{before}");
        assert!(after < before && after > 0.037, "{after}");
    }
}

#[cfg(test)]
mod argb_tests {
    use super::*;

    #[test]
    fn scale_rgb_truncates_per_channel() {
        let got = Argb(0xFF78_A7FF).scale_rgb(0.5, 0.5, 0.6);
        assert_eq!(got, Argb(0xFF3C_5399), "{:08X}", got.0);
    }

    #[test]
    fn scale_rgb_clamps_above_full() {
        assert_eq!(
            Argb(0xFF80_8080).scale_rgb(4.0, 4.0, 4.0),
            Argb(0xFFFF_FFFF)
        );
    }

    #[test]
    fn greyscale_uses_the_luminance_weights() {
        assert_eq!(Argb(0xFF78_A7FF).greyscale(), Argb(0xFFA2_A2A2));
    }

    #[test]
    fn blend_to_gray_matches_the_rain_layer() {
        let got = Argb(0xFF78_A7FF).blend_to_gray(0.6, 0.75);
        assert_eq!(got, Argb(0xFF66_7288), "{:08X}", got.0);
    }

    #[test]
    fn alpha_blend_composites_source_over_destination() {
        let got = Argb(0xFFFF_FFFF).alpha_blend(Argb(0x8000_0000));
        assert_eq!(got, Argb(0xFF7F_7F7F), "{:08X}", got.0);
    }

    #[test]
    fn alpha_blend_short_circuits_at_both_ends() {
        let dest = Argb(0xFF12_3456);
        assert_eq!(dest.alpha_blend(Argb(0x0000_0000)), dest);
        assert_eq!(dest.alpha_blend(Argb(0xFFAB_CDEF)), Argb(0xFFAB_CDEF));
    }

    #[test]
    fn opaque_and_alpha_f32_agree_with_argb() {
        assert_eq!(Argb(0x3378_A7FF).opaque(), Argb(0xFF78_A7FF));
        assert!((Argb(0x3378_A7FF).alpha_f32() - 51.0 / 255.0).abs() < 1e-6);
    }
}
