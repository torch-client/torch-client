use bevy::platform::collections::HashMap;

use bevy::asset::{Asset, RenderAssetUsages, embedded_asset};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::camera::{CameraUpdateSystems, ClearColorConfig, RenderTarget};
use bevy::image::ImageSampler;
use bevy::mesh::{MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{Material, MaterialPipeline, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, ColorWrites,
    CompareFunction, RenderPipelineDescriptor, SpecializedMeshPipelineError, TextureFormat,
};
use bevy::shader::ShaderRef;
use bevy::transform::TransformSystems;

use super::entity_material::{ALPHA_CUTOUT, EntityMaterial, LightMode, Lit};
use super::systems::{CameraStart, WorldCamera, scene_camera};

pub const GLOW_LAYER: usize = 6;
pub const GLOW_COMPOSITE_LAYER: usize = 7;

const GLOW_CAMERA_ORDER: isize = 1;
const COMPOSITE_CAMERA_ORDER: isize = 2;

const MATERIAL_CACHE_CAP: usize = 256;

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Glowing(pub u32);

#[derive(Component)]
struct GlowTwin {
    source: Entity,
    stamp: u32,
}

#[derive(Component)]
struct GlowSource(Entity);

#[derive(Component)]
pub struct GlowCamera;

#[derive(Component)]
pub struct GlowCompositeCamera;

#[derive(Resource, Default)]
struct GlowState {
    image: Option<Handle<Image>>,
    size: UVec2,
    composite: Handle<GlowCompositeMaterial>,
    materials: HashMap<(Option<AssetId<Image>>, u32, i32), Handle<EntityMaterial>>,
    stamp: u32,
    stack: Vec<Entity>,
}

pub fn set_glowing(commands: &mut Commands, root: Entity, held: &mut u32, want: u32) {
    if *held == want {
        return;
    }
    *held = want;
    if want == 0 {
        commands.entity(root).try_remove::<Glowing>();
    } else {
        commands.entity(root).try_insert(Glowing(want));
    }
}

pub struct OutlinePlugin;

impl Plugin for OutlinePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "outline_composite.wgsl");
        app.add_plugins(MaterialPlugin::<GlowCompositeMaterial>::default())
            .init_resource::<GlowState>()
            .add_systems(Startup, setup)
            .add_systems(Update, drive_glow_cameras)
            .add_systems(
                PostUpdate,
                (
                    sync_glow_twins.before(TransformSystems::Propagate),
                    follow_world_camera
                        .before(TransformSystems::Propagate)
                        .before(CameraUpdateSystems),
                ),
            );
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut composites: ResMut<Assets<GlowCompositeMaterial>>,
    mut state: ResMut<GlowState>,
) {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[-1.0, -1.0, 0.0], [3.0, -1.0, 0.0], [-1.0, 3.0, 0.0]],
    );
    state.composite = composites.add(GlowCompositeMaterial {
        glow: Handle::default(),
    });
    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(state.composite.clone()),
        Transform::IDENTITY,
        RenderLayers::layer(GLOW_COMPOSITE_LAYER),
        NoFrustumCulling,
    ));

    commands.spawn((
        GlowCamera,
        scene_camera(
            GLOW_CAMERA_ORDER,
            ClearColorConfig::Custom(Color::NONE),
            CameraStart::Inactive,
            GLOW_LAYER,
        ),
        Projection::Perspective(PerspectiveProjection::default()),
        Transform::IDENTITY,
    ));
    commands.spawn((
        GlowCompositeCamera,
        scene_camera(
            COMPOSITE_CAMERA_ORDER,
            ClearColorConfig::None,
            CameraStart::Inactive,
            GLOW_COMPOSITE_LAYER,
        ),
        Transform::IDENTITY,
    ));
}

#[allow(clippy::type_complexity)]
fn drive_glow_cameras(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut composites: ResMut<Assets<GlowCompositeMaterial>>,
    mut state: ResMut<GlowState>,
    roots: Query<(), With<Glowing>>,
    world: Query<
        &Camera,
        (
            With<WorldCamera>,
            Without<GlowCamera>,
            Without<GlowCompositeCamera>,
        ),
    >,
    mut glow: Query<
        (Entity, &mut Camera),
        (
            With<GlowCamera>,
            Without<WorldCamera>,
            Without<GlowCompositeCamera>,
        ),
    >,
    mut composite: Query<
        &mut Camera,
        (
            With<GlowCompositeCamera>,
            Without<WorldCamera>,
            Without<GlowCamera>,
        ),
    >,
) {
    let size = world
        .single()
        .ok()
        .filter(|camera| camera.is_active)
        .and_then(Camera::physical_target_size);
    let on = !roots.is_empty() && !super::terrain::pack_drawing() && size.is_some();
    let Ok((glow_entity, mut glow_camera)) = glow.single_mut() else {
        return;
    };

    if on
        && let Some(size) = size
        && (state.image.is_none() || state.size != size)
    {
        let mut image = Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8Unorm, None);
        image.sampler = ImageSampler::nearest();
        let handle = images.add(image);
        commands
            .entity(glow_entity)
            .insert(RenderTarget::Image(handle.clone().into()));
        if let Some(material) = composites.get_mut(&state.composite) {
            material.glow = handle.clone();
        }
        state.image = Some(handle);
        state.size = size;
    }

    if glow_camera.is_active != on {
        glow_camera.is_active = on;
    }
    if let Ok(mut camera) = composite.single_mut()
        && camera.is_active != on
    {
        camera.is_active = on;
    }
}

fn follow_world_camera(
    world: Query<(&Transform, Ref<Projection>), (With<WorldCamera>, Without<GlowCamera>)>,
    mut glow: Query<(&Camera, &mut Transform, &mut Projection), With<GlowCamera>>,
    mut synced: Local<bool>,
) {
    let (Ok((world_transform, world_projection)), Ok((camera, mut transform, mut projection))) =
        (world.single(), glow.single_mut())
    else {
        return;
    };
    if !camera.is_active {
        *synced = false;
        return;
    }
    if *transform != *world_transform {
        *transform = *world_transform;
    }
    if !*synced || world_projection.is_changed() {
        *projection = (*world_projection).clone();
        *synced = true;
    }
}

#[allow(clippy::type_complexity)]
fn sync_glow_twins(
    mut commands: Commands,
    mut state: ResMut<GlowState>,
    mut materials: ResMut<Assets<EntityMaterial>>,
    roots: Query<(Entity, &Glowing)>,
    children: Query<&Children>,
    sources: Query<
        (
            &Mesh3d,
            &MeshMaterial3d<EntityMaterial>,
            Option<&GlowSource>,
        ),
        Without<GlowTwin>,
    >,
    mut twins: Query<(
        Entity,
        &mut GlowTwin,
        &mut Mesh3d,
        &mut MeshMaterial3d<EntityMaterial>,
    )>,
) {
    if roots.is_empty() && twins.is_empty() {
        return;
    }
    let state = &mut *state;
    state.stamp = state.stamp.wrapping_add(1);
    let stamp = state.stamp;

    if !super::terrain::pack_drawing() {
        for (root, &Glowing(argb)) in &roots {
            state.stack.clear();
            state.stack.push(root);
            while let Some(node) = state.stack.pop() {
                if let Ok(kids) = children.get(node) {
                    state.stack.extend(kids.iter());
                }
                let Ok((mesh, material, source)) = sources.get(node) else {
                    continue;
                };
                let Some(source_material) = materials.get(&material.0) else {
                    continue;
                };
                let mode = source_material.params.mode;
                if mode == LightMode::Emissive as u32 || mode == LightMode::Text as u32 {
                    continue;
                }
                let texture = source_material.texture.clone();
                let depth_bias = source_material.depth_bias;
                let glow_material = glow_material(state, &mut materials, texture, argb, depth_bias);

                match source.and_then(|s| twins.get_mut(s.0).ok()) {
                    Some((_, mut twin, mut twin_mesh, mut twin_material)) => {
                        twin.stamp = stamp;
                        if twin_mesh.0 != mesh.0 {
                            twin_mesh.0 = mesh.0.clone();
                        }
                        if twin_material.0 != glow_material {
                            twin_material.0 = glow_material;
                        }
                    }
                    None => {
                        let twin = commands
                            .spawn((
                                Mesh3d(mesh.0.clone()),
                                MeshMaterial3d(glow_material),
                                Transform::IDENTITY,
                                Visibility::Inherited,
                                RenderLayers::layer(GLOW_LAYER),
                                NoFrustumCulling,
                                GlowTwin {
                                    source: node,
                                    stamp,
                                },
                                ChildOf(node),
                            ))
                            .id();
                        #[cfg(feature = "builtin_shaders")]
                        commands.entity(twin).insert(bevy::light::NotShadowCaster);
                        commands.entity(node).insert(GlowSource(twin));
                    }
                }
            }
        }
    }

    for (entity, twin, _, _) in &twins {
        if twin.stamp != stamp {
            commands.entity(entity).try_despawn();
            commands.entity(twin.source).try_remove::<GlowSource>();
        }
    }
}

fn glow_material(
    state: &mut GlowState,
    materials: &mut Assets<EntityMaterial>,
    texture: Option<Handle<Image>>,
    argb: u32,
    depth_bias: i32,
) -> Handle<EntityMaterial> {
    let key = (texture.as_ref().map(Handle::id), argb, depth_bias);
    if let Some(handle) = state.materials.get(&key) {
        return handle.clone();
    }
    if state.materials.len() >= MATERIAL_CACHE_CAP {
        state.materials.clear();
    }
    let mut material = EntityMaterial::new(
        texture,
        LightMode::Outline,
        AlphaMode::Mask(ALPHA_CUTOUT),
        0.0,
        Lit::full_bright(),
        depth_bias,
    );
    let channel = |shift: u32| ((argb >> shift) & 0xFF) as f32 / 255.0;
    material.params.tint = Vec4::new(channel(16), channel(8), channel(0), 1.0);
    let handle = materials.add(material);
    state.materials.insert(key, handle.clone());
    handle
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct GlowCompositeMaterial {
    #[texture(0)]
    glow: Handle<Image>,
}

fn shader() -> ShaderRef {
    super::embedded_shader("renderer/outline_composite.wgsl")
}

impl Material for GlowCompositeMaterial {
    crate::renderer::material_shader_fns!();

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    crate::renderer::material_no_prepass_no_shadows!();

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout
            .0
            .get_layout(&[Mesh::ATTRIBUTE_POSITION.at_shader_location(0)])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        descriptor.primitive.cull_mode = None;
        if let Some(depth_stencil) = descriptor.depth_stencil.as_mut() {
            depth_stencil.depth_compare = CompareFunction::Always;
            depth_stencil.depth_write_enabled = false;
        }
        if let Some(fragment) = descriptor.fragment.as_mut()
            && let Some(Some(target)) = fragment.targets.first_mut()
        {
            target.blend = Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::SrcAlpha,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::Zero,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
            });
            target.write_mask = ColorWrites::COLOR;
        }
        Ok(())
    }
}
