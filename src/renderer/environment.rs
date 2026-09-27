use bevy::prelude::Resource;

use crate::util::biome_color::{BiomeEnv, ColorAttr, FloatAttr};

use super::dimension::Dimension;
use super::timeline::{Argb, Lerp, SkyState};

#[derive(Clone, Copy, Debug)]
pub struct Bases {
    pub sky_color: Argb,
    pub fog_color: Argb,
    pub cloud_color: Argb,
    pub sky_light_color: Argb,
    pub water_fog_color: Argb,
    pub fog_distance: (f32, f32),
    pub sky_fog_end: f32,
    pub water_fog_distance: (f32, f32),
}

impl Bases {
    pub fn for_dimension(dim: Dimension) -> Bases {
        let timeline = super::timeline::datapack_bases();
        let overworld = matches!(dim, Dimension::Overworld);
        Bases {
            sky_color: if overworld {
                timeline.sky_color
            } else {
                dim.base_sky_color()
            },
            fog_color: if overworld {
                timeline.fog_color
            } else {
                dim.base_fog_color()
            },
            cloud_color: if overworld {
                timeline.cloud_color
            } else {
                dim.cloud_color().unwrap_or(timeline.cloud_color)
            },
            sky_light_color: if overworld {
                timeline.sky_light_color
            } else {
                dim.sky_light_color().unwrap_or(timeline.sky_light_color)
            },
            water_fog_color: dim.base_water_fog_color(),
            fog_distance: dim.base_fog_distance(),
            sky_fog_end: dim.base_sky_fog_end(),
            water_fog_distance: dim.base_water_fog_distance(),
        }
    }
}

const KERNEL: [f32; 7] = [0.0, 1.0, 4.0, 6.0, 4.0, 1.0, 0.0];

pub const BREADTH: usize = 6;

pub const RADIUS: i32 = 2;

pub const SAMPLE_COUNT: usize = BREADTH * BREADTH * BREADTH;

pub fn axis_weights(fraction: f32) -> [f32; BREADTH] {
    std::array::from_fn(|i| KERNEL[i + 1] + fraction * (KERNEL[i] - KERNEL[i + 1]))
}

pub const fn sample_index(x: usize, y: usize, z: usize) -> usize {
    (z * BREADTH + x) * BREADTH + y
}

pub const MAX_SOURCES: usize = 16;

#[derive(Clone, Copy)]
pub struct Blend {
    rows: [u8; MAX_SOURCES],
    weights: [f32; MAX_SOURCES],
    len: usize,
}

impl Default for Blend {
    fn default() -> Self {
        Blend {
            rows: [0; MAX_SOURCES],
            weights: [0.0; MAX_SOURCES],
            len: 0,
        }
    }
}

impl Blend {
    pub fn bucket_for(&mut self, row: u8) -> usize {
        if let Some(at) = self.rows[..self.len].iter().position(|r| *r == row) {
            return at;
        }
        if self.len == MAX_SOURCES {
            return MAX_SOURCES - 1;
        }
        self.rows[self.len] = row;
        self.len += 1;
        self.len - 1
    }

    pub fn clear_weights(&mut self) {
        self.weights[..self.len].fill(0.0);
    }

    pub fn add_weight(&mut self, bucket: usize, weight: f32) {
        self.weights[bucket] += weight;
    }

    pub fn resolve(&self, bases: &Bases) -> BiomeLayer {
        if self.len == 0 {
            return BiomeLayer::from(bases);
        }
        if self.len == 1 {
            return BiomeLayer::fold(crate::util::biome_color::env(self.rows[0]), bases);
        }

        let mut total = 0.0f32;
        let mut sky = [0.0f32; 3];
        let mut fog = [0.0f32; 3];
        let mut water_fog = [0.0f32; 3];
        let mut scalars = [0.0f32; 5];

        for i in 0..self.len {
            let weight = self.weights[i];
            if weight <= 0.0 {
                continue;
            }
            let folded = BiomeLayer::fold(crate::util::biome_color::env(self.rows[i]), bases);
            total += weight;
            accumulate_rgb(&mut sky, folded.sky_color, weight);
            accumulate_rgb(&mut fog, folded.fog_color, weight);
            accumulate_rgb(&mut water_fog, folded.water_fog_color, weight);
            scalars[0] += folded.fog_distance.0 * weight;
            scalars[1] += folded.fog_distance.1 * weight;
            scalars[2] += folded.sky_fog_end * weight;
            scalars[3] += folded.water_fog_distance.0 * weight;
            scalars[4] += folded.water_fog_distance.1 * weight;
        }

        if total <= 0.0 {
            return BiomeLayer::from(bases);
        }
        let inv = 1.0 / total;
        BiomeLayer {
            sky_color: mean_rgb(sky, inv),
            fog_color: mean_rgb(fog, inv),
            water_fog_color: mean_rgb(water_fog, inv),
            fog_distance: (scalars[0] * inv, scalars[1] * inv),
            sky_fog_end: scalars[2] * inv,
            water_fog_distance: (scalars[3] * inv, scalars[4] * inv),
        }
    }
}

fn accumulate_rgb(into: &mut [f32; 3], color: Argb, weight: f32) {
    into[0] += color.channel(16) as f32 * weight;
    into[1] += color.channel(8) as f32 * weight;
    into[2] += color.channel(0) as f32 * weight;
}

fn mean_rgb(sum: [f32; 3], inv_total: f32) -> Argb {
    let ch = |v: f32| ((v * inv_total) as u32).min(255);
    Argb(0xFF00_0000 | (ch(sum[0]) << 16) | (ch(sum[1]) << 8) | ch(sum[2]))
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BiomeLayer {
    pub sky_color: Argb,
    pub fog_color: Argb,
    pub water_fog_color: Argb,
    pub fog_distance: (f32, f32),
    pub sky_fog_end: f32,
    pub water_fog_distance: (f32, f32),
}

impl From<&Bases> for BiomeLayer {
    fn from(bases: &Bases) -> Self {
        BiomeLayer {
            sky_color: bases.sky_color,
            fog_color: bases.fog_color,
            water_fog_color: bases.water_fog_color,
            fog_distance: bases.fog_distance,
            sky_fog_end: bases.sky_fog_end,
            water_fog_distance: bases.water_fog_distance,
        }
    }
}

impl BiomeLayer {
    fn fold(env: &BiomeEnv, bases: &Bases) -> BiomeLayer {
        BiomeLayer {
            sky_color: fold_color(env.sky_color, bases.sky_color),
            fog_color: fold_color(env.fog_color, bases.fog_color),
            water_fog_color: fold_color(env.water_fog_color, bases.water_fog_color),
            fog_distance: (
                fold_float(env.fog_start_distance, bases.fog_distance.0),
                fold_float(env.fog_end_distance, bases.fog_distance.1),
            ),
            sky_fog_end: fold_float(env.sky_fog_end_distance, bases.sky_fog_end),
            water_fog_distance: (
                fold_float(env.water_fog_start_distance, bases.water_fog_distance.0),
                fold_float(env.water_fog_end_distance, bases.water_fog_distance.1),
            ),
        }
    }

    pub fn lerp(alpha: f32, from: &BiomeLayer, to: &BiomeLayer) -> BiomeLayer {
        let f = |a: f32, b: f32| a + alpha * (b - a);
        BiomeLayer {
            sky_color: Argb::lerp(alpha, from.sky_color, to.sky_color),
            fog_color: Argb::lerp(alpha, from.fog_color, to.fog_color),
            water_fog_color: Argb::lerp(alpha, from.water_fog_color, to.water_fog_color),
            fog_distance: (
                f(from.fog_distance.0, to.fog_distance.0),
                f(from.fog_distance.1, to.fog_distance.1),
            ),
            sky_fog_end: f(from.sky_fog_end, to.sky_fog_end),
            water_fog_distance: (
                f(from.water_fog_distance.0, to.water_fog_distance.0),
                f(from.water_fog_distance.1, to.water_fog_distance.1),
            ),
        }
    }
}

fn fold_color(attr: Option<ColorAttr>, base: Argb) -> Argb {
    match attr {
        None => base,
        Some(ColorAttr::Set(rgb)) => Argb(0xFF00_0000 | rgb),
        Some(ColorAttr::Multiply(rgb)) => {
            let alpha = base.alpha() << 24;
            Argb(alpha | (base.multiply(Argb(0xFF00_0000 | rgb)).0 & 0x00FF_FFFF))
        }
    }
}

fn fold_float(attr: Option<FloatAttr>, base: f32) -> f32 {
    match attr {
        None => base,
        Some(FloatAttr::Set(v)) => v,
        Some(FloatAttr::Multiply(m)) => base * m,
    }
}

struct WeatherEntry {
    sky: (f32, f32),
    cloud: (f32, f32),
    tint: Argb,
    sky_light_color: Argb,
    sky_light_alpha: f32,
}

const NIGHT_SKY_LIGHT_COLOR: u32 = 0x007A_7AFF;

const WEATHER_SKY_LIGHT_FACTOR: f32 = 0.24;

const RAIN: WeatherEntry = WeatherEntry {
    sky: (0.6, 0.75),
    cloud: (0.24, 0.5),
    tint: Argb(0xFF7F_7F99),
    sky_light_color: Argb(0x4F00_0000 | NIGHT_SKY_LIGHT_COLOR),
    sky_light_alpha: 0.3125,
};

const THUNDER: WeatherEntry = WeatherEntry {
    sky: (0.24, 0.94),
    cloud: (0.095, 0.94),
    tint: Argb(0xFF3F_3F4C),
    sky_light_color: Argb(0x8600_0000 | NIGHT_SKY_LIGHT_COLOR),
    sky_light_alpha: 0.52734375,
};

impl WeatherEntry {
    fn apply(&self, sky: &mut SkyState, level: f32) {
        let blend_color = |current: Argb, modified: Argb| Argb::lerp(level, current, modified);
        sky.sky_color = blend_color(
            sky.sky_color,
            sky.sky_color.blend_to_gray(self.sky.0, self.sky.1),
        );
        sky.cloud_color = blend_color(
            sky.cloud_color,
            sky.cloud_color.blend_to_gray(self.cloud.0, self.cloud.1),
        );
        sky.fog_color = blend_color(sky.fog_color, sky.fog_color.multiply(self.tint));
        sky.sunrise_color = blend_color(sky.sunrise_color, sky.sunrise_color.multiply(self.tint));
        sky.sky_light_color = blend_color(
            sky.sky_light_color,
            sky.sky_light_color.alpha_blend(self.sky_light_color),
        );
        let factor = sky.sky_light_factor
            + self.sky_light_alpha * (WEATHER_SKY_LIGHT_FACTOR - sky.sky_light_factor);
        sky.sky_light_factor += level * (factor - sky.sky_light_factor);
        sky.star_brightness -= level * sky.star_brightness;
    }
}

pub fn apply_weather(sky: &mut SkyState, rain: f32, thunder: f32) {
    if rain <= 0.0 && thunder <= 0.0 {
        return;
    }
    let rain_level = rain - thunder;
    if rain_level > 0.0 {
        RAIN.apply(sky, rain_level);
    }
    if thunder > 0.0 {
        THUNDER.apply(sky, thunder);
    }
}

static RAIN_LEVEL: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static THUNDER_LEVEL: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn rain_level() -> f32 {
    f32::from_bits(RAIN_LEVEL.load(std::sync::atomic::Ordering::Relaxed))
}

pub fn thunder_level() -> f32 {
    f32::from_bits(THUNDER_LEVEL.load(std::sync::atomic::Ordering::Relaxed)) * rain_level()
}

fn clamp_level(level: f32) -> f32 {
    if !(level > 0.0) {
        return 0.0;
    }
    level.min(1.0)
}

pub fn set_rain_level(level: f32) {
    RAIN_LEVEL.store(
        clamp_level(level).to_bits(),
        std::sync::atomic::Ordering::Relaxed,
    );
}

pub fn set_thunder_level(level: f32) {
    THUNDER_LEVEL.store(
        clamp_level(level).to_bits(),
        std::sync::atomic::Ordering::Relaxed,
    );
}

pub fn reset_weather() {
    set_rain_level(0.0);
    set_thunder_level(0.0);
}

fn weather_darken(color: Argb, rain: f32, thunder: f32) -> Argb {
    let mut color = color;
    if rain > 0.0 {
        let rgb = 1.0 - rain * 0.5;
        color = color.scale_rgb(rgb, rgb, 1.0 - rain * 0.4);
    }
    if thunder > 0.0 {
        let all = 1.0 - thunder * 0.5;
        color = color.scale_rgb(all, all, all);
    }
    color
}

pub fn fog_base_color(
    sky: &SkyState,
    sky_fog_end: f32,
    forward_x: f32,
    render_distance: i32,
    rain: f32,
    thunder: f32,
) -> Argb {
    let mut fog = sky.fog_color;
    if render_distance >= 4 {
        let sun_side = if sky.sun_angle.sin() > 0.0 { -1.0 } else { 1.0 };
        let facing = forward_x * sun_side;
        if facing > 0.0 {
            let alpha = sky.sunrise_color.alpha_f32();
            if alpha > 0.0 {
                fog = Argb::lerp(facing * alpha, fog, sky.sunrise_color.opaque());
            }
        }
    }
    let sky_color = weather_darken(sky.sky_color, rain, thunder);
    let end = (sky_fog_end / 16.0).min(render_distance as f32);
    let mix = 0.25 + (end / 32.0).clamp(0.0, 1.0) * 0.75;
    let mix = 1.0 - mix.powf(0.25);
    Argb::lerp(mix, fog, sky_color)
}

#[derive(Default)]
pub struct RainFog {
    multiplier: f32,
}

impl RainFog {
    pub fn advance(
        &mut self,
        rain: f32,
        delta_ticks: f32,
        sky_light: impl FnOnce() -> f32,
        has_precipitation: bool,
    ) -> f32 {
        if rain <= 0.0 && self.multiplier <= 0.0 {
            self.multiplier = 0.0;
            return 0.0;
        }
        let target = if rain <= 0.0 {
            0.0
        } else {
            let light = ((sky_light() - 8.0) / 7.0).clamp(0.0, 1.0);
            rain * light * if has_precipitation { 1.0 } else { 0.5 }
        };
        self.multiplier += (target - self.multiplier) * delta_ticks * 0.2;
        self.multiplier
    }

    pub fn apply(&self, distance: (f32, f32)) -> (f32, f32) {
        if self.multiplier <= 0.0 {
            return distance;
        }
        let start = distance.0 - 160.0 * self.multiplier;
        let floor = 96.0f32.min(distance.1);
        (start, floor.max(distance.1 - 256.0 * self.multiplier))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Precipitation {
    None,
    Rain,
    Snow,
}

pub fn precipitation_at(env: &BiomeEnv, y: i32, sea_level: i32) -> Precipitation {
    if !env.has_precipitation {
        return Precipitation::None;
    }
    let snow_level = sea_level + 17;
    let mut temperature = env.temperature;
    if env.frozen {
        temperature = 0.0;
    }
    if y > snow_level {
        temperature -= (y - snow_level) as f32 * 0.05 / 40.0;
    }
    if temperature >= 0.15 {
        Precipitation::Rain
    } else {
        Precipitation::Snow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_axis_weights_are_the_kernel_slid_along() {
        assert_eq!(axis_weights(0.0), [1.0, 4.0, 6.0, 4.0, 1.0, 0.0]);
        assert_eq!(axis_weights(1.0), [0.0, 1.0, 4.0, 6.0, 4.0, 1.0]);
        let mid = axis_weights(0.5);
        assert_eq!(mid, [0.5, 2.5, 5.0, 5.0, 2.5, 0.5]);
        for step in 0..=10 {
            let total: f32 = axis_weights(step as f32 / 10.0).iter().sum();
            assert!((total - 16.0).abs() < 1e-4, "{total}");
        }
    }

    fn bases() -> Bases {
        Bases {
            sky_color: Argb(0xFF78_A7FF),
            fog_color: Argb(0xFFC0_D8FF),
            cloud_color: Argb(0xCCFF_FFFF),
            sky_light_color: Argb(0xFFFF_FFFF),
            water_fog_color: Argb(0xFF05_0533),
            fog_distance: (0.0, 1024.0),
            sky_fog_end: 512.0,
            water_fog_distance: (-8.0, 96.0),
        }
    }

    #[test]
    fn one_source_is_that_source() {
        let mut blend = Blend::default();
        let bucket = blend.bucket_for(0);
        blend.add_weight(bucket, 216.0);
        let layer = blend.resolve(&bases());
        assert_eq!(layer.sky_color, bases().sky_color);
        assert_eq!(layer.fog_distance, (0.0, 1024.0));
    }

    #[test]
    fn an_empty_blend_is_the_bases() {
        let layer = Blend::default().resolve(&bases());
        assert_eq!(layer.fog_color, Argb(0xFFC0_D8FF));
        assert_eq!(layer.water_fog_distance, (-8.0, 96.0));
    }

    #[test]
    fn buckets_merge_by_row() {
        let mut blend = Blend::default();
        for _ in 0..SAMPLE_COUNT {
            let bucket = blend.bucket_for(7);
            blend.add_weight(bucket, 1.0);
        }
        assert_eq!(blend.len, 1);
    }

    #[test]
    fn overflow_folds_into_the_last_bucket() {
        let mut blend = Blend::default();
        for row in 0..40u8 {
            let bucket = blend.bucket_for(row);
            blend.add_weight(bucket, 1.0);
        }
        assert_eq!(blend.len, MAX_SOURCES);
    }

    #[test]
    fn no_weather_changes_nothing() {
        let before = super::super::timeline::sample(6000.0);
        let mut after = before;
        apply_weather(&mut after, 0.0, 0.0);
        assert_eq!(before.sky_color, after.sky_color);
        assert_eq!(before.fog_color, after.fog_color);
        assert_eq!(before.star_brightness, after.star_brightness);
    }

    #[test]
    fn full_rain_greys_the_sky() {
        let mut sky = super::super::timeline::sample(6000.0);
        let clear = sky.sky_color;
        sky.star_brightness = 0.5;
        apply_weather(&mut sky, 1.0, 0.0);
        assert_eq!(sky.star_brightness, 0.0);
        assert_eq!(sky.sky_color, clear.blend_to_gray(0.6, 0.75));
        assert!(sky.sky_light_factor < 1.0);
    }

    #[test]
    fn thunder_takes_over_from_rain() {
        let base = super::super::timeline::sample(6000.0);
        let mut both = base;
        apply_weather(&mut both, 1.0, 1.0);
        let mut thunder_only = base;
        THUNDER.apply(&mut thunder_only, 1.0);
        assert_eq!(both.sky_color, thunder_only.sky_color);
    }

    #[test]
    fn rain_fog_is_smoothed_and_lazy() {
        let mut fog = RainFog::default();
        assert_eq!(
            fog.advance(0.0, 1.0, || panic!("read the light while dry"), true),
            0.0
        );

        let first = fog.advance(1.0, 1.0, || 15.0, true);
        assert!(first > 0.0 && first < 1.0, "{first}");
        let second = fog.advance(1.0, 1.0, || 15.0, true);
        assert!(second > first, "{second} vs {first}");
    }

    #[test]
    fn rain_fog_ignores_the_indoors() {
        let mut fog = RainFog::default();
        assert_eq!(fog.advance(1.0, 1.0, || 4.0, true), 0.0);
    }

    #[test]
    fn precipitation_follows_temperature_and_height() {
        let warm = BiomeEnv {
            has_precipitation: true,
            temperature: 0.8,
            ..BiomeEnv::NONE
        };
        let cold = BiomeEnv {
            has_precipitation: true,
            temperature: 0.0,
            ..BiomeEnv::NONE
        };
        let dry = BiomeEnv {
            has_precipitation: false,
            temperature: 2.0,
            ..BiomeEnv::NONE
        };
        assert_eq!(precipitation_at(&warm, 64, 63), Precipitation::Rain);
        assert_eq!(precipitation_at(&cold, 64, 63), Precipitation::Snow);
        assert_eq!(precipitation_at(&dry, 64, 63), Precipitation::None);
        assert_eq!(
            precipitation_at(&warm, 63 + 17 + 521, 63),
            Precipitation::Snow
        );
    }
}

#[derive(Resource)]
pub struct Environment {
    pub sky: SkyState,
    pub fog_distance: (f32, f32),
    pub render_fog_distance: (f32, f32),
    pub sky_fog_end: f32,
    pub water_fog_color: Argb,
    pub water_fog_distance: (f32, f32),
    pub rain: f32,
    pub thunder: f32,
    pub render_distance: i32,
    pub rain_fog: RainFog,
}

impl Default for Environment {
    fn default() -> Self {
        let bases = Bases::for_dimension(Dimension::Overworld);
        Environment {
            sky: super::timeline::sample(0.0),
            fog_distance: bases.fog_distance,
            render_fog_distance: (115.2, 128.0),
            sky_fog_end: bases.sky_fog_end,
            water_fog_color: bases.water_fog_color,
            water_fog_distance: bases.water_fog_distance,
            rain: 0.0,
            thunder: 0.0,
            render_distance: 8,
            rain_fog: RainFog::default(),
        }
    }
}

impl Environment {
    pub fn resolve(
        &mut self,
        dim: Dimension,
        ticks: f64,
        layer: &BiomeLayer,
        render_distance: i32,
        delta_ticks: f32,
        sky_light: impl FnOnce() -> f32,
        has_precipitation: bool,
    ) {
        let bases = Bases::for_dimension(dim);

        let mut sky = super::timeline::sample_with(
            ticks,
            &super::timeline::TrackBases {
                sky_color: layer.sky_color,
                fog_color: layer.fog_color,
                cloud_color: bases.cloud_color,
                sky_light_color: bases.sky_light_color,
            },
        );
        if !dim.runs_day_timeline() {
            sky.sky_color = layer.sky_color;
            sky.fog_color = layer.fog_color;
            sky.cloud_color = bases.cloud_color;
            sky.sky_light_color = bases.sky_light_color;
            if let Some(factor) = dim.sky_light_factor() {
                sky.sky_light_factor = factor;
            }
        }

        let (rain, thunder) = if dim.can_have_weather() {
            (rain_level(), thunder_level())
        } else {
            (0.0, 0.0)
        };
        apply_weather(&mut sky, rain, thunder);

        self.rain_fog
            .advance(rain, delta_ticks, sky_light, has_precipitation);
        let environmental = self.rain_fog.apply(layer.fog_distance);

        let radius = (render_distance.max(2) * 16) as f32;
        let span = (radius / 10.0).clamp(4.0, 64.0);
        self.fog_distance = environmental;
        self.render_fog_distance = (radius - span, radius);
        self.sky_fog_end = layer.sky_fog_end.min(radius);
        self.water_fog_color = layer.water_fog_color;
        self.water_fog_distance = layer.water_fog_distance;
        self.sky = sky;
        self.rain = rain;
        self.thunder = thunder;
        self.render_distance = render_distance;
    }
}
