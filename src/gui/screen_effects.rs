use crate::gui::painter::{self, Painter};
use crate::util::mth::as_8bit_channel;

const FIRE_X_OFFSET: f32 = 0.24;
const FIRE_Y_OFFSET: f32 = -0.3;
const FIRE_LOWERED_BY: f32 = 0.2;
const FIRE_TURN_DEGREES: f32 = 10.0;
const FIRE_HALF_SIZE: f32 = 0.5;
const FIRE_Z: f32 = -0.5;
const FIRE_ALPHA: f32 = 0.9;
const FIRE_STRIPS: usize = 8;

const VIEW_BLOCKED_ARGB: u32 = 0xFF0D_0D0D;

const VIEW_BLOCK_WIDTH_SPREAD: f64 = 0.8;
const VIEW_BLOCK_HEIGHT_SPREAD: f64 = 0.1;

const NAUSEA_TINT: [f32; 3] = [0.2, 0.4, 0.2];
const NAUSEA_SIZE: (f32, f32) = (2.0, 1.0);

const PORTAL_FRAMES: u64 = 32;
const PORTAL_MIN_ALPHA: f32 = 0.2;

#[derive(Clone, Copy, Debug, Default)]
pub struct ScreenEffects {
    pub view_blocked: bool,
    pub on_fire: bool,
    pub confusion: f32,
    pub portal: f32,
    pub portal_frame: u64,
    pub vignette: u32,
}

fn vignette_color(border: &azalea::world::WorldBorder, x: f64, z: f64) -> u32 {
    let dist = f64::from(border.distance_to_border(x, z) as f32);
    let moving = (border.lerp_speed() * f64::from(border.warning_time))
        .min((border.lerp_target() - border.size()).abs());
    let warning = f64::from(border.warning_blocks).max(moving);
    if dist >= warning {
        return 0;
    }
    let strength = (1.0 - (dist / warning) as f32).clamp(0.0, 1.0);
    let gb = as_8bit_channel(strength);
    0xFF00_0000 | gb << 8 | gb
}

pub(crate) fn screen_effect_scale(screen_effects: u32) -> f32 {
    screen_effects.min(100) as f32 / 100.0
}

impl ScreenEffects {
    pub fn read(
        view: &crate::renderer::frame_view::FrameView,
        detached: bool,
        screen_effects: u32,
    ) -> ScreenEffects {
        if !view.in_world {
            return ScreenEffects::default();
        }
        let first_person = !detached && !view.sleeping;
        let spectator = view.gamemode == crate::session::Gamemode::Spectator;
        let view_blocked = first_person && !spectator && view_blocking(view);
        let portal = view.portal;
        let scale = screen_effect_scale(screen_effects);
        let confusion = if portal <= 0.0 && view.nausea > 0.0 && scale < 1.0 {
            view.nausea * (1.0 - scale)
        } else {
            0.0
        };
        ScreenEffects {
            view_blocked,
            on_fire: first_person && !spectator && view.local_body.on_fire,
            confusion,
            portal,
            portal_frame: view.game_time % PORTAL_FRAMES,
            vignette: vignette_color(&view.border, view.player_pos[0], view.player_pos[2]),
        }
    }
}

fn view_blocking(view: &crate::renderer::frame_view::FrameView) -> bool {
    let Some(world) = crate::client::tracking::current_world()
        .lock()
        .unwrap()
        .clone()
    else {
        return false;
    };
    let world = crate::client::tracking::read_lock(&world, "view_blocking");
    let body = &view.local_body;
    let [x, y, z] = view.player_pos;
    let eye = y + body.eye_height as f64;
    let width = body.bounding_box[0] as f64 * VIEW_BLOCK_WIDTH_SPREAD;
    let height = VIEW_BLOCK_HEIGHT_SPREAD * body.scale as f64;
    (0..8).any(|i: u32| {
        let corner = |bit: u32| ((i >> bit) % 2) as f64 - 0.5;
        let pos = azalea_core::position::BlockPos::new(
            (x + corner(0) * width).floor() as i32,
            (eye + corner(1) * height).floor() as i32,
            (z + corner(2) * width).floor() as i32,
        );
        world
            .get_block_state(pos)
            .is_some_and(crate::util::block_model::is_solid)
    })
}

pub fn draw(p: &mut Painter, fx: &ScreenEffects, vw: f32, vh: f32) {
    if fx.view_blocked {
        p.fill(0.0, 0.0, vw, vh, VIEW_BLOCKED_ARGB);
    }
    if fx.on_fire {
        draw_fire(p, vw, vh);
    }
}

fn draw_fire(p: &mut Painter, vw: f32, vh: f32) {
    let tan_half = (crate::renderer::hand::HAND_FOV_DEGREES.to_radians() / 2.0).tan();
    let aspect = vw / vh;
    let project = |[x, y, z]: [f32; 3]| {
        let depth = -z;
        let ndc_x = x / (depth * tan_half * aspect);
        let ndc_y = y / (depth * tan_half);
        [(ndc_x + 1.0) * 0.5 * vw, (1.0 - ndc_y) * 0.5 * vh]
    };
    let argb = painter::argb(FIRE_ALPHA, 0x00FF_FFFF);
    for i in 0..2 {
        let side = if i == 0 { 1.0 } else { -1.0 };
        let (sin, cos) = (-side * FIRE_TURN_DEGREES).to_radians().sin_cos();
        let place = |lx: f32, ly: f32| {
            let x = lx * cos + FIRE_Z * sin;
            let z = -lx * sin + FIRE_Z * cos;
            project([
                x + side * FIRE_X_OFFSET,
                ly + FIRE_Y_OFFSET - FIRE_LOWERED_BY,
                z,
            ])
        };
        for k in 0..FIRE_STRIPS {
            let x0 = -FIRE_HALF_SIZE + k as f32 / FIRE_STRIPS as f32;
            let x1 = x0 + 1.0 / FIRE_STRIPS as f32;
            let (u0, u1) = (FIRE_HALF_SIZE - x0, FIRE_HALF_SIZE - x1);
            p.sprite_quad(
                "fire_overlay",
                [
                    place(x0, FIRE_HALF_SIZE),
                    place(x1, FIRE_HALF_SIZE),
                    place(x1, -FIRE_HALF_SIZE),
                    place(x0, -FIRE_HALF_SIZE),
                ],
                [[u0, 0.0], [u1, 0.0], [u1, 1.0], [u0, 1.0]],
                argb,
                false,
            );
        }
    }
}

pub fn draw_portal(p: &mut Painter, fx: &ScreenEffects, vw: f32, vh: f32) {
    let mut alpha = fx.portal;
    if alpha <= 0.0 {
        return;
    }
    if alpha < 1.0 {
        alpha = alpha.powi(4) * (1.0 - PORTAL_MIN_ALPHA) + PORTAL_MIN_ALPHA;
    }
    let argb = painter::white(alpha);
    let v0 = fx.portal_frame as f32 / PORTAL_FRAMES as f32;
    let v1 = v0 + 1.0 / PORTAL_FRAMES as f32;
    p.sprite_quad(
        "portal_overlay",
        [[0.0, 0.0], [vw, 0.0], [vw, vh], [0.0, vh]],
        [[0.0, v0], [1.0, v0], [1.0, v1], [0.0, v1]],
        argb,
        false,
    );
}

pub fn draw_confusion(p: &mut Painter, fx: &ScreenEffects, vw: f32, vh: f32) {
    let strength = fx.confusion;
    if strength <= 0.0 {
        return;
    }
    let size = crate::util::mth::lerp(strength, NAUSEA_SIZE.0, NAUSEA_SIZE.1);
    let (w, h) = (vw * size, vh * size);
    let (x, y) = ((vw - w) / 2.0, (vh - h) / 2.0);
    let channel = |c: f32| as_8bit_channel((c * strength).clamp(0.0, 1.0));
    let argb = 0xFF00_0000
        | channel(NAUSEA_TINT[0]) << 16
        | channel(NAUSEA_TINT[1]) << 8
        | channel(NAUSEA_TINT[2]);
    p.sprite_quad(
        "nausea_overlay",
        [[x, y], [x + w, y], [x + w, y + h], [x, y + h]],
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        argb,
        true,
    );
}
