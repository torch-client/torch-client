use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use crate::entities::state::Display;
use crate::entities::{CameraView, VIEW_RANGE_BLOCKS, display_orientation, display_quat};
use crate::gui::painter::{Painter, PainterBuffers};
use crate::gui::render::GuiAssets;
use crate::renderer::AppState;
use crate::renderer::entity_material::{EntityMaterial, EntityParams, LightMode, Lit};
use crate::renderer::frame_view::{FrameView, FrameViewSystems};
use crate::renderer::systems::WorldCamera;
use crate::text::{Font, LINE_HEIGHT, Span};

pub struct WorldTextPlugin;

impl Plugin for WorldTextPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            sync_text_displays
                .after(FrameViewSystems)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

const TEXT_LINE_HEIGHT: f32 = LINE_HEIGHT + 1.0;

const FONT_UNIT: f32 = 0.025;

const DEFAULT_BACKGROUND: u32 = 0x3F00_0000;

#[derive(Resource)]
struct WorldTextAssets {
    meshes: [Handle<Mesh>; 2],
}

#[derive(Clone, Copy)]
enum Pass {
    Normal = 0,
    SeeThrough = 1,
}

struct Layout {
    text: Vec<Span>,
    line_width: f32,
    lines: Vec<Vec<Span>>,
    widths: Vec<f32>,
    width: f32,
    height: f32,
    generation: u64,
}

#[derive(Default)]
struct Layouts {
    by_entity: HashMap<i32, Layout>,
    generation: u64,
}

impl Layouts {
    fn get(&mut self, id: i32, font: &Font, display: &Display) -> &Layout {
        let generation = self.generation;
        let entry = self.by_entity.entry(id).or_insert_with(|| Layout {
            text: Vec::new(),
            line_width: f32::NAN,
            lines: Vec::new(),
            widths: Vec::new(),
            width: 0.0,
            height: 0.0,
            generation,
        });
        entry.generation = generation;
        if entry.line_width != display.line_width || entry.text != display.text {
            entry.text.clear();
            entry.text.extend_from_slice(&display.text);
            entry.line_width = display.line_width;
            entry.lines = font.wrap(&display.text, display.line_width);
            entry.widths.clear();
            entry
                .widths
                .extend(entry.lines.iter().map(|line| font.width(line)));
            entry.width = entry.widths.iter().copied().fold(0.0f32, f32::max);
            entry.height = entry.lines.len() as f32 * TEXT_LINE_HEIGHT - 1.0;
        }
        entry
    }

    fn sweep(&mut self) {
        let generation = self.generation;
        self.by_entity
            .retain(|_, layout| layout.generation == generation);
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_text_displays(
    mut commands: Commands,
    view: Res<FrameView>,
    time: Res<Time>,
    gui: Option<Res<GuiAssets>>,
    assets: Option<Res<WorldTextAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<EntityMaterial>>,
    camera: Query<&Transform, With<WorldCamera>>,
    mut layouts: Local<Layouts>,
    mut drawn: Local<bool>,
) {
    crate::prof_span!("render:text_displays");
    let Some(gui) = gui else { return };
    let assets = match assets {
        Some(assets) => assets,
        None => {
            let handles = setup(&mut commands, &mut meshes, &mut materials, &gui);
            commands.insert_resource(WorldTextAssets { meshes: handles });
            return;
        }
    };
    let Ok(cam_tf) = camera.single() else { return };
    let camera = CameraView::new(cam_tf.rotation, cam_tf.translation);
    let eye = camera.position;
    let frame = (time.elapsed_secs() * 20.0) as u32;

    let partial = view.partial;
    let mut painted: Option<([Painter<'_>; 2], [Vec<[f32; 3]>; 2])> = None;
    layouts.generation = layouts.generation.wrapping_add(1);
    for anim in view.entities.iter() {
        let Some(display) = anim.shared().display.as_deref() else {
            continue;
        };
        if display.text.is_empty() {
            continue;
        }
        let pos = Vec3::from(anim.position(partial));
        if pos.distance(eye) > display.view_range * VIEW_RANGE_BLOCKS {
            continue;
        }
        if painted.is_none() {
            let Some(taken) = take_buffers(&mut meshes, &assets, &gui.atlas, frame) else {
                return;
            };
            painted = Some(taken);
        }
        let (entity_yaw, entity_pitch) = anim.look(partial);
        let matrix = pose(display, pos, entity_yaw, entity_pitch, &camera);
        let pass = if display.flags & Display::FLAG_SEE_THROUGH != 0 {
            Pass::SeeThrough
        } else {
            Pass::Normal
        };
        let layout = layouts.get(anim.id, &gui.atlas.font, display);
        if let Some((painters, _)) = &mut painted {
            paint(&mut painters[pass as usize], display, layout, matrix);
        }
    }
    layouts.sweep();

    match painted {
        Some((painters, normals)) => {
            put_back(&mut meshes, &assets, painters, normals);
            *drawn = true;
        }
        None if *drawn => {
            if let Some((painters, normals)) = take_buffers(&mut meshes, &assets, &gui.atlas, frame)
            {
                put_back(&mut meshes, &assets, painters, normals);
            }
            *drawn = false;
        }
        None => {}
    }
}

fn take_buffers<'a>(
    meshes: &mut Assets<Mesh>,
    assets: &WorldTextAssets,
    atlas: &'a crate::gui::atlas::GuiAtlas,
    frame: u32,
) -> Option<([Painter<'a>; 2], [Vec<[f32; 3]>; 2])> {
    if assets
        .meshes
        .iter()
        .any(|handle| meshes.get(handle).is_none())
    {
        return None;
    }
    let mut bufs = [PainterBuffers::default(), PainterBuffers::default()];
    let mut normals: [Vec<[f32; 3]>; 2] = [Vec::new(), Vec::new()];
    for (i, handle) in assets.meshes.iter().enumerate() {
        let mesh = meshes.get_mut(handle)?;
        bufs[i] = PainterBuffers {
            positions: f32x3(mesh.remove_attribute(Mesh::ATTRIBUTE_POSITION)),
            uvs: f32x2(mesh.remove_attribute(Mesh::ATTRIBUTE_UV_0)),
            colors: f32x4(mesh.remove_attribute(Mesh::ATTRIBUTE_COLOR)),
            indices: match mesh.remove_indices() {
                Some(Indices::U32(v)) => v,
                _ => Vec::new(),
            },
        };
        normals[i] = f32x3(mesh.remove_attribute(Mesh::ATTRIBUTE_NORMAL));
    }
    let [normal_bufs, see_through_bufs] = bufs;
    let mut painters = [
        Painter::with_buffers(atlas, frame, normal_bufs),
        Painter::with_buffers(atlas, frame, see_through_bufs),
    ];
    for painter in &mut painters {
        painter.unihex_enabled = false;
    }
    Some((painters, normals))
}

fn paint(painter: &mut Painter, display: &Display, layout: &Layout, pose: Mat4) {
    let start = painter.positions.len();

    let (width, height) = (layout.width, layout.height);

    let background = if display.flags & Display::FLAG_DEFAULT_BACKGROUND != 0 {
        DEFAULT_BACKGROUND
    } else {
        display.background
    };
    if background & 0xFF00_0000 != 0 {
        painter.fill(-1.0, -1.0, width + 1.0, height + 1.0, background);
    }

    let shadow = display.flags & Display::FLAG_SHADOW != 0;
    let opacity = display.opacity as f32 / 255.0;
    for (i, (line, line_width)) in layout.lines.iter().zip(&layout.widths).enumerate() {
        let line_width = *line_width;
        let left = display.flags & Display::FLAG_ALIGN_LEFT != 0;
        let right = display.flags & Display::FLAG_ALIGN_RIGHT != 0;
        let x = match (left, right) {
            (true, false) => 0.0,
            (false, true) => width - line_width,
            _ => (width - line_width) / 2.0,
        };
        painter.text_faded(line, x, i as f32 * TEXT_LINE_HEIGHT, shadow, opacity);
    }

    let pose = pose * Mat4::from_translation(Vec3::new(1.0 - width / 2.0, -height, 0.0));
    for position in &mut painter.positions[start..] {
        *position = pose
            .transform_point3(Vec3::new(position[0], position[1], 0.0))
            .to_array();
    }
}

fn pose(
    display: &Display,
    pos: Vec3,
    entity_yaw: f32,
    entity_pitch: f32,
    camera: &CameraView,
) -> Mat4 {
    let orientation = display_orientation(display.billboard, entity_yaw, entity_pitch, camera);

    let transformation = Mat4::from_translation(Vec3::from(display.translation))
        * Mat4::from_quat(display_quat(display.left_rotation))
        * Mat4::from_scale(Vec3::from(display.scale))
        * Mat4::from_quat(display_quat(display.right_rotation));

    Mat4::from_translation(pos)
        * Mat4::from_quat(orientation)
        * transformation
        * Mat4::from_scale(Vec3::new(FONT_UNIT, -FONT_UNIT, FONT_UNIT))
}

fn put_back(
    meshes: &mut Assets<Mesh>,
    assets: &WorldTextAssets,
    painters: [Painter<'_>; 2],
    mut normals: [Vec<[f32; 3]>; 2],
) {
    for (i, painter) in painters.into_iter().enumerate() {
        let bufs = painter.into_buffers();
        let Some(mesh) = meshes.get_mut(&assets.meshes[i]) else {
            continue;
        };
        normals[i].clear();
        normals[i].resize(bufs.positions.len(), [0.0, 0.0, 1.0]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, bufs.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, bufs.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, std::mem::take(&mut normals[i]));
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, bufs.colors);
        mesh.insert_indices(Indices::U32(bufs.indices));
    }
}

fn setup(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<EntityMaterial>,
    gui: &GuiAssets,
) -> [Handle<Mesh>; 2] {
    let mut handles = Vec::with_capacity(2);
    for see_through in [false, true] {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
        mesh.insert_indices(Indices::U32(Vec::new()));
        let mesh = meshes.add(mesh);
        handles.push(mesh.clone());

        let material = materials.add(EntityMaterial {
            texture: Some(gui.image.clone()),
            params: EntityParams::new(LightMode::Text, 0.0, Lit::full_bright()),
            alpha_mode: AlphaMode::Blend,
            depth_bias: 0,
            see_through,
        });

        let mut spawned = commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::Visible,
            NoFrustumCulling,
        ));
        #[cfg(feature = "builtin_shaders")]
        spawned.insert(bevy::light::NotShadowCaster);
    }
    [handles[0].clone(), handles[1].clone()]
}

fn f32x3(values: Option<VertexAttributeValues>) -> Vec<[f32; 3]> {
    match values {
        Some(VertexAttributeValues::Float32x3(v)) => v,
        _ => Vec::new(),
    }
}

fn f32x2(values: Option<VertexAttributeValues>) -> Vec<[f32; 2]> {
    match values {
        Some(VertexAttributeValues::Float32x2(v)) => v,
        _ => Vec::new(),
    }
}

fn f32x4(values: Option<VertexAttributeValues>) -> Vec<[f32; 4]> {
    match values {
        Some(VertexAttributeValues::Float32x4(v)) => v,
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_billboard_faces_the_camera() {
        let display = Display {
            billboard: crate::entities::state::Billboard::Center,
            ..Display::default()
        };
        let (yaw, pitch) = (0.7_f32, -0.3_f32);
        let view = CameraView {
            yaw,
            pitch,
            ..CameraView::default()
        };
        let m = pose(&display, Vec3::ZERO, 123.0, 45.0, &view);
        let camera = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
        let right = m.transform_point3(Vec3::X) - m.transform_point3(Vec3::ZERO);
        let down = m.transform_point3(Vec3::Y) - m.transform_point3(Vec3::ZERO);
        assert!((right.normalize() - camera * Vec3::X).length() < 1.0e-5);
        assert!((down.normalize() - camera * Vec3::NEG_Y).length() < 1.0e-5);
        let other = pose(&display, Vec3::ZERO, -60.0, 12.0, &view);
        assert!((m.to_cols_array()[0] - other.to_cols_array()[0]).abs() < 1.0e-6);
    }

    #[test]
    fn fixed_billboard_faces_south_at_zero() {
        let display = Display::default();
        let view = CameraView {
            yaw: 1.0,
            pitch: 1.0,
            ..CameraView::default()
        };
        let m = pose(&display, Vec3::ZERO, 0.0, 0.0, &view);
        let right = (m.transform_point3(Vec3::X) - m.transform_point3(Vec3::ZERO)).normalize();
        let down = (m.transform_point3(Vec3::Y) - m.transform_point3(Vec3::ZERO)).normalize();
        assert!((right - Vec3::X).length() < 1.0e-5);
        assert!((down - Vec3::NEG_Y).length() < 1.0e-5);
        let step = (m.transform_point3(Vec3::X) - m.transform_point3(Vec3::ZERO)).length();
        assert!((step - FONT_UNIT).abs() < 1.0e-6);
    }

    #[test]
    fn transformation_scales_the_text() {
        let display = Display {
            scale: [2.0, 2.0, 2.0],
            translation: [0.0, 3.0, 0.0],
            ..Display::default()
        };
        let m = pose(&display, Vec3::ZERO, 0.0, 0.0, &CameraView::default());
        let origin = m.transform_point3(Vec3::ZERO);
        assert!((origin.y - 3.0).abs() < 1.0e-5);
        let step = (m.transform_point3(Vec3::X) - origin).length();
        assert!((step - FONT_UNIT * 2.0).abs() < 1.0e-6);
    }
}
