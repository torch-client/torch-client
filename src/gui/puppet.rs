use std::f32::consts::PI;
use std::sync::{Arc, Mutex, OnceLock};

use bevy::prelude::*;
use image::RgbaImage;

use crate::entities::geom;
use crate::entities::quads::QuadMesh;
use crate::gui::atlas::{PUPPET_ARMOR_SLOT, PUPPET_SKIN_SLOT, PUPPET_SLOTS, Region};
use crate::gui::painter::Painter;
use crate::renderer::anim::AnimSample;
use crate::renderer::player_model::{
    ARMOR_LAYERS, ARMOR_PARTS, AVATAR_SCALE, MODEL_Y_OFFSET, OVERLAY_BITS, PARTS, SLIM_ARMS,
};
use crate::renderer::skin::SKIN_SHEET;

const SHEET_PX: u32 = 64;
const ARMOR_SHEET_H: u32 = 32;

const ARMOR_DRAW_ORDER: [usize; 4] = [2, 3, 1, 0];

pub struct Figure<'a> {
    pub sample: &'a AnimSample,
    pub slim: bool,
    pub parts: u8,
    pub armor: [bool; 4],
}

pub fn show_skin(pixels: &Arc<Vec<u8>>) {
    show(0, pixels);
}

fn show_armor(slot: usize, pixels: &Arc<Vec<u8>>) {
    show(1 + slot, pixels);
}

static SHOWN: Mutex<[Option<Arc<Vec<u8>>>; PUPPET_SLOTS]> =
    Mutex::new([const { None }; PUPPET_SLOTS]);

fn show(index: usize, pixels: &Arc<Vec<u8>>) {
    let mut shown = SHOWN.lock().unwrap();
    if shown[index]
        .as_ref()
        .is_some_and(|s| Arc::ptr_eq(s, pixels))
    {
        return;
    }
    crate::gui::ping::note_slot(PUPPET_SKIN_SLOT + index, pixels.clone());
    shown[index] = Some(pixels.clone());
}

pub fn follows_mouse(
    p: &mut Painter,
    figure: &Figure,
    [x0, y0, x1, y1]: [f32; 4],
    size: f32,
    offset_y: f32,
    mouse: Option<Vec2>,
) {
    let Some(skin) = p.atlas.server_icon(PUPPET_SKIN_SLOT) else {
        return;
    };
    let armor: [Option<Region>; 4] = std::array::from_fn(|slot| {
        let cell = p.atlas.server_icon(PUPPET_ARMOR_SLOT + slot)?;
        figure.armor[slot].then_some(Region {
            h: ARMOR_SHEET_H,
            ..cell
        })
    });

    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (x_angle, y_angle) = mouse.map_or((0.0, 0.0), |m| {
        (((cx - m.x) / 40.0).atan(), ((cy - m.y) / 40.0).atan())
    });
    let mut s = figure.sample.clone();
    s.body_yaw = 180.0 + x_angle * 20.0;
    s.head_yaw = x_angle * 20.0;
    s.pitch = if s.fall_flying { 0.0 } else { -y_angle * 20.0 };

    let pose = crate::renderer::player_model::pose_humanoid(&s);
    let root = crate::renderer::player_model::root_pose(&s, 1.0);
    let height = crate::session::player_dimensions(entity_pose(&s), 1.0).height;

    let view = Quat::from_rotation_z(PI) * Quat::from_rotation_x((y_angle * 20.0).to_radians());
    let lift = Vec3::new(0.0, height / 2.0 + offset_y, 0.0);
    let drop = root.model_offset + Vec3::new(0.0, -AVATAR_SCALE * MODEL_Y_OFFSET, 0.0);
    let k = AVATAR_SCALE / 16.0;
    let mirror = Quat::from_rotation_z(PI);
    let outer = view * root.rotation;
    let placed: [(Quat, Vec3); 6] = pose.parts().map(|part| {
        let t = crate::renderer::player_model::model_transform(&part);
        (
            outer * mirror * t.rotation,
            lift + outer * (drop + mirror * (k * t.translation)),
        )
    });

    let model = &geometry()[figure.slim as usize];
    let mut order: [usize; 6] = std::array::from_fn(|i| i);
    let depth: [f32; 6] = std::array::from_fn(|i| {
        let (turn, at) = placed[i];
        (at + k * (turn * model[i].centre)).z
    });
    order.sort_by(|a, b| depth[*b].total_cmp(&depth[*a]));

    for part in order {
        let (turn, at) = placed[part];
        for shell in &model[part].shells {
            let region = match shell.sheet {
                Sheet::Skin => skin,
                Sheet::Overlay(bit) if figure.parts & (1 << bit) != 0 => skin,
                Sheet::Overlay(_) => continue,
                Sheet::Armor(slot) => match armor[slot] {
                    Some(region) => region,
                    None => continue,
                },
            };
            for face in &shell.faces {
                if (turn * face.out).z >= 0.0 {
                    continue;
                }
                let corners = face.corners.map(|c| {
                    let v = at + k * (turn * Vec3::from(c));
                    [cx + v.x * size, cy + v.y * size]
                });
                p.region_quad(
                    region,
                    corners,
                    face.uv,
                    [face.shade, face.shade, face.shade, 1.0],
                );
            }
        }
    }
}

fn entity_pose(s: &AnimSample) -> azalea::entity::Pose {
    use azalea::entity::Pose;
    if s.sleeping {
        Pose::Sleeping
    } else if s.fall_flying {
        Pose::FallFlying
    } else if s.spin_attack {
        Pose::SpinAttack
    } else if s.visually_swimming {
        Pose::Swimming
    } else if s.crouching {
        Pose::Crouching
    } else {
        Pose::Standing
    }
}

struct Face {
    corners: [[f32; 3]; 4],
    uv: [[f32; 2]; 4],
    out: Vec3,
    shade: f32,
}

#[derive(Clone, Copy)]
enum Sheet {
    Skin,
    Overlay(u8),
    Armor(usize),
}

struct Shell {
    sheet: Sheet,
    faces: Vec<Face>,
}

struct Part {
    centre: Vec3,
    shells: Vec<Shell>,
}

fn geometry() -> &'static [[Part; 6]; 2] {
    static GEOMETRY: OnceLock<[[Part; 6]; 2]> = OnceLock::new();
    GEOMETRY.get_or_init(|| [model(false), model(true)])
}

fn model(slim: bool) -> [Part; 6] {
    let mut parts: [Part; 6] = std::array::from_fn(|_| Part {
        centre: Vec3::ZERO,
        shells: Vec::new(),
    });
    for (i, def) in PARTS.iter().enumerate() {
        let (origin, size) = SLIM_ARMS
            .iter()
            .find(|a| slim && a.0 == i)
            .map_or((def.origin, def.size), |&(_, origin, size)| (origin, size));
        let cube = geom::CubeDef {
            tex: def.tex,
            origin,
            size,
            grow: geom::Grow::all(def.grow),
            mirror: false,
            tex_scale: [1.0, 1.0],
            faces: geom::Faces::ALL,
        };
        let (part, sheet) = if i < 6 {
            (i, Sheet::Skin)
        } else {
            (i - 6, Sheet::Overlay(OVERLAY_BITS[i - 6]))
        };
        if i < 6 {
            parts[part].centre = Vec3::from(origin) + Vec3::from(size) / 2.0;
        }
        parts[part].shells.push(Shell {
            sheet,
            faces: faces(&cube, SKIN_SHEET),
        });
    }
    for slot in ARMOR_DRAW_ORDER {
        let layer = ARMOR_LAYERS[slot]();
        let sheet = [layer.tex_w as f32, layer.tex_h as f32];
        for (name, part) in ARMOR_PARTS {
            let Some(node) = layer.mesh.find(name) else {
                continue;
            };
            for cube in node.cubes() {
                parts[part].shells.push(Shell {
                    sheet: Sheet::Armor(slot),
                    faces: faces(cube, sheet),
                });
            }
        }
    }
    parts
}

fn faces(cube: &geom::CubeDef, sheet: [f32; 2]) -> Vec<Face> {
    let mut mesh = QuadMesh::new();
    geom::push_cube(&mut mesh, cube, sheet, 1.0);
    let centre = Vec3::from(cube.origin) + Vec3::from(cube.size) / 2.0;
    mesh.positions()
        .chunks_exact(4)
        .zip(mesh.uvs().chunks_exact(4))
        .map(|(c, uv)| {
            let out = c.iter().map(|&v| Vec3::from(v)).sum::<Vec3>() / 4.0 - centre;
            let shade = if out.y.abs() > out.x.abs().max(out.z.abs()) {
                if out.y < 0.0 { 1.0 } else { 0.5 }
            } else if out.z.abs() > out.x.abs() {
                0.8
            } else {
                0.6
            };
            Face {
                corners: [c[0], c[1], c[2], c[3]],
                uv: [uv[0], uv[1], uv[2], uv[3]],
                out,
                shade,
            }
        })
        .collect()
}

#[derive(Clone, PartialEq)]
enum SkinKey {
    #[cfg(feature = "skins")]
    Url(Arc<str>),
    Default(u8),
}

#[derive(Default)]
pub struct Worn {
    sample: AnimSample,
    slim: bool,
    parts: u8,
    skin: Option<(SkinKey, Option<Arc<Vec<u8>>>)>,
    armor: [Option<(&'static str, Option<Arc<Vec<u8>>>)>; 4],
    assets_seen: u32,
}

impl Worn {
    pub fn draw(
        &self,
        p: &mut Painter,
        rect: [f32; 4],
        size: f32,
        offset_y: f32,
        mouse: Option<Vec2>,
    ) {
        if !self.skin.as_ref().is_some_and(|(_, px)| px.is_some()) {
            return;
        }
        let figure = Figure {
            sample: &self.sample,
            slim: self.slim,
            parts: self.parts,
            armor: std::array::from_fn(|i| {
                self.armor[i].as_ref().is_some_and(|(_, px)| px.is_some())
            }),
        };
        follows_mouse(p, &figure, rect, size, offset_y, mouse);
    }
}

pub fn update(
    mut state: ResMut<crate::gui::GuiState>,
    view: Option<Res<crate::renderer::FrameView>>,
    #[cfg(feature = "skins")] skins: Option<Res<crate::renderer::skin::SkinTextures>>,
    #[cfg(feature = "skins")] images: Res<Assets<Image>>,
) {
    use crate::gui::Screen;
    if !matches!(state.screen, Screen::Inventory | Screen::Creative) {
        return;
    }
    let Some(view) = view else { return };
    let worn = &mut state.puppet;
    worn.sample.clone_from(&view.local);

    let generation = crate::renderer::reload::generation();
    if worn.assets_seen != generation {
        worn.assets_seen = generation;
        worn.skin = None;
        worn.armor = Default::default();
    }

    #[cfg(feature = "skins")]
    let downloaded = {
        let skin = &view.local_skin;
        worn.slim = skin.slim();
        worn.parts = skin.parts;
        skin.refs.body.as_ref().and_then(|url| {
            let image = images.get(skins.as_ref()?.get(url)?)?;
            Some((SkinKey::Url(url.clone()), image))
        })
    };
    #[cfg(feature = "skins")]
    let key = match &downloaded {
        Some((key, _)) => key.clone(),
        None => SkinKey::Default(view.local_skin.default_index),
    };
    #[cfg(not(feature = "skins"))]
    let key = {
        worn.slim = false;
        worn.parts = u8::MAX;
        SkinKey::Default(0)
    };

    if worn.skin.as_ref().is_none_or(|(seen, _)| *seen != key) {
        let pixels = match &key {
            #[cfg(feature = "skins")]
            SkinKey::Url(_) => downloaded.and_then(|(_, image)| {
                let size = image.texture_descriptor.size;
                RgbaImage::from_raw(size.width, size.height, image.data.clone()?)
            }),
            SkinKey::Default(_index) => {
                #[cfg(feature = "skins")]
                let path = crate::renderer::skin::default_skin_path(*_index);
                #[cfg(not(feature = "skins"))]
                let path = crate::renderer::skin::DEFAULT_SKIN.to_owned();
                crate::platform::assets::open_image(&crate::assets_root().join(path))
                    .ok()
                    .map(|i| i.to_rgba8())
            }
        };
        let pixels = pixels.map(|image| Arc::new(fit(image, SHEET_PX, SHEET_PX).into_raw()));
        worn.skin = Some((key, pixels));
    }
    if let Some((_, Some(pixels))) = &worn.skin {
        show_skin(pixels);
    }

    for (slot, &item) in view.local.armor.items.iter().enumerate() {
        if worn.armor[slot].as_ref().map(|(seen, _)| *seen) != item {
            worn.armor[slot] = item.map(|item| {
                let sheet = crate::renderer::player_model::armor_texture(slot, item)
                    .and_then(|path| crate::entities::decode_entity_texture(&path))
                    .map(|image| {
                        let mut cell = RgbaImage::new(SHEET_PX, SHEET_PX);
                        image::imageops::replace(
                            &mut cell,
                            &fit(image, SHEET_PX, ARMOR_SHEET_H),
                            0,
                            0,
                        );
                        Arc::new(cell.into_raw())
                    });
                (item, sheet)
            });
        }
        if let Some((_, Some(pixels))) = &worn.armor[slot] {
            show_armor(slot, pixels);
        }
    }
}

fn fit(image: RgbaImage, w: u32, h: u32) -> RgbaImage {
    if image.dimensions() == (w, h) {
        image
    } else {
        image::imageops::resize(&image, w, h, image::imageops::FilterType::Nearest)
    }
}
