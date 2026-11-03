use bevy::asset::{Asset, RenderAssetUsages, embedded_asset};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::math::{DVec3, IVec3};
use bevy::mesh::{
    Indices, Mesh, MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology, VertexFormat,
};
use bevy::pbr::{Material, MaterialPipeline, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, DepthBiasState,
    RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use super::AppState;
use crate::gui::GuiState;

pub const ATTRIBUTE_WALL: MeshVertexAttribute =
    MeshVertexAttribute::new("WorldBorder_Wall", 930, VertexFormat::Uint32);

#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct BorderParams {
    pub bounds: Vec4,
    pub tint: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct WorldBorderMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub texture: Handle<Image>,
    #[uniform(2)]
    pub params: BorderParams,
}

impl Material for WorldBorderMaterial {
    crate::renderer::material_shader_fns!();

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    crate::renderer::material_no_prepass_no_shadows!();

    fn depth_bias(&self) -> f32 {
        1.0e7
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            ATTRIBUTE_WALL.at_shader_location(2),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        descriptor.primitive.cull_mode = None;
        if let Some(target) = descriptor
            .fragment
            .as_mut()
            .and_then(|f| f.targets.first_mut())
            .and_then(Option::as_mut)
        {
            target.blend = Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::SrcAlpha,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::Zero,
                    operation: BlendOperation::Add,
                },
            });
        }
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.bias = DepthBiasState {
                constant: 3,
                slope_scale: 3.0,
                clamp: 0.0,
            };
        }
        Ok(())
    }
}

fn shader() -> ShaderRef {
    super::embedded_shader("renderer/world_border.wgsl")
}

pub struct WorldBorderPlugin;

impl Plugin for WorldBorderPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "world_border.wgsl");
        app.add_plugins(MaterialPlugin::<WorldBorderMaterial>::default())
            .add_systems(
                Update,
                setup.run_if(
                    resource_exists::<super::systems::AssetsReady>
                        .and(not(resource_exists::<BorderAssets>)),
                ),
            )
            .add_systems(
                PostUpdate,
                update_border
                    .in_set(super::CameraPinnedMeshes)
                    .run_if(in_state(AppState::InGame))
                    .run_if(resource_exists::<BorderAssets>),
            );
    }
}

#[derive(Component)]
struct BorderLayer;

#[derive(Clone, Copy, PartialEq)]
struct BuildKey {
    bounds: [f64; 4],
    section: IVec3,
}

#[derive(Resource)]
struct BorderAssets {
    mesh: Handle<Mesh>,
    material: Handle<WorldBorderMaterial>,
    built: Option<BuildKey>,
    last_min: DVec3,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<WorldBorderMaterial>>,
) {
    let path = crate::textures_dir().join("misc").join("forcefield.png");
    let texture = super::sky::add_celestial(&mut images, path, true);
    let material = materials.add(WorldBorderMaterial {
        texture,
        params: BorderParams::default(),
    });
    let mesh = meshes.add(Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    ));
    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material.clone()),
        Transform::default(),
        Visibility::Hidden,
        NoFrustumCulling,
        BorderLayer,
    ));
    commands.insert_resource(BorderAssets {
        mesh,
        material,
        built: None,
        last_min: DVec3::ZERO,
    });
}

fn in_range(cam: DVec3, [min_x, max_x, min_z, max_z]: [f64; 4], rd: f64) -> bool {
    let deep_inside =
        cam.x < max_x - rd && cam.x > min_x + rd && cam.z < max_z - rd && cam.z > min_z + rd;
    let far_outside =
        cam.x < min_x - rd || cam.x > max_x + rd || cam.z < min_z - rd || cam.z > max_z + rd;
    !deep_inside && !far_outside
}

fn update_border(
    view: Res<super::FrameView>,
    gui: Res<GuiState>,
    origin: Res<super::RenderOrigin>,
    mut assets: ResMut<BorderAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WorldBorderMaterial>>,
    mut layer: Query<(&mut Transform, &mut Visibility), With<BorderLayer>>,
) {
    let Ok((mut transform, mut visibility)) = layer.single_mut() else {
        return;
    };
    let border = &view.border;
    let partial = view.partial;
    let bounds = [
        border.min_x(partial),
        border.max_x(partial),
        border.min_z(partial),
        border.max_z(partial),
    ];
    let rd = f64::from(gui.options.render_distance * 16);
    let cam = view.camera_world;
    if !view.in_world || !in_range(cam, bounds, rd) {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    }
    visibility.set_if_neq(Visibility::Visible);

    let section = IVec3::new(
        (cam.x.floor() as i32) >> 4,
        (cam.y.floor() as i32) >> 4,
        (cam.z.floor() as i32) >> 4,
    );
    let key = BuildKey { bounds, section };
    if assets.built != Some(key) {
        let (mesh, last_min) = write_mesh(bounds, cam, rd);
        if let Some(slot) = meshes.get_mut(&assets.mesh) {
            *slot = mesh;
        }
        assets.built = Some(key);
        assets.last_min = last_min;
    }
    let at = origin.to_render(assets.last_min);
    if transform.translation != at {
        transform.translation = at;
    }

    let [min_x, max_x, min_z, max_z] = bounds;
    let low = origin.to_render(DVec3::new(min_x, 0.0, min_z));
    let high = origin.to_render(DVec3::new(max_x, 0.0, max_z));
    let [r, g, b, _] = crate::gui::painter::unpack(0xFF00_0000 | border.status().color());
    let params = BorderParams {
        bounds: Vec4::new(low.x, high.x, low.z, high.z),
        tint: Vec4::new(r, g, b, rd as f32),
    };
    if materials
        .get(&assets.material)
        .is_some_and(|m| m.params != params)
        && let Some(material) = materials.get_mut(&assets.material)
    {
        material.params = params;
    }
}

fn write_mesh(
    [border_min_x, border_max_x, border_min_z, border_max_z]: [f64; 4],
    cam: DVec3,
    rd: f64,
) -> (Mesh, DVec3) {
    const CLOUD_RANGE_BLOCKS: f64 = 128.0 * 16.0;
    let half = (rd * 4.0).max(CLOUD_RANGE_BLOCKS) as f32;
    let v0 = -((cam.y * 0.5) - (cam.y * 0.5).floor()) as f32;
    let v1 = v0 + half;
    let min_z = (cam.z - rd).floor().max(border_min_z);
    let max_z = (cam.z + rd).ceil().min(border_max_z);
    let u0z = (min_z.floor() as i64 & 1) as f32 * 0.5;
    let u1z = (max_z - min_z) as f32 / 2.0;
    let min_x = (cam.x - rd).floor().max(border_min_x);
    let max_x = (cam.x + rd).ceil().min(border_max_x);
    let u0x = (min_x.floor() as i64 & 1) as f32 * 0.5;
    let u1x = (max_x - min_x) as f32 / 2.0;

    let w = (max_x - min_x) as f32;
    let d = (max_z - min_z) as f32;
    let south = (border_max_z - min_z) as f32;
    let east = (border_max_x - min_x) as f32;
    let positions: [[f32; 3]; 16] = [
        [0.0, -half, south],
        [w, -half, south],
        [w, half, south],
        [0.0, half, south],
        [0.0, -half, 0.0],
        [0.0, -half, d],
        [0.0, half, d],
        [0.0, half, 0.0],
        [w, -half, 0.0],
        [0.0, -half, 0.0],
        [0.0, half, 0.0],
        [w, half, 0.0],
        [east, -half, d],
        [east, -half, 0.0],
        [east, half, 0.0],
        [east, half, d],
    ];
    let quad = |u0: f32, u1: f32| [[u0, v1], [u1 + u0, v1], [u1 + u0, v0], [u0, v0]];
    let uvs: Vec<[f32; 2]> = [
        quad(u0x, u1x),
        quad(u0z, u1z),
        quad(u0x, u1x),
        quad(u0z, u1z),
    ]
    .concat();
    let walls: Vec<u32> = (0..16).map(|i| i / 4).collect();
    let indices: Vec<u32> = (0..4u32)
        .flat_map(|q| crate::entities::quads::QUAD_INDICES.map(|i| q * 4 + i))
        .collect();

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.to_vec());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(ATTRIBUTE_WALL, walls);
    mesh.insert_indices(Indices::U32(indices));
    (mesh, DVec3::new(min_x, 0.0, min_z))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_gate_matches_extract() {
        let b = [-100.0, 100.0, -100.0, 100.0];
        assert!(!in_range(DVec3::ZERO, b, 50.0));
        assert!(in_range(DVec3::new(80.0, 0.0, 0.0), b, 50.0));
        assert!(in_range(DVec3::new(120.0, 0.0, 0.0), b, 50.0));
        assert!(!in_range(DVec3::new(200.0, 0.0, 0.0), b, 50.0));
    }
}
