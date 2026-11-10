use azalea_registry::builtin::EntityKind;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::{Mesh, Quat, Transform, Vec3};

use crate::entities::TexturePath;
use crate::entities::feed::DRAGON_DEATH_TICKS;
use crate::entities::models::monsters::dragon;
use crate::entities::registry::Registry;
use crate::entities::state::{DragonPose, EntityState};
use crate::entities::{Blend, RenderSpec, RootPose};
use crate::renderer::entity_material::ALPHA_CUTOUT;
use crate::util::javarandom::JavaRandom;
use crate::util::mth::srgb_to_linear;

const TEXTURE: &str = "entity/enderdragon/dragon";

const DYING_TEXTURE: &str =
    "dissolve/entity/enderdragon/dragon+entity/enderdragon/dragon_exploding";

pub(crate) const DISSOLVE_PREFIX: &str = "dissolve/";

pub(crate) fn dissolve(rest: &str) -> Option<image::RgbaImage> {
    let (texture, mask) = rest.split_once('+')?;
    let mut out = crate::entities::decode_entity_texture(texture)?;
    let mask = crate::entities::decode_entity_texture(mask)?;
    let (w, h) = out.dimensions();
    let (mw, mh) = mask.dimensions();
    let cutout = (ALPHA_CUTOUT * 255.0).ceil() as u8;
    for (x, y, px) in out.enumerate_pixels_mut() {
        let m = mask.get_pixel(x * mw / w.max(1), y * mh / h.max(1))[3];
        px[3] = if px[3] < cutout { 0 } else { 255 - m };
    }
    Some(out)
}

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::EnderDragon,
        RenderSpec::new("ender_dragon", dragon::layer, texture, dragon::setup_anim)
            .with_root(root)
            .with_cutoff(dissolve_cutoff),
    );
    registry.add(
        EntityKind::EnderDragon,
        RenderSpec::new(
            "ender_dragon_eyes",
            dragon::layer,
            eyes_texture,
            dragon::setup_anim,
        )
        .with_root(root)
        .with_blend(Blend::Additive),
    );
    registry.add(
        EntityKind::EnderDragon,
        RenderSpec::built("ender_dragon_rays", rays_key, rays_mesh, untextured)
            .with_root(rays_root)
            .with_visible(is_dying)
            .with_blend(Blend::Additive)
            .ignoring_invisibility(),
    );
}

fn is_dying(st: &EntityState) -> bool {
    st.death_time > 0.0
}

fn texture(st: &EntityState) -> TexturePath {
    if is_dying(st) {
        DYING_TEXTURE.into()
    } else {
        TEXTURE.into()
    }
}

fn dissolve_cutoff(st: &EntityState) -> f32 {
    if is_dying(st) {
        st.death_time / DRAGON_DEATH_TICKS
    } else {
        ALPHA_CUTOUT
    }
}

fn eyes_texture(_st: &EntityState) -> TexturePath {
    "entity/enderdragon/dragon_eyes".into()
}

fn pose(st: &EntityState) -> &DragonPose {
    st.extras.dragon.as_deref().unwrap_or(&DragonPose::REST)
}

fn rotation(st: &EntityState) -> Quat {
    let pose = pose(st);
    let yr = pose.historical(7).y_rot;
    let rot2 = (pose.historical(5).y - pose.historical(10).y) as f32;
    Quat::from_rotation_y((-yr).to_radians()) * Quat::from_rotation_x((rot2 * 10.0).to_radians())
}

fn root(st: &EntityState) -> RootPose {
    RootPose {
        rotation: rotation(st),
        scale: st.scale,
        hook: Transform::from_xyz(0.0, 0.0, 1.0),
        ..RootPose::default()
    }
}

fn rays_root(st: &EntityState) -> RootPose {
    RootPose {
        rotation: rotation(st),
        scale: st.scale,
        hook: Transform {
            translation: Vec3::new(0.0, 2.501, -1.0),
            rotation: Quat::from_rotation_z(std::f32::consts::PI),
            scale: Vec3::ONE,
        },
        ..RootPose::default()
    }
}

fn untextured(_st: &EntityState) -> Option<TexturePath> {
    None
}

fn rays_key(st: &EntityState) -> TexturePath {
    (st.death_time.floor() as i32).to_string().into()
}

const HALF_SQRT_3: f32 = 0.866_025_4;
const RAY_BANDS: usize = 4;

fn rays_mesh(st: &EntityState) -> Mesh {
    let death_time = st.death_time.floor() / DRAGON_DEATH_TICKS;
    let over_drive = (if death_time > 0.8 {
        (death_time - 0.8) / 0.2
    } else {
        0.0
    })
    .min(1.0);
    let inner_alpha = 1.0 - over_drive;
    let mut random = JavaRandom::new(432);
    let ray_count = ((death_time + death_time * death_time) / 2.0 * 60.0).floor() as i32;
    let tau = std::f32::consts::TAU;

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut pose = Quat::IDENTITY;
    let share = |band: usize| 1.0 - band as f32 / RAY_BANDS as f32;
    let color = |t: f32| {
        let a = inner_alpha;
        [
            srgb_to_linear(t * a),
            srgb_to_linear(t * t * a),
            srgb_to_linear(t * a),
            0.0,
        ]
    };
    for _ in 0..ray_count.max(0) {
        let mut angle = || random.next_f32() * tau;
        let (a, b, c, d, e, f) = (angle(), angle(), angle(), angle(), angle(), angle());
        let ray = Quat::from_rotation_x(a)
            * Quat::from_rotation_y(b)
            * Quat::from_rotation_z(c)
            * Quat::from_rotation_x(d)
            * Quat::from_rotation_y(e)
            * Quat::from_rotation_z(f + death_time * std::f32::consts::FRAC_PI_2);
        pose *= ray;
        let length = random.next_f32() * 20.0 + 5.0 + over_drive * 10.0;
        let width = random.next_f32() * 2.0 + 1.0 + over_drive * 2.0;
        let left = pose * Vec3::new(-HALF_SQRT_3 * width, length, -0.5 * width);
        let right = pose * Vec3::new(HALF_SQRT_3 * width, length, -0.5 * width);
        let bottom = pose * Vec3::new(0.0, length, width);
        for (p, q) in [(left, right), (right, bottom), (bottom, left)] {
            for band in 0..RAY_BANDS {
                let (t0, t1) = (share(band), share(band + 1));
                let corners = [
                    p * (1.0 - t0),
                    p * (1.0 - t1),
                    q * (1.0 - t1),
                    q * (1.0 - t0),
                ];
                positions.extend(corners.map(|v| v.to_array()));
                colors.extend([color(t0), color(t1), color(t1), color(t0)]);
            }
        }
    }

    let quads = positions.len() / 4;
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vec![[0.0f32, 1.0, 0.0]; positions.len()],
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; positions.len()]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    let indices: Indices = crate::entities::quads::quad_indices(quads);
    mesh.insert_indices(indices);
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_tick_draws_sixty_rays() {
        let mut st = EntityState::new(1, EntityKind::EnderDragon);
        st.death_time = 200.0;
        let mesh = rays_mesh(&st);
        assert_eq!(mesh.count_vertices(), 60 * 3 * RAY_BANDS * 4);
    }

    #[test]
    fn the_cutoff_runs_with_death_time() {
        let mut st = EntityState::new(1, EntityKind::EnderDragon);
        assert_eq!(dissolve_cutoff(&st), ALPHA_CUTOUT);
        st.death_time = 50.0;
        assert!((dissolve_cutoff(&st) - 0.25).abs() < 1e-6);
    }
}
