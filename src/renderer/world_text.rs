use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::primitives::{Aabb, Frustum};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::math::Vec3A;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use crate::entities::render::objects::frame::{map_levels, map_pixel_pose};
use crate::entities::state::Display;
use crate::entities::{CameraView, VIEW_RANGE_BLOCKS, display_orientation, display_quat};
use crate::gui::painter::{Painter, PainterBuffers};
use crate::gui::render::GuiAssets;
use crate::renderer::AppState;
use crate::renderer::entity_material::{EntityMaterial, EntityParams, LightMode, Lit};
use crate::renderer::frame_view::{FrameView, FrameViewSystems};
use crate::text::{Font, LINE_HEIGHT, Span};

pub struct WorldTextPlugin;

impl Plugin for WorldTextPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            sync_text_displays
                .after(FrameViewSystems)
                .after(crate::entities::EntitySync)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

const TEXT_LINE_HEIGHT: f32 = LINE_HEIGHT + 1.0;

const FONT_UNIT: f32 = 0.025;

const DEFAULT_BACKGROUND: u32 = 0x3F00_0000;

#[derive(Resource)]
struct WorldTextAssets {
    meshes: [Handle<Mesh>; PASSES],
}

#[derive(Clone, Copy)]
enum Pass {
    Normal = 0,
    SeeThrough = 1,
    MapLabels = 2,
}

const PASSES: usize = 3;

impl Pass {
    const ALL: [Pass; PASSES] = [Pass::Normal, Pass::SeeThrough, Pass::MapLabels];

    fn see_through(self) -> bool {
        matches!(self, Pass::SeeThrough)
    }

    fn layering(self) -> i8 {
        match self {
            Pass::MapLabels => 2,
            Pass::Normal | Pass::SeeThrough => 0,
        }
    }
}

const MAP_LABEL_HALF_EXTENT: f32 = 0.75;

type Taken<'a> = (Painter<'a>, Vec<[f32; 3]>);

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
    fonts: u32,
}

impl Layouts {
    fn get(&mut self, id: i32, font: &Font, display: &Display) -> &Layout {
        let fonts = crate::renderer::reload::generation();
        if self.fonts != fonts {
            self.fonts = fonts;
            self.by_entity.clear();
        }
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

    fn keep(&mut self, id: i32) {
        let generation = self.generation;
        if let Some(layout) = self.by_entity.get_mut(&id) {
            layout.generation = generation;
        }
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
    origin: Res<crate::renderer::RenderOrigin>,
    (lightmap, rigs): (
        Res<crate::renderer::systems::LightmapState>,
        Res<crate::entities::EntityRigs>,
    ),
    mut layouts: Local<Layouts>,
    frusta: Query<&Frustum, With<crate::renderer::systems::WorldCamera>>,
    mut drawn: Local<[bool; PASSES]>,
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
    let origin = *origin;
    let camera = CameraView::of(&view);
    let frame = (time.elapsed_secs() * 20.0) as u32;

    let partial = view.partial;
    let frustum = frusta.single().ok();
    let mut painted: [Option<Taken<'_>>; PASSES] = [None, None, None];
    let mut light = None;
    layouts.generation = layouts.generation.wrapping_add(1);
    for anim in view.entities.iter() {
        let shared = anim.shared();
        if let Some(markers) = shared.map_markers.as_deref()
            && markers.list.iter().any(|d| d.name.is_some())
            && rigs.contains(anim.id)
        {
            let world = anim.position(partial);
            let pos = origin.to_render_array(world);
            let center = Vec3A::from(pos);
            let visible = frustum.is_none_or(|frustum| {
                frustum.intersects_obb_identity(&Aabb {
                    center,
                    half_extents: Vec3A::splat(MAP_LABEL_HALF_EXTENT),
                })
            });
            if visible
                && let Some(painter) = painter_for(
                    &mut painted[Pass::MapLabels as usize],
                    &mut meshes,
                    &assets,
                    Pass::MapLabels,
                    &gui.atlas,
                    frame,
                )
            {
                let pose = Mat4::from_translation(pos)
                    * map_pixel_pose(
                        shared.hanging_direction,
                        shared.item_rotation,
                        anim.is_invisible(),
                    )
                    .to_matrix();
                let start = painter.colors.len();
                crate::renderer::maps::paint_labels(painter, &markers.list, pose);
                let light =
                    light.get_or_insert_with(|| crate::client::worldsync::light_map().read());
                let st = anim.sample(partial);
                let (block, sky) = map_levels(&st, crate::entities::entity_levels(&**light, &st));
                let [r, g, b] =
                    crate::renderer::lightmap::light_color(&lightmap.current, block, sky);
                for color in &mut painter.colors[start..] {
                    color[0] *= r;
                    color[1] *= g;
                    color[2] *= b;
                }
            }
        }
        let Some(display) = shared.display.as_deref() else {
            continue;
        };
        if display.text.is_empty() {
            continue;
        }
        let world = anim.position(partial);
        let limit = display.view_range * VIEW_RANGE_BLOCKS;
        if camera.distance_squared_to(world) > limit * limit {
            continue;
        }
        let pos = origin.to_render_array(world);
        if let Some(frustum) = frustum
            && display.width != 0.0
            && display.height != 0.0
        {
            let half = Vec3A::new(
                display.width * 0.5,
                display.height * 0.5,
                display.width * 0.5,
            );
            let center = Vec3A::from(pos) + Vec3A::new(0.0, half.y, 0.0);
            if !frustum.intersects_obb_identity(&Aabb {
                center,
                half_extents: half,
            }) {
                layouts.keep(anim.id);
                continue;
            }
        }
        let pass = if display.flags & Display::FLAG_SEE_THROUGH != 0 {
            Pass::SeeThrough
        } else {
            Pass::Normal
        };
        let Some(painter) = painter_for(
            &mut painted[pass as usize],
            &mut meshes,
            &assets,
            pass,
            &gui.atlas,
            frame,
        ) else {
            continue;
        };
        let (entity_yaw, entity_pitch) = anim.look(partial);
        let matrix = pose(display, pos, entity_yaw, entity_pitch, &camera);
        let layout = layouts.get(anim.id, &gui.atlas.font, display);
        paint(painter, display, layout, matrix);
    }
    layouts.sweep();

    for pass in Pass::ALL {
        let i = pass as usize;
        match painted[i].take() {
            Some(taken) => {
                put_back(&mut meshes, &assets, pass, taken);
                drawn[i] = true;
            }
            None if drawn[i] => {
                if let Some(taken) = take_buffers(&mut meshes, &assets, pass, &gui.atlas, frame) {
                    put_back(&mut meshes, &assets, pass, taken);
                }
                drawn[i] = false;
            }
            None => {}
        }
    }
}

fn painter_for<'s, 'a>(
    slot: &'s mut Option<Taken<'a>>,
    meshes: &mut Assets<Mesh>,
    assets: &WorldTextAssets,
    pass: Pass,
    atlas: &'a crate::gui::atlas::GuiAtlas,
    frame: u32,
) -> Option<&'s mut Painter<'a>> {
    if slot.is_none() {
        *slot = Some(take_buffers(meshes, assets, pass, atlas, frame)?);
    }
    slot.as_mut().map(|(painter, _)| painter)
}

fn take_buffers<'a>(
    meshes: &mut Assets<Mesh>,
    assets: &WorldTextAssets,
    pass: Pass,
    atlas: &'a crate::gui::atlas::GuiAtlas,
    frame: u32,
) -> Option<Taken<'a>> {
    let mut mesh = meshes.get_mut(&assets.meshes[pass as usize])?;
    let bufs = PainterBuffers {
        positions: f32x3(mesh.remove_attribute(Mesh::ATTRIBUTE_POSITION)),
        uvs: f32x2(mesh.remove_attribute(Mesh::ATTRIBUTE_UV_0)),
        colors: f32x4(mesh.remove_attribute(Mesh::ATTRIBUTE_COLOR)),
        indices: match mesh.remove_indices() {
            Some(Indices::U32(v)) => v,
            _ => Vec::new(),
        },
        textures: Vec::new(),
    };
    let normals = f32x3(mesh.remove_attribute(Mesh::ATTRIBUTE_NORMAL));
    let mut painter = Painter::with_buffers(atlas, frame, bufs);
    painter.unihex_enabled = false;
    Some((painter, normals))
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
    pass: Pass,
    (painter, mut normals): Taken<'_>,
) {
    let bufs = painter.into_buffers();
    let Some(mut mesh) = meshes.get_mut(&assets.meshes[pass as usize]) else {
        return;
    };
    normals.clear();
    normals.resize(bufs.positions.len(), [0.0, 0.0, 1.0]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, bufs.positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, bufs.uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, bufs.colors);
    mesh.insert_indices(Indices::U32(bufs.indices));
}

fn setup(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<EntityMaterial>,
    gui: &GuiAssets,
) -> [Handle<Mesh>; PASSES] {
    Pass::ALL.map(|pass| {
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

        let material = materials.add(EntityMaterial {
            texture: Some(gui.image.clone()),
            params: EntityParams::new(LightMode::Text, 0.0, Lit::full_bright()),
            glint_texture: None,
            alpha_mode: AlphaMode::Blend,
            depth_bias: 0,
            sort_bias: 0.0,
            see_through: pass.see_through(),
            layering: pass.layering(),
        });

        #[cfg_attr(not(feature = "builtin_shaders"), allow(unused_variables, unused_mut))]
        let mut spawned = commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::Visible,
            NoFrustumCulling,
        ));
        #[cfg(feature = "builtin_shaders")]
        spawned.insert(bevy::light::NotShadowCaster);
        mesh
    })
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
