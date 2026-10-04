use super::dimension::Dimension;
use super::timeline::Argb;

#[derive(Clone, Copy, Default)]
pub struct State {
    pub sky_factor: f32,
    pub block_factor: f32,
    pub night_vision_factor: f32,
    pub darkness_scale: f32,
    pub boss_overlay_darkening: f32,
    pub brightness_factor: f32,
    pub block_light_tint: [f32; 3],
    pub sky_light_color: [f32; 3],
    pub ambient_color: [f32; 3],
    pub night_vision_color: [f32; 3],
}

const BLOCK_LIGHT_TINT: Argb = Argb(0xFFFF_D84C);

fn rgb(argb: Argb) -> [f32; 3] {
    let [r, g, b, _] = argb.to_rgba_f32();
    [r, g, b]
}

impl State {
    pub fn sample(
        dim: Dimension,
        sky: &super::timeline::SkyState,
        flicker: f32,
        gamma: f32,
    ) -> Self {
        Self {
            sky_factor: dim.sky_light_factor().unwrap_or(sky.sky_light_factor),
            block_factor: flicker + 1.4,
            night_vision_factor: 0.0,
            darkness_scale: 0.0,
            boss_overlay_darkening: 0.0,
            brightness_factor: gamma,
            block_light_tint: rgb(BLOCK_LIGHT_TINT),
            sky_light_color: rgb(dim.sky_light_color().unwrap_or(sky.sky_light_color)),
            ambient_color: rgb(dim.ambient_light_color()),
            night_vision_color: [0.0; 3],
        }
    }
}

pub fn levels_at(eye: [f32; 3]) -> (u8, u8) {
    let (x, y, z) = (
        eye[0].floor() as i32,
        eye[1].floor() as i32,
        eye[2].floor() as i32,
    );
    resolved_levels(crate::client::worldsync::light_map().read().at(x, y, z))
}

pub fn resolved_levels(levels: (u8, u8)) -> (u8, u8) {
    if crate::renderer::lighting_enabled() {
        levels
    } else {
        (15, 15)
    }
}

pub fn light_color(state: &State, block: u8, sky: u8) -> [f32; 3] {
    texel(state, block as u32, sky as u32).map(to_linear)
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CellLight {
    pub color: [f32; 3],
    pub floor: [f32; 3],
    pub block: [f32; 3],
    pub sky: [f32; 3],
    #[cfg(feature = "shader_support")]
    pub levels: [u8; 2],
}

impl CellLight {
    pub fn full_bright() -> CellLight {
        CellLight {
            color: [1.0; 3],
            floor: [0.0; 3],
            block: [0.0; 3],
            sky: [1.0; 3],
            #[cfg(feature = "shader_support")]
            levels: [15, 15],
        }
    }
}

pub fn cell_light(state: &State, block: u8, sky: u8) -> CellLight {
    cell_light_with_floor(state, block, sky, light_color(state, 0, 0))
}

pub fn cell_light_with_floor(state: &State, block: u8, sky: u8, floor: [f32; 3]) -> CellLight {
    let whole = light_color(state, block, sky);
    let separate = |row: [f32; 3]| {
        [
            (row[0] - floor[0]).max(0.0),
            (row[1] - floor[1]).max(0.0),
            (row[2] - floor[2]).max(0.0),
        ]
    };
    CellLight {
        color: whole,
        floor,
        block: separate(light_color(state, block, 0)),
        sky: separate(light_color(state, 0, sky)),
        #[cfg(feature = "shader_support")]
        levels: [block, sky],
    }
}

pub struct LightCells {
    floor: [f32; 3],
    cells: [Option<CellLight>; 256],
}

impl Default for LightCells {
    fn default() -> Self {
        LightCells {
            floor: [0.0; 3],
            cells: [None; 256],
        }
    }
}

impl LightCells {
    pub fn begin(&mut self, state: &State) {
        self.floor = light_color(state, 0, 0);
        self.cells = [None; 256];
    }

    pub fn get(&mut self, state: &State, block: u8, sky: u8) -> CellLight {
        let (block, sky) = (block.min(15), sky.min(15));
        let slot = (block as usize) << 4 | sky as usize;
        match self.cells[slot] {
            Some(light) => light,
            None => {
                let light = cell_light_with_floor(state, block, sky, self.floor);
                self.cells[slot] = Some(light);
                light
            }
        }
    }
}

pub fn entity_light(state: &State, eye: [f32; 3], on_fire: bool) -> CellLight {
    let (block, sky) = levels_at(eye);
    cell_light(state, if on_fire { 15 } else { block }, sky)
}

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn tick_flicker(flicker: f32, noise: [f32; 3]) -> f32 {
    let step = (noise[0] - noise[1]) * noise[1] * noise[2] * 0.1;
    (flicker + step) * 0.9
}

fn get_brightness(level: f32) -> f32 {
    level / (4.0 - 3.0 * level)
}

fn parabolic_mix_factor(level: f32) -> f32 {
    (2.0 * level - 1.0) * (2.0 * level - 1.0)
}

fn not_gamma(color: [f32; 3]) -> [f32; 3] {
    let max = color[0].max(color[1]).max(color[2]);
    if max <= 0.0 {
        return color;
    }
    let inverted = 1.0 - max;
    let scaled = 1.0 - inverted * inverted * inverted * inverted;
    color.map(|c| c * (scaled / max))
}

pub fn texel(state: &State, block: u32, sky: u32) -> [f32; 3] {
    let block_level = block as f32 / 15.0;
    let sky_level = sky as f32 / 15.0;

    let block_brightness = get_brightness(block_level) * state.block_factor;
    let sky_brightness = get_brightness(sky_level) * state.sky_factor;

    let night_vision = state
        .night_vision_color
        .map(|c| c * state.night_vision_factor);
    let mut color = [0f32; 3];
    for i in 0..3 {
        color[i] = state.ambient_color[i].max(night_vision[i]);
        color[i] += state.sky_light_color[i] * sky_brightness;
    }

    let mix = 0.9 * parabolic_mix_factor(block_level);
    for i in 0..3 {
        let tint = state.block_light_tint[i] + (1.0 - state.block_light_tint[i]) * mix;
        color[i] += tint * block_brightness;
    }

    for i in 0..3 {
        let darkened = color[i] * [0.7, 0.6, 0.6][i];
        color[i] += (darkened - color[i]) * state.boss_overlay_darkening;
        color[i] -= state.darkness_scale;
        color[i] = color[i].clamp(0.0, 1.0);
    }

    let lifted = not_gamma(color);
    for i in 0..3 {
        color[i] += (lifted[i] - color[i]) * state.brightness_factor;
        color[i] = color[i].clamp(0.0, 1.0);
    }
    color
}

#[allow(dead_code, reason = "the allocating form, used by this module's tests")]
pub fn image_data(state: &State) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 * 16 * 4);
    write_image_data(state, &mut out);
    out
}

pub fn write_image_data(state: &State, out: &mut Vec<u8>) {
    out.clear();
    for sky in 0..16u32 {
        for block in 0..16u32 {
            let c = texel(state, block, sky);
            for channel in c {
                out.push((channel.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            }
            out.push(255);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noon() -> State {
        State::sample(
            Dimension::Overworld,
            &crate::renderer::timeline::sample(6000.0),
            0.0,
            0.5,
        )
    }

    #[test]
    fn noon_full_sky_is_bright() {
        let c = texel(&noon(), 0, 15);
        assert!(c.iter().all(|v| *v > 0.9), "{c:?}");
    }

    #[test]
    fn no_light_at_all_is_dim_but_not_black() {
        let c = texel(&noon(), 0, 0);
        assert!(c.iter().all(|v| (0.05..0.15).contains(v)), "{c:?}");
    }

    #[test]
    fn the_nether_floor_is_warm_and_brighter_than_the_overworld() {
        let nether = texel(
            &State::sample(
                Dimension::Nether,
                &crate::renderer::timeline::sample(6000.0),
                0.0,
                0.5,
            ),
            0,
            0,
        );
        let overworld = texel(&noon(), 0, 0);
        assert!(nether[0] > overworld[0], "{nether:?} vs {overworld:?}");
        assert!(nether[0] > nether[2], "{nether:?}");
    }

    #[test]
    fn the_nether_ignores_sky_light() {
        let s = State::sample(
            Dimension::Nether,
            &crate::renderer::timeline::sample(6000.0),
            0.0,
            0.5,
        );
        assert_eq!(texel(&s, 0, 15), texel(&s, 0, 0));
    }

    #[test]
    fn midnight_sky_light_is_dim_and_blue() {
        let day = texel(&noon(), 0, 15);
        let night = texel(
            &State::sample(
                Dimension::Overworld,
                &crate::renderer::timeline::sample(18000.0),
                0.0,
                0.5,
            ),
            0,
            15,
        );
        assert!(night[2] < day[2]);
        assert!(night[0] < night[2], "{night:?}");
    }

    #[test]
    fn block_light_is_warmer_than_sky_light() {
        let c = texel(
            &State::sample(
                Dimension::Overworld,
                &crate::renderer::timeline::sample(18000.0),
                0.0,
                0.5,
            ),
            8,
            0,
        );
        assert!(c[0] > c[2], "{c:?}");
    }

    #[test]
    fn the_image_is_sixteen_by_sixteen_rgba() {
        assert_eq!(image_data(&noon()).len(), 16 * 16 * 4);
    }
}
